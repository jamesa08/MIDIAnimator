// run from /src-tauri
// cargo test --test wave_test
// the Preset Animation node's shapes: baked keys follow their functions, rest where they should and turn flat
use motionkeys::graph::executors::animation::preset_animation;
use motionkeys::graph::executors::io::Inputs;
use motionkeys::graph::model::mode_default;
use motionkeys::scene_generics::AnimCurve;
use motionkeys::utils::curve::{check_keys, evaluate, slope, Key};
use motionkeys::utils::momentum::phases;
use motionkeys::utils::wave::{Cymbal, Drumstick, Spring, TOLERANCE};
use serde_json::json;

const FPS: f64 = 24.0;

fn spring() -> Spring {
    Spring {
        start: 0.0,
        end: 1.5,
        amplitude: 0.25,
        // 8 swings each way over its 1.5 seconds
        period: 0.375,
        dampen: 0.5,
        rest: 0.1,
        flip: false,
    }
}

// the node's cymbal at 60 fps: a 35 frame period over 240 frames, 6.1% detune
fn cymbal(angle: f64, detune: f64) -> Cymbal {
    Cymbal {
        spring: Spring {
            end: 4.0,
            period: 35.0 / 60.0,
            amplitude: 0.27,
            dampen: 0.42,
            ..spring()
        },
        angle: angle.to_radians(),
        detune,
        rest: [0.0, 0.2],
    }
}

fn drumstick() -> Drumstick {
    // the phases' lengths in frames: windup 8, hit 5, recoil 6, rest 8
    Drumstick {
        windup: 8.0 / FPS,
        windup_value: 0.6,
        hit: 5.0 / FPS,
        hit_value: -0.3,
        recoil: 6.0 / FPS,
        recoil_value: 0.15,
        settle: 8.0 / FPS,
        rest: 0.0,
    }
}

// the furthest the keys are from the function, sampled finely from a little before the keys to a little after
fn worst(keys: &[Key], value: impl Fn(f64) -> f64) -> f64 {
    let (a, b) = (keys[0].time - 0.1, keys.last().unwrap().time + 0.1);
    (0..=20_000).map(|i| a + (b - a) * i as f64 / 20_000.0).map(|t| (evaluate(keys, t).unwrap() - value(t)).abs()).fold(0.0, f64::max)
}

fn range(keys: &[Key]) -> f64 {
    let values = keys.iter().map(|k| k.value);
    values.clone().fold(f64::NEG_INFINITY, f64::max) - values.fold(f64::INFINITY, f64::min)
}

// MARK: - Spring

#[test]
fn a_spring_follows_its_damped_sine() {
    for dampen in [0.0, 0.5, 1.0] {
        let s = Spring {
            dampen,
            ..spring()
        };
        let keys = s.keys();
        assert_eq!(check_keys(&keys), None);
        // the tolerance, with a little room for the check sampling between the baker's samples
        assert!(worst(&keys, |t| s.value(t)) <= TOLERANCE * range(&keys) * 1.5, "dampen {dampen} strays {}", worst(&keys, |t| s.value(t)));
        // a few keys a swing, not a key a frame
        assert!(keys.len() <= 4 * 8 + 2, "dampen {dampen} has {} keys", keys.len());
    }
}

#[test]
fn a_spring_starts_and_ends_at_rest_and_flips() {
    let s = spring();
    let keys = s.keys();
    let (first, last) = (&keys[0], keys.last().unwrap());
    assert_eq!((first.time, first.value), (0.0, 0.1));
    assert!((last.time - 1.5).abs() < 1e-12 && (last.value - 0.1).abs() < 1e-12);
    // a whole number of swings ends still, its last handle is flat
    assert!(slope(&keys, 1.5 - 1e-6, true).abs() < 1e-3);
    // the first swing goes up, flipped it goes down
    let peak = keys[1].value;
    assert!(peak > 0.1);
    let flipped = Spring {
        flip: true,
        ..s
    }
    .keys();
    assert!((flipped[1].value - (0.2 - peak)).abs() < 1e-9);
}

#[test]
fn a_springs_turns_are_flat_keys() {
    let s = spring();
    let keys = s.keys();
    // every turn of the wave has a key with level handles on it
    let turns: Vec<&Key> = keys.iter().filter(|k| k.left[1] == k.value && k.right[1] == k.value && k.time > 0.0 && k.time < 1.5).collect();
    assert_eq!(turns.len(), 8);
}

// MARK: - Cymbal

