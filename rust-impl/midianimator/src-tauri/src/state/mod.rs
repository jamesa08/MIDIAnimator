use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::Arc;
use std::sync::{MutexGuard, PoisonError};
use std::{collections::HashMap, sync::Mutex};
use tauri::Emitter;
use tauri_plugin_dialog::DialogExt;

use crate::blender::scene_data::{compare_scene_data, get_scene_data};
use crate::graph::execute::{forget_memo, run_instance};
use crate::scene_generics::Scene;

pub mod graph;
pub mod history;

lazy_static! {
    pub static ref STATE: Mutex<AppState> = Mutex::new(AppState::default());
    pub static ref WINDOW: Arc<Mutex<Option<tauri::WebviewWindow>>> = Arc::new(Mutex::new(None));
    // last status sent to the front end, only changes are sent
    static ref LAST_STATUS: Mutex<Option<ProjectStatus>> = Mutex::new(None);
}

/// state struct for the application
///
/// this is a global state that is shared between the front end and the backend.
///
/// front end also has its own state, but this is the global state that is shared between the two.
///
/// note: the only way to update this state is through the backend, and the front end can only read from it.
///       if you wanted to change a variable, you will have to create a command in the backend that will update the state.
///       the front end gets it as a `StateView`
#[derive(Clone, Debug)]
pub struct AppState {
    pub ready: bool,
    pub connected: bool,
    pub default_nodes: HashMap<String, serde_json::Value>,
    pub connected_application: String,
    pub connected_version: String,
    pub connected_file_name: String,
    /// goes up every time a tab's graph changes, so the frontend can tell an older graph from a newer one
    pub graph_rev: u64,
    /// goes up with every state sent to the frontend, so it can tell an older state from a newer one
    pub state_rev: u64,

    /// the open tabs in the order they're shown, there's always at least one. each is its own file, like an instance of
    /// the app of its own
    pub instances: Vec<InstanceState>,
    /// the tab on screen
    pub active_instance_id: String,
    /// the tab Blender is linked to (live). it stays linked while Blender is away, and is linked again when it's back
    pub connected_instance_id: Option<String>,
    /// numbers the next tab's id, ids are never reused
    next_instance: u64,
}

/// one tab: a file's node graph with the scene data it runs on, what it gave in its last run and the window's layout
#[derive(Clone, Debug)]
pub struct InstanceState {
    pub id: String,
    /// the name of a tab that hasn't been saved ("Graph 1"), a saved tab is named after its file
    pub label: String,
    /// the file the tab was opened from or last saved to
    pub path: Option<String>,
    /// the window's layout in this tab (panels, floating windows), the frontend's, saved with the file
    pub layout: serde_json::Value,
    pub scene_data: HashMap<String, Scene>,
    pub rf_instance: HashMap<String, serde_json::Value>,
    pub executed_results: HashMap<String, serde_json::Value>,
    pub executed_inputs: HashMap<String, serde_json::Value>,
    pub pending_scene_data: Option<HashMap<String, Scene>>,
    pub execution_paused: bool,
    /// path of the node group open in the editor (`group-1/group-2`), empty at the top level
    pub open_group: String,
    /// `AppState::graph_rev` when this tab's graph last changed
    pub graph_rev: u64,
    /// the graph as of the last save or load (or when the tab was made), compared to find unsaved changes
    saved: serde_json::Value,
}

impl InstanceState {
    /// a tab with an empty graph, nothing to save
    pub fn new(id: String, label: String) -> Self {
        let mut instance = Self {
            id,
            label,
            path: None,
            layout: serde_json::Value::Null,
            scene_data: HashMap::new(),
            rf_instance: empty_graph(),
            executed_results: HashMap::new(),
            executed_inputs: HashMap::new(),
            pending_scene_data: None,
            execution_paused: false,
            open_group: String::new(),
            graph_rev: 0,
            saved: serde_json::Value::Null,
        };
        instance.mark_saved();
        instance
    }

