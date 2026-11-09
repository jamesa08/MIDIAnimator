use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::fs;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::{MutexGuard, PoisonError};
use std::{collections::HashMap, sync::Mutex};
use tauri::Emitter;
use tauri_plugin_dialog::DialogExt;

use crate::bridge::curves::{carry_over_curves, curve_sources, prune_curves, unwatch_curves};
use crate::bridge::scene_data::{compare_scene_data, get_scene};
use crate::graph::execute::{forget_memo, run_instance};
use crate::scene_generics::Scene;
use crate::utils::console::{self, Entry, Level, Source};

pub mod graph;
pub mod history;

lazy_static! {
    pub static ref STATE: Mutex<AppState> = Mutex::new(AppState::default());
    pub static ref WINDOW: Arc<Mutex<Option<tauri::WebviewWindow>>> = Arc::new(Mutex::new(None));
    // last status sent to the front end, only changes are sent
    static ref LAST_STATUS: Mutex<Option<ProjectStatus>> = Mutex::new(None);
    // wakes `wait_ready` when the front end calls `ready`
    static ref READY: tokio::sync::Notify = tokio::sync::Notify::new();
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
    /// the connected app's id from its bridge's hello ("blender"), empty while none is connected
    pub connected_application: String,
    /// the connected app's display name ("Blender")
    pub connected_name: String,
    pub connected_version: String,
    pub connected_file_name: String,
    /// the icon the bridge sent for its app, a data URL
    pub connected_icon: Option<String>,
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
    /// the objects Blender's tracker sends keyframes for, the ones the live tab's graph reads (bridge/curves.rs). none
    /// when it isn't known (a tab just went live, Blender reconnected, the live tab closed), the next sync sends them
    pub watched_curves: Option<BTreeSet<String>>,
    /// the last full run, what it wrote to Blender. none until the first one
    pub last_write: Option<LastWrite>,
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
    /// the scene the tab runs on, the one Blender sent last, none before it's ever linked
    pub scene: Option<Scene>,
    pub rf_instance: HashMap<String, serde_json::Value>,
    pub executed_results: HashMap<String, serde_json::Value>,
    pub executed_inputs: HashMap<String, serde_json::Value>,
    /// paths of the nodes writing to Blender whose last write isn't what the graph gives now, as of the last run
    pub stale_writes: Vec<String>,
    /// the scene waiting for review while the tab is paused
    pub pending_scene: Option<Scene>,
    pub execution_paused: bool,
    /// objects over the keyframe limit and their keyframe counts, waiting to be imported or left out (bridge/curves.rs)
    pub pending_curve_import: Option<BTreeMap<String, u64>>,
    /// objects over the keyframe limit that were imported, they don't ask again
    pub approved_curves: BTreeSet<String>,
    /// objects over the keyframe limit that were left out, until they're picked again
    pub declined_curves: BTreeSet<String>,
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
            scene: None,
            rf_instance: empty_graph(),
            executed_results: HashMap::new(),
            executed_inputs: HashMap::new(),
            stale_writes: Vec::new(),
            pending_scene: None,
            execution_paused: false,
            pending_curve_import: None,
            approved_curves: BTreeSet::new(),
            declined_curves: BTreeSet::new(),
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
            connected_name: "".to_string(),
            connected_version: "".to_string(),
            connected_file_name: "".to_string(),
            connected_icon: None,
            graph_rev: 0,
            state_rev: 0,
            active_instance_id: first.id.clone(),
            instances: vec![first],
            connected_instance_id: None,
            watched_curves: None,
            last_write: None,
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

    /// Blender connected: errors that only said it wasn't (a scene writer's, or the group's around one, which ends with
    /// the writer's) are dropped from every tab, whether or not a run follows
    pub fn clear_not_connected(&mut self) {
        use crate::graph::executors::{io::node_error, scene::NOT_CONNECTED};
        for instance in &mut self.instances {
            instance.executed_results.retain(|_, result| !node_error(result).is_some_and(|error| error.ends_with(NOT_CONNECTED)));
        }
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

    /// where the tab that has a file open is in the tab bar, by the file's real path
    pub fn file_tab(&self, path: &str) -> Option<usize> {
        let path = crate::settings::real_path(path);
        self.instances.iter().position(|instance| instance.path.as_deref().is_some_and(|other| crate::settings::real_path(other) == path))
    }

    /// the tab that has a file open if it has unsaved changes, opening the file again drops them
    pub fn unsaved_file_tab(&self, path: &str) -> Option<String> {
        self.file_tab(path).map(|index| &self.instances[index]).filter(|instance| instance.unsaved()).map(|instance| instance.id.clone())
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
            connected_name: self.connected_name.clone(),
            connected_version: self.connected_version.clone(),
            connected_file_name: self.connected_file_name.clone(),
            connected_icon: self.connected_icon.clone(),
            port: crate::ipc::bound_port(),
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
            scene: active.scene.clone(),
            executed_results: active.executed_results.clone(),
            executed_inputs: active.executed_inputs.clone(),
            stale_writes: active.stale_writes.clone(),
            last_write: self.last_write.clone(),
            pending_scene: active.pending_scene.clone(),
            execution_paused: active.execution_paused,
            pending_curve_import: active.pending_curve_import.clone(),
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
    pub connected_name: String,
    pub connected_version: String,
    pub connected_file_name: String,
    pub connected_icon: Option<String>,
    /// the port Blender connects to
    pub port: u16,
    pub state_rev: u64,
    pub tabs: Vec<TabInfo>,
    pub active_tab: String,
    pub graph_rev: u64,
    pub rf_instance: HashMap<String, serde_json::Value>,
    pub scene: Option<Scene>,
    pub executed_results: HashMap<String, serde_json::Value>,
    pub executed_inputs: HashMap<String, serde_json::Value>,
    pub stale_writes: Vec<String>,
    pub last_write: Option<LastWrite>,
    pub pending_scene: Option<Scene>,
    pub execution_paused: bool,
    pub pending_curve_import: Option<BTreeMap<String, u64>>,
    pub open_group: String,
    pub layout: serde_json::Value,
}

/// what a full run wrote to Blender. `seq` goes up with every one, so the frontend can tell a new write from the last
/// one. `error`: the first error a node writing to Blender gave. `written`: how many of them wrote, `objects` and
/// `keyframes` how much they wrote together. `ms`: how long the run took
#[derive(Serialize, Clone, Debug)]
pub struct LastWrite {
    pub seq: u64,
    pub error: Option<String>,
    pub written: usize,
    pub objects: usize,
    pub keyframes: usize,
    pub ms: u64,
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
    let view = {
        let mut state = lock();
        state.ready = true;
        state.view()
    };
    READY.notify_waiters();
    view
}

/// waits until the front end has called `ready`, nothing is sent to it before that (`update_state`)
pub async fn wait_ready() {
    loop {
        // made before the check, so a `ready` between the check and the wait still wakes it
        let notified = READY.notified();
        if lock().ready {
            return;
        }
        notified.await;
    }
}

/// the state as it is, for windows other than the main one (floating panels) when they open
#[tauri::command]
pub fn get_state() -> StateView {
    lock().view()
}

#[tauri::command]
pub fn log(message: String) {
    println!("{}", message);
}

/// sends state to front end
/// emits via command "update_state"
pub fn update_state() {
    println!("BACKEND STATE UPDATE");
    let view = {
        let mut state = lock();
        // nothing is sent before the front end is ready, `ready` gives it the whole state. never waits for it: this can
        // run on the main thread, which `ready` needs
        if !state.ready {
            return;
        }
        state.view()
    };
    if let Some(window) = WINDOW.lock().unwrap_or_else(PoisonError::into_inner).as_ref() {
        window.emit("update_state", view).ok();
    }
    notify_project_status();
}

/// `update_state` after the tab on screen changed to `id`, then that tab's graph again (`push_graph`): a graph the
/// front end got for it before a state showing it was dropped (it wasn't on screen yet), and this state can be older
/// than that graph. returns the `state_rev` the front end's state shows the change as of
fn update_shown(id: &str) -> u64 {
    update_state();
    push_graph(id);
    lock().state_rev
}

/// a project file, one tab's: its scene, node graph and window layout
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SavedProject {
    /// none in files saved before a tab got one scene, the scene comes back when the tab goes live
    #[serde(default)]
    pub scene: Option<Scene>,
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

/// updates a graph saved by an older version, see `graph::builtin::migrate`, and drops the node sizes older versions
/// saved. the graph is always written back the way `Graph` writes it (`0` positions become `0.0`), so the first edit
/// after loading doesn't also show up as those changes
pub fn migrate_rf_instance(rf_instance: &mut HashMap<String, serde_json::Value>) {
    let Ok(mut graph) = crate::graph::model::Graph::from_rf(rf_instance) else {
        return;
    };
    crate::graph::builtin::migrate(&mut graph);
    graph.strip_measured();
    *rf_instance = graph.to_rf();
}

/// the frontend opened a node group in a tab (or went back out), the groups on that path record their insides so they
/// show values
#[tauri::command]
pub async fn set_open_group(tab: String, path: String) {
    let paused = {
        let mut state = lock();
        let Some(instance) = state.instance_mut(&tab) else {
            return;
        };
        if instance.open_group == path {
            return;
        }
        instance.open_group = path;
        instance.execution_paused
    };
    // the panels follow the open group now, the run (which waits on the curves and other runs first) sends its values
    update_state();
    if paused {
        return;
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
    // worked out, compared and sent under one lock, so of two at once the newer status is always the one sent last
    let mut last = LAST_STATUS.lock().unwrap_or_else(PoisonError::into_inner);
    let status = get_project_status();
    if last.as_ref() == Some(&status) {
        return;
    }
    *last = Some(status.clone());

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
            let mut dialog = window.as_ref().ok_or("no window")?.dialog().file().set_title("Save Project").add_filter("MotionKeys Project", &["mkproj"]).set_file_name(format!("{}.mkproj", name));
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
    let (saved_data, key) = {
        let mut state = lock();
        let instance = state.instance_mut(id).ok_or_else(|| format!("no tab '{}'", id))?;
        // the graph being written is what's saved, an edit landing during the write is still unsaved after it. taken
        // from the tab's own graph, which unsaved changes are compared to, not the copy written below
        let key = graph_key(&instance.rf_instance);
        // curves of objects the graph stopped reading are kept until now. node sizes are never saved, the canvas
        // measures them
        let mut rf_instance = instance.rf_instance.clone();
        if let Ok(mut graph) = crate::graph::model::Graph::from_rf(&instance.rf_instance) {
            let sources = curve_sources(&graph, instance.scene.as_ref());
            prune_curves(&mut instance.scene, &sources);
            graph.strip_measured();
            rf_instance = graph.to_rf();
        }
        let saved_data = SavedProject {
            scene: instance.scene.clone(),
            rf_instance,
            layout: instance.layout.clone(),
        };
        (saved_data, key)
    };

    // serialize and write the file
    let written = serde_json::to_string_pretty(&saved_data).map_err(|e| format!("Serialization error: {}", e)).and_then(|json| fs::write(path, json).map_err(|e| format!("File write error: {}", e)));
    if let Err(e) = written {
        console::push(Entry::new(Level::Error, Source::App, format!("Couldn't save {}: {}", path, e)).tab(id));
        return Err(e);
    }
    let real = crate::settings::real_path(path);
    {
        let mut state = lock();
        if let Some(instance) = state.instance_mut(id) {
            instance.path = Some(path.to_string());
            instance.saved = key;
        }
        // another tab that had this file open (Save As, or MCP saving over it) doesn't anymore: what it has is in no file
        // now, so it's an unsaved tab and closing it asks. it keeps its name
        for other in state.instances.iter_mut().filter(|other| other.id != id && other.path.as_deref().is_some_and(|file| crate::settings::real_path(file) == real)) {
            other.label = other.name();
            other.path = None;
            other.saved = serde_json::Value::Null;
        }
    }
    crate::settings::note_recent(path);
    // the tab is named after its file now
    update_state();

    Ok(path.to_string())
}

/// a project file picked to open (`pick_project`), and the tab that has it open with unsaved changes if one does:
/// opening the file again drops them
#[derive(Serialize, Clone, PartialEq, Debug)]
pub struct PickedProject {
    pub path: String,
    pub unsaved_tab: Option<String>,
}

/// asks for a project file, or takes the recent project at `recent` in the list (File > Open Recent's items name their
/// project's path instead, the list can change while the menu is open). `try_load_project` opens it
#[tauri::command]
pub async fn pick_project(recent: Option<usize>) -> Result<PickedProject, String> {
    let path = match recent {
        Some(index) => crate::settings::recent_projects().get(index).cloned().ok_or_else(|| format!("no recent project {}", index))?,
        None => {
            // the window lock is released before the dialog opens: the menu (File > New Tab) and every state sent need it,
            // and the main thread they can run on is the one the dialog waits for
            let dialog = {
                let window = WINDOW.lock().unwrap_or_else(PoisonError::into_inner);
                window.as_ref().ok_or("no window")?.dialog().file().set_title("Load Project").add_filter("MotionKeys Project", &["mkproj"])
            };
            dialog.blocking_pick_file().ok_or("Load cancelled")?.to_string()
        }
    };
    let unsaved_tab = lock().unsaved_file_tab(&path);
    Ok(PickedProject {
        path,
        unsaved_tab,
    })
}

/// opens a project file (`pick_project`), see `open_project`, dropping the unsaved changes of a tab that has it open
/// (`try_load_project` keeps them). a recent project whose file is gone leaves the list
#[tauri::command]
pub async fn load_project(path: String) -> Result<StateView, String> {
    let loaded = load(&path, true).await?;
    Ok(loaded.state.unwrap_or_else(|| lock().view()))
}

/// what `try_load_project` did: `state` once the file is open, or `unsaved_tab`, the tab that has the file open with
/// unsaved changes, when the file wasn't opened
#[derive(Serialize, Clone, Debug)]
pub struct LoadedProject {
    pub state: Option<StateView>,
    pub unsaved_tab: Option<String>,
}

/// `load_project`, except a tab that has the file open with unsaved changes is kept unless `discard` is true: the
/// frontend asks about them first, and changes made after it asked (MCP) aren't dropped without asking again
#[tauri::command]
pub async fn try_load_project(path: String, discard: Option<bool>) -> Result<LoadedProject, String> {
    load(&path, discard == Some(true)).await
}

/// opens a project file like `open_project`, unless the tab that has it open has unsaved changes and they're not to be
/// dropped (`discard`). a recent project whose file is gone leaves the list
async fn load(path: &str, discard: bool) -> Result<LoadedProject, String> {
    let opened = match open_file_unless_unsaved(path, discard) {
        Ok(Opening::Done(opened)) => opened,
        Ok(Opening::Unsaved(tab)) => {
            return Ok(LoadedProject {
                state: None,
                unsaved_tab: Some(tab),
            })
        }
        Err(e) => {
            console::log(Level::Error, Source::App, format!("Couldn't open {}: {}", path, e));
            if !std::path::Path::new(path).exists() {
                crate::settings::forget_recent(path);
            }
            return Err(e);
        }
    };
    start_instance(opened.id().to_string()).await;
    Ok(LoadedProject {
        state: Some(lock().view()),
        unsaved_tab: None,
    })
}

/// opens a project file in a tab of its own, see `open_file`, then links Blender to it or runs it (`start_instance`).
/// returns the tab
pub async fn open_project(path: &str) -> Result<String, String> {
    let opened = open_file(path).inspect_err(|e| console::log(Level::Error, Source::App, format!("Couldn't open {}: {}", path, e)))?;
    start_instance(opened.id().to_string()).await;
    Ok(lock().active_instance_id.clone())
}

/// where `open_file` put a file, a new tab that's on screen
#[derive(Debug, PartialEq)]
pub enum Opened {
    /// no tab had it open
    New(String),
    /// read again in place of the tab that had it open, `replaced`, which is gone with its unsaved changes
    Reopened {
        id: String,
        replaced: String,
    },
}

impl Opened {
    /// the new tab
    pub fn id(&self) -> &str {
        match self {
            Opened::New(id)
            | Opened::Reopened {
                id,
                ..
            } => id,
        }
    }
}

/// reads a project file into a new tab and shows it, with nothing to undo and no results yet. a file a tab already has
/// open is read again in that tab's place (its unsaved changes are dropped, ask first: `unsaved_file_tab`). otherwise it
/// takes the place of the tab on screen if that one is an empty graph nobody touched, or goes after the others. call
/// `start_instance` after (`open_project` does)
pub fn open_file(path: &str) -> Result<Opened, String> {
    match open_file_unless_unsaved(path, true)? {
        Opening::Done(opened) => Ok(opened),
        Opening::Unsaved(tab) => Err(format!("{} has unsaved changes", tab)),
    }
}

/// what `open_file_unless_unsaved` did
pub enum Opening {
    /// the file is open, see `Opened`
    Done(Opened),
    /// the file isn't open: the tab that has it open has unsaved changes, that tab is kept
    Unsaved(String),
}

/// `open_file`, except a tab that has the file open with unsaved changes is kept unless `discard` is true. it's checked
/// as the tab would be replaced, so changes made after anyone asked aren't dropped
pub fn open_file_unless_unsaved(path: &str, discard: bool) -> Result<Opening, String> {
    let path = crate::settings::real_path(path);

    // read and parse the project file, a tab that has it open stays if it can't be
    let json = fs::read_to_string(&path).map_err(|e| format!("File read error: {}", e))?;
    let saved_data: SavedProject = serde_json::from_str(&json).map_err(|e| format!("Deserialization error: {}", e))?;

    let (id, replaced, reopened) = {
        let mut state = lock();
        // the tab that has the file open, or an untouched tab on screen, makes way for it. Blender stays linked to its place
        let open = state.file_tab(&path);
        // unless it has unsaved changes that aren't to be dropped, ones made after anyone asked count too
        if let Some(index) = open.filter(|&index| !discard && state.instances[index].unsaved()) {
            return Ok(Opening::Unsaved(state.instances[index].id.clone()));
        }
        let id = state.next_id();
        let mut instance = InstanceState::new(id.clone(), String::new());
        instance.path = Some(path.clone());
        instance.scene = saved_data.scene;
        instance.rf_instance = saved_data.rf_instance;
        migrate_rf_instance(&mut instance.rf_instance);
        instance.layout = saved_data.layout;
        instance.mark_saved();
        state.graph_rev += 1;
        instance.graph_rev = state.graph_rev;

        let active = state.instances.iter().position(|instance| instance.id == state.active_instance_id).unwrap_or(0);
        let replaced = match open.or(state.instances[active].untouched().then_some(active)) {
            Some(index) => {
                let old = std::mem::replace(&mut state.instances[index], instance);
                if state.connected_instance_id.as_deref() == Some(&old.id) {
                    state.connected_instance_id = Some(id.clone());
                }
                Some(old.id)
            }
            None => {
                state.instances.push(instance);
                None
            }
        };
        state.active_instance_id = id.clone();
        (id, replaced, open.is_some())
    };
    if let Some(old) = &replaced {
        history::forget(old);
        forget_memo(old);
    }

    // tell the front end about the new tab and its empty history
    crate::settings::note_recent(&path);
    update_shown(&id);
    history::notify_active();
    Ok(Opening::Done(match replaced.filter(|_| reopened) {
        Some(replaced) => Opened::Reopened {
            id,
            replaced,
        },
        None => Opened::New(id),
    }))
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
        link_or_log(id).await.ok();
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
    update_shown(&id);
    history::notify_active();
    id
}

/// what `try_close_instance` errs with when the tab has unsaved changes
const UNSAVED_CHANGES: &str = "Unsaved changes";
/// what `try_close_instance` errs with when the tab is the last one, the window closes instead
const LAST_TAB: &str = "Last tab";

/// closes a tab, the one to its right is shown if it was on screen. Blender unlinks if it was linked to it.
/// the last tab isn't closed (the window closes instead), returns whether it closed. its unsaved changes are dropped,
/// the frontend closes with `try_close_instance`
#[tauri::command]
pub async fn close_instance(id: String) -> bool {
    close_tab(id, true).await.unwrap_or(false)
}

/// `close_instance`, except a tab with unsaved changes is kept unless `discard` is true, erring with "Unsaved changes":
/// the frontend asks about them first, and changes made after it asked (MCP) aren't dropped without asking again. the
/// last tab errs with "Last tab", it can be the last one by now (MCP closed the others) and the window closes instead
#[tauri::command]
pub async fn try_close_instance(id: String, discard: Option<bool>) -> Result<bool, String> {
    // only for the tab itself, one that's gone (closed meanwhile) just isn't closed
    {
        let state = lock();
        if state.instances.len() <= 1 && state.instances.iter().any(|instance| instance.id == id) {
            return Err(LAST_TAB.to_string());
        }
    }
    close_tab(id, discard == Some(true)).await
}

/// closes a tab (`close_instance`), one with unsaved changes only with `discard`
async fn close_tab(id: String, discard: bool) -> Result<bool, String> {
    let (run, shown, unlinked) = {
        let mut state = lock();
        if state.instances.len() <= 1 {
            return Ok(false);
        }
        let Some(index) = state.instances.iter().position(|instance| instance.id == id) else {
            return Ok(false);
        };
        // checked under the lock that closes it, so no edit lands in between
        if !discard && state.instances[index].unsaved() {
            return Err(UNSAVED_CHANGES.to_string());
        }
        state.instances.remove(index);
        let unlinked = state.connected_instance_id.as_deref() == Some(&id);
        if unlinked {
            state.connected_instance_id = None;
        }
        let (run, shown) = if state.active_instance_id != id {
            (None, None)
        } else {
            let next = &state.instances[index.min(state.instances.len() - 1)];
            let run = (!next.execution_paused).then(|| next.id.clone());
            let shown = next.id.clone();
            state.active_instance_id = shown.clone();
            (run, Some(shown))
        };
        (run, shown, unlinked)
    };
    history::forget(&id);
    forget_memo(&id);
    // a tab that's on screen now gets its graph again, see `update_shown`
    if let Some(shown) = &shown {
        update_shown(shown);
    } else {
        update_state();
    }
    history::notify_active();
    // no tab reads keyframes from Blender now
    if unlinked {
        unwatch_curves().await;
    }
    // the tab shown now may have missed scene changes while it was in the background
    if let Some(next) = run {
        run_instance(next, true).await;
    }
    Ok(true)
}

/// shows a tab and runs it (it may have missed scene changes while it was in the background). returns the `state_rev`
/// the frontend's state shows it on screen as of (that state or a newer one), none if there's no such tab
#[tauri::command]
pub async fn switch_active_instance(id: String) -> Option<u64> {
    let run = {
        let mut state = lock();
        let instance = state.instance(&id)?;
        if state.active_instance_id == id {
            return Some(state.state_rev);
        }
        let paused = instance.execution_paused;
        state.active_instance_id = id.clone();
        !paused
    };
    let shown_at = update_shown(&id);
    history::notify_active();
    if run {
        run_instance(id, true).await;
    }
    Some(shown_at)
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
    link_or_log(id).await
}

/// `link`, with why it couldn't in the console
async fn link_or_log(id: String) -> Result<(), String> {
    link(id.clone()).await.inspect_err(|e| console::push(Entry::new(Level::Warn, Source::Bridge, format!("Couldn't go live: {}", e)).tab(&id)))
}

/// links Blender to a tab, which then gets Blender's scene changes and is the only one that writes to Blender. the scene
/// Blender has now is compared with the tab's: when objects or collections differ the tab pauses so the changes can be
/// reviewed first (`check_scene_changes`), otherwise it takes the scene and runs
pub async fn link(id: String) -> Result<(), String> {
    {
        let mut state = lock();
        if !state.connected {
            return Err(crate::graph::executors::scene::NOT_CONNECTED.to_string());
        }
        if state.instance(&id).is_none() {
            return Err(format!("no tab '{}'", id));
        }
        state.connected_instance_id = Some(id.clone());
        // the tab's run tells the tracker what to watch and fetches the curves (bridge/curves.rs)
        state.watched_curves = None;
        let name = state.instance(&id).map(InstanceState::name).unwrap_or_default();
        console::push(Entry::new(Level::Info, Source::Bridge, format!("{} is live", name)).tab(&id));
    }
    update_state();

    // scenes Blender sends unasked wait until the reply below is in place, so a newer one isn't compared against or
    // overwritten by the older reply
    let held = crate::ipc::hold_scenes();
    // the tab stays live when Blender doesn't answer, the tracker's next update brings the scene
    let mut fresh = get_scene().await?;
    let run = {
        let mut state = lock();
        // the tab was closed or another one went live while Blender answered
        if state.connected_instance_id.as_deref() != Some(&id) {
            return Ok(());
        }
        let Some(instance) = state.instance_mut(&id) else {
            return Ok(());
        };
        // Blender's scene only has the curves its tracker watches, the tab keeps its own until the run fetches them again
        carry_over_curves(&instance.scene, &mut fresh, &BTreeSet::new());
        if compare_scene_data(instance.scene.as_ref(), &fresh).has_changes() {
            instance.pending_scene = Some(fresh);
            instance.execution_paused = true;
            println!("Scene data changes detected, execution paused for validation.");
            false
        } else {
            // nothing differs from the tab's scene, so a review left from before (an older pending scene) has nothing
            // to show: the tab runs on this one
            instance.scene = Some(fresh);
            instance.pending_scene = None;
            instance.execution_paused = false;
            true
        }
    };
    // the state isn't held any more, a scene held back applies now
    drop(held);
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
    link_or_log(id).await.ok();
}
