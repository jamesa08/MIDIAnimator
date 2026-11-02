// run from /src-tauri
// cargo test --test frames_test
//
// fixture: tests/fixtures/simple_scene_3_executor.mkproj, saved from the app, with short node ids

use std::collections::{BTreeSet, HashMap};

use serde_json::{json, Value};
use motionkeys::graph::builtin::all_groups;
use motionkeys::graph::clipboard::{copy, paste};
use motionkeys::graph::edit;
use motionkeys::graph::frames::Size;
use motionkeys::graph::history::{Diff, History, Source, Step};
use motionkeys::graph::model::{node_specs, Graph, NodeSpec, Position, Specs};
use motionkeys::graph::ops::{apply, Added, Ctx, Op};
use motionkeys::graph::outline::{node_block, Detail, OutlineCtx};
use motionkeys::state::{migrate_rf_instance, SavedProject};

/// loads the node specs from default_nodes.json
fn specs() -> Vec<NodeSpec> {
    let data = std::fs::read_to_string("src/configs/default_nodes.json").unwrap();
    let default_nodes: HashMap<String, Value> = serde_json::from_str(&data).unwrap();
    node_specs(&default_nodes)
}

/// the fixture project, migrated like the app does on load, with some nodes where the tests can measure them:
/// get_midi_file-1 at 0,0 and viewer-1 at 300,100
fn fixture() -> Graph {
    let data = std::fs::read_to_string("tests/fixtures/simple_scene_3_executor.mkproj").unwrap();
    let mut project: SavedProject = serde_json::from_str(&data).unwrap();
    migrate_rf_instance(&mut project.rf_instance);
    let mut graph = Graph::from_rf(&project.rf_instance).unwrap();
    run(&mut graph, None, json!([{ "op": "move", "positions": { "get_midi_file-1": { "x": 0, "y": 0 }, "viewer-1": { "x": 300, "y": 100 } } }])).unwrap();
    graph
}

/// the sizes the editor measured: get_midi_file-1 200x100, viewer-1 100x50
fn sizes() -> HashMap<String, Size> {
    HashMap::from([
        (
            "get_midi_file-1".to_string(),
            Size {
                width: 200.0,
                height: 100.0,
            },
        ),
        (
            "viewer-1".to_string(),
            Size {
                width: 100.0,
                height: 50.0,
            },
        ),
    ])
}

/// applies ops from JSON the way the editor sends them, with its sizes, returns the nodes they added
fn run(graph: &mut Graph, scope: Option<&str>, ops: Value) -> Result<Vec<Added>, String> {
    let specs = specs();
    let (results, sizes) = (HashMap::new(), sizes());
    let ctx = Ctx {
        specs: &specs,
        results: &results,
        sizes: &sizes,
    };
    let ops: Vec<Op> = serde_json::from_value(ops).unwrap();
    let mut added = Vec::new();
    for op in &ops {
        apply(graph, scope, op, &ctx, &mut added)?;
    }
    Ok(added)
}

/// the frame a node is in
fn frame_of<'a>(graph: &'a Graph, id: &str) -> Option<&'a str> {
    graph.node(id).unwrap().frame()
}

/// a frame's position and size
fn rect(graph: &Graph, id: &str) -> (f64, f64, f64, f64) {
    let frame = graph.node(id).unwrap();
    (frame.position.x, frame.position.y, frame.extra["width"].as_f64().unwrap(), frame.extra["height"].as_f64().unwrap())
}

fn selected(graph: &Graph) -> BTreeSet<String> {
    graph.nodes.iter().filter(|n| n.extra.get("selected") == Some(&json!(true))).map(|n| n.id.clone()).collect()
}

fn ids(graph: &Graph) -> BTreeSet<String> {
    graph.nodes.iter().map(|n| n.id.clone()).collect()
}