    pub fn mark_saved(&mut self) {
        self.saved = graph_key(&self.rf_instance);
    }

    /// true if the graph changed since the tab was last saved, opened or made
    pub fn unsaved(&self) -> bool {
        self.saved != graph_key(&self.rf_instance)
    }

    /// the tab's name: its file's name (without .mkproj), or its label if it hasn't been saved
    pub fn name(&self) -> String {
        self.path.as_deref().and_then(|path| std::path::Path::new(path).file_stem()).map(|stem| stem.to_string_lossy().to_string()).unwrap_or_else(|| self.label.clone())
    }

    /// a tab that hasn't been saved or touched, opening a file uses it instead of adding a tab
    pub fn untouched(&self) -> bool {
        self.path.is_none() && !self.unsaved() && self.rf_instance.get("nodes").and_then(|nodes| nodes.as_array()).is_none_or(|nodes| nodes.is_empty())
    }
}

impl Default for AppState {
    fn default() -> Self {
        let first = InstanceState::new("tab-1".to_string(), "Graph 1".to_string());
        Self {
            ready: false,
            connected: false,
            default_nodes: HashMap::new(),
            connected_application: "".to_string(),
            connected_version: "".to_string(),
            connected_file_name: "".to_string(),
            graph_rev: 0,
            state_rev: 0,
            active_instance_id: first.id.clone(),
            instances: vec![first],
            connected_instance_id: None,
            next_instance: 2,
        }
    }
}

impl AppState {
    pub fn instance(&self, id: &str) -> Option<&InstanceState> {
        self.instances.iter().find(|instance| instance.id == id)
    }

    pub fn instance_mut(&mut self, id: &str) -> Option<&mut InstanceState> {
        self.instances.iter_mut().find(|instance| instance.id == id)
    }

    /// the tab on screen
    pub fn active(&self) -> &InstanceState {
        self.instance(&self.active_instance_id).unwrap_or(&self.instances[0])
    }

    pub fn active_mut(&mut self) -> &mut InstanceState {
        let index = self.instances.iter().position(|instance| instance.id == self.active_instance_id).unwrap_or(0);
        &mut self.instances[index]
    }

    /// true if the tab is linked to Blender and Blender is connected
    pub fn is_live(&self, id: &str) -> bool {
        self.connected && self.connected_instance_id.as_deref() == Some(id)
    }

    /// a tab id that hasn't been used, ids are never reused
    fn next_id(&mut self) -> String {
        let id = format!("tab-{}", self.next_instance);
        self.next_instance += 1;
        id
    }

    /// adds an empty tab after the others, named "Graph" with the lowest number no other tab is named. returns its id
    pub fn add_instance(&mut self) -> String {
        let id = self.next_id();
        let label = (1..).map(|n| format!("Graph {}", n)).find(|label| !self.instances.iter().any(|instance| &instance.name() == label)).unwrap_or_default();
        self.instances.push(InstanceState::new(id.clone(), label));
        id
    }

    /// true if any tab has unsaved changes
    pub fn unsaved(&self) -> bool {
        self.instances.iter().any(InstanceState::unsaved)
    }

    /// the state as the front end sees it, a newer one each time
    pub fn view(&mut self) -> StateView {
        self.state_rev += 1;
        let active = self.active();
        StateView {
            ready: self.ready,
            connected: self.connected,
            default_nodes: self.default_nodes.clone(),
            connected_application: self.connected_application.clone(),
            connected_version: self.connected_version.clone(),
            connected_file_name: self.connected_file_name.clone(),
            state_rev: self.state_rev,
            tabs: self
                .instances
                .iter()
                .map(|instance| TabInfo {
                    id: instance.id.clone(),
                    label: instance.name(),
                    path: instance.path.clone(),
                    linked: self.connected_instance_id.as_ref() == Some(&instance.id),
                })
                .collect(),
            active_tab: active.id.clone(),
            graph_rev: active.graph_rev,
            rf_instance: active.rf_instance.clone(),
            scene_data: active.scene_data.clone(),
            executed_results: active.executed_results.clone(),
            executed_inputs: active.executed_inputs.clone(),
            pending_scene_data: active.pending_scene_data.clone(),
            execution_paused: active.execution_paused,
            open_group: active.open_group.clone(),
            layout: active.layout.clone(),
        }
    }
}

