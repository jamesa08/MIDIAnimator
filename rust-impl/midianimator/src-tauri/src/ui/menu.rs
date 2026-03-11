use crate::command::event;
use crate::ui::keybinds;
use tauri::{
    menu::{Menu, MenuItemBuilder, SubmenuBuilder},
    AppHandle, Runtime,
};

static KEYBINDS: &str = include_str!("../configs/keybinds.json");

pub fn build_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let settings = MenuItemBuilder::with_id("settings", "Settings").accelerator(keybinds::get_keybind(KEYBINDS, "settings".to_string())).build(app)?;

    // recreate the app submenu with the settings item inserted
    let app_submenu = SubmenuBuilder::new(app, app.package_info().name.as_str()).about(None).separator().item(&settings).separator().quit().build()?;

    let menu = tauri::menu::MenuBuilder::new(app).item(&app_submenu).build()?;

    Ok(menu)
}

pub fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, event: &tauri::menu::MenuEvent) {
    match event.id().as_ref() {
        "settings" => {
            event::open_settings(app);
        }
        _ => {
            println!("Unknown event: {}", event.id().as_ref());
        }
    }
}
