// run from /src-tauri
// cargo test --test mapper_test
//
// Animation Generator's time and amplitude mappers (src/utils/mapper.rs): what they do to a note's keys, the variables
// they get, their errors, and the expressions note tables turn into

use serde_json::{json, Value};

use motionkeys::graph::executors::animation::animation_generator;
use motionkeys::graph::executors::io::Inputs;
use motionkeys::midi::MIDINote;
use motionkeys::utils::animation::{combine_curve_keys, note_curve_keys, AnimationGenerator, BlendKeyframe};
use motionkeys::utils::mapper::NoteContext;
use motionkeys::utils::note_table::{self, Fallback, NoteTable};

// MARK: - Helpers

fn key(time: f64, value: f64) -> Value {
    json!([time, value, "linear"])
}

// a cymbal hit on location[2]: up to 1 at 0.1 s, back to rest at 0.5 s
fn hit() -> Value {
    json!([{ "channel": "location[2]", "keys": [key(0.0, 0.0), key(0.1, 1.0), key(0.5, 0.0)] }])
}

// a generator made by the node, with these inputs on top of the hit
fn generator(inputs: Value) -> Result<AnimationGenerator, String> {
    let mut all = json!({ "name": "crash", "note_on_keyframes": hit() });
    for (k, v) in inputs.as_object().unwrap() {
        all[k] = v.clone();
    }
    let inputs = Inputs::from(all.as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.clone())).collect::<std::collections::HashMap<_, _>>());
    let outputs = animation_generator(&inputs)?.to_json();
    Ok(serde_json::from_value(outputs["generator"].clone()).unwrap())
}

fn note(number: u8, velocity: u8, on: f64, off: f64) -> MIDINote {
    MIDINote::new(0, number, velocity, on, off)
}

// the keys every note gives on "Cymbal", combined with the generator's overlap mode
fn play(gen: &AnimationGenerator, notes: &[MIDINote], fps: Option<f64>) -> Result<Vec<BlendKeyframe>, String> {
    let mut chunks = vec![];
    for (index, n) in notes.iter().enumerate() {
        let ctx = NoteContext {
            notes,
            index: Some(index),
            fps,
        };
        chunks.extend(note_curve_keys("Cymbal", gen, n, &ctx)?);
    }
    let objects = ["Cymbal".to_string()];
    Ok(combine_curve_keys(objects.iter(), chunks)?.remove("Cymbal").unwrap())
}

// times and values rounded, curve keys are f32 so 0.1 comes back as 0.100000001
fn pairs(keys: &[BlendKeyframe]) -> Vec<(f64, f64)> {
    keys.iter().map(|k| ((k.key.time * 1e6).round() / 1e6, (k.key.value * 1e6).round() / 1e6)).collect()
}

// the value of the first key of a lone note, for reading what a variable is
fn first_value(amplitude: &str, notes: &[MIDINote], which: usize, fps: Option<f64>) -> f64 {
    let gen = generator(json!({ "amplitude_mapper": amplitude })).unwrap();
    let ctx = NoteContext {
        notes,
        index: Some(which),
        fps,
    };
    note_curve_keys("Cymbal", &gen, &notes[which], &ctx).unwrap()[0].keyframes[0].value
}

// MARK: - Cymbal choke

#[test]
fn cymbal_choke_expression_and_table_give_the_same_keys() {
    // the cymbal (46) and its choke note (52) on one cymbal: the choke's keys go to 0 and overlap next cuts the ringing
    // cymbal where the choke starts
    let notes = [note(46, 127, 0.0, 0.05), note(52, 127, 0.3, 0.35)];
    let expression = generator(json!({ "amplitude_mapper": "y if note == 46 else 0", "animation_overlap": "next" })).unwrap();
    let keys = pairs(&play(&expression, &notes, Some(24.0)).unwrap());
    assert_eq!(keys, vec![(0.0, 0.0), (0.1, 1.0), (0.3, 0.0), (0.4, 0.0), (0.8, 0.0)]);

    // the same choke from a Note Table: 46 keeps its value (x1), every other note is 0
    let table = NoteTable {
        shape: "constant".into(),
        min: 1.0,
        fallback: Fallback::Zero,
        ..Default::default()
    };
    let crash = [("Crash".to_string(), vec![46u8])].into_iter().collect();
    let text = note_table::expression(&table, &note_table::preview(&table, &crash).unwrap());
    assert_eq!(text, "y if note == 46 else 0");
    let tabled = generator(json!({ "amplitude_mapper": text, "animation_overlap": "next" })).unwrap();
    assert_eq!(pairs(&play(&tabled, &notes, Some(24.0)).unwrap()), keys);

    // without the mapper the choke would hit the cymbal again
    let plain = generator(json!({ "animation_overlap": "next" })).unwrap();
    assert_eq!(pairs(&play(&plain, &notes, Some(24.0)).unwrap())[3], (0.4, 1.0));
}

