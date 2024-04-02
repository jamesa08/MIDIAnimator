use midly::{num::u28, MetaMessage, MidiMessage, Smf, TrackEvent, TrackEventKind};
use std::collections::HashMap;
use std::fmt;

// MARK: - MIDINote

#[derive(Debug, Clone)]
pub struct MIDINote {
    pub channel: u8,
    pub note_number: u8,
    pub velocity: u8,
    pub time_on: f64,
    pub time_off: f64,
}

impl MIDINote {
    pub fn new(channel: u8, note_number: u8, velocity: u8, time_on: f64, time_off: f64) -> Self {
        MIDINote {
            channel,
            note_number,
            velocity,
            time_on,
            time_off,
        }
    }
}

impl fmt::Display for MIDINote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MIDINote(channel={}, note_number={}, velocity={}, time_on={}, time_off={})",
            self.channel, self.note_number, self.velocity, self.time_on, self.time_off
        )
    }
}

// equality
impl PartialEq for MIDINote {
    fn eq(&self, other: &Self) -> bool {
        self.time_on == other.time_on
    }
}


// comparison
impl PartialOrd for MIDINote {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.time_on.partial_cmp(&other.time_on)
    }
}

// MARK: - MIDIEvent

#[derive(Debug, Clone)]
pub struct MIDIEvent {
    pub channel: u8,
    pub value: f64,
    pub time: f64,
}

impl MIDIEvent {
    pub fn new(channel: u8, value: f64, time: f64) -> Self {
        MIDIEvent {
            channel,
            value,
            time,
        }
    }
}

impl fmt::Display for MIDIEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MIDIEvent(channel={}, value={}, time={})",
            self.channel, self.value, self.time
        )
    }
}

impl PartialEq for MIDIEvent {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time
    }
}

impl PartialOrd for MIDIEvent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.time.partial_cmp(&other.time)
    }
}

#[derive(Debug, Clone)]
pub struct MIDITrack {
    pub name: String,
    pub notes: Vec<MIDINote>,
    pub control_change: HashMap<u8, Vec<MIDIEvent>>,
    pub pitchwheel: Vec<MIDIEvent>,
    pub aftertouch: Vec<MIDIEvent>,
    note_table: HashMap<(u8, u8), Vec<MIDINote>>,
}

impl MIDITrack {
    pub fn new(name: &str) -> Self {
        MIDITrack {
            name: name.to_string(),
            notes: Vec::new(),
            control_change: HashMap::new(),
            pitchwheel: Vec::new(),
            aftertouch: Vec::new(),
            note_table: HashMap::new(),
        }
    }

    pub fn add_note_on(&mut self, channel: u8, note_number: u8, velocity: u8, time_on: f64) {
        let key = (channel, note_number);
        let note = MIDINote::new(channel, note_number, velocity, time_on, -1.0);

        if let Some(notes) = self.note_table.get_mut(&key) {
            notes.push(note.clone());
        } else {
            self.note_table.insert(key, vec![note.clone()]);
        }

        self.notes.push(note);
    }

    pub fn add_note_off(&mut self, channel: u8, note_number: u8, velocity: u8, time_off: f64) {
        let key = (channel, note_number);

        if let Some(notes) = self.note_table.get_mut(&key) {
            if let Some(note) = notes.first_mut() {
                note.time_off = time_off;
                notes.remove(0);
            } else {
                panic!("NoteOff message has no NoteOn message! Your MIDI File may be corrupt. Please open an issue on GitHub.");
            }
        }
    }

    pub fn add_control_change(&mut self, control_number: u8, channel: u8, value: u8, time: f64) {
        let event = MIDIEvent::new(channel, value as f64, time);

        if let Some(events) = self.control_change.get_mut(&control_number) {
            events.push(event);
        } else {
            self.control_change.insert(control_number, vec![event]);
        }
    }

    pub fn add_pitchwheel(&mut self, channel: u8, value: f64, time: f64) {
        self.pitchwheel.push(MIDIEvent::new(channel, value, time));
    }

    pub fn add_aftertouch(&mut self, channel: u8, value: f64, time: f64) {
        self.aftertouch.push(MIDIEvent::new(channel, value, time));
    }

    fn is_empty(&self) -> bool {
        self.notes.is_empty()
            && self.control_change.is_empty()
            && self.pitchwheel.is_empty()
            && self.aftertouch.is_empty()
    }

    pub fn all_used_notes(&self) -> Vec<u8> {
        let mut used_notes: Vec<u8> = self.notes.iter().map(|note| note.note_number).collect();
        used_notes.sort_unstable();
        used_notes.dedup();
        used_notes
    }
}

