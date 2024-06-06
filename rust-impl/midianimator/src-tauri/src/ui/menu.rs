use tauri::{CustomMenuItem, Menu, MenuEntry, WindowMenuEvent};
use crate::ui::keybinds;
use crate::api::event;

static KEYBINDS: &str = include_str!("../configs/keybinds.json");

pub fn build_menu(app_name: &str) -> Menu {
    let mut os_menu = Menu::os_default(app_name);
    
    for item in &mut os_menu.items.iter_mut() {
        // modify the menu items here
        if let MenuEntry::Submenu(submenu) = item {
            println!("{:?}", submenu);
            if submenu.title == app_name.to_string() {
                // index 0 is the about menu item, lets insert after that
                submenu.inner.items.insert(1, MenuEntry::CustomItem(
                    CustomMenuItem::new("settings", "Settings")
                        .accelerator(keybinds::get_keybind(KEYBINDS, "settings".to_string()))
                ));
            }
        }
    }

    return os_menu;
}

pub fn handle_menu_event(event: WindowMenuEvent) {
    match event.menu_item_id() {
        "settings" => {
            event::open_settings(event);
        },
        _ => {
            println!("Unknown event: {}", event.menu_item_id());
        }
    }
}