// run from /src-tauri
// cargo test --test ops_test
//
// fixture: tests/fixtures/simple_scene_3_executor.mkproj, saved from the app, with short node ids

use std::collections::{BTreeSet, HashMap};

use serde_json::{json, Value};
use motionkeys::graph::history::{Diff, History, Source, Step};
use motionkeys::graph::model::{node_specs, Graph, NodeSpec, Position};
use motionkeys::graph::clipboard;
use motionkeys::graph::ops::{apply, describe, unused, Added, Ctx, Op};
use motionkeys::state::{migrate_rf_instance, SavedProject};

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
        sizes: &HashMap::new(),
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

// unused nodes feed no viewer, scene writer or group output. frames, the group input and output and half a used zone
// never are, and deleting them keeps the frames
#[test]
fn unused_nodes() {
    let specs = specs();
    let unused = |graph: &Graph, scope: Option<&str>| -> BTreeSet<String> { unused(graph, scope, &specs).unwrap().into_iter().collect() };
    let set = |ids: &[&str]| -> BTreeSet<String> { ids.iter().map(|id| id.to_string()).collect() };
    let mut graph = fixture();
    // everything in the fixture feeds the viewer
    assert_eq!(unused(&graph, None), set(&[]));

    // a node feeding nothing, in a frame, and a chain feeding nothing
    run(&mut graph, None, json!([{ "op": "add_nodes", "nodes": [{ "type": "get_midi_file", "position": { "x": 0, "y": 0 } }, { "type": "get_midi_track_data", "position": { "x": 300, "y": 0 } }, { "type": "scene_link", "position": { "x": 0, "y": 500 } }] }])).unwrap();
    run(&mut graph, None, json!([{ "op": "connect", "from_node": "get_midi_file-2", "from_output": "tracks", "to_node": "get_midi_track_data-2", "to_input": "tracks" }, { "op": "join_in_frame", "nodes": ["scene_link-2"] }])).unwrap();
    assert_eq!(unused(&graph, None), set(&["get_midi_file-2", "get_midi_track_data-2", "scene_link-2"]));

    // a zone with nothing connected goes as a whole. one whose output feeds the viewer keeps its input, and what feeds
    // that, even with nothing between them
    run(&mut graph, None, json!([{ "op": "add_nodes", "nodes": [{ "type": "for_each_input", "position": { "x": 0, "y": 900 } }] }])).unwrap();
    assert_eq!(unused(&graph, None), set(&["get_midi_file-2", "get_midi_track_data-2", "scene_link-2", "for_each_input-1", "for_each_output-1"]));
    run(&mut graph, None, json!([{ "op": "connect", "from_node": "get_midi_track_data-2", "from_output": "notes", "to_node": "for_each_input-1", "to_input": "items" }, { "op": "connect", "from_node": "for_each_output-1", "from_output": "results", "to_node": "viewer-1", "to_input": "data" }])).unwrap();
    assert_eq!(unused(&graph, None), set(&["scene_link-2", "get_midi_file-1", "get_midi_track_data-1", "evaluate_instrument-1", "assign_notes_to_objects-1", "animation_generator-1", "keyframes_from_object-1", "scene_link-1"]));

    // a group node is an output when its group writes to Blender, even with nothing connected to it
    run(&mut graph, None, json!([{ "op": "add_nodes", "nodes": [{ "type": "scene_writer", "position": { "x": 900, "y": 0 } }] }])).unwrap();
    run(&mut graph, None, json!([{ "op": "connect", "from_node": "evaluate_instrument-1", "from_output": "keyframes", "to_node": "scene_writer-1", "to_input": "keyframes" }])).unwrap();
    run(&mut graph, None, json!([{ "op": "group", "nodes": ["scene_writer-1"] }])).unwrap();
    assert_eq!(unused(&graph, None), set(&["scene_link-2"]));
    // inside, the group output is an output and the group input is never unused
    assert_eq!(unused(&graph, Some("node_group")), set(&[]));
    run(&mut graph, Some("node_group"), json!([{ "op": "add_nodes", "nodes": [{ "type": "viewer", "position": { "x": 0, "y": 0 } }, { "type": "get_midi_file", "position": { "x": 0, "y": 300 } }] }])).unwrap();
    assert_eq!(unused(&graph, Some("node_group")), set(&["get_midi_file-1"]));

    // a built-in group's graph, everything feeds its group output
    assert_eq!(unused(&graph, Some("evaluate_instrument")), set(&[]));

    // deleted together their frame stays
    let ids: Vec<String> = unused(&graph, None).into_iter().collect();
    run(&mut graph, None, json!([{ "op": "delete", "nodes": ids, "keep_frames": true }])).unwrap();
    assert!(graph.node("scene_link-2").is_none());
    assert!(graph.nodes.iter().any(|n| n.is_frame()));
    assert_eq!(unused(&graph, None), set(&[]));
}

