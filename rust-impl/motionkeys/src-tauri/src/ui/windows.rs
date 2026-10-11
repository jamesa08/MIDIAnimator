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

/// the height of a window's own toolbar (src/windows/Graph.tsx), the traffic lights are centered on it
#[cfg(target_os = "macos")]
const TOOLBAR_HEIGHT: f64 = 36.0;
/// how far below the y the title bar is grown by the traffic lights' middle ends up (their title bar is grown to the
/// buttons' height + y and they keep their place from its bottom). measured from screenshots at y = 11 and 20
#[cfg(target_os = "macos")]
const TRAFFIC_LIGHT_MIDDLE: f64 = 2.25;
/// how far in from the left they sit
#[cfg(target_os = "macos")]
const TRAFFIC_LIGHT_X: f64 = 14.0;

/// how a window opens: its size (logical pixels) and how small it can be dragged
#[derive(Clone, Copy, Debug)]
pub struct WindowOptions {
    pub width: f64,
    pub height: f64,
    pub min_size: Option<(f64, f64)>,
    /// keeps running while the app is in the background
    pub live: bool,
    /// the page draws its own toolbar, on macOS the title bar goes and the traffic lights sit centered on the toolbar's
    /// TOOLBAR_HEIGHT (like the main window's)
    pub toolbar: bool,
}

// opens a window that's revealed once its page has drawn, focuses it if it's already open.
// only macOS waits for the page: webview2 doesn't run animation frames for a hidden window, so its page would never report
// ready. elsewhere it's shown once built, the white background keeps it from flashing
pub fn open_window<R: Runtime>(app: &AppHandle<R>, label: &str, url: &str, title: &str, options: WindowOptions) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(label) {
        #[cfg(target_os = "macos")]
        return window.set_focus();
        #[cfg(not(target_os = "macos"))]
        {
            reveal(&window);
            return Ok(());
        }
    }

    // registered before the page can load and report ready
    #[cfg(target_os = "macos")]
    {
        PENDING_REVEAL.lock().unwrap().push(label.to_string());
        let window = window_builder(app, label, url, title, options).build()?;
        if options.toolbar {
            place_traffic_lights(&window, TRAFFIC_LIGHT_X, TOOLBAR_HEIGHT / 2.0);
        }
        prepare_hidden(&window);
        smooth_zoom(&window);
        Ok(())
    }

    // webview2 deadlocks building a window on the main thread (menu events, run_app_command): the event loop stops
    // handling every other window call, so it's built from a thread of its own.
    // https://github.com/MicrosoftEdge/WebView2Feedback/issues/1600
    #[cfg(not(target_os = "macos"))]
    {
        let (app, label, url, title) = (app.clone(), label.to_string(), url.to_string(), title.to_string());
        std::thread::spawn(move || match window_builder(&app, &label, &url, &title, options).build() {
            Ok(window) => reveal(&window),
            Err(e) => eprintln!("Error creating window {}: {:?}", label, e),
        });
        Ok(())
    }
}

fn window_builder<'a, R: Runtime>(app: &'a AppHandle<R>, label: &str, url: &str, title: &str, options: WindowOptions) -> WebviewWindowBuilder<'a, R, AppHandle<R>> {
    let mut builder = WebviewWindowBuilder::new(app, label, WebviewUrl::App(url.into())).title(title).inner_size(options.width, options.height).center().visible(false).accept_first_mouse(true).background_color(tauri::window::Color(255, 255, 255, 255));
    if let Some((width, height)) = options.min_size {
        builder = builder.min_inner_size(width, height);
    }
    // a live window keeps up with the graph while the app is in the background, like the floating panels
    if options.live {
        builder = builder.background_throttling(tauri::utils::config::BackgroundThrottlingPolicy::Disabled);
    }
    #[cfg(target_os = "macos")]
    if options.toolbar {
        builder = builder.title_bar_style(tauri::TitleBarStyle::Overlay).hidden_title(true);
    }
    builder
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

/// the page zooms zoom in and out step through, like a browser's. the settings window offers the same (ZOOMS in
/// src/windows/Settings.tsx)
const ZOOM_STEPS: [f64; 11] = [0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0];

/// the saved page zoom (appearance.zoom)
fn zoom() -> f64 {
    crate::settings::get_setting("appearance.zoom").as_f64().unwrap_or(1.0)
}

// gives a page the saved zoom, the splash keeps its own size
pub fn apply_zoom<R: Runtime>(webview: &tauri::Webview<R>) {
    if webview.label() != "splash" {
        webview.set_zoom(zoom()).ok();
    }
}

