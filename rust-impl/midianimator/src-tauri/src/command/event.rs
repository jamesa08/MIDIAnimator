use tauri::{AppHandle, Runtime};

use crate::ui::windows::open_window;

pub fn open_settings<R: Runtime>(app: &AppHandle<R>) {
    if let Err(e) = open_window(app, "Settings", "/#/settings", "Settings", 800.0, 600.0) {
        eprintln!("Error creating settings window: {:?}", e);
    }
}
