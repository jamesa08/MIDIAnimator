// run from /src-tauri
// cargo test --test tabs_test
// fixture: tests/fixtures/simple_scene_3_executor.mkproj, saved from the app, with short node ids
use serde_json::{json, Value};
use std::collections::HashMap;
use std::future::Future;
use std::sync::{Mutex, MutexGuard};
use MIDIAnimator::graph::execute::run_instance;
use MIDIAnimator::graph::history::{Source, Step};
use MIDIAnimator::state::history::{self, Capture};
use MIDIAnimator::state::{close_instance, create_instance, get_project_status, migrate_rf_instance, open_file, rename_instance, save_instance_to, set_layout, switch_active_instance, AppState, Opened, SavedProject, STATE};

// the tests share the global state, so they run one at a time
static STATE_TEST: Mutex<()> = Mutex::new(());

// a fresh state with one tab and no history, ready so commands can send it (there's no window in tests)
fn reset() -> MutexGuard<'static, ()> {
    let guard = STATE_TEST.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut state = AppState::default();
    state.ready = true;
    *STATE.lock().unwrap() = state;
    history::clear_all();
    guard
}

fn block<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Runtime::new().unwrap().block_on(future)
}

// a graph of viewer nodes with these ids
fn graph_with(ids: &[&str]) -> HashMap<String, Value> {
    let nodes: Vec<Value> = ids.iter().map(|id| json!({"id": id, "type": "viewer", "position": {"x": 0.0, "y": 0.0}, "data": {"inputs": {}}})).collect();
    HashMap::from([("nodes".to_string(), json!(nodes)), ("edges".to_string(), json!([])), ("viewport".to_string(), json!({"x": 0, "y": 0, "zoom": 1}))])
}

// replaces a tab's graph as one undo step
fn edit(tab: &str, ids: &[&str]) {
    let mut state = STATE.lock().unwrap();
    history::commit(&mut state, tab, graph_with(ids), Capture::Record(Step::new("add_nodes", Source::Ui)));
}

fn node_ids(tab: &str) -> Vec<String> {
    let state = STATE.lock().unwrap();
    let nodes = state.instance(tab).unwrap().rf_instance["nodes"].as_array().cloned().unwrap_or_default();
    nodes.iter().map(|node| node["id"].as_str().unwrap().to_string()).collect()
}

fn tabs() -> Vec<(String, String)> {
    STATE.lock().unwrap().instances.iter().map(|instance| (instance.id.clone(), instance.name())).collect()
}

// a project file path in the temp folder, unique to this test run
fn temp_path(name: &str) -> String {
    std::env::temp_dir().join(format!("motionkeys_tabs_test_{}_{}.mkproj", std::process::id(), name)).to_str().unwrap().to_string()
}

fn active() -> String {
    STATE.lock().unwrap().active_instance_id.clone()
}

#[test]
fn new_tabs_get_fresh_ids_and_the_lowest_free_label() {
    let _guard = reset();
    assert_eq!(create_instance(), "tab-2");
    assert_eq!(create_instance(), "tab-3");
    assert_eq!(active(), "tab-3");

    // an id isn't used again, a label is
    assert!(block(close_instance("tab-2".to_string())));
    assert_eq!(create_instance(), "tab-4");
    assert_eq!(tabs(), [("tab-1".to_string(), "Graph 1".to_string()), ("tab-3".to_string(), "Graph 3".to_string()), ("tab-4".to_string(), "Graph 2".to_string())]);
}

#[test]
fn closing_the_shown_tab_shows_the_one_to_its_right() {
    let _guard = reset();
    create_instance();
    create_instance();
    block(switch_active_instance("tab-2".to_string()));

    assert!(block(close_instance("tab-2".to_string())));
    assert_eq!(active(), "tab-3");
    // the last one in the bar shows the one to its left
    assert!(block(close_instance("tab-3".to_string())));
    assert_eq!(active(), "tab-1");
    // the last tab stays, the window closes instead
    assert!(!block(close_instance("tab-1".to_string())));
    assert_eq!(tabs().len(), 1);
}

#[test]
fn closing_another_tab_keeps_blender_linked() {
    let _guard = reset();
    {
        let mut state = STATE.lock().unwrap();
        state.connected = true;
        state.connected_instance_id = Some("tab-1".to_string());
    }
    create_instance();
    create_instance();

    // the tab on screen isn't the linked one
    assert!(block(close_instance("tab-3".to_string())));
    assert_eq!(STATE.lock().unwrap().connected_instance_id.as_deref(), Some("tab-1"));
    assert!(STATE.lock().unwrap().is_live("tab-1"));

    // the linked one unlinks
    assert!(block(close_instance("tab-1".to_string())));
    assert_eq!(STATE.lock().unwrap().connected_instance_id, None);
}

#[test]
fn undo_is_per_tab() {
    let _guard = reset();
    edit("tab-1", &["viewer-1"]);
    create_instance();
    edit("tab-2", &["viewer-1", "viewer-2"]);

    // undoing in one tab leaves the other alone
    history::step("tab-2", false).unwrap();
    assert!(node_ids("tab-2").is_empty());
    assert_eq!(node_ids("tab-1"), ["viewer-1"]);
    assert_eq!(history::info("tab-1").current, 1);
    assert_eq!(history::info("tab-2").current, 0);
    assert_eq!(history::info("tab-2").entries.len(), 1);

    // a tab's history goes when it's closed
    block(close_instance("tab-2".to_string()));
    assert!(history::info("tab-2").entries.is_empty());
    assert_eq!(history::info("tab-1").current, 1);
}

