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
                // tao only moves the traffic lights to trafficLightPosition when its view draws, which a window
                // revealed this way skips until the first resize
                use objc2::runtime::{AnyObject, Bool};
                let view: *mut AnyObject = objc2::msg_send![ns_window, contentView];
                if !view.is_null() {
                    let _: () = objc2::msg_send![view, setNeedsDisplay: Bool::YES];
                }
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

// smooth live resize, in two parts.
// webkit (macOS 14+) resizes the page in the web process without holding the window for it, so during a live resize
// the page lands a frame or more after the window edge has moved. each resize step waits for webkit to present a frame
// at the new size before appkit commits the window frame, like chromium's WindowResizeHelperMac. webkit's own
// setFrameSize: holds the window until the next presentation update the same way, but only for full-screen transitions.
// remove once webkit syncs it itself: https://github.com/WebKit/WebKit/pull/72971
// waiting caps resizing at the page's frame rate, and a change to the size the page is laid out at repaints every layer
// of it (LocalFrameView::setNeedsFullRepaint), which takes two frames. so for the drag the page is laid out at a fixed
// size (the screen's) and the page fits the app to the window itself (src/utils/liveResize.ts), repainting only what moved
#[cfg(target_os = "macos")]
static WEBVIEW_SET_FRAME_SIZE: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
#[cfg(target_os = "macos")]
static WEBVIEW_WILL_START_LIVE_RESIZE: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
#[cfg(target_os = "macos")]
static WEBVIEW_DID_END_LIVE_RESIZE: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
#[cfg(target_os = "macos")]
static RESIZE_SYNC_MODE: std::sync::OnceLock<usize> = std::sync::OnceLock::new();

// _WKLayoutMode
#[cfg(target_os = "macos")]
const LAYOUT_MODE_VIEW_SIZE: usize = 0;
#[cfg(target_os = "macos")]
const LAYOUT_MODE_FIXED_SIZE: usize = 1;

#[cfg(target_os = "macos")]
thread_local! {
    // a step is being waited on, a nested resize step doesn't wait again
    static RESIZE_WAITING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    // webkit missed a step's deadline, the rest of this live resize doesn't wait
    static RESIZE_GAVE_UP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    // the fixed size the page is laid out at during this live resize, none outside of one
    static RESIZE_LAYOUT_SIZE: std::cell::Cell<Option<(f64, f64)>> = const { std::cell::Cell::new(None) };
}

#[cfg(target_os = "macos")]
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRunLoopGetMain() -> *mut std::ffi::c_void;
    fn CFRunLoopAddCommonMode(run_loop: *mut std::ffi::c_void, mode: *const std::ffi::c_void);
    fn CFRunLoopRunInMode(mode: *const std::ffi::c_void, seconds: f64, return_after_source_handled: u8) -> i32;
}

// overrides setFrameSize: and the live resize start and end on wry's WKWebView subclass, once for every webview.
// the subclass is found from the window's webview, its runtime name is generated
#[cfg(target_os = "macos")]
pub fn sync_live_resize<R: Runtime>(window: &WebviewWindow<R>) {
    window.with_webview(|webview| unsafe { add_live_resize_sync(webview.inner() as *const objc2::runtime::AnyObject) }).ok();
}

