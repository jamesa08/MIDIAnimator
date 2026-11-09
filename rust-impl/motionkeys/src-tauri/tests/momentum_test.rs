// run from /src-tauri
// cargo test --test momentum_test
// the momentum overlap on keyed references of different kinds (percussion, scale bounces, swings, plucks, elastic
// rings, a curve baked to a key every frame), checked against what it promises at every gap
use motionkeys::graph::executors::animation::animation_generator;
use motionkeys::graph::executors::io::Inputs;
use motionkeys::utils::animation::{combine_curve_keys, finish_keys, CurveKeys};
use motionkeys::utils::curve::{auto_handles, check_keys, evaluate, slope, split_at, Interpolation, Key};
use motionkeys::utils::momentum::{momentum_keys, MomentumNote};
use serde_json::json;

const FPS: f64 = 24.0;
// sampling step, seconds
const DT: f64 = 0.05 / FPS;

// a reference keyed in frames around its hit at 0, (frame, value, sharp). sharp keys get vector handles (straight at
// their neighbors, a drumstick's hit), the rest blender's auto clamped ones
fn reference(points: &[(f64, f64, bool)]) -> Vec<Key> {
    let mut keys: Vec<Key> = points.iter().map(|&(frame, value, _)| Key::auto(frame / FPS, value)).collect();
    auto_handles(&mut keys);
    for (i, &(_, _, sharp)) in points.iter().enumerate() {
        if !sharp {
            continue;
        }
        let (time, value) = (keys[i].time, keys[i].value);
        if i > 0 {
            let (t, v) = (keys[i - 1].time, keys[i - 1].value);
            keys[i].left = [time + (t - time) / 3.0, value + (v - value) / 3.0];
        }
        if i + 1 < keys.len() {
            let (t, v) = (keys[i + 1].time, keys[i + 1].value);
            keys[i].right = [time + (t - time) / 3.0, value + (v - value) / 3.0];
        }
    }
    keys
}

// a damped ring: a key on every half period, alternating, from `start`
fn ring(amp: f64, period: f64, tau: f64, cycles: usize, start: f64) -> Vec<(f64, f64, bool)> {
    (1..=cycles * 2)
        .map(|k| {
            let u = start + k as f64 * period / 2.0;
            let sign = if k % 2 == 1 {
                -1.0
            } else {
                1.0
            };
            (u, sign * amp * (-(u - start) / tau).exp(), false)
        })
        .collect()
}

fn references() -> Vec<(&'static str, Vec<Key>)> {
    let mut spring = vec![(-2.0, 0.0, false), (0.0, 1.0, true)];
    spring.extend(ring(1.0, 8.0, 10.0, 4, 0.0));
    spring.push((40.0, 0.0, false));
    let mut antenna = vec![(-1.0, 0.0, false), (0.0, 0.0, false), (2.0, 25.0, false)];
    antenna.extend(ring(25.0, 6.0, 14.0, 5, 2.0));
    antenna.push((40.0, 0.0, false));
    let baked: Vec<(f64, f64, bool)> = (-2..=40)
        .map(|u| {
            let u = u as f64;
            let v = if u < 0.0 {
                if u == -2.0 {
                    0.0
                } else {
                    0.5
                }
            } else {
                (-u / 10.0).exp()
                    * (2.0 * std::f64::consts::PI * u / 8.0).cos()
                    * if u >= 38.0 {
                        (40.0 - u) / 2.0
                    } else {
                        1.0
                    }
            };
            (u, v, u == 0.0)
        })
        .collect();
    vec![
        ("drumstick", reference(&[(-11.0, 20.0, false), (-4.0, 60.0, false), (0.0, 0.0, true), (5.0, 30.0, false), (13.0, 20.0, false)])),
        ("drum wobble", reference(&[(-11.0, 20.0, false), (-4.0, 60.0, false), (0.0, 0.0, true), (4.0, 30.0, false), (8.0, 15.0, false), (12.0, 24.0, false), (17.0, 20.0, false)])),
        ("horn scale", reference(&[(-6.0, 1.0, false), (-3.0, 0.9, false), (0.0, 1.25, false), (4.0, 0.95, false), (8.0, 1.04, false), (12.0, 1.0, false)])),
        ("piano key", reference(&[(-2.0, 0.0, false), (0.0, -5.0, true), (8.0, -5.0, false), (14.0, 0.0, false)])),
        ("gong", reference(&[(-3.0, 0.0, false), (0.0, 0.0, false), (6.0, 20.0, false), (14.0, -14.0, false), (22.0, 9.0, false), (30.0, -5.0, false), (40.0, 2.0, false), (48.0, 0.0, false)])),
        ("pluck", reference(&[(-1.0, 0.0, false), (0.0, 1.0, true), (3.0, -0.6, false), (6.0, 0.35, false), (9.0, -0.15, false), (14.0, 0.0, false)])),
        ("laser tilt", reference(&[(-16.0, 0.0, false), (0.0, 45.0, false), (4.0, 40.0, false), (10.0, 0.0, false)])),
        ("spring", reference(&spring)),
        ("elastic in out", reference(&[(-14.0, 0.0, false), (-11.0, 0.04, false), (-8.0, -0.08, false), (-5.0, 0.16, false), (-2.0, -0.3, false), (0.0, 1.0, true), (3.0, -0.55, false), (6.0, 0.3, false), (9.0, -0.16, false), (12.0, 0.09, false), (15.0, -0.05, false), (20.0, 0.0, false)])),
        ("jelly", reference(&[(-8.0, 1.0, false), (-4.0, 0.85, false), (0.0, 1.35, false), (4.0, 0.8, false), (8.0, 1.15, false), (12.0, 0.92, false), (16.0, 1.04, false), (20.0, 0.98, false), (26.0, 1.0, false)])),
        ("antenna", reference(&antenna)),
        ("baked spring", reference(&baked)),
    ]
}