/// what the front end gets of the state: the app's parts, the tabs, and the tab on screen's parts
#[derive(Serialize, Clone, Debug)]
pub struct StateView {
    pub ready: bool,
    pub connected: bool,
    pub default_nodes: HashMap<String, serde_json::Value>,
    pub connected_application: String,
    pub connected_version: String,
    pub connected_file_name: String,
    pub state_rev: u64,
    pub tabs: Vec<TabInfo>,
    pub active_tab: String,
    pub graph_rev: u64,
    pub rf_instance: HashMap<String, serde_json::Value>,
    pub scene_data: HashMap<String, Scene>,
    pub executed_results: HashMap<String, serde_json::Value>,
    pub executed_inputs: HashMap<String, serde_json::Value>,
    pub pending_scene_data: Option<HashMap<String, Scene>>,
    pub execution_paused: bool,
    pub open_group: String,
    pub layout: serde_json::Value,
}

/// a tab in the tab bar, `label` is its name. `path`: its file, none if it hasn't been saved. `linked`: Blender is linked
/// to it, it's live while Blender is connected
#[derive(Serialize, Clone, Debug)]
pub struct TabInfo {
    pub id: String,
    pub label: String,
    pub path: Option<String>,
    pub linked: bool,
}

/// locks the state, a poisoned lock (a panic somewhere else) still has a usable state
pub fn lock() -> MutexGuard<'static, AppState> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// a graph with nothing in it
fn empty_graph() -> HashMap<String, serde_json::Value> {
    HashMap::from([("nodes".to_string(), serde_json::json!([])), ("edges".to_string(), serde_json::json!([])), ("viewport".to_string(), serde_json::json!({ "x": 0, "y": 0, "zoom": 1 }))])
}

/// this commmand is called when the front end is loaded and ready to receive commands
#[tauri::command]
pub fn ready() -> StateView {
    println!("READY");
    let mut state = lock();
    state.ready = true;
    state.view()
}

#[tauri::command]
pub fn log(message: String) {
    println!("{}", message);
}

/// sends state to front end
/// emits via command "update_state"
pub fn update_state() {
    println!("BACKEND STATE UPDATE");
    loop {
        let state = STATE.lock().unwrap();

        if state.ready {
            // if the app is ready, we can send the state
            break;
        }
        drop(state);
    }
    let view = lock().view();
    if let Some(window) = WINDOW.lock().unwrap_or_else(PoisonError::into_inner).as_ref() {
        window.emit("update_state", view).ok();
    }
    notify_project_status();
}

/// a project file, one tab's: its scene data, node graph and window layout
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SavedProject {
    pub scene_data: HashMap<String, Scene>,
    pub rf_instance: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub layout: serde_json::Value,
}

/// sends a tab's node graph to every window ("graph_changed"), lighter than `update_state` for edits that only change
/// the graph. call without holding the state or window locks
pub fn push_graph(id: &str) {
    let payload = {
        let state = lock();
        let Some(instance) = state.instance(id) else {
            return;
        };
        serde_json::json!({ "tab": id, "graph_rev": instance.graph_rev, "rf_instance": instance.rf_instance })
    };
    if let Some(window) = WINDOW.lock().unwrap_or_else(PoisonError::into_inner).as_ref() {
        window.emit("graph_changed", payload).ok();
    }
    notify_project_status();
}

