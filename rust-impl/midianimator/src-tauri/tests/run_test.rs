// run from /src-tauri
// cargo test --test run_test
// benchmark: cargo test --release --test run_test -- --ignored --nocapture

use std::collections::{BTreeMap, HashMap};
use std::time::Instant;

use serde_json::{json, Map, Value};
use MIDIAnimator::graph::builtin::{builtin_groups, migrate};
use MIDIAnimator::graph::executors::io::{node_error, NodeFunction, BAD_INPUTS_KEY};
use MIDIAnimator::midi::MIDINote;
use MIDIAnimator::utils::animation::{combine_curve_keys, note_curve_keys, BlendKeyframe, ObjectMap};
use MIDIAnimator::graph::model::{node_specs, Graph, GroupDef, HandleSpec, HandleSpecs, NodeSpec, Position, RfEdge, RfNode};
use MIDIAnimator::graph::run::{run, Memo, Record, RunCtx};
use MIDIAnimator::node_registry::get_node_registry;

/// loads the node specs from default_nodes.json
fn specs() -> Vec<NodeSpec> {
    let data = std::fs::read_to_string("src/configs/default_nodes.json").unwrap();
    let default_nodes: HashMap<String, Value> = serde_json::from_str(&data).unwrap();
    node_specs(&default_nodes)
}

/// a node with values set on it, the type comes from the id (`note_targets-1`)
fn node(id: &str, data: Value) -> RfNode {
    let mut data: Map<String, Value> = data.as_object().cloned().unwrap_or_default();
    data.entry("inputs").or_insert(json!({}));
    RfNode {
        id: id.to_string(),
        node_type: id.rsplit_once('-').unwrap().0.to_string(),
        position: Position::default(),
        data,
        extra: Map::new(),
    }
}

/// a graph from nodes and (from_node, from_output, to_node, to_input) links
fn graph(nodes: Vec<RfNode>, links: &[(&str, &str, &str, &str)]) -> Graph {
    Graph {
        nodes,
        edges: links.iter().map(|(fnode, fout, tnode, tin)| RfEdge::new(fnode, fout, tnode, tin)).collect(),
        ..Default::default()
    }
}

fn group(graph: Graph) -> GroupDef {
    GroupDef {
        name: "Test".to_string(),
        description: String::new(),
        category: String::new(),
        interface: HandleSpecs::default(),
        graph,
    }
}

/// a socket on a group's interface
fn socket(id: &str, data_type: &str) -> HandleSpec {
    HandleSpec {
        id: id.to_string(),
        name: id.to_string(),
        data_type: data_type.to_string(),
        description: String::new(),
        hidden: false,
        multi: false,
        default: None,
    }
}

/// group: input › note targets › output
fn targets_group() -> GroupDef {
    let inner = graph(vec![node("group_input-1", json!({})), node("note_targets-1", json!({})), node("group_output-1", json!({}))], &[("group_input-1", "object_map", "note_targets-1", "object_map"), ("note_targets-1", "targets", "group_output-1", "targets")]);
    let mut def = group(inner);
    def.interface.inputs.push(socket("object_map", "ObjectMap"));
    def.interface.outputs.push(socket("targets", "HashMap<u8, Array<NoteTarget>>"));
    def
}

/// runs a graph with these groups, returns the record and the run's error
fn run_graph(root: &Graph, groups: &BTreeMap<String, GroupDef>, realtime: bool) -> (Record, Option<String>) {
    run_with(root, groups, "", realtime)
}

/// `run` with a group open in the editor
fn run_with(root: &Graph, groups: &BTreeMap<String, GroupDef>, inspect: &str, realtime: bool) -> (Record, Option<String>) {
    let (record, error, _) = run_memo(root, groups, inspect, realtime, Memo::default());
    (record, error)
}