// the reference moved to a note at `hit`
fn at_hit(reference: &[Key], hit: f64) -> Vec<Key> {
    reference.iter().map(|key| key.moved(hit)).collect()
}

fn notes(reference: &[Key], hits: &[f64], dampen: f64, use_anchor_value: bool) -> Vec<MomentumNote> {
    hits.iter()
        .map(|&hit| MomentumNote {
            hit,
            keys: at_hit(reference, hit),
            dampen,
            use_anchor_value,
        })
        .collect()
}

fn combined(reference: &[Key], hits: &[f64], dampen: f64, use_anchor_value: bool) -> Vec<Key> {
    let mut keys = momentum_keys(notes(reference, hits, dampen, use_anchor_value));
    finish_keys(&mut keys);
    keys
}

// how a curve moves between two times: fastest, lowest, highest
struct Motion {
    speed: f64,
    lo: f64,
    hi: f64,
}

fn motion(keys: &[Key], from: f64, to: f64) -> Motion {
    let (mut speed, mut lo, mut hi) = (0.0f64, f64::INFINITY, f64::NEG_INFINITY);
    let mut prev = evaluate(keys, from).unwrap();
    let mut t = from + DT;
    while t <= to {
        let v = evaluate(keys, t).unwrap();
        speed = speed.max((v - prev).abs() / DT);
        lo = lo.min(v);
        hi = hi.max(v);
        prev = v;
        t += DT;
    }
    Motion {
        speed,
        lo,
        hi,
    }
}

// the hardest a curve turns between two times, away from the times in `skip` (hits and sharp keys turn on purpose)
fn turn(keys: &[Key], from: f64, to: f64, skip: &[f64]) -> f64 {
    let mut top = 0.0f64;
    let mut t = from + DT;
    while t < to {
        if !skip.iter().any(|s| (t - s).abs() < 0.25 / FPS) {
            let a = evaluate(keys, t - DT).unwrap() - 2.0 * evaluate(keys, t).unwrap() + evaluate(keys, t + DT).unwrap();
            top = top.max(a.abs() / (DT * DT));
        }
        t += DT;
    }
    top
}

// times where a curve has a sharp key, relative to its hit
fn sharp_times(reference: &[Key]) -> Vec<f64> {
    reference.iter().filter(|key| (slope(reference, key.time, true) - slope(reference, key.time, false)).abs() > 1e-6 * (1.0 + slope(reference, key.time, true).abs())).map(|key| key.time).collect()
}

