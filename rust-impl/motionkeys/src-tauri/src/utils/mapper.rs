// the time and amplitude mappers of Animation Generator, like the old add-on's x and y mappers (instruments.py): each
// key of a note's curves goes through them before the note's offset is added and before velocity intensity. a mapper is
// an expression (utils/expr.rs), typed in or made by a Note Table (utils/note_table.rs).
//
// times are frames like in the old add-on, so expressions written for it keep working. motionkeys keeps seconds, the
// scene's fps converts (24 when no scene is linked)

use serde::{Deserialize, Serialize};

use crate::midi::MIDINote;
use crate::utils::animation::sec_to_frames;
use crate::utils::expr::{self, compile, Env, ExprError, Program, Value};

/// the frame rate mappers use when there's no scene to take it from
pub const DEFAULT_FPS: f64 = 24.0;

/// the variables both mappers have, in the order their values are given
pub const MAPPER_VARS: [&str; 19] = ["x", "frames", "sec", "y", "note", "vel", "dur", "dur_sec", "t", "t_sec", "octave", "pc", "index", "gap", "gap_sec", "prev", "next", "held", "fps"];

// where each variable is in MAPPER_VARS
const X: usize = 0;
const FRAMES: usize = 1;
const SEC: usize = 2;
const Y: usize = 3;
const NOTE: usize = 4;
const VEL: usize = 5;
const DUR: usize = 6;
const DUR_SEC: usize = 7;
const T: usize = 8;
const T_SEC: usize = 9;
const OCTAVE: usize = 10;
const PC: usize = 11;
const INDEX: usize = 12;
const GAP: usize = 13;
const GAP_SEC: usize = 14;
const PREV: usize = 15;
const NEXT: usize = 16;
const HELD: usize = 17;
const FPS: usize = 18;

// the variables that change with the scene's fps
const FRAME_VARS: [&str; 6] = ["x", "frames", "dur", "t", "gap", "fps"];

// MARK: - Mappers

/// which mapper: the time mapper gives a key's new time in frames, the amplitude mapper its new value
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum MapperKind {
    Time,
    Amplitude,
}

impl MapperKind {
    /// its name on the node, errors start with it
    pub fn name(self) -> &'static str {
        match self {
            MapperKind::Time => "Time Mapper",
            MapperKind::Amplitude => "Amplitude Mapper",
        }
    }
}

/// compiles a mapper with the mapper variables, `None` when it's empty (it changes nothing). errors name the mapper
pub fn compile_mapper(text: &str, kind: MapperKind) -> Result<Option<Program>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    compile(text, &Env::new(&MAPPER_VARS)).map(Some).map_err(|e| format!("{}: {}", kind.name(), e))
}

/// a number written short: rounded to 9 decimals so float noise like 0.30000000000000004 goes, no trailing zeros
pub fn format_number(value: f64) -> String {
    let text = format!("{:.9}", value);
    let text = text.trim_end_matches('0').trim_end_matches('.');
    match text {
        "-0" | "" => "0".to_string(),
        other => other.to_string(),
    }
}

// MARK: - Running

/// the notes around a note, for the mapper variables that need them (index, gap, prev, next, held)
#[derive(Debug, Clone, Copy, Default)]
pub struct NoteContext<'a> {
    /// every note of the track the note is in, in order
    pub notes: &'a [MIDINote],
    /// where the note is in `notes`, found by looking when it isn't given
    pub index: Option<usize>,
    /// the scene's frames per second, `DEFAULT_FPS` when there's no scene
    pub fps: Option<f64>,
}

impl NoteContext<'_> {
    /// the fps mappers run with
    pub fn fps(&self) -> f64 {
        self.fps.filter(|fps| fps.is_finite() && *fps > 0.0).unwrap_or(DEFAULT_FPS)
    }
}

// the same note, every field. MIDINote's == only compares start times, the notes of a chord are equal by it
fn same_note(a: &MIDINote, b: &MIDINote) -> bool {
    a.channel == b.channel && a.note_number == b.note_number && a.velocity == b.velocity && a.time_on == b.time_on && a.time_off == b.time_off
}

/// the mapper variables' values for one note, the key's own (x, y, ...) are set per key
#[derive(Debug, Clone)]
pub struct NoteVars {
    pub values: Vec<Value>,
}

impl NoteVars {
    /// a note with nothing around it
    pub fn standalone(note: u8, velocity: u8, fps: f64) -> Self {
        let mut values = vec![Value::Float(0.0); MAPPER_VARS.len()];
        values[NOTE] = Value::Int(note as i64);
        values[VEL] = Value::Int(velocity as i64);
        values[OCTAVE] = Value::Int(note as i64 / 12 - 1);
        values[PC] = Value::Int(note as i64 % 12);
        values[INDEX] = Value::Int(0);
        values[GAP] = Value::Float(f64::INFINITY);
        values[GAP_SEC] = Value::Float(f64::INFINITY);
        values[PREV] = Value::Int(-1);
        values[NEXT] = Value::Int(-1);
        values[HELD] = Value::Int(1);
        values[FPS] = Value::Float(fps);
        Self {
            values,
        }
    }