// MARK: - Time

#[test]
fn time_mapper_shifts_one_note_in_frames() {
    let notes = [note(36, 127, 0.0, 0.1), note(38, 127, 1.0, 1.1)];
    let gen = generator(json!({ "time_mapper": "x + 12 if note == 38 else x" })).unwrap();
    // 12 frames at 24 fps is half a second, the kick is left alone
    let keys = pairs(&play(&gen, &notes, Some(24.0)).unwrap());
    assert_eq!(keys, vec![(0.0, 0.0), (0.1, 1.0), (0.5, 0.0), (1.5, 0.0), (1.6, 1.0), (2.0, 0.0)]);
    // at 30 fps 12 frames is 0.4 s
    let keys = pairs(&play(&gen, &notes[1..], Some(30.0)).unwrap());
    assert_eq!(keys, vec![(1.4, 0.0), (1.5, 1.0), (1.9, 0.0)]);
    // no scene: 24 fps
    assert_eq!(pairs(&play(&gen, &notes[1..], None).unwrap())[0], (1.5, 0.0));
}

#[test]
fn time_mapper_frames_match_the_old_add_on() {
    // x is the key's frame on the reference curve: 0.1 s is frame 2.4 at 24 fps, x * 2 puts it at frame 4.8
    let gen = generator(json!({ "time_mapper": "x * 2" })).unwrap();
    let keys = pairs(&play(&gen, &[note(60, 127, 0.0, 0.1)], Some(24.0)).unwrap());
    assert_eq!(keys, vec![(0.0, 0.0), (0.2, 1.0), (1.0, 0.0)]);
}

#[test]
fn time_mapper_keys_out_of_order_are_sorted() {
    // reversing time flips the curve, the keys come out in time order
    let gen = generator(json!({ "time_mapper": "12 - x" })).unwrap();
    let keys = pairs(&play(&gen, &[note(60, 127, 1.0, 1.1)], Some(24.0)).unwrap());
    assert_eq!(keys, vec![(1.0, 0.0), (1.4, 1.0), (1.5, 0.0)]);
}

// MARK: - Order

#[test]
fn velocity_is_applied_after_the_amplitude_mapper() {
    // min(y, 0.5) caps the hit at 0.5, then velocity 64 scales it. the other way round 64/127 would be capped instead
    let gen = generator(json!({ "amplitude_mapper": "min(y, 0.5)", "velocity_intensity": 1.0 })).unwrap();
    let keys = play(&gen, &[note(60, 64, 0.0, 0.1)], Some(24.0)).unwrap();
    assert!((keys[1].key.value - 0.5 * 64.0 / 127.0).abs() < 1e-12, "{:?}", pairs(&keys));
}

#[test]
fn mappers_run_before_the_note_offset_and_anchor() {
    // x is relative to the curve: a note at 2 s with an anchor of 0.5 s still sees x = 0 for its first key
    let gen = generator(json!({ "amplitude_mapper": "x + 7", "note_on_anchor_point": 0.5 })).unwrap();
    let keys = play(&gen, &[note(60, 127, 2.0, 2.1)], Some(24.0)).unwrap();
    assert_eq!(pairs(&keys)[0], (2.5, 7.0));
}

#[test]
fn empty_mappers_change_nothing() {
    let notes = [note(60, 100, 0.0, 0.1), note(62, 50, 0.2, 0.3)];
    let plain = generator(json!({})).unwrap();
    let empty = generator(json!({ "time_mapper": "", "amplitude_mapper": "  " })).unwrap();
    let identity = generator(json!({ "time_mapper": "x", "amplitude_mapper": "y" })).unwrap();
    let keys = pairs(&play(&plain, &notes, Some(24.0)).unwrap());
    assert_eq!(pairs(&play(&empty, &notes, Some(24.0)).unwrap()), keys);
    assert_eq!(pairs(&play(&identity, &notes, Some(24.0)).unwrap()), keys);
    assert!(plain.mappers().unwrap().is_identity());
}

// MARK: - Variables