fn span(reference: &[Key]) -> (f64, f64) {
    (-reference[0].time, reference[reference.len() - 1].time)
}

// three crowded notes and one with room, `gap` seconds apart
fn hits_for(reference: &[Key], gap: f64) -> Vec<f64> {
    let (lead, tail) = span(reference);
    let start = 1.0 + lead;
    vec![start, start + gap, start + 2.0 * gap, start + 2.0 * gap + gap.max(lead + tail + 2.0 / FPS)]
}

#[test]
fn momentum_never_moves_faster_than_keyed_lands_every_hit_and_stays_in_range() {
    for (name, reference) in references() {
        let (lead, tail) = span(&reference);
        let keyed = motion(&reference, -lead, tail);
        let range = keyed.hi - keyed.lo;
        let hit_value = evaluate(&reference, 0.0).unwrap();
        // from 2 frames apart, closer than that there's no time to reach the hit at any speed
        let mut frames = 2.0;
        while frames <= (lead + tail) * FPS + 6.0 {
            let gap = frames / FPS;
            let hits = hits_for(&reference, gap);
            for dampen in [0.0, 0.5, 1.0] {
                for use_anchor_value in [true, false] {
                    let keys = combined(&reference, &hits, dampen, use_anchor_value);
                    assert_eq!(check_keys(&keys), None, "{name} at {frames} frames");
                    let m = motion(&keys, hits[0] - lead - 0.1, hits[3] + tail + 0.1);
                    let at = format!("{name}, {frames} frames apart, dampen {dampen}, anchor point value {use_anchor_value}");
                    assert!(m.speed <= keyed.speed * 1.06, "{at}: moves {:.2}x as fast as keyed", m.speed / keyed.speed);
                    assert!(m.lo >= keyed.lo - 1e-3 * range && m.hi <= keyed.hi + 1e-3 * range, "{at}: leaves the keyed range");
                    if use_anchor_value || dampen == 0.0 {
                        for hit in &hits {
                            let error = (evaluate(&keys, *hit).unwrap() - hit_value).abs();
                            assert!(error < 1e-6 * (1.0 + range), "{at}: misses a hit by {error}");
                        }
                    }
                }
            }
            frames += 0.5;
        }
    }
}

#[test]
fn bridges_turn_no_harder_than_the_notes_do() {
    for (name, reference) in references() {
        let (lead, tail) = span(&reference);
        let mut skip_rel = sharp_times(&reference);
        skip_rel.push(0.0);
        let keyed = turn(&reference, -lead, tail, &skip_rel);
        let mut frames = 3.0;
        while frames <= (lead + tail) * FPS + 2.0 {
            let hits = hits_for(&reference, frames / FPS);
            let keys = combined(&reference, &hits, 0.0, true);
            let skip: Vec<f64> = hits.iter().flat_map(|hit| skip_rel.iter().map(move |s| hit + s)).collect();
            let worst = turn(&keys, hits[0] - lead, hits[3] + tail, &skip);
            assert!(worst <= keyed * 1.6, "{name}, {frames} frames apart: turns {:.2}x as hard as keyed", worst / keyed);
            frames += 0.5;
        }
    }
}

#[test]
fn notes_with_room_play_exactly_as_keyed() {
    for (name, reference) in references() {
        let (lead, tail) = span(&reference);
        let hits = [1.0, 1.0 + lead + tail + 0.1];
        for dampen in [0.0, 1.0] {
            let keys = combined(&reference, &hits, dampen, true);
            let mut u = -lead;
            while u <= tail {
                let error = (evaluate(&keys, hits[0] + u).unwrap() - evaluate(&reference, u).unwrap()).abs();
                assert!(error < 1e-9, "{name} with dampen {dampen} changed at {u}");
                u += DT;
            }
        }
    }
}

// the highest point between two hits
fn peak(keys: &[Key], a: f64, b: f64) -> f64 {
    motion(keys, a, b).hi
}

