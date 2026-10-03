// run from /src-tauri
// cargo test --test clipboard_test
//
// fixture: tests/fixtures/simple_scene_3_executor.mkproj, saved from the app, with short node ids

use std::collections::{BTreeSet, HashMap};

use serde_json::{json, Value};
use MIDIAnimator::graph::clipboard::{copy, node_names, paste};
use MIDIAnimator::graph::model::{node_specs, Graph, NodeSpec, Position};
use MIDIAnimator::graph::ops::{apply, Added, Ctx, Op};
use MIDIAnimator::state::{migrate_rf_instance, SavedProject};

fn specs() -> Vec<NodeSpec> {
    let data = std::fs::read_to_string("src/configs/default_nodes.json").unwrap();
    let default_nodes: HashMap<String, Value> = serde_json::from_str(&data).unwrap();
    node_specs(&default_nodes)
}

fn fixture() -> Graph {
    let data = std::fs::read_to_string("tests/fixtures/simple_scene_3_executor.mkproj").unwrap();
    let mut project: SavedProject = serde_json::from_str(&data).unwrap();
    migrate_rf_instance(&mut project.rf_instance);
    Graph::from_rf(&project.rf_instance).unwrap()
}

fn at(x: f64, y: f64) -> Position {
    Position {
        x,
        y,
    }
}

fn paste_text(graph: &mut Graph, scope: Option<&str>, text: &str, position: Position) -> Result<Vec<Added>, String> {
    let mut added = Vec::new();
    paste(graph, scope, &specs(), text, &position, &mut added)?;
    Ok(added)
}

fn run(graph: &mut Graph, scope: Option<&str>, ops: Value) {
    let specs = specs();
    let results = HashMap::new();
    let ctx = Ctx {
        specs: &specs,
        results: &results,
    };
    let ops: Vec<Op> = serde_json::from_value(ops).unwrap();
    let mut added = Vec::new();
    for op in &ops {
        apply(graph, scope, op, &ctx, &mut added).unwrap();
    }
}

fn connections(graph: &Graph) -> BTreeSet<String> {
    graph.edges.iter().map(|e| format!("{}.{} -> {}.{}", e.from_node(), e.from_output(), e.to_node(), e.to_input())).collect()
}

fn selected(graph: &Graph) -> BTreeSet<String> {
    graph.nodes.iter().filter(|n| n.extra.get("selected") == Some(&json!(true))).map(|n| n.id.clone()).collect()
}

// pasting gives new ids, keeps the connections between the copied nodes and their values, centered on the cursor
#[test]
fn copy_paste_round_trip() {
    let mut graph = fixture();
    let text = copy(&graph, None, &["get_midi_file-1".to_string(), "get_midi_track_data-1".to_string(), "viewer-1".to_string()]).unwrap();
    let payload: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(payload["format"], json!("motionkeys/nodes"));
    // the UI only fields stay behind
    assert!(payload["nodes"].as_array().unwrap().iter().all(|n| n.get("measured").is_none() && n.get("selected").is_none()));
    // only the connection between copied nodes
    assert_eq!(payload["edges"].as_array().unwrap().len(), 1);

    let added = paste_text(&mut graph, None, &text, at(1000.0, 1000.0)).unwrap();
    let ids: BTreeSet<&str> = added.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(ids, BTreeSet::from(["get_midi_file-2", "get_midi_track_data-2", "viewer-2"]));
    assert!(connections(&graph).contains("get_midi_file-2.tracks -> get_midi_track_data-2.tracks"));
    assert_eq!(graph.node("get_midi_file-2").unwrap().input_value("file_path"), graph.node("get_midi_file-1").unwrap().input_value("file_path"));
    assert_eq!(selected(&graph), ids.iter().map(|s| s.to_string()).collect());

    // the middle of the pasted nodes is the cursor
    let xs: Vec<f64> = added.iter().map(|a| a.position.x).collect();
    let ys: Vec<f64> = added.iter().map(|a| a.position.y).collect();
    let mid = |v: &[f64]| (v.iter().cloned().fold(f64::INFINITY, f64::min) + v.iter().cloned().fold(f64::NEG_INFINITY, f64::max)) / 2.0;
    assert!((mid(&xs) - 1000.0).abs() < 1e-6 && (mid(&ys) - 1000.0).abs() < 1e-6);
}

// a zone end brings its partner, an end pasted alone isn't paired
#[test]
fn zones() {
    let mut graph = fixture();
    run(&mut graph, None, json!([{ "op": "add_nodes", "nodes": [{ "type": "for_each_input", "position": { "x": 0, "y": 0 } }] }]));
    let text = copy(&graph, None, &["for_each_output-1".to_string()]).unwrap();
    paste_text(&mut graph, None, &text, at(0.0, 500.0)).unwrap();
    assert_eq!(graph.node("for_each_input-2").unwrap().data["zone"], json!("for_each_output-2"));
    assert_eq!(graph.node("for_each_output-2").unwrap().data["zone"], json!("for_each_input-2"));
}

