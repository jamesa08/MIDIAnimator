use midly::{MetaMessage, MidiMessage, Smf, TrackEventKind};
use core::panic;
use std::collections::HashMap;
use std::fmt;

use crate::utils::{closest_tempo, gm_program_to_name};


// TODO eventually refactor into more of The Rust Way
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

fn tick2second(tick: u32, ticks_per_beat: f64, tempo: f64) -> f64 {
    let scale = tempo * 1e-6 / ticks_per_beat;
    return tick as f64 * scale;
}
// MARK: - MIDITrack

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
            if let Some(note_from_tb) = table.first_mut() {
                note_from_tb.time_off = time_off;
                
                // remove this note for this note number b/c we have the note off for this note
                let note = table.remove(0);
                
                // after note is deleted from the table, add it to the actual notes list
                self.notes.push(note);
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
        used_notes.sort_unstable();  // sort
        used_notes.dedup();  // remove duplicates
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
        // instance variables
        let mut tracks: Vec<MIDITrack> = Vec::new();

        let bytes = std::fs::read(midi_file)?;
        let smf = Smf::parse(&bytes)?;
        
        let midi_type = smf.header.format;
        let mut midi_tracks: &mut Vec<MIDITrack> = &mut Vec::new();
        let mut temp_tracks: Vec<MIDITrack> = Vec::new();
        
        let mut tempo_map: Vec<(f64, f64)> = Vec::new();
        
        let ticks_per_beat = match smf.header.timing {
            midly::Timing::Metrical(tpq) => tpq.as_int() as f64,  // normal ticks per beat
            midly::Timing::Timecode(fps, tpf) => {
                let ticks_per_second = (fps.as_f32() * tpf as f32) as u32;
                let beats_per_minute = 120;
                (ticks_per_second as f64 / beats_per_minute as f64 * 60.0) as f64
            }
        };

        if midi_type == midly::Format::Sequential {
            panic!("Type 2 / Sequential format are not supported!");
        }
        let mut new_tracks; 
        if midi_type == midly::Format::SingleTrack {
            // Type 0
            // Tracks depend on MIDI channels for the different tracks
            // Instance in 16 blank MIDI tracks
            new_tracks = vec![MIDITrack::new(""); 16];
            midi_tracks = &mut new_tracks;
        } else {
            // Type 1
            // Tracks are independent of MIDI channels
            
            // get tempo map first
            let mut time: f64 = 0.0;
            let mut current_tempo: f64 = 500000.0;

            for track in smf.tracks.iter() {
                for msg in track {
                    // TODO investigate if we need to add time first or last, also in main for loop
                    time += tick2second(msg.delta.as_int(), ticks_per_beat, current_tempo);
                    
                    if let TrackEventKind::Meta(MetaMessage::Tempo(tempo)) = msg.kind {
                        current_tempo = tempo.as_int() as f64;  // FIXME I think this is needed, not in original implementation
                        tempo_map.push((time, current_tempo));
                    }
                    // update time with new tempo
                }
            }
            println!("{:?}", tempo_map);
        }
        let mut i = 0;
        for track in smf.tracks.iter() {
            let mut time: f64 = 0.0;
            let mut tempo: f64 = 500000.0;
            let mut current_channel: i32;
            let mut current_track = &mut MIDITrack::new("");

            if midi_type == midly::Format::SingleTrack {
                // Type 0
                current_track = &mut midi_tracks[0];
            }  // type 1 is already handled

            for msg in track {
                time += tick2second(msg.delta.as_int(), ticks_per_beat, tempo);
                println!("{:?}", time);
                // channel messages
                if let TrackEventKind::Midi { channel, message: _ } = msg.kind {
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

                    if let MetaMessage::Tempo(new_tempo) = meta_message {
                        tempo = new_tempo.as_int() as f64;
                    }
                // midi messages
                } else if let TrackEventKind::Midi { channel, message } = msg.kind {
                    if let MidiMessage::NoteOn { key, vel } = message {
                        // velocity 0 note_on messages need to be note_off
                        if vel == 0 {
                            current_track.add_note_off(channel.as_int() as u8, key.as_int() as u8, vel.as_int() as u8, time as f64);
                        } else {
                            current_track.add_note_on(channel.as_int() as u8, key.as_int() as u8, vel.as_int() as u8, time as f64);
                        }
                    } else if let MidiMessage::NoteOff { key, vel } = message {
                        current_track.add_note_off(channel.as_int() as u8, key.as_int() as u8, vel.as_int() as u8, time as f64);
                    } else if let MidiMessage::ProgramChange { program } = message {
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
                    } else if let MidiMessage::Controller { controller, value } = message {
                        current_track.add_control_change(controller.as_int() as u8, channel.as_int() as u8, value.as_int() as u8, time as f64);
                    } else if let MidiMessage::PitchBend { bend } = message {
                        current_track.add_pitchwheel(channel.as_int() as u8, bend.0.as_int() as f64 / 8192.0 - 1.0, time as f64);
                    } else if let MidiMessage::Aftertouch { key, vel } = message {
                        // FIXME fix this, need to add key
                        current_track.add_aftertouch(channel.as_int() as u8, vel.as_int() as f64 / 127.0, time as f64);
                    }
                    if midi_type == midly::Format::Parallel {
                        tempo = closest_tempo(&tempo_map, time, false).1;
                        println!("{:?}", tempo);
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
                
                tracks.push(track.clone());
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
