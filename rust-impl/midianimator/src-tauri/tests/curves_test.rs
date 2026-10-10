use serde_json::json;
use std::collections::{BTreeSet, HashMap};
use MIDIAnimator::blender::curves::{carry_over_curves, curve_sources, merge_curves, prune_curves};
use MIDIAnimator::graph::model::Graph;
use MIDIAnimator::scene_generics::{AnimCurve, Scene};

fn names(list: &[&str]) -> BTreeSet<String> {
    list.iter().map(|name| name.to_string()).collect()
}

/// a curve on `location[0]` with one key at `value`
fn curve(value: f32) -> serde_json::Value {
    json!({ "array_index": 0, "auto_smoothing": "NONE", "data_path": "location", "extrapolation": "CONSTANT", "range": [0.0, 1.0], "keyframe_points": [{ "amplitude": 0.0, "back": 0.0, "easing": "AUTO", "handle_left": [0.0, value], "handle_left_type": "AUTO", "handle_right": [0.0, value], "handle_right_type": "AUTO", "interpolation": "BEZIER", "co": [0.0, value], "period": 0.0 }] })
}

/// a scene named Scene with one group of objects, each with the curves given (`None` for none)
fn scene(objects: &[(&str, Option<f32>)]) -> HashMap<String, Scene> {
    let objects: Vec<_> = objects.iter().map(|(name, value)| json!({ "name": name, "position": {"x": 0.0, "y": 0.0, "z": 0.0}, "rotation": {"x": 0.0, "y": 0.0, "z": 0.0}, "scale": {"x": 1.0, "y": 1.0, "z": 1.0}, "blend_shapes": { "keys": [], "reference": null }, "anim_curves": value.map_or(vec![], |v| vec![curve(v)]) })).collect();
    serde_json::from_value(json!({ "Scene": { "name": "Scene", "object_groups": [{ "name": "Drums", "objects": objects }] } })).unwrap()
}

/// the first key's value of each object with curves
fn values(scenes: &HashMap<String, Scene>) -> Vec<(String, f32)> {
    scenes["Scene"].object_groups[0].objects.iter().filter_map(|object| Some((object.name.clone(), object.anim_curves.first()?.keyframe_points[0].co[1]))).collect()
}

#[test]
fn curve_sources_reads_every_keyframes_from_object_in_the_graph_and_its_groups() {
    let graph: Graph = serde_json::from_value(json!({
        "nodes": [
            { "id": "keyframes_from_object-1", "type": "keyframes_from_object", "data": { "inputs": { "object_name": "bounce" } } },
            { "id": "keyframes_from_object-2", "type": "keyframes_from_object", "data": { "inputs": { "object_name": "" } } },
            { "id": "keyframes_from_object-3", "type": "keyframes_from_object", "data": {} },
            { "id": "scene_link-1", "type": "scene_link", "data": { "inputs": { "object_name": "not a source" } } }
        ],
        "edges": [],
        "groups": {
            "group-1": { "name": "Hits", "nodes": [{ "id": "keyframes_from_object-1", "type": "keyframes_from_object", "data": { "inputs": { "object_name": "hit" } } }], "edges": [] }
        }
    }))
    .unwrap();
    assert_eq!(curve_sources(&graph), names(&["bounce", "hit"]));
}

#[test]
fn merge_curves_sets_the_objects_given() {
    let mut scenes = scene(&[("bounce", Some(1.0)), ("Cube_60", None)]);
    let fetched: HashMap<String, Vec<AnimCurve>> = HashMap::from([("bounce".to_string(), vec![serde_json::from_value(curve(2.0)).unwrap()]), ("missing".to_string(), vec![])]);
    merge_curves(&mut scenes, &fetched);
    assert_eq!(values(&scenes), vec![("bounce".to_string(), 2.0)]);
}

#[test]
fn carry_over_keeps_curves_of_objects_not_watched() {
    let old = scene(&[("bounce", Some(1.0)), ("hit", Some(2.0)), ("Cube_60", None)]);
    // hit is watched, Blender's scene has its curves (here: none left, its keys were deleted)
    let mut new = scene(&[("bounce", None), ("hit", None), ("Cube_60", None)]);
    carry_over_curves(&old, &mut new, &names(&["hit"]));
    assert_eq!(values(&new), vec![("bounce".to_string(), 1.0)]);
}

#[test]
fn carry_over_keeps_the_new_curves_it_has() {
    let old = scene(&[("bounce", Some(1.0))]);
    let mut new = scene(&[("bounce", Some(3.0))]);
    carry_over_curves(&old, &mut new, &BTreeSet::new());
    assert_eq!(values(&new), vec![("bounce".to_string(), 3.0)]);
}

#[test]
fn prune_drops_curves_of_objects_not_read() {
    let mut scenes = scene(&[("bounce", Some(1.0)), ("old_pick", Some(2.0))]);
    prune_curves(&mut scenes, &names(&["bounce"]));
    assert_eq!(values(&scenes), vec![("bounce".to_string(), 1.0)]);
}