#[test]
fn variables_from_the_track() {
    // a, then b and c together while a still rings, then d alone
    let notes = [note(60, 90, 0.0, 1.0), note(62, 80, 0.5, 0.75), note(64, 70, 0.5, 2.0), note(65, 60, 3.0, 3.5)];
    let fps = Some(24.0);
    // note length and start
    assert_eq!(first_value("dur", &notes, 0, fps), 24.0);
    assert_eq!(first_value("dur_sec", &notes, 1, fps), 0.25);
    assert_eq!(first_value("t", &notes, 3, fps), 72.0);
    assert_eq!(first_value("t_sec", &notes, 3, fps), 3.0);
    assert_eq!(first_value("dur", &notes, 0, Some(30.0)), 30.0);
    // time since the previous note started, inf for the first
    assert_eq!(first_value("gap", &notes, 1, fps), 12.0);
    assert_eq!(first_value("gap", &notes, 2, fps), 0.0);
    assert_eq!(first_value("gap_sec", &notes, 3, fps), 2.5);
    assert_eq!(first_value("1 if gap == inf else 0", &notes, 0, fps), 1.0);
    // the notes around it, -1 at the ends
    assert_eq!(first_value("prev", &notes, 0, fps), -1.0);
    assert_eq!(first_value("prev", &notes, 2, fps), 62.0);
    assert_eq!(first_value("next", &notes, 2, fps), 65.0);
    assert_eq!(first_value("next", &notes, 3, fps), -1.0);
    assert_eq!(first_value("index", &notes, 2, fps), 2.0);
    // notes sounding at its start, itself included: a, b and c at 0.5 s
    assert_eq!(first_value("held", &notes, 0, fps), 1.0);
    assert_eq!(first_value("held", &notes, 1, fps), 3.0);
    assert_eq!(first_value("held", &notes, 2, fps), 3.0);
    assert_eq!(first_value("held", &notes, 3, fps), 1.0);
    // the note itself
    assert_eq!(first_value("note + vel / 1000", &notes, 1, fps), 62.08);
    assert_eq!(first_value("octave", &notes, 0, fps), 4.0);
    assert_eq!(first_value("pc", &notes, 2, fps), 4.0);
    assert_eq!(first_value("fps", &notes, 0, Some(29.97)), 29.97);
    assert_eq!(first_value("fps", &notes, 0, None), 24.0);
    // the key: its frame and seconds and value
    assert_eq!(first_value("frames + sec + y + 1", &notes, 0, fps), 1.0);
}

#[test]
fn the_index_is_found_when_it_isnt_given() {
    // chord notes start together, the right one is found by all of its fields
    let notes = [note(60, 90, 0.0, 1.0), note(64, 80, 0.0, 1.0), note(67, 70, 0.0, 1.0)];
    let gen = generator(json!({ "amplitude_mapper": "index * 10 + prev" })).unwrap();
    let ctx = NoteContext {
        notes: &notes,
        index: None,
        fps: None,
    };
    let value = note_curve_keys("Cymbal", &gen, &notes[2], &ctx).unwrap()[0].keyframes[0].value;
    assert_eq!(value, 20.0 + 64.0);
    // a note on its own: no track
    let alone = note_curve_keys("Cymbal", &gen, &notes[2], &NoteContext::default()).unwrap()[0].keyframes[0].value;
    assert_eq!(alone, -1.0);
}

// MARK: - Errors

#[test]
fn compile_errors_fail_the_generator_and_name_the_mapper() {
    assert_eq!(generator(json!({ "amplitude_mapper": "y if nte == 46 else 0" })).unwrap_err(), "Amplitude Mapper: unknown name 'nte', did you mean 'note'?");
    assert_eq!(generator(json!({ "time_mapper": "x +" })).unwrap_err(), "Time Mapper: expected a value, found the end");
    assert_eq!(generator(json!({ "time_mapper": "lambda: x" })).unwrap_err(), "Time Mapper: not supported: lambda");
    assert_eq!(generator(json!({ "time_mapper": 5 })).unwrap_err(), "Time Mapper must be text, got 5");
}

#[test]
fn run_time_errors_name_the_mapper_and_the_note() {
    let gen = generator(json!({ "amplitude_mapper": "y / (note - 46)" })).unwrap();
    assert!(play(&gen, &[note(47, 127, 0.0, 0.1)], None).is_ok());
    let error = play(&gen, &[note(46, 127, 0.0, 0.1)], None).unwrap_err();
    assert_eq!(error, "Amplitude Mapper: division by zero (note 46, key at frame 0)");
    // an index past the list
    let gen = generator(json!({ "time_mapper": "x + [0, 1][note - 60]" })).unwrap();
    assert!(play(&gen, &[note(62, 127, 0.0, 0.1)], None).unwrap_err().starts_with("Time Mapper: list index out of range"));
    // inf and NaN never become keys
    let gen = generator(json!({ "amplitude_mapper": "gap" })).unwrap();
    assert_eq!(play(&gen, &[note(60, 127, 0.0, 0.1)], None).unwrap_err(), "Amplitude Mapper: result is inf, not a finite number (note 60, key at frame 0)");
}

// MARK: - Saved data

#[test]
fn a_mapper_saved_inside_an_object_reads_its_expression() {
    // how the mappers were saved for a while
    let gen = generator(json!({ "amplitude_mapper": { "mode": "expression", "rows": [], "expression": "y * 2" } })).unwrap();
    assert_eq!(gen.amplitude_mapper, "y * 2");
}
