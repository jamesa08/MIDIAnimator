use midly::{MetaMessage, MidiMessage, Smf, TrackEventKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

use crate::utils::gm_program_to_name;

// TODO eventually refactor into more of The Rust Way
// MARK: - MIDINote

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
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
        write!(f, "MIDINote(channel={}, note_number={}, velocity={}, time_on={}, time_off={})", self.channel, self.note_number, self.velocity, self.time_on, self.time_off)
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

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
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
        write!(f, "MIDIEvent(channel={}, value={}, time={})", self.channel, self.value, self.time)
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

// MARK: - TempoMap

/// the tempo until a file sets one, in microseconds per beat (120 BPM)
const DEFAULT_TEMPO: f64 = 500000.0;

/// turns ticks into seconds. tempo applies to the whole file (type 1 files keep it in track 0, some put changes in other
/// tracks too), so every track's times come from the same tempo changes
struct TempoMap {
    /// (tick, seconds, microseconds per beat) where each tempo starts, the first starts at tick 0
    changes: Vec<(u64, f64, f64)>,
    ticks_per_beat: f64,
    /// SMPTE timed files count ticks in real time, tempo changes don't move them
    ticks_per_second: Option<f64>,
}

impl TempoMap {
    fn new(smf: &Smf) -> Self {
        let (ticks_per_beat, ticks_per_second) = match smf.header.timing {
            midly::Timing::Metrical(tpq) => (tpq.as_int() as f64, None),
            midly::Timing::Timecode(fps, tpf) => (1.0, Some(fps.as_f32() as f64 * tpf as f64)),
        };

        // every tempo change at its tick, from all tracks
        let mut tempos: Vec<(u64, f64)> = Vec::new();
        for track in smf.tracks.iter() {
            let mut tick: u64 = 0;
            for msg in track {
                tick += msg.delta.as_int() as u64;
                if let TrackEventKind::Meta(MetaMessage::Tempo(tempo)) = msg.kind {
                    tempos.push((tick, tempo.as_int() as f64));
                }
            }
        }
        // stable, so the last change on a tick wins
        tempos.sort_by_key(|&(tick, _)| tick);

        // each change starts where the time at the previous tempo got to
        let mut map = TempoMap {
            changes: vec![(0, 0.0, DEFAULT_TEMPO)],
            ticks_per_beat,
            ticks_per_second,
        };
        for (tick, tempo) in tempos {
            let seconds = map.seconds(tick);
            let last = map.changes.last_mut().unwrap();
            if last.0 == tick {
                last.2 = tempo;
            } else {
                map.changes.push((tick, seconds, tempo));
            }
        }
        map
    }

    fn seconds(&self, tick: u64) -> f64 {
        if let Some(ticks_per_second) = self.ticks_per_second {
            return tick as f64 / ticks_per_second;
        }
        // the last change at or before this tick, the first is always at tick 0
        let (start, seconds, tempo) = self.changes[self.changes.partition_point(|change| change.0 <= tick) - 1];
        seconds + (tick - start) as f64 * tempo * 1e-6 / self.ticks_per_beat
    }
}

// MARK: - MIDITrack

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MIDITrack {
    pub name: String,
    pub notes: Vec<MIDINote>,
    pub control_change: HashMap<u8, Vec<MIDIEvent>>,
    pub pitchwheel: Vec<MIDIEvent>,
    pub aftertouch: Vec<MIDIEvent>,
    // only used while parsing, and tuple keys can't be JSON
    #[serde(skip)]
    #[schemars(skip)]
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
            notes.push(note);
        } else {
            self.note_table.insert(key, vec![note]);
        }
    }

    pub fn add_note_off(&mut self, channel: u8, note_number: u8, _velocity: u8, time_off: f64) {
        // find matching note on message
        let key = (channel, note_number);
        if let Some(table) = self.note_table.get_mut(&key) {
            // assume the first note on message for this note is the one that matches with this note off
            // a note off without a note on (e.g. a doubled note off) is ignored
            if let Some(note_from_tb) = table.first_mut() {
                note_from_tb.time_off = time_off;

                // remove this note for this note number b/c we have the note off for this note
                let note = table.remove(0);

                // after note is deleted from the table, add it to the actual notes list
                self.notes.push(note);
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
        self.notes.is_empty() && self.control_change.is_empty() && self.pitchwheel.is_empty() && self.aftertouch.is_empty()
    }

    pub fn all_used_notes(&self) -> Vec<u8> {
        let mut used_notes: Vec<u8> = self.notes.iter().map(|note| note.note_number).collect();
        used_notes.sort_unstable(); // sort
        used_notes.dedup(); // remove duplicates
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MIDIFile {
    tracks: Vec<MIDITrack>,
}

impl MIDIFile {
    pub fn new(midi_file: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // instance variables
        let mut tracks: Vec<MIDITrack> = Vec::new();

        let bytes = std::fs::read(midi_file)?;
        let smf = Smf::parse(&bytes)?;

        let midi_type = smf.header.format;
        let mut midi_tracks: &mut Vec<MIDITrack> = &mut Vec::new();
        let mut temp_tracks: Vec<MIDITrack> = Vec::new();

        let tempo_map = TempoMap::new(&smf);

        if midi_type == midly::Format::Sequential {
            return Err("type 2 (sequential) MIDI files are not supported".into());
        }
        let mut new_tracks;
        if midi_type == midly::Format::SingleTrack {
            // Type 0
            // Tracks depend on MIDI channels for the different tracks
            // Instance in 16 blank MIDI tracks
            new_tracks = vec![MIDITrack::new(""); 16];
            midi_tracks = &mut new_tracks;
        } // Type 1 tracks are independent of MIDI channels
        let mut i = 0;
        for track in smf.tracks.iter() {
            let mut tick: u64 = 0;
            let mut current_channel: i32;
            let mut current_track = &mut MIDITrack::new("");

            if midi_type == midly::Format::SingleTrack {
                // Type 0
                current_track = &mut midi_tracks[0];
            } // type 1 is already handled

            for msg in track {
                tick += msg.delta.as_int() as u64;
                let time = tempo_map.seconds(tick);
                // channel messages
                if let TrackEventKind::Midi {
                    channel,
                    message: _,
                } = msg.kind
                {
                    current_channel = channel.as_int() as i32;
                    // only update type 0 tracks, as type 1 tracks are independent of channels
                    if midi_type == midly::Format::SingleTrack {
                        current_track = &mut midi_tracks[current_channel as usize];
                    }
                    if current_track.name.len() == 0 {
                        i += 1;
                        current_track.name = format!("Track {}", i);
                    }
                }
                // meta messages
                if let TrackEventKind::Meta(meta_message) = msg.kind {
                    // set track name
                    if let MetaMessage::TrackName(name) = meta_message {
                        current_track.name = String::from_utf8_lossy(name).into_owned();
                    }
                // midi messages
                } else if let TrackEventKind::Midi {
                    channel,
                    message,
                } = msg.kind
                {
                    if let MidiMessage::NoteOn {
                        key,
                        vel,
                    } = message
                    {
                        // velocity 0 note_on messages need to be note_off
                        if vel == 0 {
                            current_track.add_note_off(channel.as_int() as u8, key.as_int() as u8, vel.as_int() as u8, time as f64);
                        } else {
                            current_track.add_note_on(channel.as_int() as u8, key.as_int() as u8, vel.as_int() as u8, time as f64);
                        }
                    } else if let MidiMessage::NoteOff {
                        key,
                        vel,
                    } = message
                    {
                        current_track.add_note_off(channel.as_int() as u8, key.as_int() as u8, vel.as_int() as u8, time as f64);
                    } else if let MidiMessage::ProgramChange {
                        program,
                    } = message
                    {
                        // General MIDI name
                        let mut gm_name: String = gm_program_to_name(program.as_int() as i32);
                        // General MIDI channel 10 is reserved for drums
                        if channel.as_int() == 9 {
                            gm_name = "Drumset".to_string();
                        }
                        // If track name is empty or default, set it to the GM name (if it's a GM instrument)
                        if current_track.name.len() == 0 || (midi_type == midly::Format::SingleTrack && current_track.name == format!("Track {current_channel}", current_channel = channel.as_int())) {
                            current_track.name = gm_name.clone();
                        }
                    } else if let MidiMessage::Controller {
                        controller,
                        value,
                    } = message
                    {
                        current_track.add_control_change(controller.as_int() as u8, channel.as_int() as u8, value.as_int() as u8, time as f64);
                    } else if let MidiMessage::PitchBend {
                        bend,
                    } = message
                    {
                        current_track.add_pitchwheel(channel.as_int() as u8, bend.0.as_int() as f64 / 8192.0 - 1.0, time as f64);
                    } else if let MidiMessage::Aftertouch {
                        key,
                        vel,
                    } = message
                    {
                        // FIXME fix this, need to add key
                        current_track.add_aftertouch(channel.as_int() as u8, vel.as_int() as f64 / 127.0, time as f64);
                    }
                }

                // EOF for type 1
                if midi_type == midly::Format::Parallel {
                    if let TrackEventKind::Meta(meta_message) = msg.kind {
                        if let MetaMessage::EndOfTrack = meta_message {
                            // push current track to ins var tracks
                            temp_tracks.push(current_track.clone());
                        }
                    }
                }
            }
        }

        // EOF for type 0
        if midi_type == midly::Format::SingleTrack {
            // push all midi_tracks into temp_tracks
            for track in midi_tracks.iter() {
                temp_tracks.push(track.clone());
            }
        }

        for track in temp_tracks.iter() {
            if !track.is_empty() {
                // sort notes
                let mut notes = track.notes.clone();
                notes.sort_by(|a, b| a.partial_cmp(b).unwrap());

                let mut track = track.clone();
                track.notes = notes;

                // delete note table (temporary anyway) to comply with JSON serialization
                // you cannot serialize a HashMap with a tuple as a key
                track.note_table.clear();

                tracks.push(track.clone());
            }
        }

        Ok(MIDIFile {
            tracks,
        })
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
