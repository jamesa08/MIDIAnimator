// run from /src-tauri
// cargo test --test ops_test
//
// fixture: tests/fixtures/simple_scene_3_executor.mkproj, saved from the app, with short node ids

use std::collections::{BTreeSet, HashMap};

use serde_json::{json, Value};
use MIDIAnimator::graph::history::{Diff, History, Source, Step};
use MIDIAnimator::graph::model::{node_specs, Graph, NodeSpec, Position};
use MIDIAnimator::graph::ops::{apply, describe, Added, Ctx, Op};
use MIDIAnimator::state::{migrate_rf_instance, SavedProject};

/// loads the node specs from default_nodes.json
fn specs() -> Vec<NodeSpec> {
    let data = std::fs::read_to_string("src/configs/default_nodes.json").unwrap();
    let default_nodes: HashMap<String, Value> = serde_json::from_str(&data).unwrap();
    node_specs(&default_nodes)
}

/// the fixture project, migrated like the app does on load
fn fixture() -> Graph {
    let data = std::fs::read_to_string("tests/fixtures/simple_scene_3_executor.mkproj").unwrap();
    let mut project: SavedProject = serde_json::from_str(&data).unwrap();
    migrate_rf_instance(&mut project.rf_instance);
    Graph::from_rf(&project.rf_instance).unwrap()
}

/// applies ops from JSON the way the editor sends them, returns the nodes they added
fn run(graph: &mut Graph, scope: Option<&str>, ops: Value) -> Result<Vec<Added>, String> {
    let specs = specs();
    let results = HashMap::new();
    let ctx = Ctx {
        specs: &specs,
        results: &results,
    };
    let ops: Vec<Op> = serde_json::from_value(ops).unwrap();
    let mut added = Vec::new();
    for op in &ops {
        apply(graph, scope, op, &ctx, &mut added)?;
    }
    Ok(added)
}

fn selected(graph: &Graph) -> BTreeSet<String> {
    graph.nodes.iter().filter(|n| n.extra.get("selected") == Some(&json!(true))).map(|n| n.id.clone()).collect()
}

/// every connection as `from.output -> to.input`
fn connections(graph: &Graph) -> BTreeSet<String> {
    graph.edges.iter().map(|e| format!("{}.{} -> {}.{}", e.from_node(), e.from_output(), e.to_node(), e.to_input())).collect()
}

fn ids(graph: &Graph) -> BTreeSet<String> {
    graph.nodes.iter().map(|n| n.id.clone()).collect()
}

// a for each input comes with its output, the new nodes become the only selection
#[test]
fn add_nodes_pairs_zones() {
    let mut graph = fixture();
    run(&mut graph, None, json!([{ "op": "select", "nodes": ["viewer-1"] }])).unwrap();
    let added = run(&mut graph, None, json!([{ "op": "add_nodes", "nodes": [{ "type": "for_each_input", "position": { "x": 10, "y": 20 } }] }])).unwrap();

    let ids: Vec<&str> = added.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(ids, ["for_each_input-1", "for_each_output-1"]);
    assert_eq!(
        added[1].position,
        Position {
            x: 460.0,
            y: 20.0
        }
    );
    assert_eq!(graph.node("for_each_input-1").unwrap().data["zone"], json!("for_each_output-1"));
    assert_eq!(graph.node("for_each_output-1").unwrap().data["zone"], json!("for_each_input-1"));
    assert_eq!(selected(&graph), BTreeSet::from(["for_each_input-1".to_string(), "for_each_output-1".to_string()]));

    // a group node is named after its group, the group input only goes inside a group
    let added = run(&mut graph, None, json!([{ "op": "add_nodes", "nodes": [{ "type": "group", "data": { "group_id": "evaluate_instrument" }, "position": { "x": 0, "y": 0 } }] }])).unwrap();
    assert_eq!(added[0].id, "evaluate_instrument-2");
    assert!(run(&mut graph, None, json!([{ "op": "add_nodes", "nodes": [{ "type": "group_input", "position": { "x": 0, "y": 0 } }] }])).is_err());
    assert!(run(&mut graph, None, json!([{ "op": "add_nodes", "nodes": [{ "type": "nope", "position": { "x": 0, "y": 0 } }] }])).is_err());
}

