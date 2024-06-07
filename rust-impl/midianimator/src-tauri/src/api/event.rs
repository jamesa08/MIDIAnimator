use tauri::WindowMenuEvent;
use serde_json::json;

use crate::utils::ui::get_logical_size;

pub fn open_settings(event: WindowMenuEvent) {
    // payload must conform to interface WindowOptions
    let win_size = get_logical_size(&event.window());
    let payload = json!({
        "title": "Settings",
        "url": "/#/settings",
        "x": win_size.width / 2 - 400,  // place in the center of the screen
        "y": win_size.height / 2 - 300,
        "width": 800,
        "height": 600
    });

    let res = event.window().emit("open-window", payload);
    
    if let Err(e) = res {
        eprintln!("Error creating settings window: {:?}", e);
    }
}