/// `run_with`, starting from what an earlier run left, gives what this run leaves
fn run_memo(root: &Graph, groups: &BTreeMap<String, GroupDef>, inspect: &str, realtime: bool, memo: Memo) -> (Record, Option<String>, Memo) {
    let specs = specs();
    let registry: HashMap<String, NodeFunction> = get_node_registry();
    let mut ctx = RunCtx::new(&specs, &registry, groups, realtime).with_memo(memo);
    ctx.inspect = inspect.to_string();
    let (record, error) = run(&ctx, root);
    (record, error, ctx.into_memo())
}

// MARK: - Test Data

// a linear keyframe point at (time, value)
fn key(time: f64, value: f64) -> Value {
    json!({
        "amplitude": 0.0, "back": 0.0, "easing": "AUTO", "interpolation": "LINEAR", "period": 0.0,
        "handle_left": [time, value], "handle_left_type": "AUTO_CLAMPED",
        "handle_right": [time, value], "handle_right_type": "AUTO_CLAMPED",
        "co": [time, value]
    })
}

fn generator(name: &str, property: &str, overlap: &str, peak: f64) -> Value {
    json!({
        "name": name,
        "note_on_keyframes": [key(-0.05, 0.0), key(0.0, peak), key(0.1, peak * 0.8), key(0.25, 0.0)],
        "note_on_anchor_point": 0.0,
        "note_off_keyframes": [key(0.0, 0.0), key(0.15, -peak * 0.2), key(0.3, 0.0)],
        "note_off_anchor_point": 0.0,
        "time_mapper": "", "amplitude_mapper": "",
        "velocity_intensity": 1.0, "animation_overlap": overlap, "overlap_blend": 0.1, "animation_property": property
    })
}

/// `count` notes. pitches 60-71 repeat every 12 notes and ring longer than that, so notes on the same key overlap
fn notes(count: usize) -> Value {
    let notes: Vec<Value> = (0..count)
        .map(|i| {
            let time_on = i as f64 * 0.05;
            json!({ "channel": 0, "note_number": 60 + (i * 5) % 12, "velocity": 40 + (i * 13) % 88, "time_on": time_on, "time_off": time_on + 0.8 })
        })
        .collect();
    Value::Array(notes)
}

/// one object per key plus a drum kit object two animations share, every overlap mode is used somewhere
fn object_map() -> Value {
    let modes = ["add", "min", "max", "prev", "next", "rvc", "prune", "crossfade"];
    let mut animations = Map::new();
    let mut objects = Map::new();
    for (i, pitch) in (60..72).enumerate() {
        let name = format!("hit_{}", pitch);
        animations.insert(name.clone(), generator(&name, "location[2]", modes[i % modes.len()], 1.0 + i as f64 * 0.1));
        objects.insert(format!("Key {}", pitch), json!({ name: [pitch] }));
    }
    animations.insert("bounce".to_string(), generator("bounce", "location[2]", "crossfade", 2.0));
    animations.insert("spin".to_string(), generator("spin", "rotation_euler[0]", "max", 3.0));
    objects.insert("Drum".to_string(), json!({ "bounce": [60, 62, 64], "spin": [60, 67] }));
    json!({ "animations": animations, "objects": objects })
}

// MARK: - Groups

#[test]
fn group_runs_its_graph_with_the_group_nodes_inputs() {
    let groups = BTreeMap::from([("targets".to_string(), targets_group())]);
    let root = graph(vec![node("group-1", json!({ "group_id": "targets", "inputs": { "object_map": object_map() } })), node("viewer-1", json!({}))], &[("group-1", "targets", "viewer-1", "data")]);

    // closed: the group outputs what reached its output node, nothing inside is recorded
    let (record, error) = run_graph(&root, &groups, true);
    assert_eq!(error, None);
    let targets = &record.results["group-1"]["targets"];
    assert_eq!(targets["67"][0]["object"], "Drum");
    assert_eq!(record.inputs["viewer-1"]["data"], *targets);
    assert!(!record.results.contains_key("group-1/note_targets-1"));

    // open in the editor: inner nodes are recorded under the group's path
    let (record, _) = run_with(&root, &groups, "group-1", true);
    assert_eq!(record.results["group-1/note_targets-1"]["targets"], record.results["group-1"]["targets"]);
}

