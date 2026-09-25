// run from /src-tauri
// cargo test --test graph_test
//
// fixture: tests/fixtures/simple_scene_3_executor.mkproj, saved from the app, with short node ids

use std::collections::HashMap;

use serde_json::{json, Map, Value};
use MIDIAnimator::graph::edit;
use MIDIAnimator::graph::model::{node_specs, Graph, NodeSpec, Position};
use MIDIAnimator::graph::outline::{node_block, outline, summarize, unset_inputs, Detail, OutlineCtx};
use MIDIAnimator::midi::MIDIFile;
use MIDIAnimator::scene_generics::Scene;
use MIDIAnimator::state::SavedProject;

// node ids from the fixture, used where a test needs the exact id
const TRACK_DATA: &str = "get_midi_track_data-1";
const MIDI_FILE: &str = "get_midi_file-1";
const VIEWER: &str = "viewer-1";
const EVALUATE: &str = "evaluate_instrument-1";

/// loads the node specs from default_nodes.json
fn specs() -> Vec<NodeSpec> {
    let data = std::fs::read_to_string("src/configs/default_nodes.json").unwrap();
    let default_nodes: HashMap<String, Value> = serde_json::from_str(&data).unwrap();
    node_specs(&default_nodes)
}

/// loads the fixture project, returns its graph and scene data
fn fixture() -> (Graph, HashMap<String, Scene>) {
    let data = std::fs::read_to_string("tests/fixtures/simple_scene_3_executor.mkproj").unwrap();
    let project: SavedProject = serde_json::from_str(&data).unwrap();
    (Graph::from_rf(&project.rf_instance).unwrap(), project.scene_data)
}

/// the fixture graph plus everything an `OutlineCtx` needs, results and inputs start out empty
struct Fixture {
    graph: Graph,
    specs: Vec<NodeSpec>,
    results: HashMap<String, Value>,
    inputs: HashMap<String, Value>,
    scene: HashMap<String, Scene>,
}

impl Fixture {
    fn new() -> Self {
        let (graph, scene) = fixture();
        Self {
            graph,
            specs: specs(),
            results: HashMap::new(),
            inputs: HashMap::new(),
            scene,
        }
    }

    fn ctx(&self) -> OutlineCtx<'_> {
        OutlineCtx {
            graph: &self.graph,
            specs: &self.specs,
            results: &self.results,
            inputs: &self.inputs,
            scene_data: &self.scene,
        }
    }
}

/// shortcut to turn a `json!` object into a `Map`
fn obj(value: Value) -> Map<String, Value> {
    value.as_object().unwrap().clone()
}

// checks that every node handle has a description and the types are what we expect
#[test]
fn specs_have_handle_descriptions() {
    let specs = specs();
    // every input and output should have a description (the ML model reads these)
    for spec in &specs {
        for handle in spec.handles.inputs.iter().chain(spec.handles.outputs.iter()) {
            assert!(!handle.description.is_empty(), "{}.{} has no description", spec.id, handle.id);
        }
    }
    // spot check one handle type
    let assign = specs.iter().find(|s| s.id == "assign_notes_to_objects").unwrap();
    assert_eq!(assign.input("object_groups").unwrap().data_type, "Array<ObjectGroup>");
}

// checks that reading the graph and writing it back gives the exact same JSON
#[test]
fn round_trip_keeps_unknown_fields() {
    // load the project and go through `Graph` and back
    let data = std::fs::read_to_string("tests/fixtures/simple_scene_3_executor.mkproj").unwrap();
    let project: SavedProject = serde_json::from_str(&data).unwrap();
    let graph = Graph::from_rf(&project.rf_instance).unwrap();
    let back = graph.to_rf();
    assert_eq!(serde_json::to_value(&back).unwrap(), serde_json::to_value(&project.rf_instance).unwrap());
}

// checks that node ids can be looked up by a unique prefix
#[test]
fn resolves_prefixes() {
    let f = Fixture::new();
    // unique prefix and exact id both work
    assert_eq!(f.graph.resolve("get_midi_track").unwrap(), TRACK_DATA);
    assert_eq!(f.graph.resolve(MIDI_FILE).unwrap(), MIDI_FILE);
    // an ambiguous prefix or a missing node gives a helpful error
    assert!(f.graph.resolve("get_midi").unwrap_err().contains("matches several nodes"));
    assert!(f.graph.resolve("nope").unwrap_err().contains("nodes are"));
}

