// keyboard shortcuts. every command and its default keys live in configs/keybinds.json, grouped by scope: the app, an area
// (the node editor) or a modal (grab, the add menu, dialogs). a preset only saves the keys it changed, kept in the app config
// dir with the active preset, so new commands and changed defaults still reach people with presets of their own.
// the frontend dispatches keys (src/utils/keymap.ts), the menu takes its accelerators from here (ui/menu.rs)

use crate::ui::menu;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, Runtime, WebviewWindow};

static DEFAULT_KEYMAP: &str = include_str!("../configs/keybinds.json");

/// the built-in preset, it can't be changed. changing a key while it's active starts a new preset
pub const DEFAULT_PRESET: &str = "MotionKeys Default";
/// what a preset started by changing the built-in one is called
const CUSTOM_PRESET: &str = "Custom";
/// sent to every window when the keymap or the active preset changes, payload the Keymap
pub const KEYMAP_CHANGED_EVENT: &str = "keymap_changed";

/// modifiers in the order they're written, like the mac menus (⌃⌥⇧⌘)
const MODIFIERS: [&str; 4] = ["Ctrl", "Alt", "Shift", "Cmd"];

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Command {
    pub id: String,
    pub name: String,
    pub keys: Vec<String>,
    /// a menu item runs it, the menu shows its first key that can be a menu shortcut
    #[serde(default)]
    pub menu: bool,
    /// the native menu item's key, can't be changed (cut, copy, paste, select all)
    #[serde(default)]
    pub fixed: bool,
    /// active while its key is held, one key and no modifiers (box select, pan)
    #[serde(default)]
    pub hold: bool,
    /// the key the menu item handles itself, filled in when the keymap is resolved. the frontend leaves it alone
    #[serde(default)]
    pub native: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Scope {
    pub id: String,
    pub name: String,
    /// "app", "area" or "modal"
    pub kind: String,
    pub commands: Vec<Command>,
}

/// the keymap with the active preset's keys, what the frontend gets
#[derive(Serialize, Clone, Debug)]
pub struct Keymap {
    pub preset: String,
    /// the built-in preset first, then the user's by name
    pub presets: Vec<String>,
    /// the active preset is the built-in one
    pub builtin: bool,
    /// "mac" or "other", how keys are written and which modifier CmdOrCtrl is
    pub platform: &'static str,
    pub scopes: Vec<Scope>,
}

/// what's saved: the active preset (none for the built-in one) and each preset's changed keys by "scope.command"
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
struct Saved {
    active: Option<String>,
    presets: BTreeMap<String, BTreeMap<String, Vec<String>>>,
}

lazy_static! {
    static ref DEFAULTS: Vec<Scope> = serde_json::from_str::<Vec<Scope>>(DEFAULT_KEYMAP).expect("invalid configs/keybinds.json").into_iter().map(normalize_scope).collect();
    static ref SAVED: Mutex<Saved> = Mutex::new(Saved::default());
}

fn platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "mac"
    } else {
        "other"
    }
}

/// writes a key the one way the frontend writes it: CmdOrCtrl picked for this platform, modifiers in order, no repeats
pub fn normalize(key: &str) -> String {
    let parts: Vec<&str> = key.split('+').collect();
    let (name, modifiers) = parts.split_last().unwrap_or((&"", &[]));
    let modifiers: Vec<&str> = modifiers
        .iter()
        .map(|m| match *m {
            "CmdOrCtrl" if cfg!(target_os = "macos") => "Cmd",
            "CmdOrCtrl" => "Ctrl",
            m => m,
        })
        .collect();
    let ordered: Vec<&str> = MODIFIERS.iter().copied().filter(|m| modifiers.contains(m)).collect();
    ordered.into_iter().chain(std::iter::once(*name)).collect::<Vec<_>>().join("+")
}

fn normalize_keys(keys: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for key in keys.iter().map(|k| normalize(k)) {
        if !key.is_empty() && !out.contains(&key) {
            out.push(key);
        }
    }
    out
}