#[test]
fn a_failed_node_inside_fails_the_group() {
    let groups = BTreeMap::from([("targets".to_string(), targets_group())]);
    // the object uses an animation that isn't in the map
    let bad_map = json!({ "animations": {}, "objects": { "Cube": { "missing": [60] } } });
    let root = graph(vec![node("group-1", json!({ "group_id": "targets", "inputs": { "object_map": bad_map } })), node("viewer-1", json!({}))], &[("group-1", "targets", "viewer-1", "data")]);

    let (record, error) = run_with(&root, &groups, "group-1", true);
    let inner_error = node_error(&record.results["group-1/note_targets-1"]).unwrap();
    assert!(inner_error.contains("'missing'"), "{}", inner_error);
    let group_error = node_error(&record.results["group-1"]).unwrap();
    assert!(group_error.contains("group-1/note_targets-1"), "{}", group_error);
    // the viewer after it doesn't run
    assert!(!record.results.contains_key("viewer-1"));
    assert!(error.unwrap().starts_with("group-1:"));
}

#[test]
fn a_group_cant_contain_itself() {
    let inner = graph(vec![node("group-1", json!({ "group_id": "loop" }))], &[]);
    let groups = BTreeMap::from([("loop".to_string(), group(inner))]);
    let root = graph(vec![node("group-1", json!({ "group_id": "loop" }))], &[]);
    let (record, _) = run_with(&root, &groups, "group-1", true);
    assert!(node_error(&record.results["group-1/group-1"]).unwrap().contains("contains itself"));
}

#[test]
fn a_missing_group_is_an_error() {
    let root = graph(vec![node("group-1", json!({ "group_id": "nope" }))], &[]);
    let (record, _) = run_graph(&root, &BTreeMap::new(), true);
    assert!(node_error(&record.results["group-1"]).unwrap().contains("'nope' doesn't exist"));
}

// MARK: - For Each

/// for each note: its targets, collected
fn targets_per_note() -> Graph {
    graph(vec![node("note_targets-1", json!({ "inputs": { "object_map": object_map() } })), node("for_each_input-1", json!({ "zone": "for_each_output-1", "inputs": { "items": notes(3) } })), node("targets_for_note-1", json!({})), node("for_each_output-1", json!({ "zone": "for_each_input-1" })), node("viewer-1", json!({}))], &[("note_targets-1", "targets", "targets_for_note-1", "targets"), ("for_each_input-1", "element", "targets_for_note-1", "note"), ("targets_for_note-1", "targets", "for_each_output-1", "result"), ("for_each_output-1", "results", "viewer-1", "data")])
}

#[test]
fn for_each_runs_the_zone_once_per_item() {
    let (record, error) = run_graph(&targets_per_note(), &BTreeMap::new(), true);
    assert_eq!(error, None);
    // notes 60, 65, 70: 60 hits its key and the drum twice (bounce, spin), the others just their key
    let results = record.results["for_each_output-1"]["results"].as_array().unwrap();
    let objects: Vec<Vec<&str>> = results.iter().map(|r| r.as_array().unwrap().iter().map(|t| t["object"].as_str().unwrap()).collect()).collect();
    assert_eq!(objects, vec![vec!["Drum", "Drum", "Key 60"], vec!["Key 65"], vec!["Key 70"]]);
    // the node inside is recorded for the first item
    assert_eq!(record.results["targets_for_note-1"]["targets"], results[0]);
    assert_eq!(record.results["for_each_input-1"]["index"], 0);
}

#[test]
fn for_each_with_no_items_is_empty() {
    let mut root = targets_per_note();
    root.node_mut("for_each_input-1").unwrap().inputs_mut().insert("items".to_string(), json!([]));
    let (record, _) = run_graph(&root, &BTreeMap::new(), true);
    assert_eq!(record.results["for_each_output-1"]["results"], json!([]));
}

