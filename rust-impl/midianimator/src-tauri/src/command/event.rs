use tauri::{AppHandle, Runtime};

use crate::ui::windows::open_window;

pub fn open_settings<R: Runtime>(app: &AppHandle<R>) {
    if let Err(e) = open_window(app, "Settings", "/#/settings", "Settings", 1100.0, 760.0) {
        eprintln!("Error creating settings window: {:?}", e);
    }
}