// a resize keeps the size it was given, without a height the node is as tall as it draws. both undo and are saved
#[test]
fn resize_height() {
    let mut graph = fixture();
    let start = graph.to_rf();
    let size = |graph: &Graph| graph.node("viewer-1").map(|n| (n.extra.get("width").cloned(), n.extra.get("height").cloned())).unwrap();
    run(&mut graph, None, json!([{ "op": "resize", "node": "viewer-1", "width": 300, "height": 500, "position": { "x": 0, "y": 0 } }])).unwrap();
    assert_eq!(size(&graph), (Some(json!(300.0)), Some(json!(500.0))));
    let tall = graph.to_rf();
    run(&mut graph, None, json!([{ "op": "resize", "node": "viewer-1", "width": 320, "height": null, "position": { "x": 0, "y": 0 } }])).unwrap();
    assert_eq!(size(&graph), (Some(json!(320.0)), None));

    // saved with the project, unlike what the canvas measured
    let mut saved = Graph::from_rf(&tall).unwrap();
    saved.strip_measured();
    assert_eq!(size(&saved), (Some(json!(300.0)), Some(json!(500.0))));

    let mut history = History::default();
    let mut project = start.clone();
    assert!(history.record(Diff::between(&start, &tall), Step::new("resize", Source::Ui)));
    assert!(history.record(Diff::between(&tall, &graph.to_rf()), Step::new("resize", Source::Ui)));
    history.undo(&mut project).unwrap();
    history.undo(&mut project).unwrap();
    assert_eq!(size(&Graph::from_rf(&project).unwrap()), (None, None));
    history.redo(&mut project).unwrap();
    assert_eq!(size(&Graph::from_rf(&project).unwrap()), (Some(json!(300.0)), Some(json!(500.0))));
}

// collapsing is kept on each node and saved, frames stay as they are, and it's one undo step that doesn't change what the
// graph computes
#[test]
fn collapse_state() {
    let specs = specs();
    let mut graph = fixture();
    let collapsed = |graph: &Graph| -> BTreeSet<String> { graph.nodes.iter().filter(|n| n.data.get("collapsed") == Some(&json!(true))).map(|n| n.id.clone()).collect() };
    run(&mut graph, None, json!([{ "op": "join_in_frame", "nodes": ["viewer-1"] }])).unwrap();
    let framed = graph.to_rf();

    let op = json!({ "op": "collapse", "nodes": ["viewer-1", "scene_link-1", "frame-1"], "collapsed": true });
    let before = graph.clone();
    run(&mut graph, None, json!([op.clone()])).unwrap();
    assert_eq!(collapsed(&graph), BTreeSet::from(["viewer-1".to_string(), "scene_link-1".to_string()]));
    let op: Op = serde_json::from_value(op).unwrap();
    assert_eq!(op.name(), "collapse");
    assert_eq!(describe(&op, &before, &graph, None, &specs, &[]), "Viewer, Scene Link");

    // saved with the project
    let mut saved = graph.clone();
    saved.strip_measured();
    assert_eq!(collapsed(&saved), collapsed(&graph));

    // only layout, one undo step
    let diff = Diff::between(&framed, &graph.to_rf());
    assert!(!diff.affects_output());
    let mut history = History::default();
    let mut project = framed.clone();
    assert!(history.record(diff, Step::new("collapse", Source::Ui)));
    history.undo(&mut project).unwrap();
    assert!(collapsed(&Graph::from_rf(&project).unwrap()).is_empty());
    history.redo(&mut project).unwrap();
    assert_eq!(collapsed(&Graph::from_rf(&project).unwrap()).len(), 2);

    // expanding takes it off, an unknown node changes nothing
    let expand: Op = serde_json::from_value(json!({ "op": "collapse", "nodes": [], "collapsed": false })).unwrap();
    assert_eq!(expand.name(), "expand");
    run(&mut graph, None, json!([{ "op": "collapse", "nodes": ["viewer-1"], "collapsed": false }])).unwrap();
    assert_eq!(collapsed(&graph), BTreeSet::from(["scene_link-1".to_string()]));
    assert!(run(&mut graph, None, json!([{ "op": "collapse", "nodes": ["nope", "scene_link-1"], "collapsed": false }])).is_err());
    assert_eq!(collapsed(&graph), BTreeSet::from(["scene_link-1".to_string()]));
}

