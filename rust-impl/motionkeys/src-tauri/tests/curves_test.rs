use serde_json::json;
use std::collections::{BTreeSet, HashMap};
use motionkeys::bridge::curves::{carry_over_curves, curve_sources, merge_curves, prune_curves};
use motionkeys::graph::curves::node_curves;
use motionkeys::graph::model::Graph;
use motionkeys::scene_generics::{AnimCurve, Scene};

fn names(list: &[&str]) -> BTreeSet<String> {
    list.iter().map(|name| name.to_string()).collect()
}

/// a curve on `location[0]` with one key at `value`
fn curve(value: f32) -> serde_json::Value {
    json!({ "channel": "location[0]", "keys": [[0.0, value, "linear"]] })
}

/// a scene named Scene with one group of objects, each with the curves given (`None` for none)
fn scene(objects: &[(&str, Option<f32>)]) -> Scene {
    let objects: Vec<_> = objects.iter().map(|(name, value)| json!({ "id": name, "name": name, "channels": [], "anim_curves": value.map_or(vec![], |v| vec![curve(v)]) })).collect();
    serde_json::from_value(json!({ "name": "Scene", "object_groups": [{ "name": "Drums", "objects": objects }] })).unwrap()
}

/// the first key's value of each object with curves
fn values(scene: &Scene) -> Vec<(String, f32)> {
    scene.object_groups[0].objects.iter().filter_map(|object| Some((object.name.clone(), object.anim_curves.first()?.keys[0].value as f32))).collect()
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
    assert_eq!(curve_sources(&graph, None), names(&["bounce", "hit"]));

    // a pick typed as a name is that object's id, one the scene doesn't have stays as it is
    let scene: Scene = serde_json::from_value(json!({ "name": "Scene", "object_groups": [{ "name": "Drums", "objects": [{ "id": "obj-7", "name": "bounce" }] }] })).unwrap();
    assert_eq!(curve_sources(&graph, Some(&scene)), names(&["obj-7", "hit"]));
}

#[test]
fn merge_curves_sets_the_objects_given() {
    let mut tab_scene = Some(scene(&[("bounce", Some(1.0)), ("Cube_60", None)]));
    let fetched: HashMap<String, Vec<AnimCurve>> = HashMap::from([("bounce".to_string(), vec![serde_json::from_value(curve(2.0)).unwrap()]), ("missing".to_string(), vec![])]);
    merge_curves(&mut tab_scene, &fetched);
    assert_eq!(values(tab_scene.as_ref().unwrap()), vec![("bounce".to_string(), 2.0)]);
}

#[test]
fn carry_over_keeps_the_curves_of_every_object() {
    let old = Some(scene(&[("bounce", Some(1.0)), ("hit", Some(2.0)), ("Cube_60", None)]));
    // the bridge's scene has no curves, watched objects' too
    let mut new = scene(&[("bounce", None), ("hit", None), ("Cube_60", None)]);
    carry_over_curves(&old, &mut new);
    assert_eq!(values(&new), vec![("bounce".to_string(), 1.0), ("hit".to_string(), 2.0)]);
}

#[test]
fn carry_over_keeps_the_new_curves_it_has() {
    let old = Some(scene(&[("bounce", Some(1.0))]));
    let mut new = scene(&[("bounce", Some(3.0))]);
    carry_over_curves(&old, &mut new);
    assert_eq!(values(&new), vec![("bounce".to_string(), 3.0)]);
}

#[test]
fn prune_drops_curves_of_objects_not_read() {
    let mut tab_scene = Some(scene(&[("bounce", Some(1.0)), ("old_pick", Some(2.0))]));
    prune_curves(&mut tab_scene, &names(&["bounce"]));
    assert_eq!(values(tab_scene.as_ref().unwrap()), vec![("bounce".to_string(), 1.0)]);
}

/// a scene with a cube (its own channels, then a bone's) and a lamp that lists `location[2]` differently
fn named_scene() -> Scene {
    let cube = json!({ "id": "cube-id", "name": "Cube", "rotation_order": "ZXY", "channels": [
        { "id": "location[2]", "name": "Location", "group": "Object", "axis": "Z", "semantic": "position.z" },
        { "id": "pose.bones[\"Arm\"].location[0]", "name": "Location", "group": "Arm", "axis": "X" },
    ] });
    let lamp = json!({ "id": "lamp-id", "name": "Lamp", "channels": [{ "id": "location[2]", "name": "Height", "group": "Light" }, { "id": "energy", "name": "Power", "group": "Light" }] });
    serde_json::from_value(json!({ "id": "Scene", "name": "Scene", "object_groups": [{ "id": "collection:Drums", "name": "Drums", "objects": [cube, lamp] }] })).unwrap()
}

/// each curve's group, name, axis, semantic tag and rotation order
fn described(outputs: serde_json::Value) -> Vec<[Option<String>; 5]> {
    let channels = serde_json::to_value(node_curves(Some(&named_scene()), Some(&outputs), None)).unwrap();
    channels.as_array().unwrap().iter().map(|c| ["group", "name", "axis", "semantic", "rotation_order"].map(|field| c[field].as_str().map(str::to_string))).collect()
}

fn row(fields: [Option<&str>; 5]) -> [Option<String>; 5] {
    fields.map(|field| field.map(str::to_string))
}

#[test]
fn graph_curves_are_named_by_the_channels_of_the_object_they_go_to() {
    let key = |channel: &str| json!({ "channel": channel, "key": [0.0, 1.0, "linear", 0.0, 1.0, 0.0, 1.0] });
    let written = json!({ "keys": { "cube-id": [key("location[2]"), key("pose.bones[\"Arm\"].location[0]")], "lamp-id": [key("location[2]"), key("unlisted")] } });
    assert_eq!(
        described(written),
        vec![
            row([Some("Cube"), Some("Location Z"), Some("Z"), Some("position.z"), Some("ZXY")]),
            row([Some("Cube"), Some("Arm › Location X"), Some("X"), None, Some("ZXY")]),
            // the lamp's own group is Light, its first channel's
            row([Some("Lamp"), Some("Height"), None, None, None]),
            row([Some("Lamp"), Some("unlisted"), None, None, None]),
        ]
    );
}

#[test]
fn graph_curves_without_an_object_take_the_first_object_with_the_channel() {
    let outputs = json!({ "curve": { "channel": "energy", "keys": [[0.0, 1.0, "linear"]] }, "other": { "channel": "location[2]", "keys": [[0.0, 1.0, "linear"]] } });
    // in the order of the node's outputs
    assert_eq!(described(outputs), vec![row([Some(""), Some("Power"), None, None, None]), row([Some(""), Some("Location Z"), Some("Z"), Some("position.z"), Some("ZXY")])]);
}
