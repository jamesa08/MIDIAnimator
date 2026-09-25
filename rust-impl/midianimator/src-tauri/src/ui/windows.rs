// new windows are only revealed once their page has drawn, so they never flash blank.
// webkit doesn't paint a window that isn't on screen, so they go on screen invisible (alpha 0) first
// and become visible when the page says it has drawn (window_ready). same idea as the floating panels.
// layer release on reattach traced in https://github.com/manaflow-ai/cmux/issues/4287, thanks @austinywang

use std::sync::Mutex;
use tauri::{AppHandle, Manager, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

// windows waiting for their page to draw before they're shown
static PENDING_REVEAL: Mutex<Vec<String>> = Mutex::new(Vec::new());

// the window shows through at the edges while the webview catches up to a resize (webkit draws out of process),
// white matches the ui so it doesn't flash. set natively, the backgroundColor config doesn't always reach the window.
// matching the background is the tauri team's suggestion, thanks @lucasfernog:
// https://github.com/tauri-apps/tauri/issues/13898#issuecomment-3398368987
// https://github.com/tauri-apps/tauri/issues/14288 (config color not reaching the window)
// the real fix is in webkit, thanks @JJJ: https://github.com/WebKit/WebKit/pull/72971
#[cfg(target_os = "macos")]
pub unsafe fn match_ui_background(ns_window: &objc2_app_kit::NSWindow) {
    ns_window.setBackgroundColor(Some(&objc2_app_kit::NSColor::whiteColor()));
}

// centers a window on the screen the mouse is on. tao centers new windows on NSScreen.mainScreen,
// which at launch (nothing focused yet) isn't necessarily the screen being worked on
pub fn center_on_mouse_screen<R: Runtime>(window: &WebviewWindow<R>) {
    #[cfg(target_os = "macos")]
    window
        .with_webview(|webview| {
            use objc2_app_kit::{NSEvent, NSScreen, NSWindow};
            use objc2_foundation::{MainThreadMarker, NSPoint, NSRect};
            let Some(mtm) = MainThreadMarker::new() else {
                return;
            };
            unsafe {
                let ns_window: &NSWindow = &*webview.ns_window().cast::<NSWindow>();
                let mouse = NSEvent::mouseLocation();
                let screens = NSScreen::screens(mtm);
                let Some(screen) = screens.iter().find(|screen| {
                    let frame = screen.frame();
                    mouse.x >= frame.origin.x && mouse.x < frame.origin.x + frame.size.width && mouse.y >= frame.origin.y && mouse.y < frame.origin.y + frame.size.height
                }) else {
                    return;
                };

                // centered in the area outside the menu bar and dock, kept on screen if it's bigger
                let visible = screen.visibleFrame();
                let size = ns_window.frame().size;
                let x = visible.origin.x + ((visible.size.width - size.width) / 2.0).max(0.0);
                let y = visible.origin.y + ((visible.size.height - size.height) / 2.0).max(0.0);
                ns_window.setFrame_display(NSRect::new(NSPoint::new(x.round(), y.round()), size), false);
            }
        })
        .ok();

    #[cfg(not(target_os = "macos"))]
    {
        window.center().ok();
    }
}

// puts a window on screen invisible and click through so its page can draw
pub fn prepare_hidden<R: Runtime>(window: &WebviewWindow<R>) {
    #[cfg(target_os = "macos")]
    window
        .with_webview(|webview| {
            use objc2_app_kit::NSWindow;
            unsafe {
                let ns_window: &NSWindow = &*webview.ns_window().cast::<NSWindow>();
                match_ui_background(ns_window);
                ns_window.setAlphaValue(0.0);
                ns_window.setIgnoresMouseEvents(true);
                ns_window.orderFront(None);
            }
        })
        .ok();
}

// makes a prepared window visible and focused
pub fn reveal<R: Runtime>(window: &WebviewWindow<R>) {
    #[cfg(target_os = "macos")]
    window
        .with_webview(|webview| {
            use objc2_app_kit::NSWindow;
            unsafe {
                let ns_window: &NSWindow = &*webview.ns_window().cast::<NSWindow>();
                ns_window.setAlphaValue(1.0);
                ns_window.setIgnoresMouseEvents(false);
                ns_window.makeKeyAndOrderFront(None);
            }
        })
        .ok();

    #[cfg(not(target_os = "macos"))]
    {
        window.show().ok();
        window.set_focus().ok();
    }
}

// opens a window that's revealed once its page has drawn, focuses it if it's already open
pub fn open_window<R: Runtime>(app: &AppHandle<R>, label: &str, url: &str, title: &str, width: f64, height: f64) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(label) {
        return window.set_focus();
    }

    // registered before the page can load and report ready
    PENDING_REVEAL.lock().unwrap().push(label.to_string());
    let window = WebviewWindowBuilder::new(app, label, WebviewUrl::App(url.into())).title(title).inner_size(width, height).center().visible(false).accept_first_mouse(true).background_color(tauri::window::Color(255, 255, 255, 255)).build()?;
    prepare_hidden(&window);
    #[cfg(target_os = "macos")]
    smooth_zoom(&window);
    Ok(())
}

