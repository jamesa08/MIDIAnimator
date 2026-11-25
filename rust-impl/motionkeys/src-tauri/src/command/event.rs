use tauri::{AppHandle, Runtime};

use crate::ui::windows::{open_window, WindowOptions};

pub fn open_settings<R: Runtime>(app: &AppHandle<R>) {
    if let Err(e) = open_window(
        app,
        "Settings",
        "/#/settings",
        "Settings",
        WindowOptions {
            width: 1100.0,
            height: 760.0,
            min_size: None,
            live: false,
            toolbar: false,
        },
    ) {
        eprintln!("Error creating settings window: {:?}", e);
    }
}

/// the graph window (src/windows/Graph.tsx), the curves of the selected nodes
pub fn open_graph<R: Runtime>(app: &AppHandle<R>) {
    if let Err(e) = open_window(
        app,
        "Graph",
        "/#/graph",
        "Graph",
        WindowOptions {
            width: 960.0,
            height: 520.0,
            min_size: Some((500.0, 300.0)),
            live: true,
            toolbar: true,
        },
    ) {
        eprintln!("Error creating graph window: {:?}", e);
    }
}

/// the console window (src/windows/Console.tsx), the console's entries and the last run's timings
pub fn open_console<R: Runtime>(app: &AppHandle<R>) {
    if let Err(e) = open_window(
        app,
        "Console",
        "/#/console",
        "Console",
        WindowOptions {
            width: 1000.0,
            height: 560.0,
            min_size: Some((640.0, 300.0)),
            live: true,
            toolbar: true,
        },
    ) {
        eprintln!("Error creating console window: {:?}", e);
    }
}

/// the about window, the splash screen (splash.html) again, closed with a click. focused if it's already open
pub fn open_about<R: Runtime>(app: &AppHandle<R>) {
    use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
    if let Some(window) = app.get_webview_window(crate::ui::windows::ABOUT_LABEL) {
        window.set_focus().ok();
        return;
    }
    // the splash window's options (tauri.conf.json), its page shows it once the image has loaded
    fn builder<R: Runtime>(app: &AppHandle<R>) -> WebviewWindowBuilder<'_, R, AppHandle<R>> {
        WebviewWindowBuilder::new(app, crate::ui::windows::ABOUT_LABEL, WebviewUrl::App("splash.html".into())).title(format!("About {}", app.package_info().name)).inner_size(400.0, 300.0).decorations(false).resizable(false).visible(false).center()
    }

    #[cfg(target_os = "macos")]
    match builder(app).build() {
        Ok(window) => crate::ui::windows::center_on_mouse_screen(&window),
        Err(e) => eprintln!("Error creating about window: {:?}", e),
    }
    // built from a thread of its own, webview2 deadlocks building a window on the main thread (open_window)
    #[cfg(not(target_os = "macos"))]
    {
        let app = app.clone();
        std::thread::spawn(move || {
            if let Err(e) = builder(&app).build() {
                eprintln!("Error creating about window: {:?}", e);
            }
        });
    }
}