// the note map's linked only view is kept on its node and saved, and it's one undo step that doesn't change what the
// graph computes, like collapsing
#[test]
fn linked_only_view() {
    let specs = specs();
    let mut graph = fixture();
    let start = graph.to_rf();
    let linked_only = |graph: &Graph| graph.node("assign_notes_to_objects-1").unwrap().data.get("linked_only").cloned();

    let op = json!({ "op": "linked_only", "node": "assign_notes_to_objects-1", "linked_only": true });
    let before = graph.clone();
    run(&mut graph, None, json!([op.clone()])).unwrap();
    assert_eq!(linked_only(&graph), Some(json!(true)));
    let op: Op = serde_json::from_value(op).unwrap();
    assert_eq!(op.name(), "linked_only");
    assert_eq!(describe(&op, &before, &graph, None, &specs, &[]), "Assign Notes to Objects");

    // saved with the project
    let mut saved = graph.clone();
    saved.strip_measured();
    assert_eq!(linked_only(&saved), Some(json!(true)));

    // only how it looks, one undo step
    let diff = Diff::between(&start, &graph.to_rf());
    assert!(!diff.affects_output());
    let mut history = History::default();
    let mut project = start.clone();
    assert!(history.record(diff, Step::new("linked_only", Source::Ui)));
    history.undo(&mut project).unwrap();
    assert_eq!(linked_only(&Graph::from_rf(&project).unwrap()), None);
    history.redo(&mut project).unwrap();
    assert_eq!(linked_only(&Graph::from_rf(&project).unwrap()), Some(json!(true)));

    // off takes it off, an unknown node changes nothing
    let all: Op = serde_json::from_value(json!({ "op": "linked_only", "node": "assign_notes_to_objects-1", "linked_only": false })).unwrap();
    assert_eq!(all.name(), "all_notes");
    run(&mut graph, None, json!([{ "op": "linked_only", "node": "assign_notes_to_objects-1", "linked_only": false }])).unwrap();
    assert_eq!(linked_only(&graph), None);
    assert!(run(&mut graph, None, json!([{ "op": "linked_only", "node": "nope", "linked_only": true }])).is_err());
}

// the note map's view (notes, animations or both) is kept on its node like linked only, it doesn't change what the
// graph computes
#[test]
fn map_view() {
    let mut graph = fixture();
    let start = graph.to_rf();
    let view = |graph: &Graph| graph.node("assign_notes_to_objects-1").unwrap().data.get("map_view").cloned();

    let op = json!({ "op": "map_view", "node": "assign_notes_to_objects-1", "view": "animations" });
    run(&mut graph, None, json!([op.clone()])).unwrap();
    assert_eq!(view(&graph), Some(json!("animations")));
    assert_eq!(serde_json::from_value::<Op>(op).unwrap().name(), "map_view");
    assert!(!Diff::between(&start, &graph.to_rf()).affects_output());

    // empty is both, which isn't kept. anything else is an error
    run(&mut graph, None, json!([{ "op": "map_view", "node": "assign_notes_to_objects-1", "view": "" }])).unwrap();
    assert_eq!(view(&graph), None);
    assert!(run(&mut graph, None, json!([{ "op": "map_view", "node": "assign_notes_to_objects-1", "view": "wires" }])).unwrap_err().contains("unknown map view"));
}

