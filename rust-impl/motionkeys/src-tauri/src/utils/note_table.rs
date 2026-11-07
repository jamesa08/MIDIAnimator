// the Note Table node: a value per note, from a curve over the notes of a connected note map (Assign Notes to Objects'
// Note Map) or a range of notes, with values typed in over it. it gives the expression a mapper runs
// (utils/mapper.rs), so a table feeds Animation Generator's Time Mapper or Amplitude Mapper like a typed expression.
//
// the notes are spaced evenly along the curve: with notes 36, 38, 42 and 49 the curve's t is 0, 1/3, 2/3 and 1, so a
// table looks even across the objects however far apart their notes are

use std::collections::BTreeMap;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::utils::expr::{compile, Env, Program, Value};
use crate::utils::mapper::{format_number, NoteVars, DEFAULT_FPS, MAPPER_VARS};

/// the variables a table's curve can use: `t`, 0 at the first note and 1 at the last, and the note number
pub const CURVE_VARS: [&str; 2] = ["t", "note"];

/// how many points the preview's curve is drawn with
const PREVIEW_POINTS: usize = 61;

// MARK: - Settings

/// which mapper the table is for, the variable it changes: the key's time (x, frames) or its value (y)
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Target {
    #[default]
    Amplitude,
    Time,
}

impl Target {
    fn var(self) -> &'static str {
        match self {
            Target::Amplitude => "y",
            Target::Time => "x",
        }
    }
}

/// what a note's value does to the variable: multiply it, add to it, or replace it
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Operation {
    #[default]
    Multiply,
    Add,
    Set,
}

/// where the table's notes come from: the connected note map, or every note from `from` to `to`
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    #[default]
    Map,
    Range,
}

/// what a note that isn't in the table gets: left alone, the value 0, or `fallback_value`, each through the operation
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Fallback {
    #[default]
    Keep,
    Zero,
    Value,
}

/// a value typed in for one note, it replaces the curve's
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, schemars::JsonSchema)]
pub struct Override {
    pub note: u8,
    pub value: f64,
}

/// a Note Table's settings, as the node saves them. missing fields have their defaults
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, schemars::JsonSchema)]
#[serde(default)]
pub struct NoteTable {
    pub target: Target,
    pub operation: Operation,
    pub source: Source,
    /// the notes in range mode
    pub from: u8,
    pub to: u8,
    /// the preset the curve came from, "constant" gives every note `min`. only the app reads the others
    pub shape: String,
    /// an expression in `t` (0 to 1) and `note`, it's scaled from `min` to `max`
    pub curve: String,
    pub min: f64,
    pub max: f64,
    pub overrides: Vec<Override>,
    pub fallback: Fallback,
    pub fallback_value: f64,
}

impl Default for NoteTable {
    fn default() -> Self {
        Self {
            target: Target::Amplitude,
            operation: Operation::Multiply,
            source: Source::Map,
            from: 36,
            to: 47,
            shape: "linear".to_string(),
            curve: "t".to_string(),
            min: 0.0,
            max: 1.0,
            overrides: vec![],
            fallback: Fallback::Keep,
            fallback_value: 0.0,
        }
    }
}

impl NoteTable {
    fn constant(&self) -> bool {
        self.shape == "constant"
    }

    // the value of a note no row has, `None` for a set that leaves it alone (the variable itself)
    fn default_value(&self) -> Option<f64> {
        match (self.fallback, self.operation) {
            (Fallback::Keep, Operation::Multiply) => Some(1.0),
            (Fallback::Keep, Operation::Add) => Some(0.0),
            (Fallback::Keep, Operation::Set) => None,
            (Fallback::Zero, _) => Some(0.0),
            (Fallback::Value, _) => Some(if self.fallback_value.is_finite() {
                self.fallback_value
            } else {
                0.0
            }),
        }
    }
}

// MARK: - Rows

/// one note of the table: the objects it plays (from the note map), its value and whether it was typed in
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, schemars::JsonSchema)]
pub struct Row {
    pub note: u8,
    pub objects: Vec<String>,
    pub value: f64,
    pub overridden: bool,
}