// checks that edge accessors follow the endpoints, not the (stale) edge id
#[test]
fn edges_use_data_flow_direction_despite_stale_id() {
    let mut f = Fixture::new();
    // give the viewer edge a stale id that names a different connection, like react flow can leave behind
    let index = f.graph.edges.iter().position(|e| e.source == VIEWER).unwrap();
    f.graph.edges[index].id = "xy-edge__viewer-1data-get_midi_track_data-1notes".to_string();
    // the endpoints are what count, not the id
    let edge = f.graph.edge_into(VIEWER, "data").unwrap();
    assert_eq!(edge.from_node(), EVALUATE);
    assert_eq!(edge.from_output(), "keyframes");
}

// checks that producers come before the nodes they feed
#[test]
fn topo_order_puts_producers_first() {
    let f = Fixture::new();
    let order = f.graph.topo_order();
    // index of the first node in the order whose id starts with `id`
    let pos = |id: &str| order.iter().position(|o| o.starts_with(id)).unwrap();
    assert_eq!(order.len(), f.graph.nodes.len());
    assert!(pos("get_midi_file") < pos("get_midi_track_data"));
    assert!(pos("get_midi_track_data") < pos("assign_notes_to_objects"));
    assert!(pos("scene_link") < pos("keyframes_from_object"));
    assert!(pos("assign_notes_to_objects") < pos("evaluate_instrument"));
    assert!(pos("evaluate_instrument") < pos("viewer"));
}

// checks that new nodes get short ids and are placed to the right of the graph (or of `after`)
#[test]
fn add_node_gets_short_id_and_placement() {
    let mut f = Fixture::new();
    // add one with an input set, then a second one placed after the first
    let result = edit::add_node(&mut f.graph, &f.specs, "get_midi_file", Some(&obj(json!({"file_path": "/tmp/a.mid"}))), None, None).unwrap();
    assert_eq!(result.touched, vec!["get_midi_file-2"]);
    let second = edit::add_node(&mut f.graph, &f.specs, "get_midi_file", None, None, Some("get_midi_file-2")).unwrap();
    assert_eq!(second.touched, vec!["get_midi_file-3"]);

    // the first node goes to the right of the rightmost existing node
    let first = f.graph.node("get_midi_file-2").unwrap();
    let rightmost = f.graph.nodes.iter().filter(|n| n.id != "get_midi_file-2").map(|n| n.position.x).fold(f64::MIN, f64::max);
    assert!(first.position.x > rightmost - 350.0 - 1.0);
    assert_eq!(first.input_value("file_path"), Some(&json!("/tmp/a.mid")));
    // the second node goes right next to the first
    let second = f.graph.node("get_midi_file-3").unwrap();
    assert_eq!(
        second.position,
        Position {
            x: first.position.x + 350.0,
            y: first.position.y
        }
    );

    // unknown node types and unknown inputs are refused
    assert!(edit::add_node(&mut f.graph, &f.specs, "midi", None, None, None).unwrap_err().contains("node_types_list"));
    assert!(edit::add_node(&mut f.graph, &f.specs, "get_midi_file", Some(&obj(json!({"path": "x"}))), None, None).unwrap_err().contains("no input 'path'"));
}

// checks every way connect can be refused, and that connecting to a connected input replaces the edge
#[test]
fn connect_validates_and_replaces() {
    let mut f = Fixture::new();
    // add a new midi file node to connect from
    edit::add_node(&mut f.graph, &f.specs, "get_midi_file", None, None, None).unwrap();
    let results = HashMap::new();

    // wrong types
    let err = edit::connect(&mut f.graph, &f.specs, &results, "get_midi_file-2", "tracks", "assign", "midi_notes").unwrap_err();
    assert!(err.contains("type mismatch"), "{}", err);
    // unknown handles
    assert!(edit::connect(&mut f.graph, &f.specs, &results, "get_midi_file-2", "notes", TRACK_DATA, "tracks").unwrap_err().contains("outputs are"));
    assert!(edit::connect(&mut f.graph, &f.specs, &results, "get_midi_file-2", "tracks", TRACK_DATA, "trax").unwrap_err().contains("inputs are"));
    // hidden inputs (parameters) are not connectable
    let err = edit::connect(&mut f.graph, &f.specs, &results, "scene_link", "name", "get_midi_track", "track_name").unwrap_err();
    assert!(err.contains("must not be connected") && err.contains("graph_set_inputs"), "{}", err);
    // hidden outputs are display-only and not connectable either
    let err = edit::connect(&mut f.graph, &f.specs, &results, "get_midi_file-2", "stats", "viewer", "data").unwrap_err();
    assert!(err.contains("must not be connected"), "{}", err);
    // connecting a node to itself, or creating a cycle
    assert!(edit::connect(&mut f.graph, &f.specs, &results, "evaluate", "keyframes", "evaluate", "object_map").unwrap_err().contains("itself"));
    let err = edit::connect(&mut f.graph, &f.specs, &results, "viewer", "data", "evaluate", "midi_notes");
    assert!(err.is_err());

    // replace the existing tracks connection, the edge count shouldn't change
    let edges_before = f.graph.edges.len();
    let result = edit::connect(&mut f.graph, &f.specs, &results, "get_midi_file-2", "tracks", "get_midi_track", "tracks").unwrap();
    assert!(result.message.contains("replaced"), "{}", result.message);
    assert_eq!(f.graph.edges.len(), edges_before);
    // the new edge is stored in the reversed source/target direction
    let edge = f.graph.edge_into(TRACK_DATA, "tracks").unwrap();
    assert_eq!(edge.from_node(), "get_midi_file-2");
    assert_eq!(edge.source, TRACK_DATA);
    assert_eq!(edge.target_handle.as_deref(), Some("tracks"));
    assert_eq!(edge.id, format!("xy-edge__{}tracks-get_midi_file-2tracks", TRACK_DATA));
}

