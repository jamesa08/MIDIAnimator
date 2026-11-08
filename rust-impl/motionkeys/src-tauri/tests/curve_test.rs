// run from /src-tauri
// cargo test --test curve_test
use motionkeys::scene_generics::AnimCurve;
use motionkeys::utils::animation::{combine_keyframes, finish_keys, OverlapSettings};
use motionkeys::utils::curve::{auto_handles, check_keys, drawing, evaluate, slope, sort_keys, Interpolation, Key, Segment};
use serde_json::json;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

// a bezier key with its handles
fn bezier(time: f64, value: f64, left: [f64; 2], right: [f64; 2]) -> Key {
    Key::with_handles(time, value, Interpolation::Bezier, left, right)
}

#[test]
fn curves_hold_past_their_ends() {
    let keys = [Key::linear(1.0, 2.0), Key::linear(2.0, 4.0)];
    assert_eq!(evaluate(&keys, 0.0), Some(2.0));
    assert_eq!(evaluate(&keys, 9.0), Some(4.0));
    assert_eq!(evaluate(&[], 1.0), None);
    assert_eq!(slope(&keys, 0.5, false), 0.0);
    assert_eq!(slope(&keys, 2.5, true), 0.0);
}

#[test]
fn linear_and_constant_segments() {
    let keys = [Key::linear(0.0, 0.0), Key::new(1.0, 2.0, Interpolation::Constant), Key::linear(2.0, 5.0)];
    assert!(close(evaluate(&keys, 0.25).unwrap(), 0.5));
    assert!(close(evaluate(&keys, 1.5).unwrap(), 2.0));
    assert!(close(evaluate(&keys, 2.0).unwrap(), 5.0));
    assert!(close(slope(&keys, 0.5, false), 2.0));
    // at a key, before is the segment ending there and after the one starting there
    assert!(close(slope(&keys, 1.0, true), 2.0));
    assert!(close(slope(&keys, 1.0, false), 0.0));
}

#[test]
fn a_bezier_with_handles_at_thirds_is_a_cubic_in_time() {
    // value = 3t² - 2t³ when the handle times are at a third and two thirds
    let keys = [bezier(0.0, 0.0, [-1.0 / 3.0, 0.0], [1.0 / 3.0, 0.0]), bezier(1.0, 1.0, [2.0 / 3.0, 1.0], [4.0 / 3.0, 1.0])];
    for t in [0.1, 0.25, 0.5, 0.8] {
        let expected = 3.0 * t * t - 2.0 * t * t * t;
        assert!(close(evaluate(&keys, t).unwrap(), expected), "{t}");
        assert!((slope(&keys, t, false) - (6.0 * t - 6.0 * t * t)).abs() < 1e-6, "{t}");
    }
}

#[test]
fn handles_on_their_keys_make_a_straight_line() {
    let keys = [Key::new(0.0, 0.0, Interpolation::Bezier), Key::new(2.0, 4.0, Interpolation::Bezier)];
    for t in [0.2, 1.0, 1.7] {
        assert!((evaluate(&keys, t).unwrap() - 2.0 * t).abs() < 1e-6, "{t}");
    }
}

#[test]
fn handles_that_overlap_in_time_keep_the_curve_a_function() {
    // both handles reach past the middle, the curve still only moves forward in time and stays between the keys' values
    let keys = [bezier(0.0, 0.0, [0.0, 0.0], [0.9, 0.0]), bezier(1.0, 1.0, [0.1, 1.0], [1.0, 1.0])];
    let mut last = -1.0;
    for i in 0..=100 {
        let value = evaluate(&keys, i as f64 / 100.0).unwrap();
        assert!(value >= last - 1e-12 && (0.0..=1.0).contains(&value), "{i}: {value}");
        last = value;
    }
}

#[test]
fn a_handle_longer_than_its_segment_is_shortened_toward_its_key() {
    // the right handle reaches 4 seconds into a 1 second segment, scaled to 1 second it's at (1, 0.5)
    let long = [bezier(0.0, 0.0, [0.0, 0.0], [4.0, 2.0]), bezier(1.0, 1.0, [1.0, 1.0], [1.0, 1.0])];
    let short = [bezier(0.0, 0.0, [0.0, 0.0], [1.0, 0.5]), bezier(1.0, 1.0, [1.0, 1.0], [1.0, 1.0])];
    for t in [0.2, 0.5, 0.9] {
        assert!(close(evaluate(&long, t).unwrap(), evaluate(&short, t).unwrap()), "{t}");
    }
    // the drawing shows the shortened handle, the same curve
    match &drawing(&long).segments[0] {
        Segment::Bezier {
            c1,
            ..
        } => assert_eq!(*c1, [1.0, 0.5]),
        other => panic!("{other:?}"),
    }
}

