use serde_json::Value;
use MIDIAnimator::scene_generics::KeyframePoint;
use MIDIAnimator::utils::fcurve::{FCurve, Segment};

// fixtures/blender_fcurves.json is from Blender 5.2 (--factory-startup, 24 fps): each case's keys written the way the
// scene writer writes them (keyframe_points.add, foreach_set "co", update) and read back, and two keys 0 -> 2 over one
// second with each easing, evaluated at every 1/8 second. times are seconds
fn truth() -> Value {
    serde_json::from_str(include_str!("fixtures/blender_fcurves.json")).unwrap()
}

const CASES: [(&str, &[(f64, f64)]); 5] = [("hit", &[(0.0, 0.0), (0.1, 1.0), (0.25, -0.3), (0.5, 0.0)]), ("wiggle", &[(0.0, 0.0), (1.0, 5.0), (2.0, 4.9), (3.0, 10.0), (4.0, 0.0), (4.5, 0.2), (6.0, -3.0)]), ("unsorted_duplicate", &[(1.0, 1.0), (0.0, 0.0), (1.0, 2.0), (2.0, 0.0)]), ("two", &[(0.0, 0.0), (1.0, 1.0)]), ("uneven", &[(0.0, 0.0), (0.05, 2.0), (1.0, 2.5), (1.02, -1.0), (3.0, 0.0)])];

fn close(a: f64, b: &Value) -> bool {
    (a - b.as_f64().unwrap()).abs() < 1e-4
}

#[test]
fn written_curves_get_blenders_handles() {
    let truth = truth();
    for (name, keys) in CASES {
        let curve = FCurve::written(keys.iter().copied());
        let expected = truth[name].as_array().unwrap();
        assert_eq!(curve.keys.len(), expected.len(), "{}: key count", name);
        for (i, (key, want)) in curve.keys.iter().zip(expected).enumerate() {
            for (got, want, part) in [(key.co, &want[0], "co"), (key.left, &want[1], "left"), (key.right, &want[2], "right")] {
                assert!(close(got[0], &want[0]) && close(got[1], &want[1]), "{} key {} {}: got {:?}, Blender has {}", name, i, part, got, want);
            }
        }
    }
}

fn point(time: f32, value: f32, interpolation: &str, easing: &str, case: &Value) -> KeyframePoint {
    KeyframePoint {
        amplitude: case["amplitude"].as_f64().unwrap() as f32,
        back: case["back"].as_f64().unwrap() as f32,
        easing: easing.to_string(),
        handle_left: vec![time, value],
        handle_left_type: "AUTO_CLAMPED".to_string(),
        handle_right: vec![time, value],
        handle_right_type: "AUTO_CLAMPED".to_string(),
        interpolation: interpolation.to_string(),
        co: vec![time, value],
        period: case["period"].as_f64().unwrap() as f32,
    }
}

#[test]
fn eased_segments_match_blender() {
    let truth = truth();
    for (id, case) in truth["eases"].as_object().unwrap() {
        let (interpolation, easing) = id.split_once('/').unwrap();
        let curve = FCurve::from_points(&[point(0.0, 0.0, interpolation, easing, case), point(1.0, 2.0, "BEZIER", "AUTO", case)]);
        let drawing = curve.drawing();
        let Segment::Points {
            points,
        } = &drawing.segments[0]
        else {
            panic!("{}: not sampled", id);
        };
        // the samples at each 1/8
        let step = points.len() / 8;
        for (q, want) in case["values"].as_array().unwrap().iter().enumerate() {
            let [time, value] = points[(q + 1) * step - 1];
            assert!((time - (q + 1) as f64 / 8.0).abs() < 1e-9, "{}: sample time", id);
            assert!(close(value, want), "{} at {}/8: got {}, Blender has {}", id, q + 1, value, want);
        }
    }
}