#[test]
fn a_drumstick_keeps_its_windup_like_the_doc_and_shrinks_only_when_it_has_to() {
    let (_, drumstick) = references().into_iter().next().unwrap();
    // the doc's keys (rest 20, windup 60, hit 0, recoil 30) need 24 frames between hits for the whole stroke. the windup
    // stays down to 10 frames apart. closer in the doc draws a sharp triangle at the hit's speed, momentum turns around
    // smoothly instead (no faster than keyed, no harder than keyed), so its peak is lower there and drops with the gap
    let peaks: Vec<(f64, f64)> = [3.0, 4.0, 6.0, 8.0, 10.0, 12.0, 16.0, 20.0, 26.0]
        .iter()
        .map(|&frames| {
            let hits = [1.0, 1.0 + frames / FPS];
            (frames, peak(&combined(&drumstick, &hits, 0.0, true), hits[0], hits[1]))
        })
        .collect();
    for &(frames, p) in &peaks {
        assert!(p <= 60.0 + 1e-6, "{frames} frames apart peaks at {p:.1}");
        if frames >= 10.0 {
            assert!(p > 59.0, "{frames} frames apart lost the windup, peaks at {p:.1}");
        }
    }
    assert!(peaks.windows(2).all(|w| w[1].1 >= w[0].1), "{peaks:?}");
    assert!(peaks[0].1 > 10.0, "{peaks:?}");
}

#[test]
fn the_motion_between_two_hits_only_grows_with_the_gap() {
    for (name, reference) in references() {
        let (lead, tail) = span(&reference);
        let range = motion(&reference, -lead, tail);
        let range = range.hi - range.lo;
        let (mut biggest, mut frames) = (0.0f64, 1.0);
        while frames <= (lead + tail) * FPS + 4.0 {
            let hits = [1.0, 1.0 + frames / FPS];
            let m = motion(&combined(&reference, &hits, 0.0, true), hits[0], hits[1]);
            let size = m.hi - m.lo;
            // a ripple of a few percent where the bridge changes placement
            assert!(size >= biggest - 0.03 * range, "{name}: {frames} frames apart moves {size:.3}, less than {biggest:.3} at a smaller gap");
            biggest = biggest.max(size);
            frames += 0.25;
        }
    }
}

#[test]
fn dampening_shrinks_crowded_notes_and_keeps_the_hit() {
    let (_, drumstick) = references().into_iter().next().unwrap();
    let hits = [1.0, 1.0 + 12.0 / FPS, 1.0 + 24.0 / FPS];
    let free = combined(&drumstick, &hits, 0.0, true);
    let full = combined(&drumstick, &hits, 1.0, true);
    let half = combined(&drumstick, &hits, 0.5, true);
    let (p0, p5, p1) = (peak(&free, hits[0], hits[1]), peak(&half, hits[0], hits[1]), peak(&full, hits[0], hits[1]));
    assert!(p1 < p5 && p5 < p0, "{p0} {p5} {p1}");
    for hit in hits {
        assert!(evaluate(&full, hit).unwrap().abs() < 1e-9);
    }
    // off shrinks toward rest (20 before the note), so the hit lifts off the drum
    let toward_rest = combined(&drumstick, &hits, 1.0, false);
    assert!(evaluate(&toward_rest, hits[1]).unwrap() > 1.0);
}

#[test]
fn a_chord_on_one_curve_plays_once() {
    let (_, drumstick) = references().into_iter().next().unwrap();
    let once = combined(&drumstick, &[1.0, 2.0], 0.0, true);
    let chord = combined(&drumstick, &[1.0, 1.0, 2.0], 0.0, true);
    assert_eq!(once, chord);
}

#[test]
fn splitting_a_curve_keeps_its_shape() {
    let (_, spring) = references().into_iter().nth(7).unwrap();
    for t in [-0.05, 0.03, 0.2, 0.41, 1.5, 2.5] {
        let mut keys = spring.clone();
        let at = split_at(&mut keys, t);
        assert!((keys[at].time - t).abs() < 1e-12);
        let mut u = -0.2;
        while u < 2.0 {
            assert!((evaluate(&keys, u).unwrap() - evaluate(&spring, u).unwrap()).abs() < 1e-7, "split at {t} changed {u}");
            u += 0.003;
        }
    }
    // a key already there is reused
    let mut keys = spring.clone();
    assert_eq!(split_at(&mut keys, spring[2].time), 2);
    assert_eq!(keys.len(), spring.len());
    // linear and constant segments split too
    let mut steps = vec![Key::new(0.0, 1.0, Interpolation::Constant), Key::linear(1.0, 3.0), Key::linear(2.0, 5.0)];
    split_at(&mut steps, 0.5);
    split_at(&mut steps, 1.5);
    assert_eq!(evaluate(&steps, 0.75), Some(1.0));
    assert!((evaluate(&steps, 1.75).unwrap() - 4.5).abs() < 1e-12);
}