#[test]
fn wire_keys_send_handles_only_next_to_beziers() {
    let curve = AnimCurve {
        channel: "location[2]".to_string(),
        keys: vec![Key::linear(0.0, 0.0), bezier(1.0, 1.0, [0.9, 1.0], [1.2, 1.0]), Key::new(2.0, 0.0, Interpolation::Constant), Key::linear(3.0, 1.0)],
    };
    let json = serde_json::to_value(&curve).unwrap();
    assert_eq!(json, json!({"channel": "location[2]", "keys": [[0.0, 0.0, "linear"], [1.0, 1.0, "bezier", 0.9, 1.0, 1.2, 1.0], [2.0, 0.0, "constant", 2.0, 0.0, 2.0, 0.0], [3.0, 1.0, "linear"]]}));
    assert_eq!(serde_json::from_value::<AnimCurve>(json).unwrap(), curve);

    // a bezier segment needs both ends' handles
    for keys in [json!([[0.0, 0.0, "bezier"], [1.0, 1.0, "linear"]]), json!([[0.0, 0.0, "bezier", 0.0, 0.0, 0.3, 0.0], [1.0, 1.0, "linear"]]), json!([[0.0, 0.0, "smooth"]]), json!([[0.0, 0.0]])] {
        assert!(serde_json::from_value::<AnimCurve>(json!({"channel": "x", "keys": keys})).is_err(), "{keys}");
    }
}

#[test]
fn keys_are_checked_for_order_and_finite_numbers() {
    assert_eq!(check_keys(&[Key::linear(0.0, 0.0), Key::linear(1.0, 0.0)]), None);
    assert!(check_keys(&[Key::linear(1.0, 0.0), Key::linear(1.0, 0.0)]).is_some());
    assert!(check_keys(&[Key::linear(0.0, f64::NAN)]).is_some());

    // sorting keeps the last of keys at one time
    let mut keys = vec![Key::linear(2.0, 1.0), Key::linear(1.0, 5.0), Key::linear(1.0, 7.0)];
    sort_keys(&mut keys, 1e-6);
    assert_eq!(keys.iter().map(|k| (k.time, k.value)).collect::<Vec<_>>(), [(1.0, 7.0), (2.0, 1.0)]);
}

#[test]
fn made_keys_get_smooth_handles_flat_at_peaks() {
    let mut keys = vec![Key::linear(0.0, 0.0), Key::auto(1.0, 1.0), Key::auto(2.0, 3.0), Key::auto(3.0, 0.0)];
    auto_handles(&mut keys);
    // rising through: along the line between its neighbors, a third of the way to each
    let [l, r] = [keys[1].left, keys[1].right];
    assert!(close(l[0], 2.0 / 3.0) && close(l[1], 0.5) && close(r[0], 4.0 / 3.0) && close(r[1], 1.5), "{l:?} {r:?}");
    // a peak and the last key are level
    assert_eq!((keys[2].left[1], keys[2].right[1]), (3.0, 3.0));
    assert_eq!(keys[3].left[1], 0.0);
    assert!(keys[1..].iter().all(|k| k.interpolation == Interpolation::Bezier && !k.auto));
    // a key that wasn't made keeps its handles
    assert_eq!(keys[0], Key::linear(0.0, 0.0));
}

#[test]
fn add_leaves_a_note_over_a_resting_curve_as_it_is() {
    // the old curve rests at 0 where the note plays, the note's handles stay
    let mut old = vec![Key::linear(0.0, 0.0), Key::linear(1.0, 0.0), Key::linear(3.0, 0.0)];
    let note = vec![bezier(1.0, 0.0, [0.9, 0.0], [1.2, 0.0]), bezier(2.0, 1.0, [1.8, 1.0], [2.2, 1.0])];
    let mut next = note.clone();
    combine_keyframes("add", &OverlapSettings::default(), &mut old, &mut next).unwrap();
    let note_keys: Vec<&Key> = old.iter().filter(|k| k.time == 2.0).collect();
    assert_eq!(note_keys, [&note[1]]);
}

#[test]
fn add_tilts_handles_by_the_other_curves_slope() {
    // the old curve rises 1 per second under the note, the note's level handles tilt up with it
    let mut old = vec![Key::linear(0.0, 0.0), Key::linear(4.0, 4.0)];
    let mut next = vec![bezier(1.0, 0.0, [0.5, 0.0], [1.5, 0.0]), bezier(2.0, 0.0, [1.5, 0.0], [2.5, 0.0])];
    combine_keyframes("add", &OverlapSettings::default(), &mut old, &mut next).unwrap();
    let key = old.iter().find(|k| k.time == 2.0).unwrap();
    assert!(close(key.value, 2.0));
    assert!(close(key.left[1], 1.5) && close(key.right[1], 2.5), "{key:?}");
}

#[test]
fn max_keeps_the_winning_curves_own_keys() {
    // the note plays well above the old curve's tail, its keys keep their handles and the made ones get smooth handles
    let mut old = vec![Key::linear(0.0, 0.0), Key::linear(1.0, 0.5), Key::linear(3.0, 0.0)];
    let peak = bezier(2.0, 4.0, [1.7, 4.0], [2.3, 4.0]);
    let mut next = vec![bezier(1.5, 0.0, [1.4, 0.0], [1.6, 0.0]), peak.clone(), bezier(2.5, 0.0, [2.4, 0.0], [2.6, 0.0])];
    combine_keyframes("max", &OverlapSettings::default(), &mut old, &mut next).unwrap();
    finish_keys(&mut old);
    assert!(old.contains(&peak), "{old:?}");
    assert!(check_keys(&old).is_none());
}