    /// a note's values. `held` is only counted when `count_held` is set, it looks at every note in the track
    pub fn new(note: &MIDINote, ctx: &NoteContext, count_held: bool) -> Self {
        let fps = ctx.fps();
        let mut vars = Self::standalone(note.note_number, note.velocity, fps);
        let v = &mut vars.values;
        let duration = note.time_off - note.time_on;
        v[DUR] = Value::Float(sec_to_frames(duration, fps));
        v[DUR_SEC] = Value::Float(duration);
        v[T] = Value::Float(sec_to_frames(note.time_on, fps));
        v[T_SEC] = Value::Float(note.time_on);

        // the notes before and after it in the track
        let index = ctx.index.filter(|&i| ctx.notes.get(i).is_some_and(|n| same_note(n, note))).or_else(|| ctx.notes.iter().position(|n| same_note(n, note)));
        if let Some(i) = index {
            v[INDEX] = Value::Int(i as i64);
            if let Some(prev) = i.checked_sub(1).and_then(|p| ctx.notes.get(p)) {
                let gap = note.time_on - prev.time_on;
                v[GAP] = Value::Float(sec_to_frames(gap, fps));
                v[GAP_SEC] = Value::Float(gap);
                v[PREV] = Value::Int(prev.note_number as i64);
            }
            if let Some(next) = ctx.notes.get(i + 1) {
                v[NEXT] = Value::Int(next.note_number as i64);
            }
        }

        // the notes sounding when it starts, itself included
        if count_held {
            let others = ctx.notes.iter().enumerate().filter(|(j, n)| Some(*j) != index && n.time_on <= note.time_on && note.time_on < n.time_off).count();
            v[HELD] = Value::Int(others as i64 + 1);
        }
        vars
    }

    /// the key's time (seconds, relative to the curve) and value
    pub fn set_key(&mut self, time: f64, value: f64, fps: f64) {
        let frames = sec_to_frames(time, fps);
        self.values[X] = Value::Float(frames);
        self.values[FRAMES] = Value::Float(frames);
        self.values[SEC] = Value::Float(time);
        self.values[Y] = Value::Float(value);
    }
}

/// both mappers of a generator, compiled once
#[derive(Debug, Default)]
pub struct Mappers {
    pub time: Option<Program>,
    pub amplitude: Option<Program>,
}

impl Mappers {
    pub fn compile(time: &str, amplitude: &str) -> Result<Self, String> {
        Ok(Self {
            time: compile_mapper(time, MapperKind::Time)?,
            amplitude: compile_mapper(amplitude, MapperKind::Amplitude)?,
        })
    }

    /// whether keys go through unchanged
    pub fn is_identity(&self) -> bool {
        self.time.is_none() && self.amplitude.is_none()
    }

    /// whether either mapper uses the variable
    pub fn uses(&self, var: &str) -> bool {
        self.time.iter().chain(self.amplitude.iter()).any(|p| p.uses(var))
    }

    /// whether what the mappers give depends on the fps: a time mapper always gives frames, an amplitude mapper only
    /// when it reads a frame variable
    pub fn needs_fps(&self) -> bool {
        self.time.is_some() || FRAME_VARS.iter().any(|var| self.amplitude.as_ref().is_some_and(|p| p.uses(var)))
    }

    /// a key's time (seconds, relative to its curve) and value after the mappers
    pub fn map(&self, vars: &mut NoteVars, time: f64, value: f64, fps: f64) -> Result<(f64, f64), String> {
        vars.set_key(time, value, fps);
        let fail = |kind: MapperKind, e: ExprError| format!("{}: {} (note {}, key at frame {})", kind.name(), e, vars.values[NOTE], format_number(sec_to_frames(time, fps)));
        // both mappers see the key as it was, like the old add-on
        let time = match &self.time {
            Some(program) => program.eval_number(&vars.values).map_err(|e| fail(MapperKind::Time, e))? / fps,
            None => time,
        };
        let value = match &self.amplitude {
            Some(program) => program.eval_number(&vars.values).map_err(|e| fail(MapperKind::Amplitude, e))?,
            None => value,
        };
        Ok((time, value))
    }
}

// MARK: - Commands

/// checks an expression while it's typed, `None` when it compiles. empty is fine, it changes nothing
#[tauri::command]
pub fn check_expression(text: String, vars: Vec<String>) -> Option<ExprError> {
    if text.trim().is_empty() {
        return None;
    }
    let vars: Vec<&str> = vars.iter().map(String::as_str).collect();
    expr::check(&text, &vars)
}

/// the variables mapper expressions can use
#[tauri::command]
pub fn mapper_variables() -> Vec<&'static str> {
    MAPPER_VARS.to_vec()
}
