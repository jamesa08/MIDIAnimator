// floating panel windows, behave like photoshop palettes: float above the app's windows and hide while the app is inactive.
// they're created once and never closed, hidden panels stay on screen at alpha 0 so webkit keeps their rendered content.
// ordering a window out drops the webview's layer and showing it again flashes blank until it repaints.
// the drag ghost (node preview that follows the cursor out of a floating panel) works the same way.

use tauri::Manager;

#[cfg(target_os = "macos")]
fn make_floating(window: &tauri::WebviewWindow, ghost: bool) -> Result<(), String> {
    window
        .with_webview(move |webview| {
            use objc2_app_kit::{NSFloatingWindowLevel, NSPopUpMenuWindowLevel, NSWindow, NSWindowCollectionBehavior};
            unsafe {
                let ns_window: &NSWindow = &*webview.ns_window().cast::<NSWindow>();
                // invisible and click through until shown
                ns_window.setAlphaValue(0.0);
                ns_window.setIgnoresMouseEvents(true);
                // the ghost goes above the panels, its node draws its own shadow
                ns_window.setLevel(if ghost { NSPopUpMenuWindowLevel } else { NSFloatingWindowLevel });
                if ghost {
                    ns_window.setHasShadow(false);
                }
                ns_window.setHidesOnDeactivate(true);
                // out of mission control and the cmd+` cycle, stays with the app in fullscreen
                ns_window.setCollectionBehavior(NSWindowCollectionBehavior::Transient | NSWindowCollectionBehavior::IgnoresCycle | NSWindowCollectionBehavior::FullScreenAuxiliary);
                ns_window.orderFront(None);
            }
        })
        .map_err(|e| e.to_string())
}

// called by the panel window once its page has loaded
#[tauri::command]
pub fn panel_window_init(window: tauri::WebviewWindow) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    make_floating(&window, false)?;
    Ok(())
}

// called by the drag ghost window once its page has loaded
#[tauri::command]
pub fn drag_ghost_init(window: tauri::WebviewWindow) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    make_floating(&window, true)?;
    Ok(())
}

// shows or hides a panel or the drag ghost, the ghost never takes mouse events
#[tauri::command]
pub fn floating_window_set_shown(app: tauri::AppHandle, label: String, shown: bool) -> Result<(), String> {
    let window = app.get_webview_window(&label).ok_or(format!("no floating window {}", label))?;
    let clickable = shown && label != "drag-ghost";

    #[cfg(target_os = "macos")]
    window
        .with_webview(move |webview| {
            use objc2_app_kit::NSWindow;
            unsafe {
                let ns_window: &NSWindow = &*webview.ns_window().cast::<NSWindow>();
                ns_window.setAlphaValue(if shown { 1.0 } else { 0.0 });
                ns_window.setIgnoresMouseEvents(!clickable);
                // in case it's shown before the page finished loading
                if shown {
                    ns_window.orderFront(None);
                }
            }
        })
        .map_err(|e| e.to_string())?;

    // other platforms just show and hide
    #[cfg(not(target_os = "macos"))]
    if shown {
        window.show().map_err(|e| e.to_string())?;
    } else {
        window.hide().map_err(|e| e.to_string())?;
    }

    Ok(())
}