/// the parts of a graph that get saved, selection, drag state, measured sizes and the viewport are UI only
fn graph_key(rf_instance: &HashMap<String, serde_json::Value>) -> serde_json::Value {
    let strip = |items: Option<&serde_json::Value>, keys: &[&str]| -> Vec<serde_json::Value> {
        let items = items.and_then(|items| items.as_array()).cloned().unwrap_or_default();
        items
            .into_iter()
            .map(|mut item| {
                if let Some(item) = item.as_object_mut() {
                    for key in keys {
                        item.remove(*key);
                    }
                }
                item
            })
            .collect()
    };
    serde_json::json!({
        "nodes": strip(rf_instance.get("nodes"), &["selected", "dragging", "measured", "resizing"]),
        "edges": strip(rf_instance.get("edges"), &["selected"]),
        // no groups and an empty `groups` are the same graph
        "groups": rf_instance.get("groups").filter(|groups| groups.as_object().is_some_and(|g| !g.is_empty())).cloned().map(strip_groups),
    })
}

/// a project's node groups without the UI only fields, like `graph_key`
fn strip_groups(mut groups: serde_json::Value) -> serde_json::Value {
    if let Some(groups) = groups.as_object_mut() {
        for group in groups.values_mut() {
            let Some(group) = group.as_object_mut() else {
                continue;
            };
            let inner: HashMap<String, serde_json::Value> = group.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            let key = graph_key(&inner);
            group.insert("nodes".to_string(), key["nodes"].clone());
            group.insert("edges".to_string(), key["edges"].clone());
            group.remove("viewport");
        }
    }
    groups
}

/// updates a graph saved by an older version, see `graph::builtin::migrate`. the graph is always written back the way
/// `Graph` writes it (`0` positions become `0.0`), so the first edit after loading doesn't also show up as those changes
pub fn migrate_rf_instance(rf_instance: &mut HashMap<String, serde_json::Value>) {
    let Ok(mut graph) = crate::graph::model::Graph::from_rf(rf_instance) else {
        return;
    };
    crate::graph::builtin::migrate(&mut graph);
    *rf_instance = graph.to_rf();
}

/// the frontend opened a node group in a tab (or went back out), the groups on that path record their insides so they
/// show values
#[tauri::command]
pub async fn set_open_group(tab: String, path: String) {
    {
        let mut state = lock();
        let Some(instance) = state.instance_mut(&tab) else {
            return;
        };
        if instance.open_group == path {
            return;
        }
        instance.open_group = path;
        if instance.execution_paused {
            return;
        }
    }
    run_instance(tab, true).await;
}

/// true if any tab has unsaved changes
#[tauri::command]
pub fn has_unsaved_changes() -> bool {
    lock().unsaved()
}

/// the name of the tab on screen and whether it has unsaved changes (the window title), and the tabs that do
#[derive(Serialize, Clone, PartialEq, Debug)]
pub struct ProjectStatus {
    pub name: String,
    pub unsaved: bool,
    pub unsaved_tabs: Vec<String>,
}

#[tauri::command]
pub fn get_project_status() -> ProjectStatus {
    let state = lock();
    let active = state.active();
    ProjectStatus {
        name: active.name(),
        unsaved: active.unsaved(),
        unsaved_tabs: state.instances.iter().filter(|instance| instance.unsaved()).map(|instance| instance.id.clone()).collect(),
    }
}

/// sends the project status to the front end ("project_status") and puts it in the window title, only when it changed.
/// call without holding the state or window locks
pub fn notify_project_status() {
    let status = get_project_status();
    let mut last = LAST_STATUS.lock().unwrap_or_else(PoisonError::into_inner);
    if last.as_ref() == Some(&status) {
        return;
    }
    *last = Some(status.clone());
    drop(last);

    if let Some(window) = WINDOW.lock().unwrap_or_else(PoisonError::into_inner).as_ref() {
        window
            .set_title(&format!(
                "{}{} - MotionKeys",
                status.name,
                if status.unsaved {
                    "*"
                } else {
                    ""
                }
            ))
            .ok();
        window.emit("project_status", status).ok();
    }
}

