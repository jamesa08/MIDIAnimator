use serde_json::json;
use MIDIAnimator::graph::executors::animation::{animation_generator, assign_notes_to_objects, evaluate_instrument, merge_object_maps, pad_nums};
use MIDIAnimator::graph::executors::io::Inputs;
use MIDIAnimator::graph::executors::midi::{get_midi_file, get_midi_track_data};

const TYPE_1: &str = "./tests/test_midi_type_1_rs_4_14_24.mid";

#[test]
fn get_midi_file_reads_string_path() {
    // a JSON string path loads without its quotes
    let outputs = get_midi_file(Inputs::from([("file_path", json!(TYPE_1))])).unwrap();
    assert!(!outputs["tracks"].as_array().unwrap().is_empty());
    assert!(outputs["stats"].as_str().unwrap().contains("tracks"));
}

#[test]
fn get_midi_file_empty_without_path() {
    for inputs in [Inputs::default(), Inputs::from([("file_path", json!(""))])] {
        let outputs = get_midi_file(inputs).unwrap();
        assert_eq!(outputs["tracks"], json!([]));
        assert_eq!(outputs["stats"], json!(""));
    }
}

#[test]
fn get_midi_file_errors_on_missing_file() {
    let error = get_midi_file(Inputs::from([("file_path", json!("/nope/missing.mid"))])).unwrap_err();
    assert!(error.contains("could not read MIDI file '/nope/missing.mid'"), "{}", error);
}

#[test]
fn get_midi_file_errors_on_wrong_type() {
    let error = get_midi_file(Inputs::from([("file_path", json!(42))])).unwrap_err();
    assert!(error.contains("input 'file_path' has the wrong type"), "{}", error);
}

#[test]
fn get_midi_track_data_empty_outputs_without_inputs() {
    let outputs = get_midi_track_data(Inputs::default()).unwrap();
    assert!(!outputs.contains_key("track"));
    assert_eq!(outputs["notes"], json!([]));
    assert_eq!(outputs["control_change"], json!({}));
    assert_eq!(outputs["pitchwheel"], json!([]));
    assert_eq!(outputs["aftertouch"], json!([]));
}

#[test]
fn get_midi_track_data_finds_track() {
    let tracks = get_midi_file(Inputs::from([("file_path", json!(TYPE_1))])).unwrap()["tracks"].clone();
    let name = tracks[0]["name"].clone();
    let outputs = get_midi_track_data(Inputs::from([("tracks", tracks.clone()), ("track_name", name)])).unwrap();
    assert_eq!(outputs["notes"], tracks[0]["notes"]);

    // unknown track name is an error that lists the tracks
    let error = get_midi_track_data(Inputs::from([("tracks", tracks.clone()), ("track_name", json!("nope"))])).unwrap_err();
    assert!(error.contains("track 'nope' not found"), "{}", error);
    assert!(error.contains(tracks[0]["name"].as_str().unwrap()), "{}", error);
}

#[test]
fn get_midi_track_data_errors_on_wrong_type() {
    let error = get_midi_track_data(Inputs::from([("tracks", json!("not tracks")), ("track_name", json!("a"))])).unwrap_err();
    assert!(error.contains("input 'tracks' has the wrong type"), "{}", error);
}

#[test]
fn assign_notes_errors_on_missing_group() {
    let groups = json!([{ "name": "Cubes", "objects": [] }]);
    let error = assign_notes_to_objects(Inputs::from([("object_groups", groups), ("object_group_name", json!("Spheres"))])).unwrap_err();
    assert!(error.contains("object group 'Spheres' does not exist"), "{}", error);

    // nothing picked yet is not an error
    let outputs = assign_notes_to_objects(Inputs::default()).unwrap();
    assert_eq!(outputs["object_map"], json!({ "animations": {}, "objects": {} }));
}

#[test]
fn animation_generator_defaults_without_inputs() {
    let outputs = animation_generator(Inputs::default()).unwrap();
    let generator = &outputs["generator"];
    assert_eq!(generator["note_on_keyframes"], json!([]));
    assert_eq!(generator["animation_overlap"], json!("add"));
    assert_eq!(generator["animation_property"], json!("[0]"));
}