// a new animation generator gets the first free generic name, a given name is kept
#[test]
fn add_nodes_names_animation_generators() {
    let mut graph = fixture();
    let generator = json!({ "type": "animation_generator", "position": { "x": 0, "y": 0 } });
    let named = json!({ "type": "animation_generator", "data": { "inputs": { "name": "kick" } }, "position": { "x": 0, "y": 0 } });
    let added = run(&mut graph, None, json!([{ "op": "add_nodes", "nodes": [generator, generator, named] }])).unwrap();

    let names: Vec<&Value> = added.iter().map(|a| graph.node(&a.id).unwrap().input_value("name").unwrap()).collect();
    assert_eq!(names, [&json!("Animation 1"), &json!("Animation 2"), &json!("kick")]);

    // a freed name is used again
    graph.node_mut(&added[0].id).unwrap().inputs_mut().insert("name".to_string(), json!("snare"));
    let added = run(&mut graph, None, json!([{ "op": "add_nodes", "nodes": [generator] }])).unwrap();
    assert_eq!(graph.node(&added[0].id).unwrap().input_value("name"), Some(&json!("Animation 1")));
}

// deleting half a zone deletes all of it, with every connection to the deleted nodes
#[test]
fn delete_takes_zones_and_edges() {
    let mut graph = fixture();
    run(&mut graph, None, json!([{ "op": "add_nodes", "nodes": [{ "type": "for_each_input", "position": { "x": 0, "y": 0 } }] }])).unwrap();
    let edge = graph.edges.iter().find(|e| e.to_node() == "viewer-1").unwrap().id.clone();
    run(&mut graph, None, json!([{ "op": "delete", "nodes": ["for_each_output-1", "get_midi_track_data-1"], "edges": [edge] }])).unwrap();

    assert!(graph.node("for_each_input-1").is_none());
    assert!(graph.node("for_each_output-1").is_none());
    assert!(!graph.edges.iter().any(|e| e.from_node() == "get_midi_track_data-1" || e.to_node() == "get_midi_track_data-1" || e.to_node() == "viewer-1"));
}

// duplicating copies the connections between the copied nodes only, and brings a zone's other end along
#[test]
fn duplicate_copies_inner_connections() {
    let mut graph = fixture();
    let added = run(&mut graph, None, json!([{ "op": "duplicate", "nodes": ["get_midi_file-1", "get_midi_track_data-1"], "offset": { "x": 20, "y": 20 } }])).unwrap();
    assert_eq!(added.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), ["get_midi_file-2", "get_midi_track_data-2"]);
    assert!(connections(&graph).contains("get_midi_file-2.tracks -> get_midi_track_data-2.tracks"));
    assert!(!graph.edges.iter().any(|e| e.from_node() == "get_midi_track_data-2"));
    assert_eq!(selected(&graph), BTreeSet::from(["get_midi_file-2".to_string(), "get_midi_track_data-2".to_string()]));
    let original = graph.node("get_midi_file-1").unwrap().position.clone();
    assert_eq!(
        graph.node("get_midi_file-2").unwrap().position,
        Position {
            x: original.x + 20.0,
            y: original.y + 20.0
        }
    );

    run(&mut graph, None, json!([{ "op": "add_nodes", "nodes": [{ "type": "for_each_input", "position": { "x": 0, "y": 0 } }] }])).unwrap();
    run(&mut graph, None, json!([{ "op": "duplicate", "nodes": ["for_each_output-1"], "offset": { "x": 20, "y": 20 } }])).unwrap();
    assert_eq!(graph.node("for_each_input-2").unwrap().data["zone"], json!("for_each_output-2"));
    assert_eq!(graph.node("for_each_output-2").unwrap().data["zone"], json!("for_each_input-2"));
}

// grouping moves the nodes inside with sockets for the crossing connections, ungrouping puts the same connections back
#[test]
fn group_and_ungroup() {
    let mut graph = fixture();
    let before = connections(&graph);
    let nodes = ["get_midi_track_data-1", "assign_notes_to_objects-1"];
    let added = run(&mut graph, None, json!([{ "op": "group", "nodes": nodes }])).unwrap();

    let group_node = &added[0].id;
    assert_eq!(group_node, "node_group-1");
    assert_eq!(selected(&graph), BTreeSet::from([group_node.clone()]));
    let def = &graph.groups["node_group"];
    assert_eq!(def.name, "NodeGroup");
    // tracks, object groups and the generator come in, the notes and the object map go out (notes once, used twice outside)
    let inputs: Vec<&str> = def.interface.inputs.iter().map(|h| h.id.as_str()).collect();
    let outputs: Vec<&str> = def.interface.outputs.iter().map(|h| h.id.as_str()).collect();
    assert_eq!(inputs.len(), 3, "{:?}", inputs);
    assert_eq!(outputs.len(), 2, "{:?}", outputs);
    assert!(ids(&def.graph).contains("group_input-1") && ids(&def.graph).contains("group_output-1"));
    assert!(!ids(&graph).contains("get_midi_track_data-1"));
    assert!(graph.edges.iter().all(|e| !nodes.contains(&e.from_node()) && !nodes.contains(&e.to_node())));

    // ungrouping gives back the same nodes and connections
    let added = run(&mut graph, None, json!([{ "op": "ungroup", "nodes": [group_node] }])).unwrap();
    assert_eq!(added.iter().map(|a| a.id.as_str()).collect::<BTreeSet<_>>(), BTreeSet::from(nodes));
    assert_eq!(connections(&graph), before);
    assert!(!ids(&graph).contains(group_node));
}

