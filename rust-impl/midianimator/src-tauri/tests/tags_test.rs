// run from /src-tauri
// cargo test --test tags_test
//
// signal tags (graph::tags) through the editor's ops. fixture: tests/fixtures/simple_scene_3_executor.mkproj, where
// get_midi_track_data-1 › notes feeds assign_notes_to_objects-1 › midi_notes and evaluate_instrument-1 › midi_notes

use std::collections::{BTreeSet, HashMap};

use serde_json::{json, Value};
use MIDIAnimator::graph::clipboard::copy;
use MIDIAnimator::graph::model::{node_specs, Graph, NodeSpec};
use MIDIAnimator::graph::ops::{apply, Added, Ctx, Op, Side};
use MIDIAnimator::graph::sockets;
use MIDIAnimator::graph::tags::{self, is_tagged};
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

fn tag(graph: &mut Graph, node: &str, side: &str, socket: &str, name: &str) -> Result<Vec<Added>, String> {
    run(graph, None, json!([{ "op": "set_tag", "node": node, "side": side, "socket": socket, "name": name }]))
}

/// every connection as `from.output -> to.input`, `=>` for a tagged one
fn connections(graph: &Graph) -> BTreeSet<String> {
    graph
        .edges
        .iter()
        .map(|e| {
            format!(
                "{}.{} {} {}.{}",
                e.from_node(),
                e.from_output(),
                if is_tagged(e) {
                    "=>"
                } else {
                    "->"
                },
                e.to_node(),
                e.to_input()
            )
        })
        .collect()
}

fn input_tag(graph: &Graph, node: &str, input: &str) -> Option<String> {
    tags::tag(graph.node(node).unwrap(), Side::Inputs, input)
}

fn output_tag(graph: &Graph, node: &str, output: &str) -> Option<String> {
    tags::tag(graph.node(node).unwrap(), Side::Outputs, output)
}

// a new tag on an output turns its wires into tags, removing it turns them back
#[test]
fn output_tag_converts_wires() {
    let mut graph = fixture();
    let before = connections(&graph);
    tag(&mut graph, "get_midi_track_data-1", "outputs", "notes", "notes").unwrap();
    assert!(connections(&graph).contains("get_midi_track_data-1.notes => assign_notes_to_objects-1.midi_notes"));
    assert!(connections(&graph).contains("get_midi_track_data-1.notes => evaluate_instrument-1.midi_notes"));
    assert_eq!(input_tag(&graph, "evaluate_instrument-1", "midi_notes").as_deref(), Some("notes"));
    assert_eq!(graph.edges.len(), before.len());

    tag(&mut graph, "get_midi_track_data-1", "outputs", "notes", "").unwrap();
    assert_eq!(connections(&graph), before);
    assert_eq!(input_tag(&graph, "evaluate_instrument-1", "midi_notes"), None);
    assert_eq!(output_tag(&graph, "get_midi_track_data-1", "notes"), None);
}

// renaming an output's tag renames it on every input using it, one output per name
#[test]
fn rename_follows_and_names_are_unique() {
    let mut graph = fixture();
    tag(&mut graph, "get_midi_track_data-1", "outputs", "notes", "notes").unwrap();
    tag(&mut graph, "get_midi_track_data-1", "outputs", "notes", "song").unwrap();
    assert_eq!(input_tag(&graph, "assign_notes_to_objects-1", "midi_notes").as_deref(), Some("song"));
    assert!(connections(&graph).contains("get_midi_track_data-1.notes => evaluate_instrument-1.midi_notes"));

    let err = tag(&mut graph, "scene_link-1", "outputs", "object_groups", "song").unwrap_err();
    assert!(err.contains("already the tag of get_midi_track_data-1"), "{}", err);
}