// MARK: - Files

/// saves a tab (the one on screen if none is given) to its file. one that hasn't been saved, or with `save_as`, asks
/// where first. returns the path
#[tauri::command]
pub async fn save_project(tab: Option<String>, save_as: Option<bool>) -> Result<String, String> {
    let (id, path, name) = {
        let state = lock();
        let instance = match &tab {
            Some(id) => state.instance(id).ok_or_else(|| format!("no tab '{}'", id))?,
            None => state.active(),
        };
        (instance.id.clone(), instance.path.clone(), instance.name())
    };

    let path = match path.filter(|_| save_as != Some(true)) {
        Some(path) => path,
        None => {
            // starts where the tab's file is, named after the tab
            let window = WINDOW.lock().unwrap_or_else(PoisonError::into_inner);
            let mut dialog = window.as_ref().ok_or("no window")?.dialog().file().set_title("Save Project").add_filter("MIDIAnimator Project", &["mkproj"]).set_file_name(format!("{}.mkproj", name));
            drop(window);
            if let Some(folder) = lock().instance(&id).and_then(|instance| instance.path.clone()).as_deref().and_then(|path| std::path::Path::new(path).parent().map(|p| p.to_path_buf())) {
                dialog = dialog.set_directory(folder);
            }
            dialog.blocking_save_file().ok_or("Save cancelled")?.to_string()
        }
    };
    save_instance_to(&id, &path)
}

/// saves the tab on screen to `path`, returns the path
pub fn save_project_to(path: &str) -> Result<String, String> {
    let id = lock().active_instance_id.clone();
    save_instance_to(&id, path)
}

/// writes a tab's scene data, node graph and layout to `path`, which becomes its file. returns the path
pub fn save_instance_to(id: &str, path: &str) -> Result<String, String> {
    // copy what we need out of the state so we don't hold the lock while writing
    let saved_data = {
        let state = lock();
        let instance = state.instance(id).ok_or_else(|| format!("no tab '{}'", id))?;
        SavedProject {
            scene_data: instance.scene_data.clone(),
            rf_instance: instance.rf_instance.clone(),
            layout: instance.layout.clone(),
        }
    };

    // serialize and write the file
    let json = serde_json::to_string_pretty(&saved_data).map_err(|e| format!("Serialization error: {}", e))?;
    fs::write(path, json).map_err(|e| format!("File write error: {}", e))?;
    {
        let mut state = lock();
        if let Some(instance) = state.instance_mut(id) {
            instance.path = Some(path.to_string());
            instance.mark_saved();
        }
    }
    // the tab is named after its file now
    update_state();

    Ok(path.to_string())
}

/// asks for a project file and opens it, see `open_project`
#[tauri::command]
pub async fn load_project() -> Result<StateView, String> {
    // the window lock is scoped so it's released before loading
    let file_path = {
        let window = WINDOW.lock().unwrap_or_else(PoisonError::into_inner);
        window.as_ref().ok_or("no window")?.dialog().file().set_title("Load Project").add_filter("MIDIAnimator Project", &["mkproj"]).blocking_pick_file()
    };
    let path = file_path.ok_or("Load cancelled")?;

    open_project(&path.to_string()).await?;
    Ok(lock().view())
}

/// opens a project file in a tab of its own, see `open_file`, then links Blender to it or runs it (`start_instance`).
/// returns the tab
pub async fn open_project(path: &str) -> Result<String, String> {
    match open_file(path)? {
        Opened::Already(id) => switch_active_instance(id.clone()).await,
        Opened::New(id) => start_instance(id.clone()).await,
    }
    Ok(lock().active_instance_id.clone())
}

/// where `open_file` put a file
#[derive(Debug, PartialEq)]
pub enum Opened {
    /// a tab already had it open, it wasn't read again
    Already(String),
    /// read into a new tab, which is on screen
    New(String),
}

