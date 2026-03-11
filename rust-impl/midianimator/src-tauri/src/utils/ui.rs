use tauri::{Emitter, Manager, Runtime};

pub fn get_logical_size<R: Runtime>(window: &tauri::WebviewWindow<R>) -> tauri::LogicalSize<u32> {
    let cur_monitor: tauri::Monitor = window.current_monitor().unwrap().unwrap();
    let s_factor: f64 = cur_monitor.scale_factor();
    let phys_size: &tauri::PhysicalSize<u32> = cur_monitor.size();
    let logical_size: tauri::LogicalSize<u32> = phys_size.to_logical(s_factor);

    return logical_size;
}

#[tauri::command]
pub async fn splash_progress(app: tauri::AppHandle, message: String) {
    app.emit("splash_progress", message).ok();
}

#[tauri::command]
pub fn get_build_info() -> (String, String) {
    let version = env!("CARGO_PKG_VERSION").to_string();
    let hash = env!("GIT_HASH").to_string();
    (version, hash)
}

#[tauri::command]
pub async fn close_splashscreen(app: tauri::AppHandle) {
    // 500 ms delay to make sure the user sees the splash screen and doesn't get confused when it flashes away immediately on fast machines.
    // #seemyartplz
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Close splash
    if let Some(splash) = app.get_webview_window("splash") {
        splash.close().unwrap();
    }
    // Show main window
    if let Some(main) = app.get_webview_window("main") {
        main.show().unwrap();
    }
}
