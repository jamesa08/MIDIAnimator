// run from /src-tauri
// cargo test --test diagnostics_test
use MIDIAnimator::utils::diagnostics::anonymize;

#[test]
fn home_folder_becomes_tilde() {
    let text = "running from /Users/james/Library/Logs/com.jamesa08.midianimator/motionkeys.log";
    assert_eq!(anonymize(text, "/Users/james", "james"), "running from ~/Library/Logs/com.jamesa08.midianimator/motionkeys.log");
}

#[test]
fn other_user_folders_are_hidden() {
    assert_eq!(anonymize("/Users/someone/Desktop/song.blend", "/Users/james", "james"), "/Users/<user>/Desktop/song.blend");
    assert_eq!(anonymize(r"C:\Users\James Alt\AppData", "", ""), r"C:\Users\<user>\AppData");
}

#[test]
fn user_name_only_as_a_whole_word() {
    // lsof prints the user name in its own column, the bundle id contains it as part of a word
    assert_eq!(anonymize("MotionKeys 82378 james 13u IPv4", "/Users/james", "james"), "MotionKeys 82378 <user> 13u IPv4");
    assert_eq!(anonymize("com.jamesa08.midianimator", "/Users/james", "james"), "com.jamesa08.midianimator");
}

#[test]
fn empty_home_and_user_change_nothing() {
    assert_eq!(anonymize("plain text", "", ""), "plain text");
    assert_eq!(anonymize("a/b", "/", ""), "a/b");
}
