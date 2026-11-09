// floating panel windows, behave like photoshop palettes: float above the app's windows, drop behind other apps while it's inactive
// (or hide, with the panels.hide_when_inactive setting).
// they're created once and never closed, hidden panels stay on screen at alpha 0 so webkit keeps their rendered content.
// ordering a window out drops the webview's layer and showing it again flashes blank until it repaints
// (https://github.com/manaflow-ai/cmux/issues/4287, thanks @austinywang).
// the drag ghost (node preview that follows the cursor out of a floating panel) works the same way.

#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "macos")]
use std::sync::Mutex;
use tauri::Manager;

// floating windows the front end has shown (floating_window_set_shown) and not hidden since. the one place that says
// what should be showing: the app coming back and a page finishing loading both go by it
#[cfg(target_os = "macos")]
static SHOWN: Mutex<Vec<String>> = Mutex::new(Vec::new());
// the panels are hidden because the app went inactive (panels.hide_when_inactive), until it's active again
#[cfg(target_os = "macos")]
static HIDDEN_WHILE_INACTIVE: AtomicBool = AtomicBool::new(false);

// whether the front end has a floating window shown
#[cfg(target_os = "macos")]
fn wanted(label: &str) -> bool {
    SHOWN.lock().unwrap().iter().any(|shown| shown == label)
}

// whether a floating window should be on screen: the front end has it shown and, for a panel, it isn't hidden while the
// app is inactive. the ghost is never hidden for that
#[cfg(target_os = "macos")]
fn on_screen(label: &str) -> bool {
    wanted(label) && !(label.starts_with("panel-") && HIDDEN_WHILE_INACTIVE.load(Ordering::Relaxed))
}

// a hidden window isn't resizable, macos still shows resize cursors at the edges of one at alpha 0 that ignores the mouse
#[cfg(target_os = "macos")]
fn set_resizable(ns_window: &objc2_app_kit::NSWindow, resizable: bool) {
    let mut mask = ns_window.styleMask();
    mask.set(objc2_app_kit::NSWindowStyleMask::Resizable, resizable);
    ns_window.setStyleMask(mask);
}

/// a window at alpha 0 that's still the key window keeps taking keys and menu commands (cmd+w would dock the hidden
/// panel), the main window gets them back. on the main thread
#[cfg(target_os = "macos")]
unsafe fn give_keys_to_main(app: &tauri::AppHandle, ns_window: &objc2_app_kit::NSWindow) {
    if !ns_window.isKeyWindow() {
        return;
    }
    let Some(ptr) = app.get_webview_window("main").and_then(|main| main.ns_window().ok()) else {
        return;
    };
    let main: &objc2_app_kit::NSWindow = &*ptr.cast::<objc2_app_kit::NSWindow>();
    // a minimized main window stays where it is
    if main.isVisible() {
        main.makeKeyWindow();
    }
}