// a new frame fits around the nodes with room for its label, gets the default color and becomes the selection
#[test]
fn join_fits_and_selects() {
    let mut graph = fixture();
    let added = run(&mut graph, None, json!([{ "op": "join_in_frame", "nodes": ["get_midi_file-1", "viewer-1"] }])).unwrap();

    assert_eq!(added.len(), 1);
    assert_eq!(added[0].id, "frame-1");
    assert_eq!(
        added[0].position,
        Position {
            x: -20.0,
            y: -56.0
        }
    );
    assert_eq!(frame_of(&graph, "get_midi_file-1"), Some("frame-1"));
    assert_eq!(frame_of(&graph, "viewer-1"), Some("frame-1"));
    // 0..400 wide, 0..150 tall, 20 around and 36 for the label
    assert_eq!(rect(&graph, "frame-1"), (-20.0, -56.0, 440.0, 226.0));
    assert_eq!(graph.node("frame-1").unwrap().data["color"], json!("#999999"));
    assert_eq!(selected(&graph), BTreeSet::from(["frame-1".to_string()]));

    // moving a node in it fits the frame again
    run(&mut graph, None, json!([{ "op": "move", "positions": { "viewer-1": { "x": 500, "y": 100 } } }])).unwrap();
    assert_eq!(rect(&graph, "frame-1"), (-20.0, -56.0, 640.0, 226.0));

    // nothing selected joins nothing, and a frame can't be added on its own
    assert!(run(&mut graph, None, json!([{ "op": "join_in_frame", "nodes": [] }])).is_err());
    assert!(run(&mut graph, None, json!([{ "op": "add_nodes", "nodes": [{ "type": "frame", "position": { "x": 0, "y": 0 } }] }])).is_err());
}

// a node added where it was dropped on a frame goes in it, a for each with both its ends
#[test]
fn add_nodes_in_a_frame() {
    let mut graph = fixture();
    run(&mut graph, None, json!([{ "op": "join_in_frame", "nodes": ["get_midi_file-1"] }])).unwrap();
    let added = run(&mut graph, None, json!([{ "op": "add_nodes", "nodes": [{ "type": "for_each_input", "position": { "x": 0, "y": 200 }, "frame": "frame-1" }, { "type": "viewer", "position": { "x": 0, "y": 600 } }] }])).unwrap();
    let frames: Vec<Option<&str>> = added.iter().map(|a| frame_of(&graph, &a.id)).collect();
    assert_eq!(frames, [Some("frame-1"), Some("frame-1"), None]);
    // the frame takes in what was added
    assert_eq!(rect(&graph, "frame-1").3, 376.0);
    assert!(run(&mut graph, None, json!([{ "op": "add_nodes", "nodes": [{ "type": "viewer", "position": { "x": 0, "y": 0 }, "frame": "get_midi_file-1" }] }])).is_err());
}

// frames nest: a new frame goes in the frame its nodes share, a node in a joined frame stays in it, and remove from
// frame takes a node out one level, removing a frame it leaves empty
#[test]
fn nesting_and_remove_from_frame() {
    let mut graph = fixture();
    run(&mut graph, None, json!([{ "op": "join_in_frame", "nodes": ["get_midi_file-1", "viewer-1"] }])).unwrap();
    run(&mut graph, None, json!([{ "op": "join_in_frame", "nodes": ["get_midi_file-1"] }])).unwrap();
    assert_eq!(frame_of(&graph, "frame-2"), Some("frame-1"));
    assert_eq!(frame_of(&graph, "get_midi_file-1"), Some("frame-2"));
    assert_eq!(frame_of(&graph, "viewer-1"), Some("frame-1"));
    // the outer frame fits around the inner one
    let (inner, outer) = (rect(&graph, "frame-2"), rect(&graph, "frame-1"));
    assert_eq!(inner, (-20.0, -56.0, 240.0, 176.0));
    assert_eq!(outer, (-40.0, -112.0, 460.0, 282.0));

    // the frame and a node outside it go in a new frame, what's inside the frame stays where it is
    run(&mut graph, None, json!([{ "op": "join_in_frame", "nodes": ["frame-1", "scene_link-1", "get_midi_file-1"] }])).unwrap();
    assert_eq!(frame_of(&graph, "frame-1"), Some("frame-3"));
    assert_eq!(frame_of(&graph, "scene_link-1"), Some("frame-3"));
    assert_eq!(frame_of(&graph, "get_midi_file-1"), Some("frame-2"));
    assert_eq!(frame_of(&graph, "frame-3"), None);

    // out one level, into the frame around it. a frame that still holds something stays
    run(&mut graph, None, json!([{ "op": "remove_from_frame", "nodes": ["viewer-1"] }])).unwrap();
    assert_eq!(frame_of(&graph, "viewer-1"), Some("frame-3"));
    assert!(graph.node("frame-1").is_some());
    run(&mut graph, None, json!([{ "op": "remove_from_frame", "nodes": ["get_midi_file-1"] }])).unwrap();
    assert_eq!(frame_of(&graph, "get_midi_file-1"), Some("frame-1"));
    assert!(graph.node("frame-2").is_none(), "an emptied frame goes");
    // a node in a frame that's taken out too stays in it
    run(&mut graph, None, json!([{ "op": "remove_from_frame", "nodes": ["frame-1", "get_midi_file-1"] }])).unwrap();
    assert_eq!(frame_of(&graph, "frame-1"), None);
    assert_eq!(frame_of(&graph, "get_midi_file-1"), Some("frame-1"));
    assert!(run(&mut graph, None, json!([{ "op": "remove_from_frame", "nodes": ["animation_generator-1"] }])).is_err());
}

