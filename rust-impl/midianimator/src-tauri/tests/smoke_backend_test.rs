// run from /src-tauri
// cargo test --test smoke_backend_test
//
// writes what the backend gives the frontend for the fixture project, after a run, to ../smoke/backend.json. the
// frontend smoke test (../smoke) serves it to the app in place of the backend, so it always matches the real commands
// fixture: tests/fixtures/simple_scene_3_executor.mkproj, saved from the app, with short node ids

use std::collections::HashMap;

use serde_json::{json, Value};
use MIDIAnimator::graph::builtin::get_builtin_groups;
use MIDIAnimator::graph::curves::{graph_curves, graph_value_curves};
use MIDIAnimator::graph::execute::run_instance;
use MIDIAnimator::graph::history::{Source, Step};
use MIDIAnimator::settings::get_settings;
use MIDIAnimator::state::history::{self, Capture};
use MIDIAnimator::state::{get_project_status, get_state, lock, migrate_rf_instance, AppState, SavedProject, STATE};
use MIDIAnimator::ui::keybinds::get_keymap;
use MIDIAnimator::utils::ui::get_build_info;

#[test]
fn dump_backend_for_smoke_test() {
    // a fresh state with one tab, ready so commands can send it (there's no window in tests)
    let mut state = AppState::default();
    state.ready = true;
    state.default_nodes = serde_json::from_str(&std::fs::read_to_string("src/configs/default_nodes.json").unwrap()).unwrap();
    *STATE.lock().unwrap() = state;
    history::clear_all();

    // the fixture's midi file is on the machine it was saved on, point it at the one next to it
    let data = std::fs::read_to_string("tests/fixtures/simple_scene_3_executor.mkproj").unwrap();
    let mut project: SavedProject = serde_json::from_str(&data).unwrap();
    migrate_rf_instance(&mut project.rf_instance);
    let midi = std::fs::canonicalize("tests/fixtures/piano_seq_test.mid").unwrap();
    for node in project.rf_instance.get_mut("nodes").and_then(Value::as_array_mut).unwrap() {
        if let Some(path) = node.pointer_mut("/data/inputs/file_path") {
            *path = json!(midi.to_string_lossy());
        }
    }
    {
        let mut state = lock();
        let tab = state.instance_mut("tab-1").unwrap();
        tab.rf_instance = project.rf_instance.clone();
        tab.scene_data = project.scene_data;
    }

    // one step in the history: a node moved
    let mut moved = project.rf_instance;
    if let Some(x) = moved.get_mut("nodes").and_then(|nodes| nodes.pointer_mut("/0/position/x")) {
        *x = json!(x.as_f64().unwrap_or(0.0) + 40.0);
    }
    history::commit(&mut lock(), "tab-1", moved, Capture::Record(Step::new("move", Source::Ui)));

    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(run_instance("tab-1".to_string(), true));

    let view = serde_json::to_value(get_state()).unwrap();
    let results = view["executed_results"].as_object().unwrap();
    assert!(!results.is_empty(), "the fixture didn't run");
    // each node's curves (graph window) and its results' curves (previews on nodes), the fake backend answers from these
    let curves: HashMap<String, Value> = runtime.block_on(graph_curves("tab-1".to_string(), results.keys().cloned().collect())).unwrap().into_iter().map(|curves| (curves.node.clone(), serde_json::to_value(curves).unwrap())).collect();
    let value_curves: HashMap<String, Value> = results.iter().map(|(node, outputs)| (node.clone(), serde_json::to_value(runtime.block_on(graph_value_curves(outputs.clone()))).unwrap())).collect();

    // the commands the frontend reads from, by name
    let backend: HashMap<&str, Value> = HashMap::from([("get_state", view), ("get_history", serde_json::to_value(history::info("tab-1")).unwrap()), ("get_settings", get_settings()), ("get_keymap", serde_json::to_value(get_keymap()).unwrap()), ("get_project_status", serde_json::to_value(get_project_status()).unwrap()), ("get_build_info", serde_json::to_value(get_build_info()).unwrap()), ("get_builtin_groups", serde_json::to_value(get_builtin_groups()).unwrap()), ("graph_curves", serde_json::to_value(curves).unwrap()), ("graph_value_curves", serde_json::to_value(value_curves).unwrap())]);
    std::fs::create_dir_all("../smoke").unwrap();
    std::fs::write("../smoke/backend.json", serde_json::to_string(&backend).unwrap()).unwrap();
}
