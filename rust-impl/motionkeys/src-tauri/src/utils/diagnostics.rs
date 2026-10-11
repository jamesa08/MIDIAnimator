// Settings > Blender connection > Save Diagnostics: one text file with the build, the connection state and the log,
// to attach when the Blender bridge doesn't connect. the home folder and user name are taken out before it's saved

use regex::Regex;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

use crate::ipc::bound_port;
use crate::mcp::MCP_PORT;
use crate::settings::get_setting;
use crate::state::STATE;
use crate::utils::log::log;

/// asks where to save, then writes the anonymized report there. cancelling saves nothing
#[tauri::command]
pub async fn save_diagnostics(window: tauri::WebviewWindow) -> Result<(), String> {
    let report = anonymize(&report(window.app_handle()), &std::env::var("HOME").unwrap_or_default(), &std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_default());
    let Some(path) = window.dialog().file().set_title("Save Diagnostics").add_filter("Text", &["txt"]).set_file_name("motionkeys_diagnostics.txt").blocking_save_file() else {
        return Ok(());
    };
    let path = path.into_path().map_err(|e| e.to_string())?;
    std::fs::write(&path, report).map_err(|e| e.to_string())?;
    log("saved diagnostics");
    Ok(())
}

/// swaps the home folder for ~, any other user folder for /Users/<user>, and the user name on its own for <user>
pub fn anonymize(text: &str, home: &str, user: &str) -> String {
    let mut text = if home.len() > 1 { text.replace(home, "~") } else { text.to_string() };
    text = Regex::new(r"/Users/[^/\s]+").unwrap().replace_all(&text, "/Users/<user>").into_owned();
    // Windows user folders can have spaces, so it runs to the next backslash
    text = Regex::new(r"(?i)([A-Z]:\\Users\\)[^\\\r\n]+").unwrap().replace_all(&text, "${1}<user>").into_owned();
    // whole words only, so a user named james doesn't change com.jamesa08
    if !user.is_empty() {
        text = Regex::new(&format!(r"\b{}\b", regex::escape(user))).unwrap().replace_all(&text, "<user>").into_owned();
    }
    text
}

// the build, where it runs from, the port and connection state, then the log (the older file first)
fn report(app: &tauri::AppHandle) -> String {
    let mut lines = vec!["== MotionKeys diagnostics".to_string()];
    lines.push(format!("saved: {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S")));
    lines.push(format!("version: {} ({})", env!("CARGO_PKG_VERSION"), env!("GIT_HASH")));
    lines.push(format!("os: {} {}", os_version(), std::env::consts::ARCH));
    lines.push(format!("running from: {}", std::env::current_exe().map_or("unknown".to_string(), |path| path.display().to_string())));

    // the raw setting shows a port saved wrong (a quoted "6577"), 0 means the bridge isn't listening
    lines.push(String::new());
    lines.push("== connection".to_string());
    lines.push(format!("port setting: {}", get_setting("ipc.port")));
    lines.push(format!("bridge port: {}", bound_port()));
    lines.push(format!("mcp port: {}", MCP_PORT));
    {
        let state = STATE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        lines.push(format!("front end ready: {}", state.ready));
        lines.push(format!("connected: {}", state.connected));
        lines.push(format!("application: {} {}", state.connected_application, state.connected_version));
        lines.push(format!("blend file: {}", state.connected_file_name));
        lines.push(format!("linked tab: {}", state.connected_instance_id.is_some()));
    }

    let dir = app.path().app_log_dir().ok();
    for name in ["motionkeys.old.log", "motionkeys.log"] {
        let Some(path) = dir.as_ref().map(|dir| dir.join(name)) else {
            continue;
        };
        if let Ok(log) = std::fs::read_to_string(&path) {
            lines.push(String::new());
            lines.push(format!("== {}", name));
            lines.push(log.trim_end().to_string());
        }
    }
    lines.join("\n") + "\n"
}

// e.g. "macOS 15.6.1"
fn os_version() -> String {
    #[cfg(target_os = "macos")]
    if let Ok(output) = std::process::Command::new("sw_vers").arg("-productVersion").output() {
        return format!("macOS {}", String::from_utf8_lossy(&output.stdout).trim());
    }
    std::env::consts::OS.to_string()
}
