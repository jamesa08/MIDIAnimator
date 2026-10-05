use serde_json::json;
use MIDIAnimator::graph::executors::animation::{animation_generator, assign_notes_to_objects, combine_keyframes, keyframes_from_object, merge_object_maps, note_keyframes, note_targets, pad_nums, targets_for_note};
use MIDIAnimator::graph::executors::io::Inputs;
use MIDIAnimator::graph::executors::midi::{get_midi_file, get_midi_track_data};
use MIDIAnimator::utils::animation::parse_animation_property;

const TYPE_1: &str = "./tests/test_midi_type_1_rs_4_14_24.mid";

#[test]
fn get_midi_file_reads_string_path() {
    // a JSON string path loads without its quotes
    let outputs = get_midi_file(&Inputs::from([("file_path", json!(TYPE_1))])).unwrap().to_json();
    assert!(!outputs["tracks"].as_array().unwrap().is_empty());
    assert!(outputs["stats"].as_str().unwrap().contains("tracks"));
}

#[test]
fn get_midi_file_empty_without_path() {
    for inputs in [Inputs::default(), Inputs::from([("file_path", json!(""))])] {
        let outputs = get_midi_file(&inputs).unwrap().to_json();
        assert_eq!(outputs["tracks"], json!([]));
        assert_eq!(outputs["stats"], json!(""));
    }
}

#[test]
fn get_midi_file_errors_on_missing_file() {
    let error = get_midi_file(&Inputs::from([("file_path", json!("/nope/missing.mid"))])).unwrap_err();
    assert!(error.contains("could not read MIDI file '/nope/missing.mid'"), "{}", error);
}

#[test]
fn get_midi_file_errors_on_wrong_type() {
    let error = get_midi_file(&Inputs::from([("file_path", json!(42))])).unwrap_err();
    assert!(error.contains("input 'file_path' has the wrong type"), "{}", error);
}

#[test]
fn get_midi_track_data_empty_outputs_without_inputs() {
    let outputs = get_midi_track_data(&Inputs::default()).unwrap().to_json();
    assert!(!outputs.get("track").is_some());
    assert_eq!(outputs["notes"], json!([]));
    assert_eq!(outputs["control_change"], json!({}));
    assert_eq!(outputs["pitchwheel"], json!([]));
    assert_eq!(outputs["aftertouch"], json!([]));
}

#[test]
fn get_midi_track_data_finds_track() {
    let tracks = get_midi_file(&Inputs::from([("file_path", json!(TYPE_1))])).unwrap().to_json()["tracks"].clone();
    let name = tracks[0]["name"].clone();
    let outputs = get_midi_track_data(&Inputs::from([("tracks", tracks.clone()), ("track_name", name)])).unwrap().to_json();
    assert_eq!(outputs["notes"], tracks[0]["notes"]);

    // unknown track name is an error that lists the tracks
    let error = get_midi_track_data(&Inputs::from([("tracks", tracks.clone()), ("track_name", json!("nope"))])).unwrap_err();
    assert!(error.contains("track 'nope' not found"), "{}", error);
    assert!(error.contains(tracks[0]["name"].as_str().unwrap()), "{}", error);
}

#[test]
fn get_midi_track_data_errors_on_wrong_type() {
    let error = get_midi_track_data(&Inputs::from([("tracks", json!("not tracks")), ("track_name", json!("a"))])).unwrap_err();
    assert!(error.contains("input 'tracks' has the wrong type"), "{}", error);
}

#[test]
fn assign_notes_errors_on_missing_group() {
    let groups = json!([{ "name": "Cubes", "objects": [] }]);
    let error = assign_notes_to_objects(&Inputs::from([("object_groups", groups), ("object_group_name", json!("Spheres"))])).unwrap_err();
    assert!(error.contains("object group 'Spheres' does not exist"), "{}", error);

    // nothing picked yet is not an error
    let outputs = assign_notes_to_objects(&Inputs::default()).unwrap().to_json();
    assert_eq!(outputs["object_map"], json!({ "animations": {}, "objects": {} }));
}