fn normalize_scope(mut scope: Scope) -> Scope {
    for command in &mut scope.commands {
        command.keys = normalize_keys(&command.keys);
    }
    scope
}

/// a key the menu can handle itself: with cmd or ctrl (a plain key would take typing away from text fields), on the keyboard
/// part a menu knows (not the numpad or a mouse button)
fn menu_key(key: &str) -> bool {
    let parts: Vec<&str> = key.split('+').collect();
    let name = parts.last().copied().unwrap_or("");
    (parts.contains(&"Cmd") || parts.contains(&"Ctrl")) && !name.starts_with("Num") && !name.ends_with("Mouse")
}

fn saved_path<R: Runtime>(app: &AppHandle<R>) -> Option<std::path::PathBuf> {
    app.path().app_config_dir().ok().map(|dir| dir.join("keymaps.json"))
}

/// called once from setup, before the menu is built
pub fn load_keymap<R: Runtime>(app: &AppHandle<R>) {
    let Some(path) = saved_path(app) else {
        return;
    };
    let Ok(data) = std::fs::read_to_string(&path) else {
        return;
    };
    match serde_json::from_str::<Saved>(&data) {
        Ok(saved) => *SAVED.lock().unwrap() = saved,
        Err(e) => eprintln!("ignoring invalid keymap file {:?}: {}", path, e),
    }
}