/// reads a project file into a new tab and shows it, with nothing to undo and no results yet. it takes the place of the
/// tab on screen if that one is an empty graph nobody touched, otherwise it goes after the others. a file a tab already
/// has open isn't opened twice. call `start_instance` after (`open_project` does)
pub fn open_file(path: &str) -> Result<Opened, String> {
    // the same file through another path is still the same file
    let path = fs::canonicalize(path).map(|path| path.to_string_lossy().to_string()).unwrap_or_else(|_| path.to_string());
    let same = |other: &str| fs::canonicalize(other).map(|other| other.to_string_lossy().to_string()).unwrap_or_else(|_| other.to_string()) == path;
    if let Some(instance) = lock().instances.iter().find(|instance| instance.path.as_deref().is_some_and(same)) {
        return Ok(Opened::Already(instance.id.clone()));
    }

    // read and parse the project file
    let json = fs::read_to_string(&path).map_err(|e| format!("File read error: {}", e))?;
    let saved_data: SavedProject = serde_json::from_str(&json).map_err(|e| format!("Deserialization error: {}", e))?;

    let (id, replaced) = {
        let mut state = lock();
        let id = state.next_id();
        let mut instance = InstanceState::new(id.clone(), String::new());
        instance.path = Some(path.clone());
        instance.scene_data = saved_data.scene_data;
        instance.rf_instance = saved_data.rf_instance;
        migrate_rf_instance(&mut instance.rf_instance);
        instance.layout = saved_data.layout;
        instance.mark_saved();
        state.graph_rev += 1;
        instance.graph_rev = state.graph_rev;

        // an untouched tab on screen makes way for it, Blender stays linked to its place
        let active = state.instances.iter().position(|instance| instance.id == state.active_instance_id).unwrap_or(0);
        let replaced = if state.instances[active].untouched() {
            let old = std::mem::replace(&mut state.instances[active], instance);
            if state.connected_instance_id.as_deref() == Some(&old.id) {
                state.connected_instance_id = Some(id.clone());
            }
            Some(old.id)
        } else {
            state.instances.push(instance);
            None
        };
        state.active_instance_id = id.clone();
        (id, replaced)
    };
    if let Some(old) = replaced {
        history::forget(&old);
        forget_memo(&old);
    }

    // tell the front end about the new tab and its empty history
    update_state();
    history::notify_active();
    Ok(Opened::New(id))
}

/// after a tab is opened from a file: Blender links to it (it checks the scene first) if Blender is connected and no
/// other tab is live, otherwise the tab runs on the scene it was saved with
pub async fn start_instance(id: String) {
    let (link_it, paused) = {
        let state = lock();
        let Some(instance) = state.instance(&id) else {
            return;
        };
        (state.connected && state.connected_instance_id.as_ref().is_none_or(|linked| *linked == id), instance.execution_paused)
    };
    if link_it {
        link(id).await.ok();
    } else if !paused {
        run_instance(id, true).await;
    }
}

/// stores a tab's window layout (the frontend's), it's saved with the tab's file
#[tauri::command]
pub fn set_layout(tab: String, layout: serde_json::Value) {
    if let Some(instance) = lock().instance_mut(&tab) {
        instance.layout = layout;
    }
}

// MARK: - Tabs

/// adds a tab after the others and shows it, returns its id
#[tauri::command]
pub fn create_instance() -> String {
    let id = {
        let mut state = lock();
        let id = state.add_instance();
        state.active_instance_id = id.clone();
        id
    };
    update_state();
    history::notify_active();
    id
}