#[test]
fn a_cymbal_follows_its_two_springs() {
    for (angle, detune) in [(45.0, 0.061), (0.0, 0.061), (35.0, 0.2), (200.0, 0.0)] {
        let c = cymbal(angle, detune);
        let [x, y] = c.keys();
        for (axis, keys) in [(0, &x), (1, &y)] {
            assert_eq!(check_keys(keys), None);
            let error = worst(keys, |t| c.value(axis, t));
            assert!(error <= TOLERANCE * range(keys).max(1e-9) * 1.5, "angle {angle} axis {axis} strays {error}");
        }
    }
}

#[test]
fn a_cymbal_at_45_degrees_is_a_spring_on_each_axis() {
    // like a hand keyed crash: each axis its own spring, the second one detune slower, each with the tilt's share
    let c = cymbal(45.0, 0.061);
    let share = c.spring.amplitude / 2f64.sqrt();
    let fast = Spring {
        amplitude: share,
        rest: 0.0,
        ..c.spring
    };
    let slow = Spring {
        period: c.spring.period * 1.061,
        ..fast
    };
    for i in 0..400 {
        let t = i as f64 * 0.01;
        assert!((c.value(0, t) - fast.value(t)).abs() < 1e-12);
        assert!((c.value(1, t) - 0.2 - slow.value(t)).abs() < 1e-12);
    }
}

#[test]
fn a_cymbal_turns_with_its_angle() {
    // turning the angle turns the whole crash: the tilt at every moment is the 0 degree tilt turned by it
    let flat = cymbal(0.0, 0.061);
    for angle in [30.0f64, 90.0, 225.0] {
        let turned = cymbal(angle, 0.061);
        let (sin, cos) = angle.to_radians().sin_cos();
        for i in 0..200 {
            let t = i as f64 * 0.02;
            let (x, y) = (flat.value(0, t), flat.value(1, t) - 0.2);
            assert!((turned.value(0, t) - (cos * x - sin * y)).abs() < 1e-12);
            assert!((turned.value(1, t) - 0.2 - (sin * x + cos * y)).abs() < 1e-12);
        }
    }
    // struck at 0 it first rocks only on the first curve, then the detune brings the second in
    let across = |from: f64, to: f64| {
        let (mut along, mut across) = (0.0f64, 0.0f64);
        for i in 0..=200 {
            let t = from + (to - from) * i as f64 / 200.0;
            along = along.max(flat.value(0, t).abs());
            across = across.max((flat.value(1, t) - 0.2).abs());
        }
        across / along
    };
    assert!(across(0.0, 0.3) < 0.15, "{}", across(0.0, 0.3));
    assert!(across(2.0, 3.0) > 0.6, "{}", across(2.0, 3.0));
    // without detune it only rocks, on a straight line
    let [_, y] = cymbal(0.0, 0.0).keys();
    assert!(y.iter().all(|k| (k.value - 0.2).abs() < 1e-12));
}

// MARK: - Drumstick

#[test]
fn a_drumstick_goes_through_the_ends_of_its_phases() {
    let keys = drumstick().keys();
    assert_eq!(check_keys(&keys), None);
    let points: Vec<(f64, f64)> = keys.iter().map(|k| (k.time * FPS, k.value)).collect();
    let expected = [(-13.0, 0.0), (-5.0, 0.6), (0.0, -0.3), (6.0, 0.15), (14.0, 0.0)];
    for ((t, v), (et, ev)) in points.iter().zip(expected) {
        assert!((t - et).abs() < 1e-9 && (v - ev).abs() < 1e-12);
    }
}

#[test]
fn a_drumstick_falls_like_a_throw_into_the_hit() {
    // from the still windup, the fall is a parabola: it speeds up evenly into the hit
    let d = drumstick();
    let keys = d.keys();
    for i in 1..20 {
        let t = -d.hit + d.hit * i as f64 / 20.0;
        let u = (t + d.hit) / d.hit;
        let parabola = d.windup_value + (d.hit_value - d.windup_value) * u * u;
        assert!((evaluate(&keys, t).unwrap() - parabola).abs() < 1e-9);
    }
    // and is knocked back up the same way, fastest right at the hit
    let knock = slope(&keys, 0.0, false);
    assert!((knock - 2.0 * (d.recoil_value - d.hit_value) / d.recoil).abs() < 1e-6);
}

#[test]
fn momentum_finds_a_drumsticks_windup_and_recoil() {
    // the generator's momentum overlap reads the phases off the curve, they land on the windup and recoil
    let d = drumstick();
    let [windup, recoil] = phases(&d.keys(), 0.0);
    assert!((windup + d.hit).abs() < 1e-6, "windup at {}", windup * FPS);
    assert!((recoil - d.recoil).abs() < 1e-6, "recoil at {}", recoil * FPS);
}

