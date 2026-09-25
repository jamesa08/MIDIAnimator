use super::io::{Inputs, NodeResult, Outputs};
use crate::midi::{MIDIFile, MIDITrack};

pub fn get_midi_file_statistics(midi_file: &MIDIFile) -> String {
    let track_count = midi_file.get_midi_tracks().len();
    let mut seconds: f64 = 0.0; // in ms
    for track in midi_file.get_midi_tracks() {
        let final_note = track.notes.last();
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
    return format!("{} tracks\n{}", track_count, hhmmss).to_string();
}

/// Node: get_midi_file
///
/// inputs:
/// "file_path": `String`
///
/// outputs:
/// "tracks": `Array<MIDITrack>`,
/// "stats": `String`
#[node_registry::node]
pub fn get_midi_file(inputs: Inputs) -> NodeResult {
    let mut outputs = Outputs::new();

    // no path yet, empty outputs until one is picked
    let file_path: String = inputs.or_default("file_path")?;
    if file_path.is_empty() {
        outputs.set("tracks", &Vec::<MIDITrack>::new())?;
        outputs.set("stats", "")?;
        return Ok(outputs);
    }

    // read the file, a missing or broken file is an error on the node
    let midi_file = MIDIFile::new(&file_path).map_err(|e| format!("could not read MIDI file '{}': {}", file_path, e))?;
    let midi_file_statistics = get_midi_file_statistics(&midi_file);
    outputs.set("tracks", midi_file.get_midi_tracks())?;
    outputs.set("stats", &midi_file_statistics)?;
    Ok(outputs)
}

/// Node: get_midi_track_data
///
/// inputs:
/// "tracks": `Array<MIDITrack>`,
/// "track_name": `String`
///
/// outputs:
/// "notes": `Array<MIDINote>`,
/// "control_change": `HashMap<u8, Array<MIDIEvent>>`,
/// "pitchwheel": `Array<MIDIEvent>`,
/// "aftertouch": `Array<MIDIEvent>`
#[node_registry::node]
pub fn get_midi_track_data(inputs: Inputs) -> NodeResult {
    let tracks: Vec<MIDITrack> = inputs.or_default("tracks")?;
    let track_name: String = inputs.or_default("track_name")?;

    // empty outputs until there are tracks and one is picked
    let mut track = MIDITrack::new("");
    if !tracks.is_empty() && !track_name.is_empty() {
        // a name that isn't in the file is an error, it usually means the file changed
        track = tracks.iter().find(|t| t.name == track_name).cloned().ok_or_else(|| {
            let names: Vec<&str> = tracks.iter().map(|t| t.name.as_str()).collect();
            format!("track '{}' not found, the file has: {}", track_name, names.join(", "))
        })?;
    }

    let mut outputs = Outputs::new();
    outputs.set("notes", &track.notes)?;
    outputs.set("control_change", &track.control_change)?;
    outputs.set("pitchwheel", &track.pitchwheel)?;
    outputs.set("aftertouch", &track.aftertouch)?;
    Ok(outputs)
}
