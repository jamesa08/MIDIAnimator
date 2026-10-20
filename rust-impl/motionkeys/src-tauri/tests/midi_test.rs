#[cfg(test)]
// run from /src-tauri
// cargo test
mod tests {
    use lazy_static::lazy_static;
    use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};
    use motionkeys::midi::MIDIFile;

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

    // writes a type 1 file, each track a list of (delta ticks, event)
    fn write_type_1(name: &str, tracks: Vec<Vec<(u32, TrackEventKind<'static>)>>) -> String {
        let mut smf = Smf::new(Header::new(Format::Parallel, Timing::Metrical(480.into())));
        for track in tracks {
            let mut events: Vec<TrackEvent> = track
                .into_iter()
                .map(|(delta, kind)| TrackEvent {
                    delta: delta.into(),
                    kind,
                })
                .collect();
            events.push(TrackEvent {
                delta: 0.into(),
                kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
            });
            smf.tracks.push(events);
        }
        let path = std::env::temp_dir().join(format!("motionkeys_{}_{}.mid", name, std::process::id()));
        smf.save(&path).unwrap();
        path.to_string_lossy().into_owned()
    }

    fn tempo(microseconds: u32) -> TrackEventKind<'static> {
        TrackEventKind::Meta(MetaMessage::Tempo(microseconds.into()))
    }

    fn note(on: bool) -> TrackEventKind<'static> {
        let (key, vel) = (60.into(), 100.into());
        TrackEventKind::Midi {
            channel: 0.into(),
            message: if on {
                MidiMessage::NoteOn {
                    key,
                    vel,
                }
            } else {
                MidiMessage::NoteOff {
                    key,
                    vel,
                }
            },
        }
    }

    #[test]
    fn test_tempo_changes() {
        // 120 for 2 beats (1s), 115 for 1 beat (0.521739s), then 100 from track 1, a note spans all three
        let path = write_type_1("tempo_changes", vec![vec![(0, tempo(500000)), (960, tempo(521739))], vec![(1440, tempo(600000)), (0, note(true)), (480, note(false))], vec![(960, note(true)), (960, note(false))]]);
        let midi_file = MIDIFile::new(&path).unwrap();
        let change = 1.0 + 0.521739;
        assert_eq!(midi_file.get_tempo().len(), 3);
        assert_eq!(midi_file.get_tempo()[0], (120.0, 0.0));
        assert_eq!(midi_file.get_tempo()[1], (115.0, 1.0));
        assert_eq!(midi_file.get_tempo()[2].0, 100.0);
        assert!((midi_file.get_tempo()[2].1 - change).abs() < 1e-9);

        // times after a change follow the new tempo, in every track
        let notes: Vec<(f64, f64)> = midi_file.get_midi_tracks().iter().map(|track| (track.notes[0].time_on, track.notes[0].time_off)).collect();
        assert!((notes[0].0 - change).abs() < 1e-9 && (notes[0].1 - (change + 0.6)).abs() < 1e-9, "{:?}", notes);
        assert!((notes[1].0 - 1.0).abs() < 1e-9 && (notes[1].1 - (change + 0.6)).abs() < 1e-9, "{:?}", notes);
    }

    #[test]
    fn test_tempo_defaults_to_120() {
        let path = write_type_1("no_tempo", vec![vec![], vec![(480, note(true)), (480, note(false))]]);
        let midi_file = MIDIFile::new(&path).unwrap();
        assert_eq!(midi_file.get_tempo(), &vec![(120.0, 0.0)]);
        assert_eq!(midi_file.get_midi_tracks()[0].notes[0].time_off, 1.0);
    }
}