/// an object group with one object animated on the given (data_path, array_index) curves
fn animated_object(curves: &[(&str, u32)]) -> serde_json::Value {
    let curves: Vec<_> = curves.iter().map(|(data_path, array_index)| json!({ "array_index": array_index, "auto_smoothing": "NONE", "data_path": data_path, "extrapolation": "CONSTANT", "keyframe_points": [], "range": [0.0, 1.0] })).collect();
    let object = json!({ "name": "ANIM_rig", "position": {"x": 0.0, "y": 0.0, "z": 0.0}, "rotation": {"x": 0.0, "y": 0.0, "z": 0.0}, "scale": {"x": 1.0, "y": 1.0, "z": 1.0}, "blend_shapes": { "keys": [], "reference": null }, "anim_curves": curves });
    json!([{ "name": "Rig", "objects": [object] }])
}

#[test]
fn keyframes_from_object_lists_channels_by_group() {
    let groups = animated_object(&[("location", 2), ("pose.bones[\"Arm.L\"].rotation_quaternion", 0), ("pose.bones[\"Arm.L\"][\"stretch\"]", 0), ("key_blocks[\"Smile\"].value", 0), ("[\"glow\"]", 0), ("color", 0), ("color", 3)]);
    let outputs = keyframes_from_object(&Inputs::from([("object_groups", groups), ("object_group_name", json!("Rig")), ("object_name", json!("ANIM_rig"))])).unwrap().to_json();
    let available: Vec<(String, String)> = outputs["available_channels"].as_array().unwrap().iter().map(|c| (c["group"].as_str().unwrap().to_string(), c["name"].as_str().unwrap().to_string())).collect();
    let expected = [("Object", "Location Z"), ("Arm.L", "Rotation Quaternion W"), ("Arm.L", "stretch"), ("Shape Keys", "Smile"), ("Custom Properties", "glow"), ("Object", "Color 0"), ("Object", "Color 3")];
    assert_eq!(available, expected.map(|(g, n)| (g.to_string(), n.to_string())));
    // nothing picked, no outputs
    assert_eq!(outputs["dyn_output"], json!({}));
}

#[test]
fn keyframes_from_object_outputs_picked_channels() {
    let groups = animated_object(&[("location", 2), ("pose.bones[\"Arm\"].location", 0)]);
    let channels = json!(["pose.bones[\"Arm\"].location[0]", "location[2]"]);
    let outputs = keyframes_from_object(&Inputs::from([("object_groups", groups.clone()), ("object_group_name", json!("Rig")), ("object_name", json!("ANIM_rig")), ("channels", channels)])).unwrap().to_json();
    assert_eq!(outputs["dyn_output"], json!({ "pose.bones[\"Arm\"].location[0]": "Arm › Location X", "location[2]": "Location Z" }));
    assert_eq!(outputs["location[2]"]["array_index"], json!(2));
    assert_eq!(outputs["pose.bones[\"Arm\"].location[0]"]["data_path"], json!("pose.bones[\"Arm\"].location"));

    // a channel the object doesn't have is an error
    let error = keyframes_from_object(&Inputs::from([("object_groups", groups), ("object_group_name", json!("Rig")), ("object_name", json!("ANIM_rig")), ("channels", json!(["scale[0]"]))])).unwrap_err();
    assert!(error.contains("'scale[0]' has no keyframes on 'ANIM_rig'"), "{}", error);
}

#[test]
fn animation_property_index_is_the_last_one() {
    assert_eq!(parse_animation_property("location[2]"), ("location".to_string(), 2));
    assert_eq!(parse_animation_property("pose.bones[\"Arm\"].location[1]"), ("pose.bones[\"Arm\"].location".to_string(), 1));
    assert_eq!(parse_animation_property("[\"glow\"][0]"), ("[\"glow\"]".to_string(), 0));
    assert_eq!(parse_animation_property("location"), ("location".to_string(), 0));
}

#[test]
fn animation_generator_defaults_without_inputs() {
    let outputs = animation_generator(&Inputs::default()).unwrap().to_json();
    let generator = &outputs["generator"];
    assert_eq!(generator["note_on_keyframes"], json!([]));
    assert_eq!(generator["animation_overlap"], json!("add"));
    assert_eq!(generator["animation_property"], json!("[0]"));
}