/// what the node shows: its rows, and the curve between `min` and `max` sampled evenly from the first note to the last
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, schemars::JsonSchema)]
pub struct Preview {
    pub rows: Vec<Row>,
    pub curve: Vec<f64>,
}

/// the table's notes and the objects each one plays, in note order. a note map is read backwards, objects to notes
fn table_notes(table: &NoteTable, note_map: &BTreeMap<String, Vec<u8>>) -> Vec<(u8, Vec<String>)> {
    match table.source {
        Source::Range => {
            let (from, to) = (table.from.min(table.to).min(127), table.to.max(table.from).min(127));
            (from..=to).map(|note| (note, vec![])).collect()
        }
        Source::Map => {
            let mut notes: BTreeMap<u8, Vec<String>> = BTreeMap::new();
            for (object, object_notes) in note_map {
                for &note in object_notes.iter().filter(|n| **n <= 127) {
                    let objects = notes.entry(note).or_default();
                    if !objects.contains(object) {
                        objects.push(object.clone());
                    }
                }
            }
            notes.into_iter().collect()
        }
    }
}

/// the curve compiled, `None` for a constant table. errors start with "Curve: "
fn curve(table: &NoteTable) -> Result<Option<Program>, String> {
    if table.constant() {
        return Ok(None);
    }
    let text = table.curve.trim();
    if text.is_empty() {
        return Err("Curve: empty, type a curve in t like t ** 2".to_string());
    }
    compile(text, &Env::new(&CURVE_VARS)).map(Some).map_err(|e| format!("Curve: {}", e))
}

// the curve's value from min to max at `t`
fn curve_value(table: &NoteTable, program: &Program, t: f64, note: f64) -> Result<f64, String> {
    let shape = program.eval_number(&[Value::Float(t), Value::Float(note)]).map_err(|e| format!("Curve: {} (t = {})", e, format_number(t)))?;
    Ok(table.min + (table.max - table.min) * shape)
}

/// the table's rows: each note's value on the curve (spaced evenly from 0 to 1), or the one typed in for it
pub fn preview(table: &NoteTable, note_map: &BTreeMap<String, Vec<u8>>) -> Result<Preview, String> {
    let notes = table_notes(table, note_map);
    let program = curve(table)?;
    let overrides: BTreeMap<u8, f64> = table.overrides.iter().filter(|o| o.value.is_finite()).map(|o| (o.note, o.value)).collect();
    let last = notes.len().saturating_sub(1).max(1) as f64;

    // every note, a typed in value wins over the curve
    let mut rows = vec![];
    for (i, (note, objects)) in notes.iter().enumerate() {
        let value = match (overrides.get(note), &program) {
            (Some(value), _) => *value,
            (None, None) => table.min,
            (None, Some(program)) => curve_value(table, program, i as f64 / last, *note as f64)?,
        };
        rows.push(Row {
            note: *note,
            objects: objects.clone(),
            value,
            overridden: overrides.contains_key(note),
        });
    }

    // the curve from the first note to the last, for the preview
    let mut samples = vec![];
    if let (Some(program), Some((first, _)), Some((end, _))) = (&program, notes.first(), notes.last()) {
        for i in 0..PREVIEW_POINTS {
            let t = i as f64 / (PREVIEW_POINTS - 1) as f64;
            let note = *first as f64 + (*end as f64 - *first as f64) * t;
            samples.push(curve_value(table, program, t, note).unwrap_or(f64::NAN));
        }
    }
    Ok(Preview {
        rows,
        curve: samples
            .into_iter()
            .map(|v| {
                if v.is_finite() {
                    v
                } else {
                    0.0
                }
            })
            .collect(),
    })
}

// MARK: - Expression

// a value through the operation on the variable: `y * v`, `x + v`, or `v`. `compound` values get parentheses
fn apply(table: &NoteTable, value: &str, compound: bool) -> String {
    let var = table.target.var();
    match (table.operation, value) {
        (Operation::Set, _) => value.to_string(),
        (Operation::Multiply, "1") | (Operation::Add, "0") => var.to_string(),
        (Operation::Multiply, "0") => "0".to_string(),
        (Operation::Multiply, _) if compound => format!("{} * ({})", var, value),
        (Operation::Multiply, _) => format!("{} * {}", var, value),
        (Operation::Add, _) if !compound && value.starts_with('-') => format!("{} - {}", var, &value[1..]),
        (Operation::Add, _) if compound => format!("{} + ({})", var, value),
        (Operation::Add, _) => format!("{} + {}", var, value),
    }
}

