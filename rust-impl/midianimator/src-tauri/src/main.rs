// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(non_snake_case)]

use lazy_static::lazy_static;
use tauri::{command, generate_handler, Builder};

use MIDIAnimator::structures::midi::MIDIFile;
use MIDIAnimator::structures::ipc;

lazy_static! {
    static ref MIDI_FILE_STR: &'static str = "/Users/james/github/MIDIFiles/testMidi/test_midi_2_rs_4_14_24.mid";
}


#[tokio::main]
async fn main() {
    
    ipc::start_server();

    // tauri::Builder::default()
    // .run(tauri::generate_context!())
    // .expect("error while launching MIDIAnimator!");

    // count down timer
    let mut count = 3;
    while count > 0 {
        println!("waiting {}", count);
        count -= 1;
        std::thread::sleep(std::time::Duration::from_secs(1));
    }

    let message = r#"def get_object_name():
    return bpy.data.objects['Cube'].name_full

def execution():
    return get_object_name()
"#;

    if let Some(response) = ipc::send_message(message.to_string()).await {
        println!("Received response: {}", response);
    }
    let mut count = 0;

    loop {
        std::thread::sleep(std::time::Duration::from_millis(500));
        if count == 100 {
            break;
        }
        count += 1;
    }
}


// example of a tauri front -> backend command
// #[command]
// fn greet() -> String {
//     let midi_file = MIDIFile::new(&MIDI_FILE_STR).unwrap();
//     for track in midi_file {
//         println!("Track name: {}", track.name);
//         for note in &track.notes {
//             println!("{:?}", note);
//         }
//         return format!("{:?}", track.all_used_notes()).to_string();
//         // println!("{:?}", track.all_used_notes());
//     }
//     return "".to_string();
// }