// a move can drop nodes in and out of frames, never a frame inside itself
#[test]
fn move_drops_in_and_out() {
    let mut graph = fixture();
    run(&mut graph, None, json!([{ "op": "join_in_frame", "nodes": ["get_midi_file-1"] }])).unwrap();
    run(&mut graph, None, json!([{ "op": "move", "positions": { "viewer-1": { "x": 300, "y": 0 } }, "frames": { "viewer-1": "frame-1" } }])).unwrap();
    assert_eq!(frame_of(&graph, "viewer-1"), Some("frame-1"));
    assert_eq!(rect(&graph, "frame-1"), (-20.0, -56.0, 440.0, 176.0));

    run(&mut graph, None, json!([{ "op": "move", "positions": { "viewer-1": { "x": 900, "y": 0 } }, "frames": { "viewer-1": null } }])).unwrap();
    assert_eq!(frame_of(&graph, "viewer-1"), None);
    assert_eq!(rect(&graph, "frame-1"), (-20.0, -56.0, 240.0, 176.0));

    // not inside itself, nor inside a frame inside it
    run(&mut graph, None, json!([{ "op": "join_in_frame", "nodes": ["get_midi_file-1"] }])).unwrap();
    assert!(run(&mut graph, None, json!([{ "op": "move", "positions": {}, "frames": { "frame-1": "frame-1" } }])).is_err());
    assert!(run(&mut graph, None, json!([{ "op": "move", "positions": {}, "frames": { "frame-1": "frame-2" } }])).is_err());
    assert!(run(&mut graph, None, json!([{ "op": "move", "positions": {}, "frames": { "viewer-1": "get_midi_file-1" } }])).is_err());
}

// deleting a frame keeps its nodes in the frame around it (Blender), cutting one takes them along, and deleting
// everything in a frame takes the frame
#[test]
fn delete_keeps_cut_takes() {
    let mut graph = fixture();
    run(&mut graph, None, json!([{ "op": "join_in_frame", "nodes": ["get_midi_file-1", "viewer-1"] }, { "op": "join_in_frame", "nodes": ["frame-1", "scene_link-1"] }])).unwrap();
    let before = ids(&graph);

    run(&mut graph, None, json!([{ "op": "delete", "nodes": ["frame-1"] }])).unwrap();
    assert_eq!(before.len() - 1, ids(&graph).len());
    assert_eq!(frame_of(&graph, "get_midi_file-1"), Some("frame-2"));
    assert_eq!(frame_of(&graph, "viewer-1"), Some("frame-2"));

    run(&mut graph, None, json!([{ "op": "cut", "nodes": ["frame-2"] }])).unwrap();
    for id in ["frame-2", "get_midi_file-1", "viewer-1", "scene_link-1"] {
        assert!(graph.node(id).is_none(), "{} wasn't cut", id);
    }
    assert!(graph.edges.iter().all(|e| e.from_node() != "get_midi_file-1"));

    // deleting what's in a frame takes the frame along once nothing's left in it, and a frame that only held that one
    let mut graph = fixture();
    run(&mut graph, None, json!([{ "op": "join_in_frame", "nodes": ["get_midi_file-1", "viewer-1"] }, { "op": "join_in_frame", "nodes": ["frame-1"] }])).unwrap();
    run(&mut graph, None, json!([{ "op": "delete", "nodes": ["get_midi_file-1"] }])).unwrap();
    assert!(graph.node("frame-1").is_some() && graph.node("frame-2").is_some(), "a frame with something left stays");
    run(&mut graph, None, json!([{ "op": "delete", "nodes": ["viewer-1"] }])).unwrap();
    assert!(graph.node("frame-1").is_none() && graph.node("frame-2").is_none());
    let mut graph = fixture();
    run(&mut graph, None, json!([{ "op": "join_in_frame", "nodes": ["get_midi_file-1", "viewer-1"] }])).unwrap();
    edit::remove_node(&mut graph, "get_midi_file-1").unwrap();
    edit::remove_node(&mut graph, "viewer-1").unwrap();
    assert!(graph.node("frame-1").is_none());

    // removing a frame over MCP keeps its nodes too
    let mut graph = fixture();
    run(&mut graph, None, json!([{ "op": "join_in_frame", "nodes": ["get_midi_file-1"] }])).unwrap();
    edit::remove_node(&mut graph, "frame-1").unwrap();
    assert_eq!(frame_of(&graph, "get_midi_file-1"), None);
}