// a condition matching exactly `notes` (sorted, at least one)
fn notes_condition(notes: &[u8]) -> String {
    let (first, last) = (notes[0], notes[notes.len() - 1]);
    if notes.len() == 1 {
        return format!("note == {}", first);
    }
    if (last - first) as usize + 1 == notes.len() {
        return format!("{} <= note <= {}", first, last);
    }
    // evenly spaced notes read better as a range
    let step = notes[1] - notes[0];
    if notes.len() > 3 && notes.windows(2).all(|w| w[1] - w[0] == step) {
        return format!("note in range({}, {}, {})", first, last as u16 + 1, step);
    }
    format!("note in [{}]", notes.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", "))
}

// `value(note)` as a formula when the notes are evenly spaced with values on a line, at least 3 of them
fn linear(values: &BTreeMap<u8, f64>) -> Option<(String, Vec<u8>)> {
    if values.len() < 3 {
        return None;
    }
    let notes: Vec<u8> = values.keys().copied().collect();
    let step = notes[1] - notes[0];
    if !notes.windows(2).all(|w| w[1] - w[0] == step) {
        return None;
    }
    let (n0, n1) = (notes[0] as f64, notes[notes.len() - 1] as f64);
    let (v0, v1) = (values[&notes[0]], values[&notes[notes.len() - 1]]);
    let slope = (v1 - v0) / (n1 - n0);
    let on_line = |note: u8, v: f64| (v - (v0 + (note as f64 - n0) * slope)).abs() <= 1e-9 * v.abs().max(1.0);
    if slope == 0.0 || !values.iter().all(|(&note, &v)| on_line(note, v)) {
        return None;
    }

    // the slope as a short number, or as a fraction when it repeats (1/11 reads better than 0.090909091)
    let short = format_number(slope);
    let exact = (short.parse::<f64>().unwrap_or(f64::NAN) - slope).abs() <= 1e-12 * slope.abs().max(1.0) && short.trim_start_matches('-').len() <= 8;
    let offset = if notes[0] == 0 {
        "note".to_string()
    } else {
        format!("(note - {})", notes[0])
    };
    let scaled = if exact {
        match short.as_str() {
            "1" => offset,
            _ => format!("{} * {}", offset, short),
        }
    } else {
        let (change, span) = (format_number(v1 - v0), format_number(n1 - n0));
        match change.as_str() {
            "1" => format!("{} / {}", offset, span),
            _ => format!("{} * {} / {}", offset, change, span),
        }
    };
    let text = match format_number(v0).as_str() {
        "0" => scaled,
        start => format!("{} + {}", start, scaled),
    };
    Some((text, notes))
}

// the curve written over the notes `first..=last` (one per note, nothing typed in): t is the note's place between them
fn curve_formula(table: &NoteTable, first: u8, last: u8) -> String {
    let t = format!("(note - {}) / {}", first, last - first);
    let word_t = Regex::new(r"\bt\b").expect("valid regex");
    let shape = match table.curve.trim() {
        "t" => t,
        curve => word_t.replace_all(curve, format!("({})", t).as_str()).into_owned(),
    };
    // from min to max
    let span = format_number(table.max - table.min);
    let scaled = match span.as_str() {
        "1" => shape,
        _ => format!("{} * ({})", span, shape),
    };
    match format_number(table.min).as_str() {
        "0" => scaled,
        min => format!("{} + {}", min, scaled),
    }
}