/// closes a tab, the one to its right is shown if it was on screen. Blender unlinks if it was linked to it.
/// the last tab isn't closed (the window closes instead), returns whether it closed
#[tauri::command]
pub async fn close_instance(id: String) -> bool {
    let run = {
        let mut state = lock();
        if state.instances.len() <= 1 {
            return false;
        }
        let Some(index) = state.instances.iter().position(|instance| instance.id == id) else {
            return false;
        };
        state.instances.remove(index);
        if state.connected_instance_id.as_deref() == Some(&id) {
            state.connected_instance_id = None;
        }
        if state.active_instance_id != id {
            None
        } else {
            let next = &state.instances[index.min(state.instances.len() - 1)];
            let run = (!next.execution_paused).then(|| next.id.clone());
            state.active_instance_id = next.id.clone();
            run
        }
    };
    history::forget(&id);
    forget_memo(&id);
    update_state();
    history::notify_active();
    // the tab shown now may have missed scene changes while it was in the background
    if let Some(next) = run {
        run_instance(next, true).await;
    }
    true
}

/// shows a tab and runs it (it may have missed scene changes while it was in the background)
#[tauri::command]
pub async fn switch_active_instance(id: String) {
    let run = {
        let mut state = lock();
        let Some(instance) = state.instance(&id) else {
            return;
        };
        if state.active_instance_id == id {
            return;
        }
        let paused = instance.execution_paused;
        state.active_instance_id = id.clone();
        !paused
    };
    update_state();
    history::notify_active();
    if run {
        run_instance(id, true).await;
    }
}

/// renames a tab that hasn't been saved (a saved one is named after its file), a blank label is ignored. the name is
/// what the save dialog suggests. returns whether it was renamed
#[tauri::command]
pub fn rename_instance(id: String, label: String) -> bool {
    let label = label.trim();
    if label.is_empty() {
        return false;
    }
    {
        let mut state = lock();
        let Some(instance) = state.instance_mut(&id).filter(|instance| instance.path.is_none()) else {
            return false;
        };
        instance.label = label.to_string();
    }
    update_state();
    true
}

/// moves a tab to `index` in the tab bar
#[tauri::command]
pub fn move_instance(id: String, index: usize) {
    {
        let mut state = lock();
        let Some(from) = state.instances.iter().position(|instance| instance.id == id) else {
            return;
        };
        let instance = state.instances.remove(from);
        let to = index.min(state.instances.len());
        state.instances.insert(to, instance);
    }
    update_state();
}

/// links Blender to a tab (it goes live)
#[tauri::command]
pub async fn go_live(id: String) -> Result<(), String> {
    link(id).await
}

/// links Blender to a tab, which then gets Blender's scene changes and is the only one that writes to Blender. the scene
/// Blender has now is compared with the tab's: when objects or collections differ the tab pauses so the changes can be
/// reviewed first (`check_scene_changes`), otherwise it takes the scene and runs
pub async fn link(id: String) -> Result<(), String> {
    {
        let mut state = lock();
        if !state.connected {
            return Err("Blender is not connected".to_string());
        }
        if state.instance(&id).is_none() {
            return Err(format!("no tab '{}'", id));
        }
        state.connected_instance_id = Some(id.clone());
    }
    update_state();

    let fresh = get_scene_data().await;
    let run = {
        let mut state = lock();
        // the tab was closed or another one went live while Blender answered
        if state.connected_instance_id.as_deref() != Some(&id) {
            return Ok(());
        }
        let Some(instance) = state.instance_mut(&id) else {
            return Ok(());
        };
        if compare_scene_data(&instance.scene_data, &fresh).has_changes() {
            instance.pending_scene_data = Some(fresh);
            instance.execution_paused = true;
            println!("Scene data changes detected, execution paused for validation.");
            false
        } else {
            instance.scene_data = fresh;
            !instance.execution_paused
        }
    };
    update_state();
    if run {
        run_instance(id, true).await;
    }
    Ok(())
}

/// links Blender again once it's connected: to the tab it was linked to, or the tab on screen if that one is gone
pub async fn relink() {
    let id = {
        let state = lock();
        if !state.connected {
            return;
        }
        state.connected_instance_id.clone().filter(|id| state.instance(id).is_some()).unwrap_or_else(|| state.active_instance_id.clone())
    };
    link(id).await.ok();
}