// duplicating a frame copies what's in it, a copy of a node in a frame stays in that frame
#[test]
fn duplicate_brings_contents() {
    let mut graph = fixture();
    run(&mut graph, None, json!([{ "op": "join_in_frame", "nodes": ["get_midi_file-1", "get_midi_track_data-1"] }])).unwrap();
    let added = run(&mut graph, None, json!([{ "op": "duplicate", "nodes": ["frame-1"], "offset": { "x": 20, "y": 20 } }])).unwrap();
    let copies: BTreeSet<&str> = added.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(copies, BTreeSet::from(["frame-2", "get_midi_file-2", "get_midi_track_data-2"]));
    assert_eq!(frame_of(&graph, "get_midi_file-2"), Some("frame-2"));
    assert_eq!(frame_of(&graph, "get_midi_track_data-2"), Some("frame-2"));
    assert!(graph.edges.iter().any(|e| e.from_node() == "get_midi_file-2" && e.to_node() == "get_midi_track_data-2"));
    // the copy of what was selected is selected, every copy is grabbed
    assert_eq!(selected(&graph), BTreeSet::from(["frame-2".to_string()]));

    run(&mut graph, None, json!([{ "op": "duplicate", "nodes": ["get_midi_file-1"], "offset": { "x": 20, "y": 20 } }])).unwrap();
    assert_eq!(frame_of(&graph, "get_midi_file-3"), Some("frame-1"));
}

// a resized frame keeps at least its size and grows past it to take in its nodes, turning shrink back on fits it again
#[test]
fn resize_stops_shrinking() {
    let mut graph = fixture();
    run(&mut graph, None, json!([{ "op": "join_in_frame", "nodes": ["get_midi_file-1"] }])).unwrap();
    run(&mut graph, None, json!([{ "op": "resize", "node": "frame-1", "width": 600, "height": 500, "position": { "x": -100, "y": -100 } }])).unwrap();
    assert_eq!(graph.node("frame-1").unwrap().data["shrink"], json!(false));
    run(&mut graph, None, json!([{ "op": "move", "positions": { "get_midi_file-1": { "x": 50, "y": 50 } } }])).unwrap();
    assert_eq!(rect(&graph, "frame-1"), (-100.0, -100.0, 600.0, 500.0));

    // a node past its size makes it grow to take it in
    run(&mut graph, None, json!([{ "op": "move", "positions": { "get_midi_file-1": { "x": 450, "y": 50 } } }])).unwrap();
    assert_eq!(rect(&graph, "frame-1"), (-100.0, -100.0, 770.0, 500.0));
    run(&mut graph, None, json!([{ "op": "move", "positions": { "get_midi_file-1": { "x": 50, "y": 50 } } }])).unwrap();

    run(&mut graph, None, json!([{ "op": "set_frame", "node": "frame-1", "shrink": true, "color": "#336699" }])).unwrap();
    assert_eq!(rect(&graph, "frame-1"), (30.0, -6.0, 240.0, 176.0));
    assert_eq!(graph.node("frame-1").unwrap().data["color"], json!("#336699"));
    assert!(run(&mut graph, None, json!([{ "op": "set_frame", "node": "viewer-1", "shrink": true }])).is_err());
}

