#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(non_snake_case)]
#![allow(dead_code)]

use std::collections::HashMap;

use MIDIAnimator::ipc::start_server;
use MIDIAnimator::state::{update_state, STATE, WINDOW};
use MIDIAnimator::ui::menu;

use tauri::{generate_context, Manager};

#[derive(Clone, serde::Serialize)]
struct Payload {
    message: String,
}

#[tokio::main]
async fn main() {
    let context = generate_context!();

    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_macos_fps::init())
        .invoke_handler(MIDIAnimator::auto_commands::get_cmds())
        .setup(|app| {
            // build and set menu
            let menu = menu::build_menu(app.handle())?;
            app.set_menu(menu)?;

            let app_handle = app.handle().clone();
            app.on_menu_event(move |_app, event| {
                menu::handle_menu_event(&app_handle, &event);
            });

            // update the global state with the window
            let window = app.get_webview_window("main").unwrap();

            // fix macOS contrast resizing issue
            #[cfg(target_os = "macos")]
            window.with_webview(|webview| {
                use objc2_app_kit::NSWindow;
                unsafe {
                    let ns_window: &NSWindow = &*webview.ns_window().cast::<NSWindow>();
                    ns_window.setPreservesContentDuringLiveResize(false);
                }
            })?;
            *WINDOW.lock().unwrap() = Some(window);

            // load default nodes
            let resource_path = app.path().resolve("src/configs/default_nodes.json", tauri::path::BaseDirectory::Resource).unwrap();
            let data = std::fs::read_to_string(resource_path).unwrap();
            let default_nodes: HashMap<String, serde_json::Value> = serde_json::from_str(&data).unwrap();
            STATE.lock().unwrap().default_nodes = default_nodes;

            tauri::async_runtime::spawn(async move {
                update_state();
                start_server();
            });

            Ok(())
        })
        .run(context)
        .expect("error while launching MIDIAnimator!");
}