#[test]
fn a_value_from_inside_a_zone_cant_skip_its_output() {
    let mut root = targets_per_note();
    // viewer-2 reads the zone output and a node inside the zone
    root.nodes.push(node("merge_object_maps-1", json!({})));
    root.edges.push(RfEdge::new("for_each_output-1", "results", "merge_object_maps-1", "object_maps_0"));
    root.edges.push(RfEdge::new("targets_for_note-1", "targets", "merge_object_maps-1", "object_maps_1"));
    let (record, _) = run_graph(&root, &BTreeMap::new(), true);
    // the zone input runs first and shows it, nothing inside runs
    assert!(node_error(&record.results["for_each_input-1"]).unwrap().contains("only the for each output"));
    assert!(!record.results.contains_key("targets_for_note-1"));
}

#[test]
fn a_failed_item_stops_the_loop() {
    let mut root = targets_per_note();
    // the second item isn't a note
    root.node_mut("for_each_input-1").unwrap().inputs_mut().insert("items".to_string(), json!([notes(1)[0], "nope"]));
    let (record, _) = run_graph(&root, &BTreeMap::new(), true);
    let error = node_error(&record.results["for_each_output-1"]).unwrap();
    assert!(error.starts_with("item 1: targets_for_note-1:"), "{}", error);
}

// MARK: - Types

/// what's wrong with each bad input of a failed node
fn bad_inputs(record: &Record, path: &str) -> Value {
    record.results[path].get(BAD_INPUTS_KEY).cloned().unwrap_or(Value::Null)
}

#[test]
fn a_connection_of_the_wrong_type_fails_the_node_it_goes_into() {
    // note targets gives targets, targets for note's note input wants a note
    let root = graph(vec![node("note_targets-1", json!({ "inputs": { "object_map": object_map() } })), node("targets_for_note-1", json!({})), node("viewer-1", json!({}))], &[("note_targets-1", "targets", "targets_for_note-1", "targets"), ("note_targets-1", "targets", "targets_for_note-1", "note"), ("targets_for_note-1", "targets", "viewer-1", "data")]);
    let (record, error) = run_graph(&root, &BTreeMap::new(), true);
    let message = node_error(&record.results["targets_for_note-1"]).unwrap();
    assert_eq!(message, "Note expects MIDINote, but Note Targets › Targets gives HashMap<u8, Array<NoteTarget>>");
    assert!(error.unwrap().starts_with("targets_for_note-1:"));
    // only the bad input is marked, the node didn't run and neither did the one after it
    assert_eq!(bad_inputs(&record, "targets_for_note-1"), json!({ "note": message }));
    assert!(!record.results.contains_key("viewer-1"));
}

#[test]
fn a_connection_of_the_wrong_type_shows_even_when_what_feeds_it_failed() {
    let bad_map = json!({ "animations": {}, "objects": { "Cube": { "missing": [60] } } });
    let root = graph(vec![node("note_targets-1", json!({ "inputs": { "object_map": bad_map } })), node("targets_for_note-1", json!({}))], &[("note_targets-1", "targets", "targets_for_note-1", "note")]);
    let (record, _) = run_graph(&root, &BTreeMap::new(), true);
    assert!(node_error(&record.results["note_targets-1"]).is_some());
    assert!(bad_inputs(&record, "targets_for_note-1")["note"].as_str().unwrap().contains("expects MIDINote"));
}

#[test]
fn connections_are_checked_against_a_groups_sockets() {
    let groups = BTreeMap::from([("targets".to_string(), targets_group())]);
    // the group's object map input gets targets
    let root = graph(vec![node("note_targets-1", json!({ "inputs": { "object_map": object_map() } })), node("group-1", json!({ "group_id": "targets" }))], &[("note_targets-1", "targets", "group-1", "object_map")]);
    let (record, _) = run_graph(&root, &groups, true);
    assert_eq!(node_error(&record.results["group-1"]).unwrap(), "object_map expects ObjectMap, but Note Targets › Targets gives HashMap<u8, Array<NoteTarget>>");
}

