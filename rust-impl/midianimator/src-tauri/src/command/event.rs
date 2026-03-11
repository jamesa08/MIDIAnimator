use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::utils::ui::get_logical_size;

pub fn open_settings<R: Runtime>(app: &AppHandle<R>) {
    let window = app.get_webview_window("main").unwrap();
    let win_size = get_logical_size(&window);
    let payload = json!({
        "title": "Settings",
        "url": "/#/settings",
        "x": win_size.width / 2 - 400,
        "y": win_size.height / 2 - 300,
        "width": 800,
        "height": 600
    });

    if let Err(e) = window.emit("open-window", payload) {
        eprintln!("Error creating settings window: {:?}", e);
    }
}
