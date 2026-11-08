// app settings. defaults live in configs/settings.json, only what the user changed is saved in the app config dir
// and layered on top, so new settings and changed defaults still reach people who have saved settings

use lazy_static::lazy_static;
use serde_json::Value;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Emitter, Manager, Runtime};

static DEFAULT_SETTINGS: &str = include_str!("configs/settings.json");

lazy_static! {
    // defaults with the user's settings on top
    static ref SETTINGS: Mutex<Value> = Mutex::new(serde_json::from_str(DEFAULT_SETTINGS).expect("invalid configs/settings.json"));
    // only what the user changed, this is what's saved
    static ref USER_SETTINGS: Mutex<Value> = Mutex::new(Value::Object(Default::default()));
    // projects opened or saved, newest first (File > Open Recent)
    static ref RECENT: Mutex<Vec<String>> = Mutex::new(Vec::new());
}

// the app, for saving the recent projects and rebuilding the menu when a project is opened or saved
static APP: OnceLock<AppHandle> = OnceLock::new();

fn settings_path<R: Runtime>(app: &AppHandle<R>) -> Option<std::path::PathBuf> {
    app.path().app_config_dir().ok().map(|dir| dir.join("settings.json"))
}

// layers saved values over the defaults
fn merge(base: &mut Value, over: Value) {
    match (base, over) {
        (Value::Object(base), Value::Object(over)) => {
            for (key, value) in over {
                merge(base.entry(key).or_insert(Value::Null), value);
            }
        }
        (base, over) => *base = over,
    }
}

// called once from setup
pub fn load_settings(app: &tauri::AppHandle) {
    let Some(path) = settings_path(app) else {
        return;
    };
    let Ok(data) = std::fs::read_to_string(&path) else {
        return;
    };
    match serde_json::from_str::<Value>(&data) {
        Ok(mut saved) => {
            let migrated = migrate_settings(&mut saved);
            merge(&mut SETTINGS.lock().unwrap(), saved.clone());
            *USER_SETTINGS.lock().unwrap() = saved;
            // the file gets the new names too
            if migrated {
                if let Some(limit) = get_setting("bridge.keyframe_import_limit").as_u64() {
                    save_setting(app, "bridge.keyframe_import_limit", Value::from(limit)).ok();
                }
            }
        }
        Err(e) => eprintln!("ignoring invalid settings file {:?}: {}", path, e),
    }

    // a saved port that isn't valid goes back to the default, so the file matches the port the server uses
    if crate::ipc::valid_port(&get_setting("ipc.port")).is_none() {
        save_setting(app, "ipc.port", Value::from(crate::ipc::DEFAULT_PORT)).ok();
    }
}

/// moves settings saved under an old name to their new one, true if anything moved. `blender.keyframe_import_limit`
/// is `bridge.keyframe_import_limit` now, a value saved under the new name wins
pub fn migrate_settings(saved: &mut Value) -> bool {
    let Some(old) = saved.get_mut("blender").and_then(Value::as_object_mut).and_then(|blender| blender.remove("keyframe_import_limit")) else {
        return false;
    };
    if saved["blender"].as_object().is_some_and(|blender| blender.is_empty()) {
        saved.as_object_mut().unwrap().remove("blender");
    }
    if saved.get("bridge").and_then(|bridge| bridge.get("keyframe_import_limit")).is_none() {
        set_path(saved, "bridge.keyframe_import_limit", old);
    }
    true
}

// reads a setting by dotted path, e.g. "panels.hide_when_inactive". null when it doesn't exist
pub fn get_setting(path: &str) -> Value {
    let settings = SETTINGS.lock().unwrap();
    path.split('.').fold(&*settings, |value, key| &value[key]).clone()
}

// sets a value by dotted path, creating objects along the way
fn set_path(root: &mut Value, path: &str, value: Value) {
    let mut target = root;
    for key in path.split('.') {
        if !target.is_object() {
            *target = Value::Object(Default::default());
        }
        target = target.as_object_mut().unwrap().entry(key).or_insert(Value::Null);
    }
    *target = value;
}

#[tauri::command]
pub fn get_settings() -> Value {
    SETTINGS.lock().unwrap().clone()
}

// sets a setting by dotted path, saves it and tells every window
#[tauri::command]
pub fn set_setting(app: tauri::AppHandle, path: String, value: Value) -> Result<(), String> {
    save_setting(&app, &path, value)?;
    // the zoom picked in the settings window
    if path == "appearance.zoom" {
        crate::ui::windows::zoom_windows(&app);
    }
    Ok(())
}