// checks connecting to a `Dyn<T>` output only works once the node has results
#[test]
fn connect_to_dynamic_output() {
    let mut f = Fixture::new();
    // keyframes_from_object and animation_generator from the fixture
    let kfo = "keyframes_from_object-1";
    let gen = "animation_generator-1";
    // free up the input first
    edit::disconnect(&mut f.graph, gen, "note_on_keyframes").unwrap();

    // without results the dynamic output is unknown
    let err = edit::connect(&mut f.graph, &f.specs, &HashMap::new(), kfo, "location_z", gen, "note_on_keyframes").unwrap_err();
    assert!(err.contains("Dynamic outputs appear"), "{}", err);

    // fake the executed results so the dynamic output `location_z` exists
    let mut results = HashMap::new();
    results.insert(kfo.to_string(), json!({"dyn_output": {"location_z": {"data_path": "location", "array_index": 2, "keyframe_points": []}}}));
    // the hidden container output is refused and points at the dynamic outputs
    let err = edit::connect(&mut f.graph, &f.specs, &results, kfo, "dyn_output", gen, "note_on_keyframes").unwrap_err();
    assert!(err.contains("must not be connected") && err.contains("location_z"), "{}", err);
    // now connecting the dynamic output works
    edit::connect(&mut f.graph, &f.specs, &results, kfo, "location_z", gen, "note_on_keyframes").unwrap();
}

// checks disconnect, set_inputs (including unsetting with null) and remove_node
#[test]
fn disconnect_set_inputs_and_remove() {
    let mut f = Fixture::new();
    // disconnecting an input that isn't connected is an error
    assert!(edit::disconnect(&mut f.graph, "get_midi_file", "file_path").unwrap_err().contains("not connected"));
    // disconnect the viewer, the message names the node it was connected to
    let result = edit::disconnect(&mut f.graph, "viewer", "data").unwrap();
    assert!(result.message.contains(EVALUATE));
    assert!(f.graph.edge_into(VIEWER, "data").is_none());

    // set a value, then unset it with null
    edit::set_inputs(&mut f.graph, &f.specs, "get_midi_track", &obj(json!({"track_name": "Drums"}))).unwrap();
    assert_eq!(f.graph.node(TRACK_DATA).unwrap().input_value("track_name"), Some(&json!("Drums")));
    edit::set_inputs(&mut f.graph, &f.specs, "get_midi_track", &obj(json!({"track_name": null}))).unwrap();
    assert!(f.graph.node(TRACK_DATA).unwrap().input_value("track_name").is_none());
    // wrong type for an f64 input
    assert!(edit::set_inputs(&mut f.graph, &f.specs, "animation_generator", &obj(json!({"velocity_intensity": "loud"}))).unwrap_err().contains("expects f64"));
    // setting a connected input works, but the message says it's ignored
    let result = edit::set_inputs(&mut f.graph, &f.specs, "get_midi_track", &obj(json!({"tracks": []}))).unwrap();
    assert!(result.message.contains("ignored while connected"));

    // remove a node, its edges go with it and its neighbours are returned
    let nodes_before = f.graph.nodes.len();
    let result = edit::remove_node(&mut f.graph, "get_midi_track").unwrap();
    assert_eq!(f.graph.nodes.len(), nodes_before - 1);
    assert!(f.graph.edges.iter().all(|e| e.from_node() != TRACK_DATA && e.to_node() != TRACK_DATA));
    assert!(result.touched.contains(&MIDI_FILE.to_string()));
    assert!(result.message.contains("3 edges"), "{}", result.message);
}