#[test]
fn constant_and_linear_extrapolation() {
    let flat = FCurve::written([(0.0, 0.0), (1.0, 1.0)]).drawing();
    assert_eq!((flat.slope_before, flat.slope_after), (0.0, 0.0));

    let mut linear = FCurve::written([(0.0, 0.0), (1.0, 1.0), (2.0, 3.0)]);
    linear.linear_extrapolation = true;
    let drawing = linear.drawing();
    // the end handles are flat, so it carries on flat
    assert_eq!((drawing.slope_before, drawing.slope_after), (0.0, 0.0));
}

#[test]
fn single_key_draws_no_segments() {
    let drawing = FCurve::written([(1.0, 4.0)]).drawing();
    assert_eq!(drawing.keys, vec![[1.0, 4.0]]);
    assert!(drawing.segments.is_empty());
}

// MARK: - Node curves

#[test]
fn combined_keyframes_split_into_curves_per_object() {
    use MIDIAnimator::graph::curves::node_curves;
    let key = |time: f64, value: f64, path: &str, index: u32| serde_json::json!({"time": time, "value": value, "data_path": path, "array_index": index});
    let outputs = serde_json::json!({"keyframes": {
        "Cube": [key(0.0, 0.0, "location", 2), key(1.0, 1.0, "location", 2), key(0.0, 0.0, "rotation_euler", 0), key(0.5, 2.0, "", 0)],
        "Empty": [],
    }});
    let channels = node_curves(Some(&outputs), None);
    let names: Vec<(&str, &str, Option<&str>)> = channels.iter().map(|c| (c.group.as_str(), c.name.as_str(), c.axis)).collect();
    assert_eq!(names, vec![("Cube", "Location Z", Some("Z")), ("Cube", "Rotation X", Some("X"))]);
    assert!(channels.iter().all(|c| c.extend && c.pieces.len() == 1));
    assert_eq!(channels[0].pieces[0].keys, vec![[0.0, 0.0], [1.0, 1.0]]);
}

#[test]
fn notes_keys_are_pieces_of_one_curve() {
    use MIDIAnimator::graph::curves::node_curves;
    let chunk = |time: f64| {
        serde_json::json!({"object": "Cube", "data_path": "location", "array_index": 0, "animation_overlap": "add", "overlap_blend": 0.1,
        "keyframes": [{"time": time, "value": 0.0, "data_path": "location", "array_index": 0}, {"time": time + 0.5, "value": 1.0, "data_path": "location", "array_index": 0}]})
    };
    let outputs = serde_json::json!({"results": [chunk(0.0), chunk(2.0)]});
    let channels = node_curves(Some(&outputs), None);
    assert_eq!(channels.len(), 1);
    assert_eq!(channels[0].pieces.len(), 2);
    assert!(!channels[0].extend);
}

#[test]
fn nodes_without_curve_outputs_show_their_inputs() {
    use MIDIAnimator::graph::curves::node_curves;
    let inputs = serde_json::json!({"keyframes": {"Cube": [{"time": 0.0, "value": 1.0, "data_path": "scale", "array_index": 1}]}, "max_depth": 2});
    let channels = node_curves(Some(&serde_json::json!({})), Some(&inputs));
    assert_eq!(channels.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), vec!["Scale Y"]);
    // an error result has nothing to draw
    assert!(node_curves(Some(&serde_json::json!({"motionkeys_error": "no"})), None).is_empty());
}

#[test]
fn a_generators_unset_curve_isnt_listed() {
    use MIDIAnimator::graph::curves::node_curves;
    use MIDIAnimator::utils::animation::AnimationGenerator;
    let case = serde_json::json!({"amplitude": 0.8, "back": 1.7, "period": 0.17});
    let generator = AnimationGenerator {
        name: "hit".to_string(),
        note_on_keyframes: vec![point(0.0, 0.0, "BEZIER", "AUTO", &case), point(1.0, 2.0, "BEZIER", "AUTO", &case)],
        ..Default::default()
    };
    let outputs = serde_json::json!({ "generator": generator });
    let channels = node_curves(Some(&outputs), None);
    assert_eq!(channels.iter().map(|c| (c.group.as_str(), c.name.as_str())).collect::<Vec<_>>(), vec![("hit", "Note On")]);
}
