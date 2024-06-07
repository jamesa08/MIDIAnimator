use crate::structures::midi::MIDIFile;

#[tauri::command]
pub fn get_midi_file(file: &str) -> MIDIFile {
    let midi_file = MIDIFile::new(file).unwrap();
    return midi_file;
}