#[test]
fn a_dynamic_output_has_its_inner_type() {
    // keyframes from object's curves are Array<Keyframe>
    let root = graph(vec![node("keyframes_from_object-1", json!({})), node("targets_for_note-1", json!({}))], &[("keyframes_from_object-1", "location[2]", "targets_for_note-1", "note")]);
    let (record, _) = run_graph(&root, &BTreeMap::new(), true);
    assert_eq!(bad_inputs(&record, "targets_for_note-1")["note"], "Note expects MIDINote, but Keyframes from Object › location[2] gives Array<Keyframe>");
}

#[test]
fn a_missing_socket_is_left_alone() {
    let root = graph(vec![node("note_targets-1", json!({ "inputs": { "object_map": object_map() } })), node("viewer-1", json!({}))], &[("note_targets-1", "nope", "viewer-1", "data")]);
    let (record, error) = run_graph(&root, &BTreeMap::new(), true);
    assert_eq!(error, None);
    assert_eq!(record.inputs["viewer-1"]["data"], Value::Null);
}

#[test]
fn a_node_missing_an_input_waits_without_an_error() {
    // nothing goes into note targets, the targets for note after it doesn't run either
    let root = graph(vec![node("note_targets-1", json!({})), node("targets_for_note-1", json!({}))], &[("note_targets-1", "targets", "targets_for_note-1", "targets")]);
    let (record, error) = run_graph(&root, &BTreeMap::new(), true);
    assert_eq!(error, None);
    assert!(!record.results.contains_key("note_targets-1"));
    assert!(record.inputs.contains_key("note_targets-1"));
    assert!(!record.inputs.contains_key("targets_for_note-1"));

    // the same from the memo
    let root = graph(vec![node("note_targets-1", json!({ "inputs": { "object_map": object_map() } })), node("targets_for_note-1", json!({}))], &[("note_targets-1", "targets", "targets_for_note-1", "targets")]);
    let (_, _, memo) = run_memo(&root, &BTreeMap::new(), "", true, Memo::default());
    let (record, error, memo) = run_memo(&root, &BTreeMap::new(), "", true, memo);
    assert_eq!(memo.hits(), 1);
    assert_eq!(error, None);
    assert!(!record.results.contains_key("targets_for_note-1"));
}

#[test]
fn a_value_of_the_wrong_type_marks_its_input() {
    // for each elements are Any, so the connection is fine until an item isn't a note
    let mut root = targets_per_note();
    root.node_mut("for_each_input-1").unwrap().inputs_mut().insert("items".to_string(), json!(["nope"]));
    let (record, _) = run_graph(&root, &BTreeMap::new(), true);
    let message = node_error(&record.results["targets_for_note-1"]).unwrap();
    assert!(message.contains("input 'note' has the wrong type"), "{}", message);
    assert_eq!(bad_inputs(&record, "targets_for_note-1"), json!({ "note": message }));
}

#[test]
fn a_bad_connection_into_a_zone_stops_it() {
    // items has to be a list
    let mut root = targets_per_note();
    root.nodes.push(node("note_targets-2", json!({ "inputs": { "object_map": object_map() } })));
    root.edges.push(RfEdge::new("note_targets-2", "targets", "for_each_input-1", "items"));
    let (record, _) = run_graph(&root, &BTreeMap::new(), true);
    assert!(bad_inputs(&record, "for_each_input-1")["items"].as_str().unwrap().contains("expects Array<Any>"));
    assert!(!record.results.contains_key("targets_for_note-1"));
    assert!(!record.results.contains_key("viewer-1"));
}

// MARK: - Memo