fn chunk(object: &str, channel: &str, overlap: &str, hit: Option<f64>, keys: Vec<Key>) -> CurveKeys {
    CurveKeys {
        object: object.into(),
        channel: channel.into(),
        animation_overlap: overlap.into(),
        overlap_blend: 0.1,
        hit,
        overlap_dampen: 0.0,
        overlap_use_anchor_value: true,
        keyframes: keys,
    }
}

#[test]
fn combine_runs_momentum_per_curve_next_to_other_modes() {
    let (_, drumstick) = references().into_iter().next().unwrap();
    let hits = [1.0, 1.0 + 10.0 / FPS];
    let chunks = vec![chunk("stick", "rotation_euler[0]", "momentum", Some(hits[0]), at_hit(&drumstick, hits[0])), chunk("stick", "location[2]", "add", None, vec![Key::linear(0.0, 0.0), Key::linear(1.0, 1.0)]), chunk("stick", "rotation_euler[0]", "momentum", Some(hits[1]), at_hit(&drumstick, hits[1]))];
    let out = combine_curve_keys([String::from("stick")].iter(), chunks).unwrap();
    let rotation: Vec<Key> = out["stick"].iter().filter(|k| k.channel == "rotation_euler[0]").map(|k| k.key.clone()).collect();
    assert_eq!(rotation, combined(&drumstick, &hits, 0.0, true));
    assert_eq!(out["stick"].iter().filter(|k| k.channel == "location[2]").count(), 2);
}

#[test]
fn curve_keys_saved_before_momentum_still_read() {
    let chunk: CurveKeys = serde_json::from_value(json!({ "object": "a", "channel": "c", "animation_overlap": "add", "overlap_blend": 0.1, "keyframes": [[0.0, 1.0, "linear"]] })).unwrap();
    assert_eq!(chunk.hit, None);
    assert_eq!(chunk.overlap_dampen, 0.0);
    assert!(chunk.overlap_use_anchor_value);
}

#[test]
fn generator_passes_dampen_and_use_anchor_value_through() {
    let outputs = animation_generator(&Inputs::default()).unwrap().to_json();
    assert_eq!(outputs["generator"]["overlap_dampen"], json!(0.0));
    assert_eq!(outputs["generator"]["overlap_use_anchor_value"], json!(true));

    let outputs = animation_generator(&Inputs::from([("animation_overlap", json!("momentum")), ("overlap_dampen", json!(0.4)), ("overlap_use_anchor_value", json!(false))])).unwrap().to_json();
    assert_eq!(outputs["generator"]["animation_overlap"], json!("momentum"));
    assert_eq!(outputs["generator"]["overlap_dampen"], json!(0.4));
    assert_eq!(outputs["generator"]["overlap_use_anchor_value"], json!(false));

    let error = animation_generator(&Inputs::from([("overlap_dampen", json!(1.5))])).unwrap_err();
    assert!(error.contains("overlap dampen goes from 0 to 1"), "{}", error);
}

#[test]
#[ignore = "timing, run with --release --ignored"]
fn a_long_crowded_song_is_quick() {
    let (_, drumstick) = references().into_iter().next().unwrap();
    // 2000 sixteenth notes at 120 bpm, 3 frames apart, every bridge too tight for the whole stroke
    let hits: Vec<f64> = (0..2000).map(|i| 1.0 + i as f64 * 0.125).collect();
    let start = std::time::Instant::now();
    let keys = combined(&drumstick, &hits, 0.5, true);
    let took = start.elapsed();
    println!("{} notes -> {} keys in {:?}", hits.len(), keys.len(), took);
    assert!(took.as_secs_f64() < 2.0);
}