// checks the outline shows labels, values, connections and options for inputs
#[test]
fn outline_shows_labels_values_connections_and_options() {
    // fake the executed inputs/results for the track data node using a real midi file
    let mut f = Fixture::new();
    let midi = MIDIFile::new("tests/fixtures/piano_seq_test.mid").unwrap();
    let tracks = serde_json::to_value(midi.get_midi_tracks()).unwrap();
    let track_name = tracks[0]["name"].as_str().unwrap().to_string();
    let notes = tracks[0]["notes"].clone();
    f.inputs.insert(TRACK_DATA.to_string(), json!({ "tracks": tracks, "track_name": track_name }));
    f.results.insert(TRACK_DATA.to_string(), json!({ "notes": notes }));

    // concise block for the track data node
    let block = node_block(&f.ctx(), TRACK_DATA, Detail::Concise);
    println!("{}", block);
    assert!(block.starts_with(&format!("{}  \"Get MIDI Track Data\"", TRACK_DATA)));
    assert!(block.contains(&format!("in   Tracks  (tracks: Array<MIDITrack>)  <- {} › Tracks", MIDI_FILE)));
    assert!(block.contains("par  Track Name  (track_name: String) = \"Studio Grand\""));
    assert!(block.contains(&format!("options: {}", track_name)));
    assert!(block.contains("-> assign_notes_to_objects-1 › Notes"));
    assert!(block.contains(" notes · "), "{}", block);

    // full outline has descriptions and options from the scene data
    let full = outline(&f.ctx(), None, Detail::Full).unwrap();
    assert!(full.contains("# Blender data path like location[2]"));
    assert!(full.contains("options: Cubes"));
    assert!(full.contains("options: Cube.001, Cube.002"));
    // concise outline lists unset inputs compactly and shows dynamic outputs that haven't executed yet
    let concise = outline(&f.ctx(), None, Detail::Concise).unwrap();
    println!("{}", concise);
    assert!(concise.contains("unset: note_on_anchor_point"));
    assert!(concise.contains("out  location_z  (dynamic, not executed yet)  -> animation_generator-"));

    // scene_link feeds everything except the MIDI file nodes
    let scoped = outline(&f.ctx(), Some("scene_link"), Detail::Concise).unwrap();
    assert!(scoped.contains("\"Viewer\""));
    assert!(!scoped.contains("\"Get MIDI File\""));

    // viewer's only input is connected, so nothing is unset
    assert_eq!(unset_inputs(&f.ctx(), VIEWER), Vec::<String>::new());
}

// checks the one-line summaries for each value type
#[test]
fn summaries() {
    // notes: count, note range and time range
    let notes = json!([
        {"channel": 0, "note_number": 36, "velocity": 100, "time_on": 0.0, "time_off": 0.5},
        {"channel": 0, "note_number": 91, "velocity": 100, "time_on": 80.0, "time_off": 83.25}
    ]);
    assert_eq!(summarize("Array<MIDINote>", &notes), "2 notes · C2–G6 · 0.0–83.2 s");
    assert_eq!(summarize("Array<MIDINote>", &json!([])), "0 notes");

    // tracks: names and note counts
    let tracks = json!([{"name": "Studio Grand", "notes": [1, 2, 3]}, {"name": "Drums", "notes": []}]);
    assert_eq!(summarize("Array<MIDITrack>", &tracks), "2 tracks: Studio Grand (3 notes), Drums (0 notes)");

    // keyframes: data path, count and frame range
    let curve = json!({"data_path": "location", "array_index": 2, "keyframe_points": [{"co": [0.0, 0.0]}, {"co": [10.0, 1.0]}]});
    assert_eq!(summarize("Array<Keyframe>", &curve), "location[2] · 2 keyframes · frames 0–10");

    // object groups and strings
    let groups = json!([{"name": "Cubes", "objects": [{}, {}]}]);
    assert_eq!(summarize("Array<ObjectGroup>", &groups), "1 group: Cubes (2 objects)");
    assert_eq!(summarize("String", &json!("a\nb")), "\"a\\nb\"");

    // generic fallback for arrays and objects
    assert_eq!(summarize("Array<MIDIEvent>", &json!([1, 2, 3, 4])), "4 items: [1, 2, 3, …]");
    assert_eq!(summarize("ObjectMap", &json!({"animations": {}, "objects": {}})), "{animations, objects} (2 keys)");
    // long strings get truncated to 60 characters
    let long = "x".repeat(100);
    assert_eq!(summarize("String", &json!(long)), format!("\"{}…\"", "x".repeat(60)));
}