#[test]
fn an_unchanged_graph_reruns_from_the_memo() {
    let root = targets_per_note();
    let (first, _, memo) = run_memo(&root, &BTreeMap::new(), "", true, Memo::default());
    assert_eq!(memo.hits(), 0);

    // note targets has nothing connected, so it runs again (it could be reading a file), but it gives the same
    // targets and keeps its old value, so the zone and the viewer after it come from the memo
    let (second, error, memo) = run_memo(&root, &BTreeMap::new(), "", true, memo);
    assert_eq!(error, None);
    assert_eq!(memo.hits(), 2);
    // the record is the same, including what the zone's first item showed
    assert_eq!(second.results, first.results);
    assert_eq!(second.inputs, first.inputs);
}

#[test]
fn a_changed_value_reruns_what_depends_on_it() {
    let mut root = targets_per_note();
    let (_, _, memo) = run_memo(&root, &BTreeMap::new(), "", true, Memo::default());

    // one note less: the zone and the viewer after it run again (note targets always runs, nothing is connected to it)
    root.node_mut("for_each_input-1").unwrap().inputs_mut().insert("items".to_string(), notes(2));
    let (record, _, memo) = run_memo(&root, &BTreeMap::new(), "", true, memo);
    assert_eq!(memo.hits(), 0);
    assert_eq!(record.results["for_each_output-1"]["results"].as_array().unwrap().len(), 2);
}

#[test]
fn nodes_inside_groups_are_memoized_per_group_node() {
    let (root, expected) = group_and_expected(50);
    let (_, _, memo) = run_memo(&root, builtin_groups(), "", true, Memo::default());
    let (record, _, memo) = run_memo(&root, builtin_groups(), "", true, memo);
    // note targets, the outer zone and combine keyframes inside the group
    assert_eq!(memo.hits(), 3);
    assert_eq!(record.results["group-1"]["keyframes"], expected);
}

// MARK: - Evaluate Instrument

/// Evaluate Instrument computed directly with the helpers its nodes use, what the group has to match
fn reference_keyframes(object_map: &ObjectMap, notes: &[MIDINote]) -> HashMap<String, Vec<BlendKeyframe>> {
    let targets = object_map.note_targets().unwrap();
    let mut chunks = Vec::new();
    for note in notes {
        for target in targets.get(&note.note_number).into_iter().flatten() {
            chunks.extend(note_curve_keys(&target.object, object_map.generator(&target.animation, &target.object).unwrap(), note));
        }
    }
    combine_curve_keys(object_map.objects.keys(), chunks).unwrap()
}

/// the built-in group on some notes, and the keyframes it should give
fn group_and_expected(note_count: usize) -> (Graph, Value) {
    let inputs = json!({ "object_map": object_map(), "midi_notes": notes(note_count) });
    let expected = reference_keyframes(&serde_json::from_value(inputs["object_map"].clone()).unwrap(), &serde_json::from_value::<Vec<MIDINote>>(inputs["midi_notes"].clone()).unwrap());
    (graph(vec![node("group-1", json!({ "group_id": "evaluate_instrument", "inputs": inputs }))], &[]), serde_json::to_value(expected).unwrap())
}

#[test]
fn evaluate_instrument_group_matches_reference() {
    let (root, expected) = group_and_expected(200);
    let (record, error) = run_graph(&root, builtin_groups(), true);
    assert_eq!(error, None);
    let expected = expected.as_object().unwrap();
    let grouped = record.results["group-1"]["keyframes"].as_object().unwrap();
    assert_eq!(expected.len(), 13);
    for (object, keys) in expected {
        assert!(!keys.as_array().unwrap().is_empty(), "{} has no keys", object);
        assert_eq!(Some(keys), grouped.get(object), "{}", object);
    }
    assert_eq!(expected.len(), grouped.len());
}

