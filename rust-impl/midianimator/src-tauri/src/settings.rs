// app settings. defaults live in configs/settings.json, only what the user changed is saved in the app config dir
// and layered on top, so new settings and changed defaults still reach people who have saved settings

use lazy_static::lazy_static;
use serde_json::Value;
use std::sync::Mutex;
use tauri::{Emitter, Manager};

static DEFAULT_SETTINGS: &str = include_str!("configs/settings.json");

lazy_static! {
    // defaults with the user's settings on top
    static ref SETTINGS: Mutex<Value> = Mutex::new(serde_json::from_str(DEFAULT_SETTINGS).expect("invalid configs/settings.json"));
    // only what the user changed, this is what's saved
    static ref USER_SETTINGS: Mutex<Value> = Mutex::new(Value::Object(Default::default()));
}

fn settings_path(app: &tauri::AppHandle) -> Option<std::path::PathBuf> {
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
        Ok(saved) => {
            merge(&mut SETTINGS.lock().unwrap(), saved.clone());
            *USER_SETTINGS.lock().unwrap() = saved;
        }
        Err(e) => eprintln!("ignoring invalid settings file {:?}: {}", path, e),
    }
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
    let user_settings = {
        let mut user_settings = USER_SETTINGS.lock().unwrap();
        set_path(&mut user_settings, &path, value.clone());
        user_settings.clone()
    };
    let settings = {
        let mut settings = SETTINGS.lock().unwrap();
        set_path(&mut settings, &path, value);
        settings.clone()
    };

    let file = settings_path(&app).ok_or("no app config dir")?;
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&file, serde_json::to_string_pretty(&user_settings).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;

    app.emit("settings_changed", &settings).ok();
    Ok(())
}