// a frame's style: the label's size changes the room for it, lock size keeps exactly its size, defaults aren't kept
#[test]
fn style_and_lock_size() {
    let mut graph = fixture();
    run(&mut graph, None, json!([{ "op": "join_in_frame", "nodes": ["get_midi_file-1"] }])).unwrap();
    run(&mut graph, None, json!([{ "op": "set_frame", "node": "frame-1", "label_size": 40, "label_align": "center", "locked": true }])).unwrap();
    assert_eq!(rect(&graph, "frame-1"), (-20.0, -76.0, 240.0, 196.0));
    let data = &graph.node("frame-1").unwrap().data;
    assert_eq!((&data["label_size"], &data["label_align"], &data["locked"]), (&json!(40.0), &json!("center"), &json!(true)));

    // back to the defaults drops them, a size past the limits is held to them
    run(&mut graph, None, json!([{ "op": "set_frame", "node": "frame-1", "label_size": 500, "label_align": "left", "locked": false }])).unwrap();
    let data = &graph.node("frame-1").unwrap().data;
    assert_eq!(data["label_size"], json!(64.0));
    assert!(data.get("label_align").is_none() && data.get("locked").is_none());
    assert!(run(&mut graph, None, json!([{ "op": "set_frame", "node": "frame-1", "label_align": "middle" }])).is_err());

    // lock size: a node moved out sticks out of it
    run(&mut graph, None, json!([{ "op": "set_frame", "node": "frame-1", "label_size": 20, "lock_size": true }])).unwrap();
    let locked = rect(&graph, "frame-1");
    run(&mut graph, None, json!([{ "op": "move", "positions": { "get_midi_file-1": { "x": 900, "y": 900 } } }])).unwrap();
    assert_eq!(rect(&graph, "frame-1"), locked);
    assert_eq!(frame_of(&graph, "get_midi_file-1"), Some("frame-1"));
}

// every frame edit is one undo step that undoes exactly, and none of them changes what the graph computes
#[test]
fn frame_ops_undo_and_never_run() {
    let mut graph = fixture();
    let mut project = graph.to_rf();
    let mut history = History::default();
    let steps = [
        json!([{ "op": "join_in_frame", "nodes": ["get_midi_file-1", "viewer-1"] }]),
        json!([{ "op": "set_label", "node": "frame-1", "label": "Input" }]),
        json!([{ "op": "move", "positions": { "frame-1": { "x": 80, "y": 0 }, "get_midi_file-1": { "x": 100, "y": 52 }, "viewer-1": { "x": 400, "y": 152 } } }]),
        json!([{ "op": "move", "positions": { "scene_link-1": { "x": 0, "y": 200 } }, "frames": { "scene_link-1": "frame-1" } }]),
        json!([{ "op": "join_in_frame", "nodes": ["frame-1"] }]),
        json!([{ "op": "remove_from_frame", "nodes": ["viewer-1"] }]),
        json!([{ "op": "set_frame", "node": "frame-1", "color": "#aa3333" }]),
        json!([{ "op": "resize", "node": "frame-2", "width": 900, "height": 900, "position": { "x": 0, "y": 0 } }]),
        json!([{ "op": "delete", "nodes": ["frame-1"] }]),
    ];
    let mut states = vec![project.clone()];
    for ops in steps {
        run(&mut graph, None, ops.clone()).unwrap();
        let after = graph.to_rf();
        let diff = Diff::between(&project, &after);
        assert!(!diff.affects_output(), "{} would re-run the graph", ops);
        assert!(history.record(diff, Step::new("edit", Source::Ui)), "{} changed nothing", ops);
        project = after;
        states.push(project.clone());
    }
    // nodes put back by undo come without the UI only fields, not selected and no `selected` are the same
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
}