// sockets: the empty socket adds one named after the other end, removing one drops its connections inside and outside
#[test]
fn group_sockets() {
    let mut graph = fixture();
    run(&mut graph, None, json!([{ "op": "group", "nodes": ["get_midi_track_data-1"] }])).unwrap();
    let scope = Some("node_group");
    let def = &graph.groups["node_group"];
    let tracks = def.interface.inputs[0].id.clone();
    let notes = def.interface.outputs[0].id.clone();

    // a second output, from the track data's control changes
    run(&mut graph, scope, json!([{ "op": "connect", "from_node": "get_midi_track_data-1", "from_output": "control_change", "to_node": "group_output-1", "to_input": "__new__" }])).unwrap();
    let def = &graph.groups["node_group"];
    let added = def.interface.outputs.last().unwrap();
    assert_eq!((added.id.as_str(), added.name.as_str()), ("control_change", "Control Change"));
    assert!(connections(&def.graph).contains("get_midi_track_data-1.control_change -> group_output-1.control_change"));

    run(&mut graph, scope, json!([{ "op": "rename_socket", "side": "inputs", "id": tracks, "name": "Song" }])).unwrap();
    assert_eq!(graph.groups["node_group"].interface.inputs[0].name, "Song");

    // the notes output fed two nodes outside, both connections go
    run(&mut graph, scope, json!([{ "op": "remove_socket", "side": "outputs", "id": notes }])).unwrap();
    assert!(!graph.edges.iter().any(|e| e.from_node() == "node_group-1" && e.from_output() == notes));
    assert!(!graph.groups["node_group"].graph.edges.iter().any(|e| e.to_node() == "group_output-1" && e.to_input() == notes));
    assert!(graph.edges.iter().any(|e| e.to_node() == "node_group-1"));
}

// a built-in group is read-only until it's made local, reverting drops the copy
#[test]
fn built_in_groups() {
    let mut graph = fixture();
    let scope = Some("evaluate_instrument");
    let op = json!([{ "op": "add_nodes", "nodes": [{ "type": "viewer", "position": { "x": 0, "y": 0 } }] }]);
    assert!(run(&mut graph, scope, op.clone()).unwrap_err().contains("built-in"));

    run(&mut graph, scope, json!([{ "op": "make_local" }])).unwrap();
    assert!(graph.groups.contains_key("evaluate_instrument"));
    run(&mut graph, scope, op).unwrap();

    run(&mut graph, scope, json!([{ "op": "revert_group" }])).unwrap();
    assert!(graph.groups.is_empty());
}

// connecting replaces what fed the input, and refuses cycles
#[test]
fn connect_replaces_and_refuses_cycles() {
    let mut graph = fixture();
    // the generator's note on keyframes come from the object's location, feed it from the track data instead (types don't match, allowed)
    run(&mut graph, None, json!([{ "op": "connect", "from_node": "get_midi_track_data-1", "from_output": "notes", "to_node": "animation_generator-1", "to_input": "note_on_keyframes" }])).unwrap();
    let into: Vec<String> = graph.edges.iter().filter(|e| e.to_node() == "animation_generator-1" && e.to_input() == "note_on_keyframes").map(|e| e.from_node().to_string()).collect();
    assert_eq!(into, ["get_midi_track_data-1"]);

    run(&mut graph, None, json!([{ "op": "connect", "from_node": "evaluate_instrument-1", "from_output": "keyframes", "to_node": "viewer-1", "to_input": "data" }])).unwrap();
    assert!(run(&mut graph, None, json!([{ "op": "connect", "from_node": "viewer-1", "from_output": "x", "to_node": "evaluate_instrument-1", "to_input": "object_map" }])).unwrap_err().contains("cycle"));
}