// set_setting for the backend's own settings (the zoom)
pub fn save_setting<R: Runtime>(app: &AppHandle<R>, path: &str, value: Value) -> Result<(), String> {
    let user_settings = {
        let mut user_settings = USER_SETTINGS.lock().unwrap();
        set_path(&mut user_settings, path, value.clone());
        user_settings.clone()
    };
    let settings = {
        let mut settings = SETTINGS.lock().unwrap();
        set_path(&mut settings, path, value);
        settings.clone()
    };
    // debug mode is read from the threads printing, it's kept apart. the profiler's menu item shows whether it's on
    if path.starts_with("console") {
        crate::utils::console::sync_settings();
    }
    if path == crate::ui::menu::PROFILER_SETTING {
        crate::ui::menu::set_checked(app, "profiler", get_setting(path).as_bool().unwrap_or(false));
    }

    let file = settings_path(app).ok_or("no app config dir")?;
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&file, serde_json::to_string_pretty(&user_settings).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;

    app.emit("settings_changed", &settings).ok();
    Ok(())
}

// MARK: - Recent Projects

/// how many projects File > Open Recent lists
pub const RECENT_LIMIT: usize = 10;

fn recent_path<R: Runtime>(app: &AppHandle<R>) -> Option<std::path::PathBuf> {
    app.path().app_config_dir().ok().map(|dir| dir.join("recent.json"))
}

/// puts `path` first in `list`, once, keeping the newest `RECENT_LIMIT`
pub fn push_recent(list: &mut Vec<String>, path: &str) {
    list.retain(|p| p != path);
    list.insert(0, path.to_string());
    list.truncate(RECENT_LIMIT);
}

/// what the menu calls each project: its file name, with its folder after it when another one has the same name
pub fn recent_labels(list: &[String]) -> Vec<String> {
    let name = |path: &str| Path::new(path).file_name().map_or(path.to_string(), |n| n.to_string_lossy().to_string());
    let folder = |path: &str| Path::new(path).parent().and_then(Path::file_name).map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    list.iter()
        .map(|path| {
            if list.iter().filter(|other| name(other) == name(path)).count() > 1 {
                format!("{} — {}", name(path), folder(path))
            } else {
                name(path)
            }
        })
        .collect()
}

/// the recent projects, newest first
pub fn recent_projects() -> Vec<String> {
    RECENT.lock().unwrap().clone()
}

// called once from setup
pub fn load_recent(app: &tauri::AppHandle) {
    APP.set(app.clone()).ok();
    let Some(path) = recent_path(app) else {
        return;
    };
    let Ok(data) = std::fs::read_to_string(&path) else {
        return;
    };
    match serde_json::from_str::<Vec<String>>(&data) {
        Ok(list) => *RECENT.lock().unwrap() = list.into_iter().take(RECENT_LIMIT).collect(),
        Err(e) => eprintln!("ignoring invalid recent projects file {:?}: {}", path, e),
    }
}

/// a file's real path, the same file through another path is still the same file. a file that isn't there keeps its path
pub fn real_path(path: &str) -> String {
    std::fs::canonicalize(path).map(|path| path.to_string_lossy().to_string()).unwrap_or_else(|_| path.to_string())
}

/// a project was opened or saved, it goes first (by its real path)
pub fn note_recent(path: &str) {
    let path = real_path(path);
    change_recent(|list| push_recent(list, &path));
}

/// a recent project that's gone leaves the list
pub fn forget_recent(path: &str) {
    change_recent(|list| list.retain(|p| p != path));
}

/// File > Open Recent > Clear Menu
pub fn clear_recent() {
    change_recent(Vec::clear);
}

// changes the list, then saves it and rebuilds the menu that shows it. only the list changes without the app (tests)
fn change_recent(change: impl FnOnce(&mut Vec<String>)) {
    let list = {
        let mut recent = RECENT.lock().unwrap();
        let before = recent.clone();
        change(&mut recent);
        if *recent == before {
            return;
        }
        recent.clone()
    };
    let Some(app) = APP.get() else {
        return;
    };
    if let Some(file) = recent_path(app) {
        let written = file.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|_| std::fs::write(&file, serde_json::to_string_pretty(&list).unwrap_or_default()));
        if let Err(e) = written {
            eprintln!("Error saving recent projects {:?}: {}", file, e);
        }
    }
    let handle = app.clone();
    app.run_on_main_thread(move || match crate::ui::menu::build_menu(&handle) {
        Ok(menu) => {
            handle.set_menu(menu).ok();
        }
        Err(e) => eprintln!("Error rebuilding the menu: {}", e),
    })
    .ok();
}