#[cfg(target_os = "macos")]
fn make_floating(window: &tauri::WebviewWindow, ghost: bool) -> Result<(), String> {
    let label = window.label().to_string();
    window
        .with_webview(move |webview| {
            use objc2_app_kit::{NSFloatingWindowLevel, NSPopUpMenuWindowLevel, NSWindow, NSWindowCollectionBehavior};
            unsafe {
                let ns_window: &NSWindow = &*webview.ns_window().cast::<NSWindow>();
                // invisible and click through until shown, a window the front end showed before its page loaded stays shown
                let shown = on_screen(&label);
                ns_window.setAlphaValue(if shown {
                    1.0
                } else {
                    0.0
                });
                ns_window.setIgnoresMouseEvents(!shown || ghost);
                set_resizable(ns_window, shown && !ghost);
                // the ghost goes above the panels, its node draws its own shadow
                ns_window.setLevel(if ghost {
                    NSPopUpMenuWindowLevel
                } else {
                    NSFloatingWindowLevel
                });
                if ghost {
                    ns_window.setHasShadow(false);
                } else {
                    crate::ui::windows::match_ui_background(ns_window);
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
    // coming back shows what the front end has shown, including panels shown while the app was inactive and not ones it
    // hid then
    let reshow = active && HIDDEN_WHILE_INACTIVE.swap(false, Ordering::Relaxed);
    if hide {
        HIDDEN_WHILE_INACTIVE.store(true, Ordering::Relaxed);
    }

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
                set_resizable(ns_window, false);
            } else if reshow && wanted(&label) {
                set_resizable(ns_window, true);
                ns_window.setAlphaValue(1.0);
                ns_window.setIgnoresMouseEvents(false);
            }
            // appkit gives the keys back to the window that had them when the app went inactive, a panel hidden since
            // passes them on
            if active && !wanted(&label) {
                give_keys_to_main(app, ns_window);
            }
        }
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

// shows or hides a panel or the drag ghost, the ghost never takes mouse events.
// position (top left) and size are logical screen pixels, placed in the same main thread pass as the show.
// tao's setPosition/setSize are queued for later, so showing after them still draws a frame at the old spot
#[tauri::command]
pub fn floating_window_set_shown(app: tauri::AppHandle, label: String, shown: bool, position: Option<(f64, f64)>, size: Option<(f64, f64)>) -> Result<(), String> {
    let window = app.get_webview_window(&label).ok_or(format!("no floating window {}", label))?;
    let clickable = shown && label != "drag-ghost";

    #[cfg(target_os = "macos")]
    window
        .with_webview(move |webview| {
            use objc2_app_kit::{NSScreen, NSWindow};
            use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize};
            unsafe {
                let ns_window: &NSWindow = &*webview.ns_window().cast::<NSWindow>();
                // the front end's pick, on the main thread so the app going inactive or coming back sees it in order
                {
                    let mut windows = SHOWN.lock().unwrap();
                    windows.retain(|other| *other != label);
                    if shown {
                        windows.push(label.clone());
                    }
                }
                // a panel shown while it's hidden for the app being inactive waits at alpha 0 for the app to come back
                let visible = on_screen(&label);
                let clickable = clickable && visible;
                // cocoa's origin is the bottom left of the primary screen
                if let (Some(mtm), Some((x, y))) = (MainThreadMarker::new(), position) {
                    if let Some(primary) = NSScreen::screens(mtm).firstObject() {
                        let size = size.map(|(width, height)| NSSize::new(width, height)).unwrap_or(ns_window.frame().size);
                        let bottom = primary.frame().size.height - y - size.height;
                        ns_window.setFrame_display(NSRect::new(NSPoint::new(x, bottom), size), false);
                    }
                }
                ns_window.setAlphaValue(if visible {
                    1.0
                } else {
                    0.0
                });
                ns_window.setIgnoresMouseEvents(!clickable);
                set_resizable(ns_window, clickable);
                // in case it's shown before the page finished loading
                if shown {
                    ns_window.orderFront(None);
                } else {
                    give_keys_to_main(&app, ns_window);
                }
            }
        })
        .map_err(|e| e.to_string())?;

    // other platforms just show and hide
    #[cfg(not(target_os = "macos"))]
    {
        if let Some((x, y)) = position {
            window.set_position(tauri::LogicalPosition::new(x, y)).map_err(|e| e.to_string())?;
        }
        if let Some((width, height)) = size {
            window.set_size(tauri::LogicalSize::new(width, height)).map_err(|e| e.to_string())?;
        }
    }
    #[cfg(not(target_os = "macos"))]
    if shown {
        window.show().map_err(|e| e.to_string())?;
        // after the show, tao on linux panics ignoring events on a window gtk hasn't realized yet
        window.set_ignore_cursor_events(!clickable).map_err(|e| e.to_string())?;
    } else {
        window.hide().map_err(|e| e.to_string())?;
    }

    Ok(())
}