// the project's own groups come along: the same one is used, a different one with the same id comes in under a new id
#[test]
fn groups_come_along() {
    let mut source = fixture();
    run(&mut source, None, json!([{ "op": "group", "nodes": ["get_midi_track_data-1"] }]));
    let text = copy(&source, None, &["node_group-1".to_string()]).unwrap();
    let payload: Value = serde_json::from_str(&text).unwrap();
    assert!(payload["groups"].get("node_group").is_some());
    // built-in groups are used by id
    let text_builtin = copy(&source, None, &["evaluate_instrument-1".to_string()]).unwrap();
    assert!(serde_json::from_str::<Value>(&text_builtin).unwrap()["groups"].as_object().unwrap().is_empty());

    // into the same project the group is the same, it's used
    let mut same = source.clone();
    paste_text(&mut same, None, &text, at(0.0, 0.0)).unwrap();
    assert_eq!(same.groups.len(), 1);
    assert_eq!(same.node("node_group-2").unwrap().data["group_id"], json!("node_group"));

    // into a project with a different `node_group` it comes in as node_group_2, named so it doesn't clash
    let mut other = fixture();
    run(&mut other, None, json!([{ "op": "group", "nodes": ["viewer-1"] }]));
    paste_text(&mut other, None, &text, at(0.0, 0.0)).unwrap();
    assert_eq!(other.groups.len(), 2);
    let pasted = other.nodes.iter().find(|n| n.data.get("group_id") == Some(&json!("node_group_2"))).expect("group node running the new id");
    assert_eq!(pasted.id, "node_group_2-1");
    assert_eq!(other.groups["node_group_2"].name, "NodeGroup.001");

    // into a fresh project it's imported as it is
    let mut fresh = Graph::default();
    paste_text(&mut fresh, None, &text, at(0.0, 0.0)).unwrap();
    assert!(fresh.groups.contains_key("node_group"));
}

// what can't go here is left out: the group input outside a group, a group inside itself, unknown types and inputs
#[test]
fn leaves_out_what_cant_go_here() {
    let mut graph = fixture();
    run(&mut graph, None, json!([{ "op": "group", "nodes": ["get_midi_track_data-1"] }]));

    // the group input and output only go inside a group
    let inside = copy(&graph, Some("node_group"), &["group_input-1".to_string(), "get_midi_track_data-1".to_string()]).unwrap();
    // the track data node moved into the group, so its id is free again at the top level
    let added = paste_text(&mut graph, None, &inside, at(0.0, 0.0)).unwrap();
    assert_eq!(added.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), ["get_midi_track_data-1"]);
    paste_text(&mut graph, Some("node_group"), &inside, at(0.0, 0.0)).unwrap();
    assert!(graph.groups["node_group"].graph.node("group_input-2").is_some());

    // a group node pasted inside its own group would run itself
    let group_node = copy(&graph, None, &["node_group-1".to_string()]).unwrap();
    assert!(paste_text(&mut graph, Some("node_group"), &group_node, at(0.0, 0.0)).is_err());

    // unknown types and inputs, values of the wrong type
    let text = json!({ "format": "motionkeys/nodes", "version": 1, "nodes": [
        { "id": "x-1", "type": "nope", "position": { "x": 0, "y": 0 }, "data": {} },
        { "id": "get_midi_file-9", "type": "get_midi_file", "position": { "x": 0, "y": 0 }, "data": { "inputs": { "file_path": 5, "evil": "yes" }, "junk": 1 }, "onclick": "x" },
    ] })
    .to_string();
    let added = paste_text(&mut graph, None, &text, at(0.0, 0.0)).unwrap();
    assert_eq!(added.len(), 1);
    let node = graph.node(&added[0].id).unwrap();
    assert_eq!(Value::Object(node.data.clone()), json!({ "inputs": {} }));
    // only the selection, `onclick` stayed behind
    assert_eq!(node.extra.keys().collect::<Vec<_>>(), ["selected"]);
}

// text that isn't nodes changes nothing, a group that contains itself is refused
#[test]
fn rejects_bad_text() {
    let mut graph = fixture();
    let before = graph.to_rf();
    for text in ["hello", "{}", r#"{"format":"other","version":1,"nodes":[]}"#, r#"{"format":"motionkeys/nodes","version":99,"nodes":[]}"#] {
        assert!(paste_text(&mut graph, None, text, at(0.0, 0.0)).is_err(), "{}", text);
    }
    let looped = json!({ "format": "motionkeys/nodes", "version": 1,
        "nodes": [{ "id": "loop-1", "type": "group", "position": { "x": 0, "y": 0 }, "data": { "group_id": "loop" } }],
        "groups": { "loop": { "name": "Loop", "interface": { "inputs": [], "outputs": [] },
            "nodes": [{ "id": "loop-1", "type": "group", "position": { "x": 0, "y": 0 }, "data": { "group_id": "loop" } }], "edges": [] } } })
    .to_string();
    assert!(paste_text(&mut graph, None, &looped, at(0.0, 0.0)).unwrap_err().contains("contain themselves"));
    assert_eq!(graph.to_rf(), before);
}

// cut is copy then delete, as one op
#[test]
fn cut_removes() {
    let mut graph = fixture();
    run(&mut graph, None, json!([{ "op": "cut", "nodes": ["viewer-1"] }]));
    assert!(graph.node("viewer-1").is_none());
    assert!(!graph.edges.iter().any(|e| e.to_node() == "viewer-1"));
}

// cut and paste steps name their nodes for the history panel, each name once with how many, in graph order
#[test]
fn names_for_history() {
    let mut graph = fixture();
    let text = copy(&graph, None, &["get_midi_file-1".to_string(), "viewer-1".to_string()]).unwrap();
    paste_text(&mut graph, None, &text, at(0.0, 0.0)).unwrap();
    let ids: Vec<String> = ["get_midi_file-1", "get_midi_file-2", "viewer-1", "evaluate_instrument-1"].iter().map(|s| s.to_string()).collect();
    assert_eq!(node_names(&graph, None, &specs(), &ids), "Viewer, Get MIDI File ×2, Evaluate Instrument");
}