#[test]
fn a_note_moves_and_scales_the_handles_with_its_keys() {
    use motionkeys::midi::MIDINote;
    use motionkeys::utils::animation::{note_curve_keys, AnimationGenerator};
    use motionkeys::utils::mapper::NoteContext;
    let gen: AnimationGenerator = serde_json::from_value(json!({
        "name": "hit", "note_on_anchor_point": 0.0, "note_off_keyframes": [], "note_off_anchor_point": 0.0, "velocity_intensity": 1.0, "animation_overlap": "add",
        "note_on_keyframes": [{"channel": "location[2]", "keys": [[0.0, 0.0, "bezier", -0.1, 0.0, 0.1, 0.5], [0.5, 1.0, "linear", 0.4, 1.0, 0.6, 1.0]]}]
    }))
    .unwrap();
    let note: MIDINote = serde_json::from_value(json!({ "channel": 0, "note_number": 60, "velocity": 127, "time_on": 2.0, "time_off": 3.0 })).unwrap();
    let chunks = note_curve_keys("Cube", &gen, &note, &NoteContext::default()).unwrap();
    assert_eq!(chunks[0].channel, "location[2]");
    let key = &chunks[0].keyframes[0];
    assert_eq!((key.time, key.left, key.right, key.interpolation), (2.0, [1.9, 0.0], [2.1, 0.5], Interpolation::Bezier));

    // half the velocity, half the values, the handles' too
    let soft: MIDINote = serde_json::from_value(json!({ "channel": 0, "note_number": 60, "velocity": 127, "time_on": 2.0, "time_off": 3.0 }))
        .map(|mut n: MIDINote| {
            n.velocity = 127 / 2;
            n
        })
        .unwrap();
    let key = &note_curve_keys("Cube", &gen, &soft, &NoteContext::default()).unwrap()[0].keyframes[0];
    assert!(close(key.right[1], 0.5 * 63.0 / 127.0), "{key:?}");
}

/// writes a combined curve as a write body, and its values sampled with `evaluate`, to the file in MK_CURVE_DUMP, for
/// checking against a bridge (run with `-- --ignored`)
#[test]
#[ignore]
fn dump_a_combined_curve_for_a_bridge() {
    let Ok(path) = std::env::var("MK_CURVE_DUMP") else {
        return;
    };
    // two notes overlapping in max mode, with bezier handles, so there are kept keys and made ones
    let note = |start: f64, peak: f64| vec![bezier(start, 0.0, [start - 0.05, 0.0], [start + 0.05, 0.0]), bezier(start + 0.2, peak, [start + 0.12, peak], [start + 0.28, peak]), Key::linear(start + 0.6, 0.0)];
    let mut keys = note(0.0, 1.0);
    let mut next = note(0.3, 2.0);
    combine_keyframes("max", &OverlapSettings::default(), &mut keys, &mut next).unwrap();
    let mut crossfade = note(1.2, 1.5);
    combine_keyframes("crossfade", &OverlapSettings::default(), &mut keys, &mut crossfade).unwrap();
    finish_keys(&mut keys);
    let samples: Vec<[f64; 2]> = (0..=200).map(|i| i as f64 * 0.01).map(|t| [t, evaluate(&keys, t).unwrap()]).collect();
    let curve = AnimCurve {
        channel: "location[2]".to_string(),
        keys,
    };
    let body = json!({ "write": { "mode": "replace", "objects": { "Cube": [curve] } }, "samples": samples });
    std::fs::write(path, body.to_string()).unwrap();
}

/// reads a bridge's curves reply and Blender's own values from the file in MK_BRIDGE_REPLY, and checks the curve
/// MotionKeys evaluates stays within 0.1% of the value range (run with `-- --ignored`)
#[test]
#[ignore]
fn a_bridges_converted_curve_matches_its_app() {
    let Ok(path) = std::env::var("MK_BRIDGE_REPLY") else {
        return;
    };
    let data: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let objects = serde_json::from_value(data["reply"]["objects"].clone()).unwrap();
    let curves = motionkeys::bridge::curves::read_curves(objects);
    let keys = &curves["Cube"][0].keys;
    let samples: Vec<(f64, f64)> = data["samples"].as_array().unwrap().iter().map(|s| (s[0].as_f64().unwrap(), s[1].as_f64().unwrap())).collect();
    let (lo, hi) = samples.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), (_, v)| (lo.min(*v), hi.max(*v)));
    let worst = samples.iter().map(|(t, v)| (evaluate(keys, *t).unwrap() - v).abs()).fold(0.0, f64::max);
    println!("{} keys, worst {worst} of range {}", keys.len(), hi - lo);
    assert!(worst <= 0.001 * (hi - lo), "{worst}");
}