#[cfg(target_os = "macos")]
unsafe fn add_live_resize_sync(webview: *const objc2::runtime::AnyObject) {
    use objc2::runtime::{AnyClass, AnyObject, Sel};
    use objc2::{sel, Encode};
    use objc2_foundation::{NSSize, NSString};

    if WEBVIEW_SET_FRAME_SIZE.get().is_some() || webview.is_null() {
        return;
    }
    let Some(webkit) = AnyClass::get("WKWebView") else {
        return;
    };
    // wry's class is the one right under WKWebView, past any subclass the runtime adds on top (e.g. for KVO)
    let mut wry = (*webview).class();
    while let Some(superclass) = wry.superclass() {
        if std::ptr::eq(superclass, webkit) {
            break;
        }
        wry = superclass;
    }
    if !wry.superclass().is_some_and(|superclass| std::ptr::eq(superclass, webkit)) {
        return;
    }
    let (Some(set_frame_size), Some(will_start), Some(did_end)) = (webkit.instance_method(sel!(setFrameSize:)), webkit.instance_method(sel!(viewWillStartLiveResize)), webkit.instance_method(sel!(viewDidEndLiveResize))) else {
        return;
    };
    WEBVIEW_SET_FRAME_SIZE.set(set_frame_size.implementation() as usize).ok();
    WEBVIEW_WILL_START_LIVE_RESIZE.set(will_start.implementation() as usize).ok();
    WEBVIEW_DID_END_LIVE_RESIZE.set(did_end.implementation() as usize).ok();

    // a run loop mode of our own, added to the common modes so webkit's ipc and display refreshes still run in it
    let mode = NSString::from_str("MotionKeysResizeSyncMode");
    let mode_ptr = &*mode as *const NSString as usize;
    std::mem::forget(mode);
    CFRunLoopAddCommonMode(CFRunLoopGetMain(), mode_ptr as *const _);
    RESIZE_SYNC_MODE.set(mode_ptr).ok();

    let wry = wry as *const AnyClass as *mut objc2::ffi::objc_class;
    let types = std::ffi::CString::new(format!("v{}{}{}", <*mut AnyObject>::ENCODING, Sel::ENCODING, NSSize::ENCODING)).unwrap();
    let imp: extern "C" fn(*mut AnyObject, Sel, NSSize) = webview_set_frame_size;
    objc2::ffi::class_addMethod(wry, objc2::ffi::sel_registerName(c"setFrameSize:".as_ptr()), Some(std::mem::transmute(imp)), types.as_ptr());

    let types = std::ffi::CString::new(format!("v{}{}", <*mut AnyObject>::ENCODING, Sel::ENCODING)).unwrap();
    let imp: extern "C" fn(*mut AnyObject, Sel) = webview_will_start_live_resize;
    objc2::ffi::class_addMethod(wry, objc2::ffi::sel_registerName(c"viewWillStartLiveResize".as_ptr()), Some(std::mem::transmute(imp)), types.as_ptr());
    let imp: extern "C" fn(*mut AnyObject, Sel) = webview_did_end_live_resize;
    objc2::ffi::class_addMethod(wry, objc2::ffi::sel_registerName(c"viewDidEndLiveResize".as_ptr()), Some(std::mem::transmute(imp)), types.as_ptr());
}

#[cfg(target_os = "macos")]
unsafe fn evaluate_script(webview: *mut objc2::runtime::AnyObject, script: &str) {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    let script = objc2_foundation::NSString::from_str(script);
    let no_handler: Option<&block2::Block<dyn Fn(*mut AnyObject, *mut AnyObject)>> = None;
    let _: () = msg_send![webview, evaluateJavaScript: &*script, completionHandler: no_handler];
}

// lays the page out at a fixed size, at least the window's, and has the page fit the app to the window.
// sent ahead of the frame that's waited on, so it shows in it
#[cfg(target_os = "macos")]
unsafe fn send_live_size(webview: *mut objc2::runtime::AnyObject, size: objc2_foundation::NSSize) {
    use objc2::msg_send;
    use objc2_foundation::NSSize;

    let Some((mut width, mut height)) = RESIZE_LAYOUT_SIZE.get() else {
        return;
    };
    // a window dragged onto a bigger screen outgrows it, that step repaints everything again
    if size.width > width || size.height > height {
        (width, height) = (width.max(size.width), height.max(size.height));
        RESIZE_LAYOUT_SIZE.set(Some((width, height)));
        let _: () = msg_send![webview, _setFixedLayoutSize: NSSize::new(width, height)];
    }
    evaluate_script(webview, &format!("window.liveResize?.size({},{},{},{})", size.width, size.height, width, height));
}

