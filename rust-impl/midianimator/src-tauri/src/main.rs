#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(non_snake_case)]
#![allow(dead_code)]

use std::collections::HashMap;

use MIDIAnimator::ipc::start_server;
use MIDIAnimator::mcp::start_mcp_server;
use MIDIAnimator::state::{update_state, STATE, WINDOW};
use MIDIAnimator::ui::menu;
use MIDIAnimator::utils::log;

use tauri::{generate_context, Manager};

#[derive(Clone, serde::Serialize)]
struct Payload {
    message: String,
}

#[tokio::main]
async fn main() {
    let context = generate_context!();

    let builder = tauri::Builder::default();
    // one copy at a time, opening another (even a translocated copy from a different folder) shows this one and quits,
    // so two copies never fight over the Blender bridge port. release only, dev builds share the identifier
    #[cfg(not(debug_assertions))]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
        log::log("another copy was opened, showing this one instead");
        let window = app.get_webview_window("splash").or_else(|| app.get_webview_window("main"));
        if let Some(window) = window {
            window.unminimize().ok();
            window.set_focus().ok();
        }
    }));

    builder
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_macos_fps::init())
        .invoke_handler(MIDIAnimator::auto_commands::get_cmds())
        // every page opens at the saved zoom (View menu)
        .on_page_load(|webview, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Started {
                MIDIAnimator::ui::windows::apply_zoom(webview);
            }
        })
        // floating panel windows never close, quit when the main window does
        .on_window_event(|window, event| {
            if window.label() == "main" && matches!(event, tauri::WindowEvent::Destroyed) {
                window.app_handle().exit(0);
            }
        })
        .setup(|app| {
            // log file first, so everything after it is recorded
            log::init(app.handle());

            // build and set menu, with the user's keyboard shortcuts
            MIDIAnimator::ui::keybinds::load_keymap(app.handle());
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
            // splash and main window open on the screen the mouse is on
            if let Some(splash) = app.get_webview_window("splash") {
                MIDIAnimator::ui::windows::center_on_mouse_screen(&splash);
            }
            MIDIAnimator::ui::windows::center_on_mouse_screen(&window);
            // drawn invisibly behind the splash, revealed when the splash closes
            MIDIAnimator::ui::windows::prepare_hidden(&window);
            #[cfg(target_os = "macos")]
            MIDIAnimator::ui::windows::smooth_zoom(&window);
            #[cfg(target_os = "macos")]
            {
                let (x, middle) = MIDIAnimator::ui::windows::MAIN_TRAFFIC_LIGHTS;
                MIDIAnimator::ui::windows::place_traffic_lights(&window, x, middle);
            }
            // page keeps up with the window edges during live resize
            #[cfg(target_os = "macos")]
            MIDIAnimator::ui::windows::sync_live_resize(&window);
            *WINDOW.lock().unwrap() = Some(window);

            MIDIAnimator::settings::load_settings(app.handle());
            // pages that started loading before the settings did
            for window in app.webview_windows().values() {
                MIDIAnimator::ui::windows::apply_zoom(window.as_ref());
                #[cfg(target_os = "macos")]
                MIDIAnimator::ui::windows::update_traffic_lights(window);
            }

            // floating panels drop behind other apps while this one is inactive
            #[cfg(target_os = "macos")]
            MIDIAnimator::ui::panels::watch_app_active(app.handle().clone());

            // load default nodes
            let resource_path = app.path().resolve("src/configs/default_nodes.json", tauri::path::BaseDirectory::Resource).unwrap();
            let data = std::fs::read_to_string(resource_path).unwrap();
            let default_nodes: HashMap<String, serde_json::Value> = serde_json::from_str(&data).unwrap();
            STATE.lock().unwrap().default_nodes = default_nodes;

            // update_state waits for the front end to call ready, the bridge only starts after that
            tauri::async_runtime::spawn(async move {
                log::log("waiting for the front end to be ready");
                update_state();
                log::log("front end ready, starting the Blender bridge");
                start_server();
            });

            // mcp server for ML control
            tauri::async_runtime::spawn(start_mcp_server());

            Ok(())
        })
        .run(context)
        .expect("error while launching MIDIAnimator!");
}