// every page calls this after its first paint, reveals the window if it was waiting on it
#[tauri::command]
pub fn window_ready(window: WebviewWindow) {
    let mut pending = PENDING_REVEAL.lock().unwrap();
    if let Some(index) = pending.iter().position(|label| label == window.label()) {
        pending.remove(index);
        drop(pending);
        reveal(&window);
    }
}

// smooth titlebar double-click zoom. appkit's zoom animates the window without resizing the webview until it ends,
// animating setFrame through the window's animator resizes it every step like dragging to the top edge does.
// tao's window delegate doesn't implement windowShouldZoom:toFrame:, so it's added to its class.
// ported from https://github.com/tauri-apps/tao/pull/1207, thanks @Tunglies. remove once tauri ships it
// bug: https://github.com/tauri-apps/tauri/issues/13898, thanks @LintyDev for reporting and @michakfromparis for the root cause:
// https://github.com/tauri-apps/tauri/issues/13898#issuecomment-5552958772
#[cfg(target_os = "macos")]
pub fn smooth_zoom<R: Runtime>(window: &WebviewWindow<R>) {
    window
        .with_webview(|webview| unsafe {
            use objc2::runtime::{AnyObject, Bool, Sel};
            use objc2::{msg_send, Encode};
            use objc2_foundation::NSRect;

            let ns_window = webview.ns_window() as *mut AnyObject;
            let delegate: *mut AnyObject = msg_send![ns_window, delegate];
            if delegate.is_null() {
                return;
            }

            let class = (*delegate).class() as *const _ as *mut objc2::ffi::objc_class;
            let sel = objc2::ffi::sel_registerName(c"windowShouldZoom:toFrame:".as_ptr());
            let types = std::ffi::CString::new(format!("{}{}{}{}{}", Bool::ENCODING, <*mut AnyObject>::ENCODING, Sel::ENCODING, <*mut AnyObject>::ENCODING, NSRect::ENCODING)).unwrap();
            let imp: extern "C" fn(*mut AnyObject, Sel, *mut AnyObject, NSRect) -> Bool = window_should_zoom;
            // does nothing if it's already been added
            objc2::ffi::class_addMethod(class, sel, Some(std::mem::transmute(imp)), types.as_ptr());
        })
        .ok();
}

// frames to go back to when a window is unzoomed, keyed by window pointer
#[cfg(target_os = "macos")]
static ZOOM_RESTORE_FRAMES: Mutex<Vec<(usize, objc2_foundation::NSRect)>> = Mutex::new(Vec::new());

#[cfg(target_os = "macos")]
extern "C" fn window_should_zoom(_this: *mut objc2::runtime::AnyObject, _sel: objc2::runtime::Sel, window: *mut objc2::runtime::AnyObject, _frame: objc2_foundation::NSRect) -> objc2::runtime::Bool {
    use objc2::runtime::{AnyObject, Bool};
    use objc2::{class, msg_send};
    use objc2_foundation::NSRect;

    unsafe {
        let screen: *mut AnyObject = msg_send![window, screen];
        if screen.is_null() {
            return Bool::YES;
        }
        let visible: NSRect = msg_send![screen, visibleFrame];
        let frame: NSRect = msg_send![window, frame];
        let zoomed = (frame.origin.x - visible.origin.x).abs() < 1.0 && (frame.origin.y - visible.origin.y).abs() < 1.0 && (frame.size.width - visible.size.width).abs() < 1.0 && (frame.size.height - visible.size.height).abs() < 1.0;

        let target = {
            let mut frames = ZOOM_RESTORE_FRAMES.lock().unwrap();
            let saved = frames.iter().position(|(key, _)| *key == window as usize).map(|index| frames.remove(index).1);
            if zoomed {
                // nothing to go back to (e.g. opened zoomed), let appkit handle it
                match saved {
                    Some(saved) => saved,
                    None => return Bool::YES,
                }
            } else {
                frames.push((window as usize, frame));
                visible
            }
        };

        // animate the frame so the webview resizes on every step
        let context_class = class!(NSAnimationContext);
        let _: () = msg_send![context_class, beginGrouping];
        let context: *mut AnyObject = msg_send![context_class, currentContext];
        let duration: f64 = msg_send![window, animationResizeTime: target];
        let _: () = msg_send![context, setDuration: duration];
        let _: () = msg_send![context, setAllowsImplicitAnimation: Bool::YES];
        let animator: *mut AnyObject = msg_send![window, animator];
        let _: () = msg_send![animator, setFrame: target, display: Bool::YES];
        let _: () = msg_send![context_class, endGrouping];
    }

    Bool::NO
}
