use serde_json::json;
use std::collections::HashMap;
use MIDIAnimator::graph::executors::midi::{get_midi_file, get_midi_track_data};

const TYPE_1: &str = "./tests/test_midi_type_1_rs_4_14_24.mid";

#[test]
fn get_midi_file_reads_string_path() {
    // a JSON string path loads without its quotes
    let outputs = get_midi_file(HashMap::from([("file_path".to_string(), json!(TYPE_1))]));
    assert!(!outputs["tracks"].as_array().unwrap().is_empty());
    assert!(outputs["stats"].as_str().unwrap().contains("tracks"));
}

#[test]
fn get_midi_file_empty_without_path() {
    for inputs in [HashMap::new(), HashMap::from([("file_path".to_string(), json!(""))])] {
        let outputs = get_midi_file(inputs);
        assert_eq!(outputs["tracks"], json!([]));
        assert_eq!(outputs["stats"], json!(""));
    }
}

#[test]
fn get_midi_track_data_empty_outputs_without_inputs() {
    let outputs = get_midi_track_data(HashMap::new());
    assert!(!outputs.contains_key("track"));
    assert_eq!(outputs["notes"], json!([]));
    assert_eq!(outputs["control_change"], json!({}));
    assert_eq!(outputs["pitchwheel"], json!([]));
    assert_eq!(outputs["aftertouch"], json!([]));
}

#[test]
fn get_midi_track_data_finds_track() {
    let tracks = get_midi_file(HashMap::from([("file_path".to_string(), json!(TYPE_1))])).remove("tracks").unwrap();
    let name = tracks[0]["name"].clone();
    let outputs = get_midi_track_data(HashMap::from([("tracks".to_string(), tracks.clone()), ("track_name".to_string(), name)]));
    assert_eq!(outputs["notes"], tracks[0]["notes"]);

    // unknown track name keeps the empty outputs
    let outputs = get_midi_track_data(HashMap::from([("tracks".to_string(), tracks), ("track_name".to_string(), json!("nope"))]));
    assert_eq!(outputs["notes"], json!([]));
}