#[test]
fn unsaved_changes_by_tab() {
    let _guard = reset();
    assert!(!get_project_status().unsaved);

    // an edit makes its tab unsaved
    edit("tab-1", &["viewer-1"]);
    let status = get_project_status();
    assert!(status.unsaved);
    assert_eq!(status.unsaved_tabs, ["tab-1"]);

    // a new empty tab has nothing to save, and the title is the tab on screen's
    create_instance();
    let status = get_project_status();
    assert_eq!(status.name, "Graph 2");
    assert!(!status.unsaved);
    assert_eq!(status.unsaved_tabs, ["tab-1"]);

    // renaming an unsaved tab isn't a change to its file, a blank name is ignored
    assert!(rename_instance("tab-2".to_string(), "Drums".to_string()));
    assert!(!rename_instance("tab-1".to_string(), "  ".to_string()));
    assert_eq!(tabs(), [("tab-1".to_string(), "Graph 1".to_string()), ("tab-2".to_string(), "Drums".to_string())]);
    assert_eq!(get_project_status().unsaved_tabs, ["tab-1"]);
}

#[test]
fn a_saved_tab_is_its_file() {
    let _guard = reset();
    edit("tab-1", &["viewer-1"]);
    set_layout("tab-1".to_string(), json!({"panelsShown": [1]}));
    let path = temp_path("drums");
    save_instance_to("tab-1", &path).unwrap();

    // named after its file, saved, and it can't be renamed away from it
    assert_eq!(tabs()[0].1, format!("motionkeys_tabs_test_{}_drums", std::process::id()));
    assert!(!get_project_status().unsaved);
    assert!(!rename_instance("tab-1".to_string(), "Other".to_string()));

    // the file is one tab's graph, scene and layout
    let saved: SavedProject = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved.layout, json!({"panelsShown": [1]}));
    assert_eq!(saved.rf_instance["nodes"].as_array().unwrap().len(), 1);
    std::fs::remove_file(&path).ok();
}

#[test]
fn opening_a_file_takes_an_untouched_tab_or_adds_one() {
    let _guard = reset();
    edit("tab-1", &["viewer-1", "viewer-2"]);
    set_layout("tab-1".to_string(), json!({"sidesHidden": ["left"]}));
    let path = temp_path("open");
    save_instance_to("tab-1", &path).unwrap();

    // an empty tab on screen that nobody touched makes way for the file: a new tab in its place, with the file's graph
    // and layout and nothing to undo
    create_instance();
    block(close_instance("tab-1".to_string()));
    let Opened::New(opened) = open_file(&path).unwrap() else {
        panic!("should open a new tab");
    };
    assert_eq!(tabs().len(), 1);
    assert_eq!(active(), opened);
    assert_eq!(node_ids(&opened), ["viewer-1", "viewer-2"]);
    assert_eq!(STATE.lock().unwrap().instance(&opened).unwrap().layout, json!({"sidesHidden": ["left"]}));
    assert!(history::info(&opened).entries.is_empty());
    assert!(!get_project_status().unsaved);

    // the same file again just finds its tab
    assert_eq!(open_file(&path).unwrap(), Opened::Already(opened.clone()));

    // a tab that's in use stays, the file goes into a new tab after it
    let other = temp_path("other");
    save_instance_to(&opened, &other).unwrap();
    edit(&opened, &["viewer-3"]);
    let Opened::New(second) = open_file(&path).unwrap() else {
        panic!("should open a new tab");
    };
    assert_eq!(tabs().iter().map(|(id, _)| id.clone()).collect::<Vec<_>>(), [opened.clone(), second.clone()]);
    assert_eq!(node_ids(&opened), ["viewer-3"]);
    assert_eq!(active(), second);
    std::fs::remove_file(&path).ok();
    std::fs::remove_file(&other).ok();
}

#[test]
fn a_run_lands_on_its_tab() {
    let _guard = reset();
    // the fixture's graph and scene in the first tab, the second tab on screen
    let data = std::fs::read_to_string("tests/fixtures/simple_scene_3_executor.mkproj").unwrap();
    let mut project: SavedProject = serde_json::from_str(&data).unwrap();
    migrate_rf_instance(&mut project.rf_instance);
    let default_nodes: HashMap<String, Value> = serde_json::from_str(&std::fs::read_to_string("src/configs/default_nodes.json").unwrap()).unwrap();
    {
        let mut state = STATE.lock().unwrap();
        state.default_nodes = default_nodes;
        let tab = state.instance_mut("tab-1").unwrap();
        tab.rf_instance = project.rf_instance;
        tab.scene_data = project.scene_data;
    }
    create_instance();

    // scene_link gives the scene of the tab being run
    block(run_instance("tab-1".to_string(), true));
    {
        let state = STATE.lock().unwrap();
        let results = &state.instance("tab-1").unwrap().executed_results;
        let scene_link = results.iter().find(|(id, _)| id.starts_with("scene_link")).map(|(_, value)| value.to_string()).unwrap();
        assert!(scene_link.contains("Cubes"), "{}", scene_link);
        assert!(state.instance("tab-2").unwrap().executed_results.is_empty());
    }

    // only the live tab writes to Blender
    STATE.lock().unwrap().instance_mut("tab-1").unwrap().executed_results.clear();
    block(run_instance("tab-1".to_string(), false));
    assert!(STATE.lock().unwrap().instance("tab-1").unwrap().executed_results.is_empty());
}
