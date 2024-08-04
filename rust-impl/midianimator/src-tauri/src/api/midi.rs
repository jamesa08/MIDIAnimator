use std::collections::HashMap;
use crate::structures::midi::MIDIFile;

pub fn get_midi_file_statistics(midi_file: &MIDIFile) -> String {
    let track_count = midi_file.get_midi_tracks().len();
    let mut seconds: f64 = 0.0;  // in ms
    for track in midi_file.get_midi_tracks() {
        let final_note = track.notes.get(track.notes.len() - 1);
        if final_note.is_some() && final_note.unwrap().time_off > seconds {
            seconds = final_note.unwrap().time_off;
        }
    }

    let minutes = ((seconds / 60.0) % 60.0) as i32;
    let hours: i32 = ((seconds / 60.0) / 60.0) as i32;

    // build hh:mm:ss string omitting hours & minutes if they are 0
    let mut hhmmss = String::new();
    if hours > 0 {
        hhmmss.push_str(&format!("{:02}:", hours));
    }
    if minutes > 0 {
        hhmmss.push_str(&format!("{:02}:", minutes));
    }
    hhmmss.push_str(&format!("{:02}", (seconds % 60.0) as i32));

    // add "seconds" label if there are no hours or minutes
    if hours == 0 && minutes == 0 {
        hhmmss.push_str(" seconds");
    } else {
        hhmmss.push_str(" minutes");
    }

    // get track count
    return format!("{} tracks\n{}", track_count, hhmmss).to_string()
}


/// Node: get_midi_file
/// 
/// inputs: 
/// "file_path": `String`
/// 
/// outputs:
/// "tracks": `Array<MIDITrack>`,
/// "stats": `String`
#[tauri::command]
pub fn get_midi_file(inputs: HashMap<String, serde_json::Value>) -> HashMap<String, serde_json::Value> {
    let mut outputs: HashMap<String, serde_json::Value> = HashMap::new();
    if !inputs.contains_key("file_path") {
        outputs.insert("tracks".to_string(), serde_json::Value::Array(vec![]));
        outputs.insert("stats".to_string(), serde_json::Value::String("".to_string()));
        return outputs;
    }   

    let midi_file = MIDIFile::new(inputs["file_path"].to_string().as_str()).unwrap();
    let midi_file_statistics = get_midi_file_statistics(&midi_file);
    outputs.insert("tracks".to_string(), serde_json::to_value(midi_file.get_midi_tracks()).unwrap());
    outputs.insert("stats".to_string(), serde_json::to_value(midi_file_statistics).unwrap());
    return outputs;
}

/// Node: get_midi_track
/// 
/// inputs: 
/// "tracks": `Array<MIDITrack>`,
/// "track_name": `String`
/// 
/// outputs:
/// "track": `MIDITrack`
#[tauri::command]
pub fn get_midi_track(inputs: HashMap<String, serde_json::Value>) -> HashMap<String, serde_json::Value> {

    let mut outputs: HashMap<String, serde_json::Value> = HashMap::new();
    println!("GET MIDI TRACK INPUTS {:?}", inputs);
    let tracks_unwrapped = inputs["tracks"].as_array().expect("in get_midi_track, tracks is not an array").clone();

    for track in tracks_unwrapped {
        println!("track name thing {:?}", track.as_object().unwrap().get_key_value("name").unwrap().1.as_str().unwrap());
        println!("other track name thing {:?}", inputs["track_name"].as_str().unwrap());
        if track.as_object().unwrap().get_key_value("name").unwrap().1.as_str().unwrap() == inputs["track_name"].as_str().unwrap() {
            outputs.insert("track".to_string(), track);
            break;
        }
    }
    println!("GET MIDI TRACK OUTPUTS {:?}", outputs);
    
    return outputs;
}