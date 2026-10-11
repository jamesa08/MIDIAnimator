use crate::command::event;
use crate::ui::keybinds;
use tauri::{
    menu::{Menu, MenuItem, MenuItemBuilder, SubmenuBuilder},
    AppHandle, Emitter, Manager, Runtime, WebviewWindow,
};

// sent to the main window to close its tab, or the window on the last one
pub const CLOSE_TAB_EVENT: &str = "close-tab";
// sent to the main window when open, save or save as is picked from the file menu (or its shortcut), payload "open",
// "save" or "save_as". the frontend does them for the tab on screen
pub const FILE_EVENT: &str = "menu-file";
// sent to the focused window when a menu item is picked (or its shortcut pressed), payload the app command's id.
// the window runs it unless a modal is taking its keys (src/utils/keymap.ts), sending it back to run_app_command when
// it has no handler of its own
pub const APP_COMMAND_EVENT: &str = "app-command";
// sent to the main window to show or hide a panel, payload the panel id (PANELS in src/utils/panels.tsx)
pub const PANEL_TOGGLE_EVENT: &str = "panel-toggle";
/// the history panel's id
const HISTORY_PANEL: u32 = 2;

// a menu item for an app command, with its shortcut from the keymap (ui/keybinds.rs)
fn item<R: Runtime>(app: &AppHandle<R>, id: &str, label: &str) -> tauri::Result<MenuItem<R>> {
    let mut builder = MenuItemBuilder::with_id(id, label);
    if let Some(key) = keybinds::accelerator(id) {
        builder = builder.accelerator(key);
    }
    builder.build(app)
}

pub fn build_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let settings = item(app, "settings", "Settings")?;

    // quit closes the main window so it can ask to save first, the app exits once it's gone
    let quit = item(app, "quit", &format!("Quit {}", app.package_info().name))?;
    let close = item(app, "close", "Close Window")?;

    // recreate the app submenu with the settings item inserted
    let app_submenu = SubmenuBuilder::new(app, app.package_info().name.as_str()).about(None).separator().item(&settings).separator().item(&quit).build()?;
    let new_tab = item(app, "new_tab", "New Tab")?;
    let open = item(app, "open", "Open...")?;
    let save = item(app, "save", "Save")?;
    let save_as = item(app, "save_as", "Save As...")?;
    let file_submenu = SubmenuBuilder::new(app, "File").item(&new_tab).item(&open).separator().item(&save).item(&save_as).build()?;
    let history = item(app, "history", "History")?;
    let graph = item(app, "graph", "Graph")?;
    let window_submenu = SubmenuBuilder::new(app, "Window").minimize().separator().item(&history).item(&graph).separator().item(&close).build()?;

    // undo/redo are ours, the focused window decides between a text field's own undo and the graph's.
    // cut/copy/paste/select all are the native ones so text fields keep working
    let undo = item(app, "undo", "Undo")?;
    let redo = item(app, "redo", "Redo")?;
    let edit_submenu = SubmenuBuilder::new(app, "Edit").item(&undo).item(&redo).separator().cut().copy().paste().select_all().build()?;

    let zoom_in = item(app, "zoom_in", "Zoom In")?;
    let zoom_out = item(app, "zoom_out", "Zoom Out")?;
    let actual_size = item(app, "actual_size", "Actual Size")?;
    let view_submenu = SubmenuBuilder::new(app, "View").item(&actual_size).item(&zoom_in).item(&zoom_out).build()?;

    let menu = tauri::menu::MenuBuilder::new(app).item(&app_submenu).item(&file_submenu).item(&edit_submenu).item(&view_submenu).item(&window_submenu).build()?;

    Ok(menu)
}

// the focused window gets the command, it knows whether a modal or a text field has the keys. run here when no window
// is focused (the menu used from the background)
pub fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, event: &tauri::menu::MenuEvent) {
    let id = event.id().as_ref();
    match app.webview_windows().into_values().find(|window| window.is_focused().unwrap_or(false)) {
        Some(window) => {
            window.emit_to(window.label(), APP_COMMAND_EVENT, id).ok();
        }
        None => run_command(app, None, id),
    }
}

// runs an app command for `window` (the main window when none)
pub fn run_command<R: Runtime>(app: &AppHandle<R>, window: Option<&WebviewWindow<R>>, id: &str) {
    match id {
        "settings" => {
            event::open_settings(app);
        }
        "close" => {
            // the main window closes a tab first, other windows handle their own close request (panels dock)
            let Some(window) = window.cloned().or_else(|| app.get_webview_window("main")) else {
                return;
            };
            if window.label() == "main" {
                window.emit_to("main", CLOSE_TAB_EVENT, ()).ok();
            } else {
                window.close().ok();
            }
        }
        "new_tab" => {
            crate::state::create_instance();
        }
        "open" | "save" | "save_as" => {
            app.emit_to("main", FILE_EVENT, id).ok();
        }
        "history" => {
            app.emit_to("main", PANEL_TOGGLE_EVENT, HISTORY_PANEL).ok();
        }
        "graph" => {
            event::open_graph(app);
        }
        "zoom_in" => {
            crate::ui::windows::step_zoom(app, 1);
        }
        "zoom_out" => {
            crate::ui::windows::step_zoom(app, -1);
        }
        "actual_size" => {
            crate::ui::windows::step_zoom(app, 0);
        }
        "undo" => {
            tauri::async_runtime::spawn(crate::state::history::history_undo());
        }
        "redo" => {
            tauri::async_runtime::spawn(crate::state::history::history_redo());
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
            println!("Unknown app command: {}", id);
        }
    }
}