// a group goes once no group node runs it, at the top level or inside a group that's run, in the same undo step as the
// node. one being edited stays, and cut and paste brings one back
#[test]
fn unused_groups_go() {
    let mut graph = fixture();
    let groups = |graph: &Graph| -> BTreeSet<String> { graph.groups.keys().cloned().collect() };
    // node_group_2 runs inside node_group, a second node runs node_group
    run(&mut graph, None, json!([{ "op": "group", "nodes": ["get_midi_file-1"] }])).unwrap();
    run(&mut graph, Some("node_group"), json!([{ "op": "group", "nodes": ["get_midi_file-1"] }])).unwrap();
    run(&mut graph, None, json!([{ "op": "duplicate", "nodes": ["node_group-1"], "offset": { "x": 20, "y": 20 } }])).unwrap();
    assert_eq!(groups(&graph), BTreeSet::from(["node_group".to_string(), "node_group_2".to_string()]));

    // one of two nodes deleted, both groups stay
    run(&mut graph, None, json!([{ "op": "delete", "nodes": ["node_group-2"] }])).unwrap();
    assert_eq!(groups(&graph).len(), 2);

    // the last one goes with both, and undo puts them back in one step
    let before = graph.to_rf();
    run(&mut graph, None, json!([{ "op": "delete", "nodes": ["node_group-1"] }])).unwrap();
    assert!(graph.groups.is_empty());
    let mut history = History::default();
    let mut project = graph.to_rf();
    assert!(history.record(Diff::between(&before, &project), Step::new("delete", Source::Ui)));
    history.undo(&mut project).unwrap();
    let mut graph = Graph::from_rf(&project).unwrap();
    assert_eq!(groups(&graph).len(), 2);
    assert!(graph.node("node_group-1").is_some());

    // ungrouping drops the group whose node it took apart, the one inside comes out with its node
    run(&mut graph, None, json!([{ "op": "ungroup", "nodes": ["node_group-1"] }])).unwrap();
    assert_eq!(groups(&graph), BTreeSet::from(["node_group_2".to_string()]));

    // cut and paste: the copy carries the group, it comes back
    let node = graph.nodes.iter().find(|n| n.data.get("group_id") == Some(&json!("node_group_2"))).unwrap().id.clone();
    let text = clipboard::copy(&graph, None, &[node.clone()]).unwrap();
    run(&mut graph, None, json!([{ "op": "cut", "nodes": [node] }])).unwrap();
    assert!(graph.groups.is_empty());
    run(&mut graph, None, json!([{ "op": "paste", "text": text, "position": { "x": 0, "y": 0 } }])).unwrap();
    assert_eq!(groups(&graph).len(), 1);

    // a group edited with nothing running it stays while it's edited (MCP), and goes with the next edit elsewhere
    let mut graph = fixture();
    run(&mut graph, None, json!([{ "op": "group", "nodes": ["get_midi_file-1"] }])).unwrap();
    let mut def = graph.groups["node_group"].clone();
    def.name = "Spare".to_string();
    graph.groups.insert("spare".to_string(), def);
    run(&mut graph, Some("spare"), json!([{ "op": "add_nodes", "nodes": [{ "type": "viewer", "position": { "x": 0, "y": 0 } }] }])).unwrap();
    assert!(graph.groups.contains_key("spare"));
    run(&mut graph, None, json!([{ "op": "select", "nodes": [] }])).unwrap();
    assert_eq!(groups(&graph), BTreeSet::from(["node_group".to_string()]));
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

// connecting replaces what fed the input, a multi input keeps it, and cycles are refused
#[test]
fn connect_replaces_and_refuses_cycles() {
    let mut graph = fixture();
    let into = |graph: &Graph, node: &str, input: &str| -> Vec<String> { graph.edges.iter().filter(|e| e.to_node() == node && e.to_input() == input).map(|e| e.from_node().to_string()).collect() };
    // the generator's note on keyframes come from the object's location, the track data joins it (types don't match, allowed)
    let connect = json!([{ "op": "connect", "from_node": "get_midi_track_data-1", "from_output": "notes", "to_node": "animation_generator-1", "to_input": "note_on_keyframes" }]);
    run(&mut graph, None, connect.clone()).unwrap();
    assert_eq!(into(&graph, "animation_generator-1", "note_on_keyframes"), ["keyframes_from_object-1", "get_midi_track_data-1"]);
    // the same connection again isn't added twice
    run(&mut graph, None, connect).unwrap();
    assert_eq!(into(&graph, "animation_generator-1", "note_on_keyframes").len(), 2);

    // the assign node's notes come from the track data, feed them from the generator instead
    run(&mut graph, None, json!([{ "op": "connect", "from_node": "animation_generator-1", "from_output": "generator", "to_node": "assign_notes_to_objects-1", "to_input": "midi_notes" }])).unwrap();
    assert_eq!(into(&graph, "assign_notes_to_objects-1", "midi_notes"), ["animation_generator-1"]);

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