// steps every window's zoom in (1) or out (-1), 0 goes back to actual size. saved so new windows and the next launch get it
pub fn step_zoom<R: Runtime>(app: &AppHandle<R>, step: i32) {
    let current = zoom();
    let next = match step {
        0 => 1.0,
        s if s > 0 => ZOOM_STEPS.iter().copied().find(|z| *z > current + 0.001).unwrap_or(current),
        _ => ZOOM_STEPS.iter().rev().copied().find(|z| *z < current - 0.001).unwrap_or(current),
    };
    if let Err(e) = crate::settings::save_setting(app, "appearance.zoom", serde_json::json!(next)) {
        eprintln!("Error saving zoom: {}", e);
    }
    zoom_windows(app);
}

// gives every window the saved zoom
pub fn zoom_windows<R: Runtime>(app: &AppHandle<R>) {
    for window in app.webview_windows().values() {
        apply_zoom(window.as_ref());
        #[cfg(target_os = "macos")]
        update_traffic_lights(window);
    }
}

// MARK: - Traffic lights

// the traffic lights sit on the middle of a bar the page draws (the main window's tab strip, a window's toolbar), which
// grows with the page zoom while the buttons stay the same size. tauri only places them once, when the window is built
// (tao keeps that spot and puts them back on it), so windows that zoom place them here instead, with the same steps
// tao takes (inset_traffic_lights in tao's platform_impl/macos/view.rs), at the bar's middle for the zoom.
// appkit puts them back in their default spot when the title bar lays out, tao's content view drawing, a title change
// and leaving full screen are where it puts them back, so the same three are hooked here

/// the main window's traffic lights: their x and the middle of the tab strip at actual size (logical pixels)
#[cfg(target_os = "macos")]
pub const MAIN_TRAFFIC_LIGHTS: (f64, f64) = (19.0, 17.5);

/// windows that place their traffic lights here: window pointer, their x and the bar's middle at actual size
#[cfg(target_os = "macos")]
static TRAFFIC_LIGHTS: Mutex<Vec<(usize, f64, f64)>> = Mutex::new(Vec::new());
// the hooked methods' own implementations, run before the lights are placed again
#[cfg(target_os = "macos")]
static VIEW_DRAW_RECT: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
#[cfg(target_os = "macos")]
static WINDOW_SET_TITLE: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
#[cfg(target_os = "macos")]
static DELEGATE_DID_EXIT_FULL_SCREEN: std::sync::OnceLock<usize> = std::sync::OnceLock::new();

// places a window's traffic lights from now on: `x` in from the left, centered on a bar whose middle is `middle` down
// at actual size
#[cfg(target_os = "macos")]
pub fn place_traffic_lights<R: Runtime>(window: &WebviewWindow<R>, x: f64, middle: f64) {
    window
        .with_webview(move |webview| unsafe {
            use objc2::msg_send;
            use objc2::runtime::AnyObject;

            let ns_window = webview.ns_window() as *mut AnyObject;
            {
                let mut windows = TRAFFIC_LIGHTS.lock().unwrap();
                windows.retain(|(key, _, _)| *key != ns_window as usize);
                windows.push((ns_window as usize, x, middle));
            }
            let view: *mut AnyObject = msg_send![ns_window, contentView];
            let delegate: *mut AnyObject = msg_send![ns_window, delegate];
            hook(view, c"drawRect:", &VIEW_DRAW_RECT, view_draw_rect as extern "C" fn(_, _, _) as usize, &format!("v@:{}", <objc2_foundation::NSRect as objc2::Encode>::ENCODING));
            hook(ns_window, c"setTitle:", &WINDOW_SET_TITLE, window_set_title as extern "C" fn(_, _, _) as usize, "v@:@");
            hook(delegate, c"windowDidExitFullScreen:", &DELEGATE_DID_EXIT_FULL_SCREEN, delegate_did_exit_full_screen as extern "C" fn(_, _, _) as usize, "v@:@");
            inset_traffic_lights(ns_window);
        })
        .ok();
}

// places a window's traffic lights for the zoom now, they move with it
#[cfg(target_os = "macos")]
pub fn update_traffic_lights<R: Runtime>(window: &WebviewWindow<R>) {
    window.with_webview(|webview| unsafe { inset_traffic_lights(webview.ns_window() as *mut objc2::runtime::AnyObject) }).ok();
}