// an input tagged with a name no output has is broken, it connects once an output gets the name
#[test]
fn broken_tag_connects_later() {
    let mut graph = fixture();
    run(&mut graph, None, json!([{ "op": "add_nodes", "nodes": [{ "type": "viewer", "position": { "x": 0, "y": 0 } }] }])).unwrap();
    tag(&mut graph, "viewer-2", "inputs", "data", "tracks").unwrap();
    assert!(graph.edge_into("viewer-2", "data").is_none());

    tag(&mut graph, "get_midi_file-1", "outputs", "tracks", "tracks").unwrap();
    assert!(connections(&graph).contains("get_midi_file-1.tracks => viewer-2.data"));
    // the wire it had became a tag too
    assert!(connections(&graph).contains("get_midi_file-1.tracks => get_midi_track_data-1.tracks"));

    // its source gone, the input keeps its tag and is broken again
    run(&mut graph, None, json!([{ "op": "delete", "nodes": ["get_midi_file-1"] }])).unwrap();
    assert!(graph.edge_into("viewer-2", "data").is_none());
    assert_eq!(input_tag(&graph, "viewer-2", "data").as_deref(), Some("tracks"));
}

// tagging a wired input with a new name tags the wire's output, only that wire becomes a tag
#[test]
fn input_tag_takes_over_its_wire() {
    let mut graph = fixture();
    tag(&mut graph, "evaluate_instrument-1", "inputs", "midi_notes", "notes").unwrap();
    assert_eq!(output_tag(&graph, "get_midi_track_data-1", "notes").as_deref(), Some("notes"));
    assert!(connections(&graph).contains("get_midi_track_data-1.notes => evaluate_instrument-1.midi_notes"));
    assert!(connections(&graph).contains("get_midi_track_data-1.notes -> assign_notes_to_objects-1.midi_notes"));

    // removing it leaves the wire
    tag(&mut graph, "evaluate_instrument-1", "inputs", "midi_notes", "").unwrap();
    assert!(connections(&graph).contains("get_midi_track_data-1.notes -> evaluate_instrument-1.midi_notes"));

    // a wire connected to a tagged input takes the tag's place
    tag(&mut graph, "evaluate_instrument-1", "inputs", "midi_notes", "notes").unwrap();
    run(&mut graph, None, json!([{ "op": "connect", "from_node": "get_midi_track_data-1", "from_output": "notes", "to_node": "evaluate_instrument-1", "to_input": "midi_notes" }])).unwrap();
    assert_eq!(input_tag(&graph, "evaluate_instrument-1", "midi_notes"), None);
    assert!(connections(&graph).contains("get_midi_track_data-1.notes -> evaluate_instrument-1.midi_notes"));
}

// deleting a tagged connection takes its input's tag along
#[test]
fn deleting_a_tagged_edge_untags_its_input() {
    let mut graph = fixture();
    tag(&mut graph, "get_midi_track_data-1", "outputs", "notes", "notes").unwrap();
    let edge = graph.edge_into("evaluate_instrument-1", "midi_notes").unwrap().id.clone();
    run(&mut graph, None, json!([{ "op": "delete", "edges": [edge] }])).unwrap();
    assert!(graph.edge_into("evaluate_instrument-1", "midi_notes").is_none());
    assert_eq!(input_tag(&graph, "evaluate_instrument-1", "midi_notes"), None);
}

