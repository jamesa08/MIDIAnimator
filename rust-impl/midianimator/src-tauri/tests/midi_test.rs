#[cfg(test)]
// run from /src-tauri 
// cargo test
mod tests { 
    use lazy_static::lazy_static;
    use MIDIAnimator::structures::midi::MIDIFile;

    lazy_static! {
        static ref TYPE_0: &'static str = "./tests/test_midi_type_0_rs_4_14_24.mid";
        static ref TYPE_1: &'static str = "./tests/test_midi_type_1_rs_4_14_24.mid";
    }

    #[test]
    fn test_load_valid_midi_file() {
        let midi_file = MIDIFile::new(&TYPE_0).unwrap();
        assert!(midi_file.get_midi_tracks().len() > 0);
    }

    #[test]
    fn test_load_invalid_midi_file() {
        let midi_file = MIDIFile::new(&"./tests/FILE_DOES_NOT_EXIST.mid");
        assert!(midi_file.is_err());
    }

    #[test]
    fn test_get_midi_tracks_type_0() {
        let midi_file = MIDIFile::new(&TYPE_0).unwrap();
        let tracks = midi_file.get_midi_tracks();
        // assert_eq!(tracks.len(), 2); // Adjust the expected number of tracks
        println!("Number of tracks: {}", tracks.len());
        for track in tracks {
            println!("Track name: {}", track.name);
        }
    }

    #[test]
    fn test_get_midi_tracks_type_1() {
        let midi_file = MIDIFile::new(&TYPE_1).unwrap();
        let tracks = midi_file.get_midi_tracks();
        // assert_eq!(tracks.len(), 2); // Adjust the expected number of tracks
        println!("Number of tracks: {}", tracks.len());
        for track in tracks {
            println!("Track name: {}", track.name);
        }
    }

    #[test]
    fn test_find_track_by_name_type_0() {
        let midi_file = MIDIFile::new(&TYPE_0).unwrap();
        let track = midi_file.find_track("Classic Electric Piano");
        assert!(track.is_some());
    }

    #[test]
    fn test_find_track_by_name_type_1() {
        let midi_file = MIDIFile::new(&TYPE_1).unwrap();
        let track = midi_file.find_track("Classic Electric Piano");
        assert!(track.is_some());
    }

    #[test]
    fn test_merge_tracks() {
        let midi_file = MIDIFile::new(&TYPE_1).unwrap();
        let track1 = midi_file.find_track("Classic Electric Piano").unwrap();
        let track2 = midi_file.find_track("Classic Electric Piano").unwrap();
        let merged_track = midi_file.merge_tracks(track1, track2, Some("Merged Track"));
        assert_eq!(merged_track.name, "Merged Track");
    }

}
