use crate::command::event;
use crate::ui::keybinds;
use tauri::{
    menu::{Menu, MenuItemBuilder, SubmenuBuilder},
    AppHandle, Emitter, Manager, Runtime,
};

// sent to the main window to close its tab, or the window on the last one
pub const CLOSE_TAB_EVENT: &str = "close-tab";
// sent to the main window when open, save or save as is picked from the file menu (or its shortcut), payload "open",
// "save" or "save_as". the frontend does them for the tab on screen
pub const FILE_EVENT: &str = "menu-file";
// sent to the focused window when undo or redo is picked from the edit menu (or its shortcut), payload "undo" or "redo"
pub const EDIT_EVENT: &str = "menu-edit";
// sent to the main window to show or hide a panel, payload the panel id (PANELS in src/utils/panels.tsx)
pub const PANEL_TOGGLE_EVENT: &str = "panel-toggle";
/// the history panel's id
const HISTORY_PANEL: u32 = 2;

static KEYBINDS: &str = include_str!("../configs/keybinds.json");

pub fn build_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let settings = MenuItemBuilder::with_id("settings", "Settings").accelerator(keybinds::get_keybind(KEYBINDS, "settings".to_string())).build(app)?;

    // quit closes the main window so it can ask to save first, the app exits once it's gone
    let quit = MenuItemBuilder::with_id("quit", format!("Quit {}", app.package_info().name)).accelerator("CmdOrCtrl+Q").build(app)?;
    let close = MenuItemBuilder::with_id("close_window", "Close Window").accelerator(keybinds::get_keybind(KEYBINDS, "close".to_string())).build(app)?;

    // recreate the app submenu with the settings item inserted
    let app_submenu = SubmenuBuilder::new(app, app.package_info().name.as_str()).about(None).separator().item(&settings).separator().item(&quit).build()?;
    let open = MenuItemBuilder::with_id("open", "Open...").accelerator(keybinds::get_keybind(KEYBINDS, "open".to_string())).build(app)?;
    let save = MenuItemBuilder::with_id("save", "Save").accelerator(keybinds::get_keybind(KEYBINDS, "save".to_string())).build(app)?;
    let save_as = MenuItemBuilder::with_id("save_as", "Save As...").accelerator(keybinds::get_keybind(KEYBINDS, "save_as".to_string())).build(app)?;
    let file_submenu = SubmenuBuilder::new(app, "File").item(&open).separator().item(&save).item(&save_as).build()?;
    let history = MenuItemBuilder::with_id("history", "History").build(app)?;
    let window_submenu = SubmenuBuilder::new(app, "Window").minimize().separator().item(&history).separator().item(&close).build()?;

    // undo/redo are ours, the focused window decides between a text field's own undo and the graph's.
    // cut/copy/paste/select all are the native ones so text fields keep working
    let undo = MenuItemBuilder::with_id("undo", "Undo").accelerator(keybinds::get_keybind(KEYBINDS, "undo".to_string())).build(app)?;
    let redo = MenuItemBuilder::with_id("redo", "Redo").accelerator(keybinds::get_keybind(KEYBINDS, "redo".to_string())).build(app)?;
    let edit_submenu = SubmenuBuilder::new(app, "Edit").item(&undo).item(&redo).separator().cut().copy().paste().select_all().build()?;

    let menu = tauri::menu::MenuBuilder::new(app).item(&app_submenu).item(&file_submenu).item(&edit_submenu).item(&window_submenu).build()?;

    Ok(menu)
}

pub fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, event: &tauri::menu::MenuEvent) {
    match event.id().as_ref() {
        "settings" => {
            event::open_settings(app);
        }
        "close_window" => {
            // the main window closes a tab first, other windows handle their own close request (panels dock)
            let Some(window) = app.webview_windows().into_values().find(|window| window.is_focused().unwrap_or(false)) else {
                return;
            };
            if window.label() == "main" {
                window.emit_to("main", CLOSE_TAB_EVENT, ()).ok();
            } else {
                window.close().ok();
            }
        }
        "open" | "save" | "save_as" => {
            app.emit_to("main", FILE_EVENT, event.id().as_ref()).ok();
        }
        "history" => {
            app.emit_to("main", PANEL_TOGGLE_EVENT, HISTORY_PANEL).ok();
        }
        "undo" | "redo" => {
            // sent to the focused window, it knows whether a text field has focus. the main window when none is (menu used from the background)
            let windows = app.webview_windows();
            let Some(window) = windows.values().find(|window| window.is_focused().unwrap_or(false)).or_else(|| windows.get("main")) else {
                return;
            };
            window.emit_to(window.label(), EDIT_EVENT, event.id().as_ref()).ok();
        }
        "quit" => {
            if let Some(main) = app.get_webview_window("main") {
                main.set_focus().ok();
                main.close().ok();
            } else {
                app.exit(0);
            }
        }
        _ => {
            println!("Unknown event: {}", event.id().as_ref());
        }
    }
}