// a copied frame brings its nodes, label and color, pasted nodes are in the pasted frame. a node copied without its
// frame is pasted in none
#[test]
fn copy_paste_frames() {
    let mut graph = fixture();
    run(&mut graph, None, json!([{ "op": "join_in_frame", "nodes": ["get_midi_file-1", "get_midi_track_data-1"] }, { "op": "set_label", "node": "frame-1", "label": "Drums" }, { "op": "set_frame", "node": "frame-1", "color": "#33aa55" }])).unwrap();
    let text = copy(&graph, None, &["frame-1".to_string()]).unwrap();
    let mut added = Vec::new();
    paste(&mut graph, None, &specs(), &text, &Position::default(), &mut added).unwrap();
    let pasted: BTreeSet<&str> = added.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(pasted, BTreeSet::from(["frame-2", "get_midi_file-2", "get_midi_track_data-2"]));
    let frame = graph.node("frame-2").unwrap();
    assert_eq!(frame.label(), Some("Drums"));
    assert_eq!(frame.data["color"], json!("#33aa55"));
    assert_eq!(frame_of(&graph, "get_midi_file-2"), Some("frame-2"));
    assert_eq!(frame_of(&graph, "get_midi_track_data-2"), Some("frame-2"));
    assert!(graph.edges.iter().any(|e| e.from_node() == "get_midi_file-2" && e.to_node() == "get_midi_track_data-2"));

    let text = copy(&graph, None, &["get_midi_file-1".to_string()]).unwrap();
    let mut added = Vec::new();
    paste(&mut graph, None, &specs(), &text, &Position::default(), &mut added).unwrap();
    assert_eq!(frame_of(&graph, &added[0].id), None);
}

// grouping nodes in a frame puts the group node in it and leaves the frame outside, ungrouping puts the nodes back in
// it. a selected frame goes into the group with what's in it
#[test]
fn groups_and_frames() {
    let mut graph = fixture();
    run(&mut graph, None, json!([{ "op": "join_in_frame", "nodes": ["get_midi_file-1", "get_midi_track_data-1", "viewer-1"] }])).unwrap();
    run(&mut graph, None, json!([{ "op": "group", "nodes": ["get_midi_file-1", "get_midi_track_data-1"] }])).unwrap();
    assert_eq!(frame_of(&graph, "node_group-1"), Some("frame-1"));
    let inside = &graph.groups["node_group"].graph;
    assert!(inside.nodes.iter().all(|n| n.frame().is_none()));

    run(&mut graph, None, json!([{ "op": "ungroup", "nodes": ["node_group-1"] }])).unwrap();
    assert_eq!(frame_of(&graph, "get_midi_file-1"), Some("frame-1"));
    assert_eq!(frame_of(&graph, "get_midi_track_data-1"), Some("frame-1"));

    // the frame and its nodes go in, the group node takes their place (ungrouping dropped the first group, its id is free again)
    run(&mut graph, None, json!([{ "op": "group", "nodes": ["frame-1"] }])).unwrap();
    assert!(graph.node("frame-1").is_none() && graph.node("viewer-1").is_none());
    assert_eq!(frame_of(&graph, "node_group-1"), None);
    let inside = &graph.groups["node_group"].graph;
    assert_eq!(inside.node("viewer-1").unwrap().frame(), Some("frame-1"));

    // frames inside a group are edited there, and come out with their nodes
    run(&mut graph, Some("node_group"), json!([{ "op": "join_in_frame", "nodes": ["viewer-1"] }])).unwrap();
    assert_eq!(graph.groups["node_group"].graph.node("frame-2").unwrap().frame(), Some("frame-1"));
    run(&mut graph, None, json!([{ "op": "ungroup", "nodes": ["node_group-1"] }])).unwrap();
    assert_eq!(frame_of(&graph, "viewer-1"), Some("frame-2"));
    assert_eq!(frame_of(&graph, "frame-2"), Some("frame-1"));
}

// the MCP outline shows a frame and what's in it, and each node's frame
#[test]
fn outline_shows_frames() {
    let mut graph = fixture();
    run(&mut graph, None, json!([{ "op": "join_in_frame", "nodes": ["get_midi_file-1", "viewer-1"] }, { "op": "set_label", "node": "frame-1", "label": "Input" }])).unwrap();
    let specs = specs();
    let groups = all_groups(&graph);
    let empty = HashMap::new();
    let scenes = HashMap::new();
    let ctx = OutlineCtx {
        graph: &graph,
        specs: Specs {
            specs: &specs,
            groups: &groups,
            scope: None,
        },
        results: &empty,
        inputs: &empty,
        scene_data: &scenes,
    };
    let frame = node_block(&ctx, "frame-1", Detail::Concise);
    assert!(frame.contains("label \"Input\"") && frame.contains("holds: viewer-1, get_midi_file-1"), "{}", frame);
    assert!(node_block(&ctx, "viewer-1", Detail::Concise).contains("[in frame frame-1]"));
}