#[test]
fn old_evaluate_instrument_nodes_become_groups() {
    let mut root = graph(vec![node("evaluate_instrument-1", json!({})), node("viewer-1", json!({}))], &[("evaluate_instrument-1", "keyframes", "viewer-1", "data")]);
    assert!(migrate(&mut root));
    let node = root.node("evaluate_instrument-1").unwrap();
    assert_eq!(node.node_type, "group");
    assert_eq!(node.data["group_id"], "evaluate_instrument");
    assert_eq!(root.edges.len(), 1);
    assert!(!migrate(&mut root));
}

/// times the Evaluate Instrument group against computing the same thing directly in Rust, and a rerun from the memo
#[test]
#[ignore]
fn bench_evaluate_instrument_group() {
    let groups = builtin_groups().clone();
    for note_count in [100, 1_000, 5_000, 20_000] {
        let (grouped, _) = group_and_expected(note_count);
        let map: ObjectMap = serde_json::from_value(object_map()).unwrap();
        let notes: Vec<MIDINote> = serde_json::from_value(notes(note_count)).unwrap();

        // best of a few runs, the record is on like in the app
        let best = |f: &dyn Fn() -> f64| (0..5).map(|_| f()).fold(f64::INFINITY, f64::min);
        let direct_ms = best(&|| {
            let start = Instant::now();
            std::hint::black_box(reference_keyframes(&map, &notes));
            start.elapsed().as_secs_f64() * 1000.0
        });
        let group_ms = best(&|| {
            let start = Instant::now();
            let (_, error, memo) = run_memo(&grouped, &groups, "", true, Memo::default());
            let elapsed = start.elapsed().as_secs_f64() * 1000.0;
            assert_eq!(error, None);
            drop(memo);
            elapsed
        });
        let rerun_ms = best(&|| {
            let (_, _, memo) = run_memo(&grouped, &groups, "", true, Memo::default());
            let start = Instant::now();
            let (_, _, memo) = run_memo(&grouped, &groups, "", true, memo);
            let elapsed = start.elapsed().as_secs_f64() * 1000.0;
            drop(memo);
            elapsed
        });
        println!("{:>6} notes: direct {:>8.2} ms, group {:>8.2} ms, group rerun {:>7.2} ms", note_count, direct_ms, group_ms, rerun_ms);
    }
}

#[test]
fn a_realtime_run_skips_the_scene_writer_and_says_so() {
    let root = graph(vec![node("scene_writer-1", json!({ "inputs": { "keyframes": {} } }))], &[]);
    let (record, error) = run_graph(&root, &BTreeMap::new(), true);
    assert!(error.is_none());
    // skipped: no result of its own, the app keeps what its last write gave
    assert_eq!(record.skipped, vec!["scene_writer-1".to_string()]);
    assert!(!record.results.contains_key("scene_writer-1"));
}

#[test]
fn the_scene_writer_fails_when_blender_isnt_connected() {
    let root = graph(vec![node("scene_writer-1", json!({ "inputs": { "keyframes": {} } }))], &[]);
    let (record, error) = run_graph(&root, &BTreeMap::new(), false);
    assert!(record.skipped.is_empty());
    assert_eq!(node_error(&record.results["scene_writer-1"]), Some("Blender isn't connected"));
    assert!(error.unwrap().contains("Blender isn't connected"));
}

#[test]
fn a_scene_writer_inside_a_closed_group_is_still_recorded() {
    let inside = graph(vec![node("scene_writer-1", json!({ "inputs": { "keyframes": {} } }))], &[]);
    let groups = BTreeMap::from([("writes".to_string(), group(inside))]);
    let root = graph(vec![node("group-1", json!({ "group_id": "writes" }))], &[]);

    // realtime: skipped, so the app keeps the last write's result for the group node to show
    let (record, _) = run_graph(&root, &groups, true);
    assert_eq!(record.skipped, vec!["group-1/scene_writer-1".to_string()]);

    // a write: the writer's error is there even though nothing inside the group is open
    let (record, _) = run_graph(&root, &groups, false);
    assert_eq!(node_error(&record.results["group-1/scene_writer-1"]), Some("Blender isn't connected"));
}
