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

