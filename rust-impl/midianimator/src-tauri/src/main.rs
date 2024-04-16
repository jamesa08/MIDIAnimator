// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use MIDIAnimator::structures::midi::MIDIFile;

use lazy_static::lazy_static;

lazy_static! {
    static ref MIDI_FILE_STR: &'static str = "/Users/james/github/MIDIFiles/testMidi/test_midi_2_rs_4_14_24.mid";
}

fn main() {
    let midi_file = MIDIFile::new(&MIDI_FILE_STR).unwrap();
    for track in midi_file {
        println!("Track name: {}", track.name);
        for note in &track.notes {
            println!("{:?}", note);
        }
        break;
        // println!("{:?}", track.all_used_notes());
    }

    // TODO uncomment this 
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

