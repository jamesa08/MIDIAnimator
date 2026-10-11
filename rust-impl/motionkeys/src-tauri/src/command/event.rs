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