#[test]
fn evaluate_instrument_errors_on_missing_input() {
    let error = evaluate_instrument(Inputs::from([("midi_notes", json!([]))])).unwrap_err();
    assert_eq!(error, "missing input 'object_map'");
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
        "velocity_intensity": 0.0, "animation_overlap": "add", "animation_property": property
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
    let outputs = evaluate_instrument(Inputs::from([("object_map", object_map), ("midi_notes", notes)])).unwrap();
    outputs["keyframes"]["Cube"].as_array().unwrap().clone()
}

#[test]
fn evaluate_instrument_combines_overlap_per_curve() {
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
    let outputs = merge_object_maps(Inputs::from([("object_maps_0", cube_map("location[2]", 1.0)), ("object_maps_1", cube_map("rotation_euler[0]", 5.0))])).unwrap();
    let merged = &outputs["object_map"];
    assert_eq!(merged["objects"]["Cube"], json!({ "location[2]": [60], "rotation_euler[0]": [60] }));

    // evaluating the merged map matches one map with both generators
    let notes = json!([
        { "channel": 0, "note_number": 60, "velocity": 127, "time_on": 0.0, "time_off": 0.1 },
        { "channel": 0, "note_number": 60, "velocity": 127, "time_on": 0.5, "time_off": 0.6 }
    ]);
    let outputs = evaluate_instrument(Inputs::from([("object_map", merged.clone()), ("midi_notes", notes)])).unwrap();
    assert_eq!(outputs["keyframes"]["Cube"].as_array().unwrap(), &cube_keys(&[("location[2]", 1.0), ("rotation_euler[0]", 5.0)]));

    // one side unconnected passes the other through
    let outputs = merge_object_maps(Inputs::from([("object_maps_0", cube_map("location[2]", 1.0))])).unwrap();
    assert_eq!(outputs["object_map"], cube_map("location[2]", 1.0));
}

#[test]
fn merge_object_maps_errors_on_generator_name_clash() {
    // the same generator in both maps is fine
    assert!(merge_object_maps(Inputs::from([("object_maps_0", cube_map("location[2]", 1.0)), ("object_maps_1", cube_map("location[2]", 1.0))])).is_ok());

    // two different generators with one name is not
    let error = merge_object_maps(Inputs::from([("object_maps_0", cube_map("location[2]", 1.0)), ("object_maps_1", cube_map("location[2]", 2.0))])).unwrap_err();
    assert!(error.contains("two different animation generators are named 'location[2]'"), "{}", error);
}

#[test]
fn merge_object_maps_keeps_notes_per_animation() {
    // like a crash on note 49 and a ride on note 51, both on one cymbal
    let outputs = merge_object_maps(Inputs::from([("object_maps_0", note_map("location[2]", 1.0, 49)), ("object_maps_1", note_map("rotation_euler[0]", 5.0, 51))])).unwrap();
    let merged = &outputs["object_map"];
    assert_eq!(merged["objects"]["Cube"], json!({ "location[2]": [49], "rotation_euler[0]": [51] }));

    // each animation only fires on its own note
    let notes = json!([
        { "channel": 0, "note_number": 49, "velocity": 127, "time_on": 0.0, "time_off": 0.1 },
        { "channel": 0, "note_number": 51, "velocity": 127, "time_on": 2.0, "time_off": 2.1 }
    ]);
    let outputs = evaluate_instrument(Inputs::from([("object_map", merged.clone()), ("midi_notes", notes)])).unwrap();
    let keys = outputs["keyframes"]["Cube"].as_array().unwrap();
    let times = |data_path: &str| -> Vec<f64> { keys.iter().filter(|k| k["data_path"] == data_path).map(|k| k["time"].as_f64().unwrap()).collect() };
    assert_eq!(times("location"), vec![0.0, 1.0]);
    assert_eq!(times("rotation_euler"), vec![2.0, 3.0]);
}

#[test]
fn merge_object_maps_takes_any_number_of_maps() {
    // three maps in, with a gap where an input was disconnected
    let inputs = Inputs::from([("object_maps_0", note_map("location[2]", 1.0, 49)), ("object_maps_2", note_map("rotation_euler[0]", 5.0, 51)), ("object_maps_5", note_map("scale[1]", 2.0, 53))]);
    let merged = &merge_object_maps(inputs).unwrap()["object_map"];
    assert_eq!(merged["objects"]["Cube"], json!({ "location[2]": [49], "rotation_euler[0]": [51], "scale[1]": [53] }));

    // nothing connected is an empty map
    assert_eq!(merge_object_maps(Inputs::default()).unwrap()["object_map"], json!({ "animations": {}, "objects": {} }));
}
