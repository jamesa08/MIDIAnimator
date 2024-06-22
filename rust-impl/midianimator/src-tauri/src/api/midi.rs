use crate::structures::midi::MIDIFile;

#[tauri::command]
pub fn get_midi_file(file: &str) -> MIDIFile {
    let midi_file = MIDIFile::new(file).unwrap();
    return midi_file;
}

#[tauri::command]
pub fn get_midi_file_statistics(file: &str) -> String {
    let midi_file = MIDIFile::new(file).unwrap();
    let track_count = midi_file.get_midi_tracks().len();
    let mut time_constant: f64 = 0.0;  // in ms
    for track in midi_file {
        let final_note = track.notes.get(track.notes.len() - 1);
        if final_note.is_some() && final_note.unwrap().time_off > time_constant {
            time_constant = final_note.unwrap().time_off;
        }
    }
    // get track count
    return format!("{} tracks, \n{} minutes", track_count, time_constant).to_string();
    
}