// a copied input keeps its tag, a copied output gets a new name, the copies' inputs follow it
#[test]
fn duplicates_and_pastes_get_their_own_names() {
    let mut graph = fixture();
    tag(&mut graph, "get_midi_track_data-1", "outputs", "notes", "notes").unwrap();

    let added = run(&mut graph, None, json!([{ "op": "duplicate", "nodes": ["evaluate_instrument-1"], "offset": { "x": 20, "y": 20 } }])).unwrap();
    let duplicate = &added[0].id;
    assert_eq!(input_tag(&graph, duplicate, "midi_notes").as_deref(), Some("notes"));
    assert!(connections(&graph).contains(&format!("get_midi_track_data-1.notes => {}.midi_notes", duplicate)));

    let added = run(&mut graph, None, json!([{ "op": "duplicate", "nodes": ["get_midi_track_data-1", "evaluate_instrument-1"], "offset": { "x": 20, "y": 20 } }])).unwrap();
    let (source, consumer) = (&added[0].id, &added[1].id);
    assert_eq!(output_tag(&graph, source, "notes").as_deref(), Some("notes 2"));
    assert!(connections(&graph).contains(&format!("{}.notes => {}.midi_notes", source, consumer)));

    let text = copy(&graph, None, &["get_midi_track_data-1".to_string(), "assign_notes_to_objects-1".to_string()]).unwrap();
    let added = run(&mut graph, None, json!([{ "op": "paste", "text": text, "position": { "x": 0, "y": 0 } }])).unwrap();
    let pasted = |prefix: &str| added.iter().find(|a| a.id.starts_with(prefix)).unwrap().id.clone();
    let (source, consumer) = (&pasted("get_midi_track_data"), &pasted("assign_notes_to_objects"));
    assert_eq!(output_tag(&graph, source, "notes").as_deref(), Some("notes 3"));
    assert!(connections(&graph).contains(&format!("{}.notes => {}.midi_notes", source, consumer)));
}

// a tag only connects inside one graph, grouping turns the ones crossing into wires through the group's sockets
#[test]
fn grouping_turns_crossing_tags_into_wires() {
    let mut graph = fixture();
    tag(&mut graph, "get_midi_track_data-1", "outputs", "notes", "notes").unwrap();
    run(&mut graph, None, json!([{ "op": "group", "nodes": ["get_midi_file-1", "get_midi_track_data-1"] }])).unwrap();
    assert!(connections(&graph).contains("node_group-1.notes -> evaluate_instrument-1.midi_notes"));
    assert_eq!(input_tag(&graph, "evaluate_instrument-1", "midi_notes"), None);

    // and back out, the tag inside comes along
    run(&mut graph, None, json!([{ "op": "ungroup", "nodes": ["node_group-1"] }])).unwrap();
    assert_eq!(output_tag(&graph, "get_midi_track_data-1", "notes").as_deref(), Some("notes"));
    assert!(connections(&graph).contains("get_midi_track_data-1.notes -> evaluate_instrument-1.midi_notes"));
}

// a tag that would make a cycle stays broken
#[test]
fn no_cycles() {
    let mut graph = fixture();
    tag(&mut graph, "assign_notes_to_objects-1", "outputs", "object_map", "map").unwrap();
    tag(&mut graph, "get_midi_track_data-1", "inputs", "tracks", "map").unwrap();
    assert!(graph.edge_into("get_midi_track_data-1", "tracks").is_none());
    assert_eq!(input_tag(&graph, "get_midi_track_data-1", "tracks").as_deref(), Some("map"));
}

// a socket is selected on its own like an edge, tagged or not, selecting anything else deselects it
#[test]
fn select_sockets() {
    let mut graph = fixture();
    tag(&mut graph, "get_midi_track_data-1", "outputs", "notes", "notes").unwrap();
    run(&mut graph, None, json!([{ "op": "select", "sockets": [{ "node": "evaluate_instrument-1", "side": "inputs", "socket": "midi_notes" }, { "node": "viewer-1", "side": "inputs", "socket": "data" }] }])).unwrap();
    assert_eq!(graph.node("evaluate_instrument-1").unwrap().extra["selectedSockets"], json!({ "inputs": ["midi_notes"], "outputs": [] }));
    assert_eq!(sockets::selected(&graph).len(), 2);
    assert!(graph.nodes.iter().all(|n| n.extra.get("selected") != Some(&json!(true))));

    // a socket losing its tag stays selected
    tag(&mut graph, "evaluate_instrument-1", "inputs", "midi_notes", "").unwrap();
    assert_eq!(sockets::selected(&graph).len(), 2);

    run(&mut graph, None, json!([{ "op": "select", "nodes": ["viewer-1"] }])).unwrap();
    assert!(sockets::selected(&graph).is_empty());
}