#[test]
fn note_targets_errors_on_missing_input() {
    let error = note_targets(&Inputs::default()).unwrap_err();
    assert_eq!(error, "missing input 'object_map'");
}

/// Evaluate Instrument's steps, calling its nodes one after the other the way the group runs them
fn evaluate(object_map: serde_json::Value, notes: serde_json::Value) -> serde_json::Value {
    let targets = note_targets(&Inputs::from([("object_map", object_map.clone())])).unwrap().to_json()["targets"].clone();
    let mut keys = Vec::new();
    for note in notes.as_array().unwrap() {
        let for_note = targets_for_note(&Inputs::from([("targets", targets.clone()), ("note", note.clone())])).unwrap().to_json()["targets"].clone();
        for target in for_note.as_array().unwrap() {
            keys.push(note_keyframes(&Inputs::from([("object_map", object_map.clone()), ("target", target.clone()), ("note", note.clone())])).unwrap().to_json()["keys"].clone());
        }
    }
    combine_keyframes(&Inputs::from([("object_map", object_map), ("keys", json!(keys))])).unwrap().to_json()
}

#[test]
fn pad_nums_stays_in_midi_range() {
    // padding below 0 switches to above instead of underflowing
    assert_eq!(pad_nums(vec![0], 3), vec![0, 1, 2]);
    // padding above 127 switches to below
    assert_eq!(pad_nums(vec![127], 3), vec![125, 126, 127]);
    // more objects than notes in the MIDI range stops at 128
    assert_eq!(pad_nums(vec![60], 200).len(), 128);
}

// a linear keyframe point at (time, value)
fn key(time: f64, value: f64) -> serde_json::Value {
    json!({
        "amplitude": 0.0, "back": 0.0, "easing": "AUTO", "interpolation": "LINEAR", "period": 0.0,
        "handle_left": [time, value], "handle_left_type": "AUTO_CLAMPED",
        "handle_right": [time, value], "handle_right_type": "AUTO_CLAMPED",
        "co": [time, value]
    })
}

fn generator(property: &str, peak: f64) -> serde_json::Value {
    json!({
        "name": property, "note_on_keyframes": [key(0.0, 0.0), key(1.0, peak)], "note_on_anchor_point": 0.0,
        "note_off_keyframes": [], "note_off_anchor_point": 0.0, "time_mapper": "", "amplitude_mapper": "",
        "velocity_intensity": 0.0, "animation_overlap": "add", "overlap_blend": 0.1, "animation_property": property
    })
}

// keyframes for "Cube" with the given generators, two overlapping notes
fn cube_keys(properties: &[(&str, f64)]) -> Vec<serde_json::Value> {
    let animations: serde_json::Map<_, _> = properties.iter().map(|(p, peak)| (p.to_string(), generator(p, *peak))).collect();
    let cube: serde_json::Map<_, _> = properties.iter().map(|(p, _)| (p.to_string(), json!([60]))).collect();
    let object_map = json!({ "animations": animations, "objects": { "Cube": cube } });
    let notes = json!([
        { "channel": 0, "note_number": 60, "velocity": 127, "time_on": 0.0, "time_off": 0.1 },
        { "channel": 0, "note_number": 60, "velocity": 127, "time_on": 0.5, "time_off": 0.6 }
    ]);
    let outputs = evaluate(object_map, notes);
    outputs["keyframes"]["Cube"].as_array().unwrap().clone()
}

#[test]
fn evaluate_instrument_steps_combine_overlap_per_curve() {
    // two curves on one object with keys at the same times, each should match evaluating it alone
    let both = cube_keys(&[("location[2]", 1.0), ("rotation_euler[0]", 5.0)]);
    for (property, peak, data_path, index) in [("location[2]", 1.0, "location", 2), ("rotation_euler[0]", 5.0, "rotation_euler", 0)] {
        let on_curve: Vec<_> = both.iter().filter(|k| k["data_path"] == data_path && k["array_index"] == index).cloned().collect();
        assert_eq!(on_curve, cube_keys(&[(property, peak)]), "{}", property);
    }
    assert_eq!(both.len(), 8);
}

// an object map giving "Cube" one generator, triggered by note 60
fn cube_map(property: &str, peak: f64) -> serde_json::Value {
    note_map(property, peak, 60)
}

