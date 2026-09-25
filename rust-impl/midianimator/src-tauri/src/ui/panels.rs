// floating panel windows, behave like photoshop palettes: float above the app's windows, drop behind other apps while it's inactive
// (or hide, with the panels.hide_when_inactive setting).
// they're created once and never closed, hidden panels stay on screen at alpha 0 so webkit keeps their rendered content.
// ordering a window out drops the webview's layer and showing it again flashes blank until it repaints
// (https://github.com/manaflow-ai/cmux/issues/4287, thanks @austinywang).
// the drag ghost (node preview that follows the cursor out of a floating panel) works the same way.

use std::sync::Mutex;
use tauri::Manager;

// panels hidden because the app went inactive, shown again when it's active
static HIDDEN_WHILE_INACTIVE: Mutex<Vec<String>> = Mutex::new(Vec::new());

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
                ns_window.setLevel(if ghost {
                    NSPopUpMenuWindowLevel
                } else {
                    NSFloatingWindowLevel
                });
                if ghost {
                    ns_window.setHasShadow(false);
                }
                // out of mission control and the cmd+` cycle, stays with the app in fullscreen
                ns_window.setCollectionBehavior(NSWindowCollectionBehavior::Transient | NSWindowCollectionBehavior::IgnoresCycle | NSWindowCollectionBehavior::FullScreenAuxiliary);
                ns_window.orderFront(None);
            }
        })
        .map_err(|e| e.to_string())
}

// floating while the app is active, normal level (or hidden) while it isn't so they don't sit over other apps.
// called once from setup
#[cfg(target_os = "macos")]
pub fn watch_app_active(app: tauri::AppHandle) {
    use block2::RcBlock;
    use objc2_app_kit::{NSApplicationDidBecomeActiveNotification, NSApplicationDidResignActiveNotification};
    use objc2_foundation::{NSNotification, NSNotificationCenter};
    use std::ptr::NonNull;

    for (name, active) in unsafe { [(NSApplicationDidBecomeActiveNotification, true), (NSApplicationDidResignActiveNotification, false)] } {
        let app = app.clone();
        let block = RcBlock::new(move |_: NonNull<NSNotification>| set_floating_level(&app, active));
        unsafe {
            let observer = NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(Some(name), None, None, &block);
            // observes for the life of the app
            std::mem::forget(observer);
        }
    }
}

// runs on the main thread, notifications are posted there
#[cfg(target_os = "macos")]
fn set_floating_level(app: &tauri::AppHandle, active: bool) {
    use objc2_app_kit::{NSFloatingWindowLevel, NSNormalWindowLevel, NSPopUpMenuWindowLevel, NSWindow};

    let hide = !active && crate::settings::get_setting("panels.hide_when_inactive").as_bool().unwrap_or(false);
    let mut hidden = HIDDEN_WHILE_INACTIVE.lock().unwrap();

    for (label, window) in app.webview_windows() {
        let ghost = label == "drag-ghost";
        if !ghost && !label.starts_with("panel-") {
            continue;
        }
        let Ok(ptr) = window.ns_window() else {
            continue;
        };
        let ns_window: &NSWindow = unsafe { &*ptr.cast::<NSWindow>() };
        let level = match (active, ghost) {
            (false, _) => NSNormalWindowLevel,
            (true, true) => NSPopUpMenuWindowLevel,
            (true, false) => NSFloatingWindowLevel,
        };
        ns_window.setLevel(level);

        // hidden with alpha like docked panels so they come back without a flash
        if ghost {
            continue;
        }
        unsafe {
            if hide && ns_window.alphaValue() > 0.0 {
                ns_window.setAlphaValue(0.0);
                ns_window.setIgnoresMouseEvents(true);
                hidden.push(label);
            } else if active && hidden.contains(&label) {
                ns_window.setAlphaValue(1.0);
                ns_window.setIgnoresMouseEvents(false);
            }
        }
    }

    if active {
        hidden.clear();
    }
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
                ns_window.setAlphaValue(if shown {
                    1.0
                } else {
                    0.0
                });
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