// tagging several sockets at once: the inputs share the name, each output gets its own, one undo step
#[test]
fn set_tags() {
    let mut graph = fixture();
    let socket = |node: &str, side: &str, socket: &str| json!({ "node": node, "side": side, "socket": socket });
    run(
        &mut graph,
        None,
        json!([{ "op": "set_tags", "name": "midi", "sockets": [
            socket("get_midi_track_data-1", "outputs", "notes"),
            socket("get_midi_track_data-1", "outputs", "control_change"),
            socket("evaluate_instrument-1", "inputs", "midi_notes"),
            socket("assign_notes_to_objects-1", "inputs", "midi_notes"),
        ] }]),
    )
    .unwrap();
    assert_eq!(output_tag(&graph, "get_midi_track_data-1", "notes").as_deref(), Some("midi"));
    assert_eq!(output_tag(&graph, "get_midi_track_data-1", "control_change").as_deref(), Some("midi 2"));
    assert_eq!(input_tag(&graph, "evaluate_instrument-1", "midi_notes").as_deref(), Some("midi"));
    assert!(connections(&graph).contains("get_midi_track_data-1.notes => evaluate_instrument-1.midi_notes"));
    assert!(connections(&graph).contains("get_midi_track_data-1.notes => assign_notes_to_objects-1.midi_notes"));

    // an output can take the name another of them gives up
    run(&mut graph, None, json!([{ "op": "set_tags", "name": "midi", "sockets": [socket("get_midi_track_data-1", "outputs", "control_change"), socket("get_midi_track_data-1", "outputs", "notes")] }])).unwrap();
    assert_eq!(output_tag(&graph, "get_midi_track_data-1", "control_change").as_deref(), Some("midi"));
    assert_eq!(output_tag(&graph, "get_midi_track_data-1", "notes").as_deref(), Some("midi 2"));
    // the inputs followed their output's rename
    assert!(connections(&graph).contains("get_midi_track_data-1.notes => evaluate_instrument-1.midi_notes"));

    // an empty name removes them all
    run(&mut graph, None, json!([{ "op": "set_tags", "name": "", "sockets": [socket("get_midi_track_data-1", "outputs", "control_change"), socket("get_midi_track_data-1", "outputs", "notes")] }])).unwrap();
    assert_eq!(output_tag(&graph, "get_midi_track_data-1", "notes"), None);
    assert!(connections(&graph).contains("get_midi_track_data-1.notes -> evaluate_instrument-1.midi_notes"));
}

// deleting an input's tag takes its connection along, an output's leaves its inputs broken
#[test]
fn delete_tags() {
    let mut graph = fixture();
    tag(&mut graph, "get_midi_track_data-1", "outputs", "notes", "notes").unwrap();
    run(&mut graph, None, json!([{ "op": "delete", "sockets": [{ "node": "evaluate_instrument-1", "side": "inputs", "socket": "midi_notes" }] }])).unwrap();
    assert!(graph.edge_into("evaluate_instrument-1", "midi_notes").is_none());
    assert_eq!(input_tag(&graph, "evaluate_instrument-1", "midi_notes"), None);
    assert!(connections(&graph).contains("get_midi_track_data-1.notes => assign_notes_to_objects-1.midi_notes"));

    run(&mut graph, None, json!([{ "op": "delete", "sockets": [{ "node": "get_midi_track_data-1", "side": "outputs", "socket": "notes" }] }])).unwrap();
    assert_eq!(output_tag(&graph, "get_midi_track_data-1", "notes"), None);
    assert!(graph.edge_into("assign_notes_to_objects-1", "midi_notes").is_none());
    assert_eq!(input_tag(&graph, "assign_notes_to_objects-1", "midi_notes").as_deref(), Some("notes"));
}