#[test]
fn a_drumstick_phase_without_time_says_which() {
    let d = Drumstick {
        hit: 0.0,
        ..drumstick()
    };
    assert_eq!(d.check(), Some("hit"));
    let d = Drumstick {
        settle: -1.0,
        ..drumstick()
    };
    assert_eq!(d.check(), Some("rest"));
    assert_eq!(drumstick().check(), None);
}

// MARK: - Node

fn curve(outputs: &serde_json::Value, handle: &str) -> AnimCurve {
    serde_json::from_value(outputs[handle].clone()).unwrap()
}

#[test]
fn preset_animation_defaults_to_a_spring_on_the_first_rotation_axis() {
    let outputs = preset_animation(&Inputs::default()).unwrap().to_json();
    let spring = curve(&outputs, "keyframes");
    assert_eq!(spring.channel, "rotation_euler[0]");
    // 36 frames at 24 fps, no scene
    assert!((spring.keys.last().unwrap().time - 1.5).abs() < 1e-12);
    assert!(outputs.get("keyframes_y").is_none());
}

#[test]
fn preset_animation_reads_its_mode_defaults_from_the_spec() {
    assert_eq!(mode_default("preset_animation", "end", "spring"), Some(36.0));
    assert_eq!(mode_default("preset_animation", "end", "cymbal"), Some(240.0));
    assert_eq!(mode_default("preset_animation", "period", "cymbal"), Some(35.0));
    assert_eq!(mode_default("preset_animation", "start", "cymbal"), None);
}

#[test]
fn preset_animation_cymbal_has_its_own_defaults() {
    // the cymbal's end is 240 frames, 10 seconds at 24 fps with no scene
    let outputs = preset_animation(&Inputs::from([("mode", json!("cymbal"))])).unwrap().to_json();
    assert!((curve(&outputs, "keyframes").keys.last().unwrap().time - 10.0).abs() < 1e-12);
    // a 35 frame period: its first turn is a little before a quarter of one, where ANIM_CYM's is (frame 8.15)
    let keys = curve(&outputs, "keyframes").keys;
    let first = keys.iter().find(|k| k.time > 0.0 && k.left[1] == k.value && k.right[1] == k.value).unwrap();
    assert!((first.time * FPS - 8.15).abs() < 0.1, "first turn at frame {}", first.time * FPS);
    // one typed in still wins
    let outputs = preset_animation(&Inputs::from([("mode", json!("cymbal")), ("end", json!(48.0))])).unwrap().to_json();
    assert!((curve(&outputs, "keyframes").keys.last().unwrap().time - 2.0).abs() < 1e-12);
}

#[test]
fn preset_animation_gives_a_cymbal_two_curves() {
    let outputs = preset_animation(&Inputs::from([("mode", json!("cymbal")), ("channel_y", json!(" rotation_euler[2] "))])).unwrap().to_json();
    assert_eq!(curve(&outputs, "keyframes").channel, "rotation_euler[0]");
    assert_eq!(curve(&outputs, "keyframes_y").channel, "rotation_euler[2]");
    // both on one channel would play over each other
    let error = preset_animation(&Inputs::from([("mode", json!("cymbal")), ("channel", json!("x")), ("channel_y", json!("x"))])).unwrap_err();
    assert_eq!(error, "both curves are on x, give the second one its own channel");
}

#[test]
fn preset_animation_drumstick_phases_are_frames() {
    let outputs = preset_animation(&Inputs::from([("mode", json!("drumstick")), ("channel", json!("location[2]")), ("windup", json!(10.0)), ("hit", json!(4.0))])).unwrap().to_json();
    let stick = curve(&outputs, "keyframes");
    assert_eq!(stick.channel, "location[2]");
    // the windup starts 14 frames before the hit and peaks 4 before it, the hit lands on the note
    assert!((stick.keys[0].time - (-14.0 / FPS)).abs() < 1e-12);
    assert!((stick.keys[1].time - (-4.0 / FPS)).abs() < 1e-12);
    assert_eq!(stick.keys[2].time, 0.0);
}

#[test]
fn preset_animation_errors_on_bad_settings() {
    let error = |inputs: Inputs| preset_animation(&inputs).unwrap_err();
    assert_eq!(error(Inputs::from([("mode", json!("bongo"))])), "unknown mode 'bongo', expected one of: spring, drumstick, cymbal");
    assert_eq!(error(Inputs::from([("start", json!(10.0)), ("end", json!(4.0))])), "the end (4) has to come after the start (10)");
    assert_eq!(error(Inputs::from([("period", json!(0.0))])), "the period has to be more than 0, got 0");
    assert_eq!(error(Inputs::from([("dampen", json!(2.0))])), "dampen goes from 0 to 1, got 2");
    assert_eq!(error(Inputs::from([("mode", json!("drumstick")), ("rest_length", json!(0.0))])), "the rest has to be longer than 0 frames");
}