impl fmt::Display for MIDITrack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "MIDITrack(name='{}')", self.name)?;
        writeln!(f, "notes=[")?;
        for note in &self.notes {
            writeln!(f, "\t{},", note)?;
        }
        writeln!(f, "]")?;
        writeln!(f, "controlChanges={{")?;
        for (control_number, events) in &self.control_change {
            writeln!(f, "\tcontrol_number={}: ", control_number)?;
            for event in events {
                writeln!(f, "\t\t{},", event)?;
            }
        }
        writeln!(f, "}}")?;
        writeln!(f, "afterTouch=[")?;
        for event in &self.aftertouch {
            writeln!(f, "\t{},", event)?;
        }
        writeln!(f, "]")?;
        writeln!(f, "pitchwheel=[")?;
        for event in &self.pitchwheel {
            writeln!(f, "\t{},", event)?;
        }
        writeln!(f, "]")?;
        Ok(())
    }
}


// MARK: - MIDIFile

#[derive(Debug, Clone)]
pub struct MIDIFile {
    tracks: Vec<MIDITrack>,
}

impl MIDIFile {
    pub fn new(midi_file: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = std::fs::read(midi_file)?;
        let smf = Smf::parse(&bytes)?;

        let mut tracks = Vec::new();

        for (i, track) in smf.tracks.iter().enumerate() {
            let mut midi_track = MIDITrack::new(&format!("Track {}", i + 1));

            let mut time = 0.0;
            let mut tempo = 500000;

            for event in track {
                match event {
                    TrackEvent { delta, kind: TrackEventKind::Midi { channel, message } } => {
                        time += *delta as f64 / smf.header.timing.ticks_per_beat() as f64 * tempo as f64 / 1_000_000.0;

                        match message {
                            // ...
                            MidiMessage::Controller { controller, value } => {
                                midi_track.add_control_change(
                                    controller.as_int() as u8,
                                    (*channel).into(),
                                    value.as_int() as u8,
                                    time,
                                );
                            }
                            MidiMessage::PitchBend { bend } => {
                                midi_track.add_pitchwheel(
                                    (*channel).into(),
                                    bend.0 as f64 / 8192.0 - 1.0,
                                    time,
                                );
                            }
                            MidiMessage::NoteOff { key, vel } => {
                                midi_track.add_note_off((*channel).into(), key.as_int(), vel.as_int(), time);
                            }
                            MidiMessage::Controller { controller, value } => {
                                midi_track.add_control_change(
                                    controller.as_int(),
                                    (*channel).into(),
                                    value.as_int(),
                                    time,
                                );
                            }
                            MidiMessage::PitchBend { bend } => {
                                midi_track.add_pitchwheel(
                                    (*channel).into(),
                                    bend.0 as f64 / 8192.0 - 1.0,
                                    time,
                                );
                            }
                            _ => {}
                        }
                    }
                    TrackEvent { delta, kind: TrackEventKind::Meta(MetaMessage::Tempo(new_tempo)) } => {
                        time += *delta as f64 / smf.header.timing.as_int() as f64 * tempo as f64 / 1_000_000.0;
                        tempo = u28::from(new_tempo).as_int() as u32;
                    }
                    TrackEvent { delta, kind: TrackEventKind::Meta(MetaMessage::TrackName(name)) } => {
                        midi_track.name = String::from_utf8_lossy(name).into_owned();
                    }
                    _ => {}
                }
            }

            if !midi_track.is_empty() {
                tracks.push(midi_track);
            }
        }

        Ok(MIDIFile { tracks })
    }

    pub fn get_midi_tracks(&self) -> &Vec<MIDITrack> {
        &self.tracks
    }

    pub fn find_track(&self, name: &str) -> Option<&MIDITrack> {
        self.tracks.iter().find(|track| track.name == name)
    }

    pub fn list_track_names(&self) -> Vec<String> {
        self.tracks.iter().map(|track| track.name.clone()).collect()
    }

    pub fn merge_tracks(&self, track1: &MIDITrack, track2: &MIDITrack, name: Option<&str>) -> MIDITrack {
        let mut merged_track = MIDITrack::new(name.unwrap_or(""));

        merged_track.notes.extend(track1.notes.iter().cloned());
        merged_track.notes.extend(track2.notes.iter().cloned());
        merged_track.notes.sort_by(|a, b| a.partial_cmp(b).unwrap());

        merged_track.control_change.extend(track1.control_change.iter().map(|(k, v)| (*k, v.clone())));
        merged_track.control_change.extend(track2.control_change.iter().map(|(k, v)| (*k, v.clone())));

        merged_track.pitchwheel.extend(track1.pitchwheel.iter().cloned());
        merged_track.pitchwheel.extend(track2.pitchwheel.iter().cloned());
        merged_track.pitchwheel.sort_by(|a, b| a.partial_cmp(b).unwrap());

        merged_track.aftertouch.extend(track1.aftertouch.iter().cloned());
        merged_track.aftertouch.extend(track2.aftertouch.iter().cloned());
        merged_track.aftertouch.sort_by(|a, b| a.partial_cmp(b).unwrap());

        merged_track
    }
}

impl fmt::Display for MIDIFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for track in &self.tracks {
            writeln!(f, "{}", track)?;
        }
        Ok(())
    }
}

impl IntoIterator for MIDIFile {
    type Item = MIDITrack;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.tracks.into_iter()
    }
}