/// the expression the table gives, readable: notes sharing a value are grouped, evenly spaced values on a line are a
/// formula, a curve over notes next to each other is written out, anything else is a list lookup. never more than
/// three nested conditionals
pub fn expression(table: &NoteTable, preview: &Preview) -> String {
    let var = table.target.var();
    let values: BTreeMap<u8, f64> = preview.rows.iter().map(|r| (r.note, r.value)).collect();
    let default = table.default_value();
    let default_text = default.map(format_number);
    let otherwise = match &default_text {
        Some(text) => apply(table, text, false),
        None => var.to_string(),
    };

    // notes whose value isn't what every other note gets
    let changed: BTreeMap<u8, String> = values.iter().map(|(&note, &v)| (note, format_number(v))).filter(|(_, v)| Some(v) != default_text.as_ref()).collect();
    if changed.is_empty() {
        return otherwise;
    }

    // a line over the whole table
    if let Some((formula, notes)) = linear(&values) {
        return format!("{} if {} else {}", apply(table, &formula, true), notes_condition(&notes), otherwise);
    }

    // a few values, each with its notes
    let mut groups: Vec<(String, Vec<u8>)> = vec![];
    for (note, value) in &changed {
        match groups.iter_mut().find(|(v, _)| v == value) {
            Some((_, notes)) => notes.push(*note),
            None => groups.push((value.clone(), vec![*note])),
        }
    }
    if groups.len() <= 3 {
        let branches: Vec<String> = groups.iter().map(|(value, notes)| format!("{} if {} else ", apply(table, value, false), notes_condition(notes))).collect();
        return format!("{}{}", branches.concat(), otherwise);
    }

    // a curve over notes next to each other, nothing typed in: the curve itself, if it gives the same values
    let notes: Vec<u8> = values.keys().copied().collect();
    let (first, last) = (notes[0], notes[notes.len() - 1]);
    if !table.constant() && table.overrides.is_empty() && (last - first) as usize + 1 == notes.len() {
        let formula = format!("{} if {} <= note <= {} else {}", apply(table, &curve_formula(table, first, last), true), first, last, otherwise);
        if gives(&formula, table, &values) {
            return formula;
        }
    }

    // a lookup from the first to the last changed note, notes between without a row get the fallback (or the variable
    // itself, for a set that leaves them alone)
    let (first, last) = (*changed.keys().next().unwrap_or(&0), *changed.keys().last().unwrap_or(&0));
    let gap = default_text.clone().unwrap_or_else(|| var.to_string());
    let list: Vec<String> = (first..=last).map(|note| values.get(&note).map(|v| format_number(*v)).unwrap_or_else(|| gap.clone())).collect();
    let index = if first == 0 {
        "note".to_string()
    } else {
        format!("note - {}", first)
    };
    let lookup = format!("[{}][{}]", list.join(", "), index);
    format!("{} if {} <= note <= {} else {}", apply(table, &lookup, false), first, last, otherwise)
}

// whether `text` gives each note's value from the table, through the operation
fn gives(text: &str, table: &NoteTable, values: &BTreeMap<u8, f64>) -> bool {
    let Ok(program) = compile(text, &Env::new(&MAPPER_VARS)) else {
        return false;
    };
    values.iter().all(|(&note, &value)| {
        let (x, y) = (3.0, 0.75);
        let mut vars = NoteVars::standalone(note, 100, DEFAULT_FPS);
        vars.set_key(x / DEFAULT_FPS, y, DEFAULT_FPS);
        let expected = applied(table, x, y, Some(value));
        program.eval_number(&vars.values).is_ok_and(|got| (got - expected).abs() <= 1e-6 * expected.abs().max(1.0))
    })
}

/// what a key at frame `x` with value `y` becomes for a note with table value `value` (`None` is a set that leaves it
/// alone), the meaning the expression has to match
pub fn applied(table: &NoteTable, x: f64, y: f64, value: Option<f64>) -> f64 {
    let current = match table.target {
        Target::Time => x,
        Target::Amplitude => y,
    };
    match (table.operation, value) {
        (_, None) => current,
        (Operation::Multiply, Some(v)) => current * v,
        (Operation::Add, Some(v)) => current + v,
        (Operation::Set, Some(v)) => v,
    }
}

/// the default for notes no row has, as `applied` takes it
pub fn fallback(table: &NoteTable) -> Option<f64> {
    table.default_value()
}