// swaps `object`'s class's method for `imp`, keeping its own (or the one it inherits) in `original`. once per method,
// every window shares tao's classes
#[cfg(target_os = "macos")]
unsafe fn hook(object: *mut objc2::runtime::AnyObject, name: &std::ffi::CStr, original: &std::sync::OnceLock<usize>, imp: usize, types: &str) {
    if object.is_null() || original.get().is_some() {
        return;
    }
    let class = (*object).class();
    let Some(own) = class.instance_method(objc2::runtime::Sel::register(name.to_str().unwrap())) else {
        return;
    };
    original.set(own.implementation() as usize).ok();
    let class = class as *const objc2::runtime::AnyClass as *mut objc2::ffi::objc_class;
    let types = std::ffi::CString::new(types).unwrap();
    objc2::ffi::class_replaceMethod(class, objc2::ffi::sel_registerName(name.as_ptr()), Some(std::mem::transmute::<usize, unsafe extern "C" fn()>(imp)), types.as_ptr());
}

#[cfg(target_os = "macos")]
extern "C" fn view_draw_rect(this: *mut objc2::runtime::AnyObject, sel: objc2::runtime::Sel, rect: objc2_foundation::NSRect) {
    unsafe {
        if let Some(original) = VIEW_DRAW_RECT.get() {
            let original: extern "C" fn(*mut objc2::runtime::AnyObject, objc2::runtime::Sel, objc2_foundation::NSRect) = std::mem::transmute(*original);
            original(this, sel, rect);
        }
        let window: *mut objc2::runtime::AnyObject = objc2::msg_send![this, window];
        inset_traffic_lights(window);
    }
}

#[cfg(target_os = "macos")]
extern "C" fn window_set_title(this: *mut objc2::runtime::AnyObject, sel: objc2::runtime::Sel, title: *mut objc2::runtime::AnyObject) {
    unsafe {
        if let Some(original) = WINDOW_SET_TITLE.get() {
            let original: extern "C" fn(*mut objc2::runtime::AnyObject, objc2::runtime::Sel, *mut objc2::runtime::AnyObject) = std::mem::transmute(*original);
            original(this, sel, title);
        }
        inset_traffic_lights(this);
    }
}

#[cfg(target_os = "macos")]
extern "C" fn delegate_did_exit_full_screen(this: *mut objc2::runtime::AnyObject, sel: objc2::runtime::Sel, notification: *mut objc2::runtime::AnyObject) {
    unsafe {
        if let Some(original) = DELEGATE_DID_EXIT_FULL_SCREEN.get() {
            let original: extern "C" fn(*mut objc2::runtime::AnyObject, objc2::runtime::Sel, *mut objc2::runtime::AnyObject) = std::mem::transmute(*original);
            original(this, sel, notification);
        }
        let window: *mut objc2::runtime::AnyObject = objc2::msg_send![notification, object];
        inset_traffic_lights(window);
    }
}

// moves a window's traffic lights onto its bar's middle for the zoom, nothing for a window that doesn't place its own
#[cfg(target_os = "macos")]
unsafe fn inset_traffic_lights(ns_window: *mut objc2::runtime::AnyObject) {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    use objc2_foundation::{NSPoint, NSRect};

    let Some((x, middle)) = TRAFFIC_LIGHTS.lock().unwrap().iter().find(|(key, _, _)| *key == ns_window as usize).map(|(_, x, middle)| (*x, *middle)) else {
        return;
    };
    let y = middle * zoom() - TRAFFIC_LIGHT_MIDDLE;

    // close, minimize, zoom
    let buttons: Vec<*mut AnyObject> = (0..3usize).map(|kind| msg_send![ns_window, standardWindowButton: kind]).collect();
    if buttons.iter().any(|button| button.is_null()) {
        return;
    }
    // the title bar grows to the buttons' height + y, they keep their place from its bottom
    let close: NSRect = msg_send![buttons[0], frame];
    let minimize: NSRect = msg_send![buttons[1], frame];
    let superview: *mut AnyObject = msg_send![buttons[0], superview];
    let title_bar: *mut AnyObject = msg_send![superview, superview];
    if title_bar.is_null() {
        return;
    }
    let window_frame: NSRect = msg_send![ns_window, frame];
    let mut title_bar_frame: NSRect = msg_send![title_bar, frame];
    title_bar_frame.size.height = close.size.height + y;
    title_bar_frame.origin.y = window_frame.size.height - title_bar_frame.size.height;
    let _: () = msg_send![title_bar, setFrame: title_bar_frame];

    let spacing = minimize.origin.x - close.origin.x;
    for (i, button) in buttons.into_iter().enumerate() {
        let frame: NSRect = msg_send![button, frame];
        let _: () = msg_send![button, setFrameOrigin: NSPoint::new(x + i as f64 * spacing, frame.origin.y)];
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