// waits (at most 100ms) for webkit to present a frame made after everything sent to the web process so far
#[cfg(target_os = "macos")]
unsafe fn wait_for_presentation(webview: *mut objc2::runtime::AnyObject) -> bool {
    use objc2::runtime::Bool;
    use objc2::{class, msg_send};
    use std::time::{Duration, Instant};

    RESIZE_WAITING.set(true);
    let done = std::rc::Rc::new(std::cell::Cell::new(false));
    let block = {
        let done = done.clone();
        block2::RcBlock::new(move || done.set(true))
    };

    // the page's commit lands in this transaction, so it goes out with the window frame instead of before it
    let transaction = class!(CATransaction);
    let _: () = msg_send![transaction, begin];
    let _: () = msg_send![transaction, setDisableActions: Bool::YES];
    let _: () = msg_send![webview, _doAfterNextPresentationUpdate: &*block];

    let mode = *RESIZE_SYNC_MODE.get().unwrap() as *const std::ffi::c_void;
    let deadline = Instant::now() + Duration::from_millis(100);
    while !done.get() {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        CFRunLoopRunInMode(mode, left.as_secs_f64(), 1);
    }
    let _: () = msg_send![transaction, commit];
    RESIZE_WAITING.set(false);
    done.get()
}

#[cfg(target_os = "macos")]
extern "C" fn webview_will_start_live_resize(this: *mut objc2::runtime::AnyObject, sel: objc2::runtime::Sel) {
    use objc2::msg_send;
    use objc2::runtime::{AnyObject, Sel};
    use objc2_foundation::{NSRect, NSSize};

    RESIZE_GAVE_UP.set(false);
    unsafe {
        let original: extern "C" fn(*mut AnyObject, Sel) = std::mem::transmute(*WEBVIEW_WILL_START_LIVE_RESIZE.get().unwrap());
        original(this, sel);

        // laid out at the screen's size for the drag, the window can't get bigger than that without leaving it
        let frame: NSRect = msg_send![this, frame];
        let window: *mut AnyObject = msg_send![this, window];
        let screen: *mut AnyObject = if window.is_null() { std::ptr::null_mut() } else { msg_send![window, screen] };
        let screen_size = if screen.is_null() {
            frame.size
        } else {
            let screen_frame: NSRect = msg_send![screen, frame];
            screen_frame.size
        };
        let (width, height) = (screen_size.width.max(frame.size.width), screen_size.height.max(frame.size.height));
        RESIZE_LAYOUT_SIZE.set(Some((width, height)));
        send_live_size(this, frame.size);
        let _: () = msg_send![this, _setFixedLayoutSize: NSSize::new(width, height)];
        let _: () = msg_send![this, _setLayoutMode: LAYOUT_MODE_FIXED_SIZE];
    }
}

#[cfg(target_os = "macos")]
extern "C" fn webview_did_end_live_resize(this: *mut objc2::runtime::AnyObject, sel: objc2::runtime::Sel) {
    use objc2::msg_send;
    use objc2::runtime::{AnyObject, Sel};

    unsafe {
        // back to laying out at the view's size, shown in one frame with the page's own sizing dropped
        if RESIZE_LAYOUT_SIZE.take().is_some() {
            evaluate_script(this, "window.liveResize?.end()");
            let _: () = msg_send![this, _setLayoutMode: LAYOUT_MODE_VIEW_SIZE];
            wait_for_presentation(this);
        }
        let original: extern "C" fn(*mut AnyObject, Sel) = std::mem::transmute(*WEBVIEW_DID_END_LIVE_RESIZE.get().unwrap());
        original(this, sel);
    }
}

#[cfg(target_os = "macos")]
extern "C" fn webview_set_frame_size(this: *mut objc2::runtime::AnyObject, sel: objc2::runtime::Sel, size: objc2_foundation::NSSize) {
    use objc2::msg_send;
    use objc2::runtime::{AnyObject, Bool, Sel};
    use objc2_foundation::{NSRect, NSSize};

    unsafe {
        // webkit sends the web process the new size
        let original: extern "C" fn(*mut AnyObject, Sel, NSSize) = std::mem::transmute(*WEBVIEW_SET_FRAME_SIZE.get().unwrap());
        let old: NSRect = msg_send![this, frame];
        original(this, sel, size);

        let live: Bool = msg_send![this, inLiveResize];
        if !live.as_bool() || (old.size.width == size.width && old.size.height == size.height) {
            return;
        }
        send_live_size(this, size);
        if RESIZE_WAITING.get() || RESIZE_GAVE_UP.get() {
            return;
        }
        if !wait_for_presentation(this) {
            RESIZE_GAVE_UP.set(true);
        }
    }
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