// edits made through ops undo back to where they started, group and selection included
#[test]
fn ops_undo() {
    let mut graph = fixture();
    let start = graph.to_rf();
    let mut history = History::default();
    let mut project = start.clone();
    let steps = [json!([{ "op": "select", "nodes": ["viewer-1", "get_midi_file-1"] }]), json!([{ "op": "duplicate", "nodes": ["viewer-1", "get_midi_file-1"], "offset": { "x": 20, "y": 20 } }]), json!([{ "op": "move", "positions": { "viewer-2": { "x": 5, "y": 5 } } }]), json!([{ "op": "group", "nodes": ["get_midi_track_data-1", "assign_notes_to_objects-1"] }]), json!([{ "op": "set_inputs", "node": "get_midi_file-2", "inputs": { "file_path": "/a.mid" } }]), json!([{ "op": "ungroup", "nodes": ["node_group-1"] }])];
    let mut states = vec![project.clone()];
    for ops in steps {
        run(&mut graph, None, ops.clone()).unwrap();
        let after = graph.to_rf();
        assert!(history.record(Diff::between(&project, &after), Step::new("edit", Source::Ui)), "{} changed nothing", ops);
        project = after;
        states.push(project.clone());
    }
    // nodes put back by undo come without the UI only fields (sizes), the editor measures them again. not selected and
    // no `selected` are the same
    let without_ui = |project: &HashMap<String, Value>| {
        let mut graph = Graph::from_rf(project).unwrap();
        let keep = |k: &String, v: &mut Value| !["dragging", "measured", "resizing"].contains(&k.as_str()) && !(k == "selected" && *v == json!(false));
        graph.nodes.iter_mut().for_each(|n| n.extra.retain(keep));
        graph.edges.iter_mut().for_each(|e| e.extra.retain(keep));
        graph.to_rf()
    };
    for state in states.iter().rev().skip(1) {
        history.undo(&mut project).unwrap();
        assert_eq!(without_ui(&project), without_ui(state));
    }
    // selecting is a step of its own, it doesn't change what the graph computes
    let mut history = History::default();
    let mut graph = Graph::from_rf(&start).unwrap();
    run(&mut graph, None, json!([{ "op": "select", "nodes": ["get_midi_file-1"] }])).unwrap();
    let diff = Diff::between(&start, &graph.to_rf());
    assert!(!diff.affects_output());
    assert!(history.record(diff, Step::new("select", Source::Ui)));
}

// each step says what it acted on, for its row in the history panel
#[test]
fn describe_steps() {
    let specs = specs();
    let step = |graph: &mut Graph, scope: Option<&str>, op: Value| -> String {
        let before = graph.clone();
        let added = run(graph, scope, json!([op.clone()])).unwrap();
        let op: Op = serde_json::from_value(op).unwrap();
        describe(&op, &before, graph, scope, &specs, &added)
    };
    let mut graph = fixture();
    let edge = graph.edges.iter().find(|e| e.to_node() == "viewer-1").unwrap().id.clone();

    assert_eq!(step(&mut graph, None, json!({ "op": "add_nodes", "nodes": [{ "type": "viewer", "position": { "x": 0, "y": 0 } }] })), "Viewer");
    assert_eq!(step(&mut graph, None, json!({ "op": "select", "nodes": ["get_midi_file-1", "viewer-2"] })), "Get MIDI File, Viewer");
    assert_eq!(step(&mut graph, None, json!({ "op": "duplicate", "nodes": ["viewer-1", "viewer-2"], "offset": { "x": 20, "y": 20 } })), "Viewer ×2");
    assert_eq!(step(&mut graph, None, json!({ "op": "set_inputs", "node": "animation_generator-1", "inputs": { "name": "x" } })), "Animation Generator");
    assert_eq!(step(&mut graph, None, json!({ "op": "connect", "from_node": "get_midi_track_data-1", "from_output": "notes", "to_node": "viewer-2", "to_input": "data" })), "Get MIDI Track Data → Viewer");
    // a connection is named by its ends
    assert_eq!(step(&mut graph, None, json!({ "op": "delete", "edges": [edge] })), "Viewer, Evaluate Instrument");
    assert_eq!(step(&mut graph, None, json!({ "op": "delete", "nodes": ["viewer-3"] })), "Viewer");
    assert_eq!(step(&mut graph, None, json!({ "op": "group", "nodes": ["get_midi_file-1", "get_midi_track_data-1"] })), "Get MIDI File, Get MIDI Track Data");

    let scope = Some("node_group");
    let socket = graph.groups["node_group"].interface.outputs[0].clone();
    assert_eq!(step(&mut graph, scope, json!({ "op": "rename_socket", "side": "outputs", "id": socket.id, "name": "Song Notes" })), "Song Notes");
    assert_eq!(step(&mut graph, scope, json!({ "op": "remove_socket", "side": "outputs", "id": socket.id })), "Song Notes");
    assert_eq!(step(&mut graph, Some("evaluate_instrument"), json!({ "op": "make_local" })), "Evaluate Instrument");
}