fn note_map(property: &str, peak: f64, note: u8) -> serde_json::Value {
    json!({ "animations": { property: generator(property, peak) }, "objects": { "Cube": { property: [note] } } })
}

#[test]
fn merge_object_maps_combines_animations_per_object() {
    let outputs = merge_object_maps(&Inputs::from([("object_maps_0", cube_map("location[2]", 1.0)), ("object_maps_1", cube_map("rotation_euler[0]", 5.0))])).unwrap().to_json();
    let merged = &outputs["object_map"];
    assert_eq!(merged["objects"]["Cube"], json!({ "location[2]": [60], "rotation_euler[0]": [60] }));

    // evaluating the merged map matches one map with both generators
    let notes = json!([
        { "channel": 0, "note_number": 60, "velocity": 127, "time_on": 0.0, "time_off": 0.1 },
        { "channel": 0, "note_number": 60, "velocity": 127, "time_on": 0.5, "time_off": 0.6 }
    ]);
    let outputs = evaluate(merged.clone(), notes);
    assert_eq!(outputs["keyframes"]["Cube"].as_array().unwrap(), &cube_keys(&[("location[2]", 1.0), ("rotation_euler[0]", 5.0)]));

    // one side unconnected passes the other through
    let outputs = merge_object_maps(&Inputs::from([("object_maps_0", cube_map("location[2]", 1.0))])).unwrap().to_json();
    assert_eq!(outputs["object_map"], cube_map("location[2]", 1.0));
}

#[test]
fn merge_object_maps_errors_on_generator_name_clash() {
    // the same generator in both maps is fine
    assert!(merge_object_maps(&Inputs::from([("object_maps_0", cube_map("location[2]", 1.0)), ("object_maps_1", cube_map("location[2]", 1.0))])).is_ok());

    // two different generators with one name is not
    let error = merge_object_maps(&Inputs::from([("object_maps_0", cube_map("location[2]", 1.0)), ("object_maps_1", cube_map("location[2]", 2.0))])).unwrap_err();
    assert!(error.contains("two different animation generators are named 'location[2]'"), "{}", error);
}

#[test]
fn merge_object_maps_keeps_notes_per_animation() {
    // like a crash on note 49 and a ride on note 51, both on one cymbal
    let outputs = merge_object_maps(&Inputs::from([("object_maps_0", note_map("location[2]", 1.0, 49)), ("object_maps_1", note_map("rotation_euler[0]", 5.0, 51))])).unwrap().to_json();
    let merged = &outputs["object_map"];
    assert_eq!(merged["objects"]["Cube"], json!({ "location[2]": [49], "rotation_euler[0]": [51] }));

    // each animation only fires on its own note
    let notes = json!([
        { "channel": 0, "note_number": 49, "velocity": 127, "time_on": 0.0, "time_off": 0.1 },
        { "channel": 0, "note_number": 51, "velocity": 127, "time_on": 2.0, "time_off": 2.1 }
    ]);
    let outputs = evaluate(merged.clone(), notes);
    let keys = outputs["keyframes"]["Cube"].as_array().unwrap();
    let times = |data_path: &str| -> Vec<f64> { keys.iter().filter(|k| k["data_path"] == data_path).map(|k| k["time"].as_f64().unwrap()).collect() };
    assert_eq!(times("location"), vec![0.0, 1.0]);
    assert_eq!(times("rotation_euler"), vec![2.0, 3.0]);
}

#[test]
fn merge_object_maps_takes_any_number_of_maps() {
    // three maps in, with a gap where an input was disconnected
    let inputs = Inputs::from([("object_maps_0", note_map("location[2]", 1.0, 49)), ("object_maps_2", note_map("rotation_euler[0]", 5.0, 51)), ("object_maps_5", note_map("scale[1]", 2.0, 53))]);
    let merged = &merge_object_maps(&inputs).unwrap().to_json()["object_map"];
    assert_eq!(merged["objects"]["Cube"], json!({ "location[2]": [49], "rotation_euler[0]": [51], "scale[1]": [53] }));

    // nothing connected is an empty map
    assert_eq!(merge_object_maps(&Inputs::default()).unwrap().to_json()["object_map"], json!({ "animations": {}, "objects": {} }));
}