fn save<R: Runtime>(app: &AppHandle<R>, saved: &Saved) -> Result<(), String> {
    let file = saved_path(app).ok_or("no app config dir")?;
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&file, serde_json::to_string_pretty(saved).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

/// the defaults with a preset's keys on top, and each menu command's native key
fn resolve(changes: Option<&BTreeMap<String, Vec<String>>>) -> Vec<Scope> {
    let mut scopes = DEFAULTS.clone();
    for scope in &mut scopes {
        for command in &mut scope.commands {
            if let Some(keys) = changes.and_then(|c| c.get(&format!("{}.{}", scope.id, command.id))) {
                if !command.fixed {
                    command.keys = normalize_keys(keys);
                }
            }
            command.native = if command.fixed {
                command.keys.first().cloned()
            } else if command.menu {
                command.keys.iter().find(|k| menu_key(k)).cloned()
            } else {
                None
            };
        }
    }
    scopes
}

/// the keymap with the active preset's keys
#[tauri::command]
pub fn get_keymap() -> Keymap {
    let saved = SAVED.lock().unwrap();
    let preset = saved.active.clone().filter(|name| saved.presets.contains_key(name));
    Keymap {
        builtin: preset.is_none(),
        preset: preset.clone().unwrap_or(DEFAULT_PRESET.to_string()),
        presets: std::iter::once(DEFAULT_PRESET.to_string()).chain(saved.presets.keys().cloned()).collect(),
        platform: platform(),
        scopes: resolve(preset.as_ref().and_then(|name| saved.presets.get(name))),
    }
}

/// the menu shortcut of an app command, none when it has no key a menu can show
pub fn accelerator(command: &str) -> Option<String> {
    get_keymap().scopes.into_iter().find(|s| s.kind == "app")?.commands.into_iter().find(|c| c.id == command)?.native
}

/// saves, then gives the menu its new shortcuts and tells every window
fn changed<R: Runtime>(app: &AppHandle<R>, saved: Saved) -> Result<(), String> {
    save(app, &saved)?;
    *SAVED.lock().unwrap() = saved;
    let menu = menu::build_menu(app).map_err(|e| e.to_string())?;
    app.set_menu(menu).map_err(|e| e.to_string())?;
    app.emit(KEYMAP_CHANGED_EVENT, get_keymap()).ok();
    Ok(())
}

/// a name for a new preset that no other preset has: `base`, then "base 2", "base 3"...
fn unused_name(saved: &Saved, base: &str) -> String {
    let mut name = base.to_string();
    let mut n = 2;
    while name == DEFAULT_PRESET || saved.presets.contains_key(&name) {
        name = format!("{} {}", base, n);
        n += 1;
    }
    name
}

/// sets a command's keys. a key another command in the same scope had is taken from it, a fixed command's keys can't be taken.
/// changing the built-in preset starts a new one
#[tauri::command]
pub fn keymap_set(app: AppHandle, scope: String, command: String, keys: Vec<String>) -> Result<(), String> {
    let mut saved = SAVED.lock().unwrap().clone();
    let active = match saved.active.clone().filter(|name| saved.presets.contains_key(name)) {
        Some(name) => name,
        None => {
            let name = unused_name(&saved, CUSTOM_PRESET);
            saved.presets.insert(name.clone(), BTreeMap::new());
            saved.active = Some(name.clone());
            name
        }
    };

    let resolved = resolve(saved.presets.get(&active));
    let target = resolved.iter().find(|s| s.id == scope).ok_or(format!("no scope {}", scope))?;
    let current = target.commands.iter().find(|c| c.id == command).ok_or(format!("no command {}.{}", scope, command))?;
    if current.fixed {
        return Err(format!("{}.{} can't be changed", scope, command));
    }
    let taken: Vec<&String> = target.commands.iter().filter(|c| c.fixed).flat_map(|c| &c.keys).collect();
    let keys: Vec<String> = normalize_keys(&keys).into_iter().filter(|k| !taken.contains(&k)).collect();

    let defaults = DEFAULTS.iter().find(|s| s.id == scope).unwrap();
    let changes = saved.presets.get_mut(&active).unwrap();
    // only what differs from the defaults is kept
    let mut set = |id: &str, keys: Vec<String>| {
        let path = format!("{}.{}", scope, id);
        if defaults.commands.iter().any(|c| c.id == id && c.keys == keys) {
            changes.remove(&path);
        } else {
            changes.insert(path, keys);
        }
    };
    for other in target.commands.iter().filter(|c| c.id != command && !c.fixed) {
        if other.keys.iter().any(|k| keys.contains(k)) {
            set(&other.id, other.keys.iter().filter(|k| !keys.contains(k)).cloned().collect());
        }
    }
    set(&command, keys);
    changed(&app, saved)
}

/// makes a preset the active one, the built-in one by its name
#[tauri::command]
pub fn keymap_select(app: AppHandle, name: String) -> Result<(), String> {
    let mut saved = SAVED.lock().unwrap().clone();
    if name != DEFAULT_PRESET && !saved.presets.contains_key(&name) {
        return Err(format!("no preset {}", name));
    }
    saved.active = (name != DEFAULT_PRESET).then_some(name);
    changed(&app, saved)
}

/// saves the active preset's keys as a preset called `name` (replacing one with that name) and makes it the active one
#[tauri::command]
pub fn keymap_save_as(app: AppHandle, name: String) -> Result<(), String> {
    let name = name.trim().to_string();
    if name.is_empty() || name == DEFAULT_PRESET {
        return Err(format!("can't save a preset called {:?}", name));
    }
    let mut saved = SAVED.lock().unwrap().clone();
    let changes = saved.active.as_ref().and_then(|active| saved.presets.get(active)).cloned().unwrap_or_default();
    saved.presets.insert(name.clone(), changes);
    saved.active = Some(name);
    changed(&app, saved)
}

/// deletes a preset, the built-in one takes over if it was active
#[tauri::command]
pub fn keymap_delete(app: AppHandle, name: String) -> Result<(), String> {
    let mut saved = SAVED.lock().unwrap().clone();
    if saved.presets.remove(&name).is_none() {
        return Err(format!("no preset {}", name));
    }
    if saved.active.as_deref() == Some(name.as_str()) {
        saved.active = None;
    }
    changed(&app, saved)
}

/// runs an app command the window it was pressed in didn't handle itself (src/utils/keymap.ts). on the main thread like a
/// menu event, a window opened from a command's thread (settings) never loads its page
#[tauri::command]
pub fn run_app_command(app: AppHandle, window: WebviewWindow, id: String) {
    let handle = app.clone();
    app.run_on_main_thread(move || menu::run_command(&handle, Some(&window), &id)).ok();
}
