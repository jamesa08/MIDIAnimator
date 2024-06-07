use crate::structures::midi::MIDIFile;

#[tauri::command]
fn get_midi_file(file: &str) -> MIDIFile {
    let midi_file = MIDIFile::new(file).unwrap();
    return midi_file;
}