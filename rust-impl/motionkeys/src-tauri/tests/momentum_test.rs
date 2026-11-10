// run from /src-tauri
// cargo test --test momentum_test
// the momentum overlap on keyed references of different kinds (percussion, scale bounces, swings, plucks, elastic
// rings, a curve baked to a key every frame), checked against what it promises at every gap
use motionkeys::graph::executors::animation::animation_generator;
use motionkeys::graph::executors::io::Inputs;
use motionkeys::utils::animation::{combine_curve_keys, finish_keys, CurveKeys};
use motionkeys::utils::curve::{auto_handles, check_keys, evaluate, slope, split_at, Interpolation, Key};
use motionkeys::utils::momentum::{momentum_keys, phases, MomentumNote};
use motionkeys::scene_generics::AnimCurve;
use serde_json::json;
use std::collections::HashMap;

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
        ("hammer", hammer()),
    ]
}

// a hammer that rests partway up: it winds up from rest, strikes, and comes back up past its recoil to rest, the same
// way the next windup goes on. close notes would stop it dead at rest on the way up. its handles are the ones blender
// gives these keys (auto clamped, vector on the hit, in degrees), steeper at the recoil than `auto_handles` makes them
fn hammer() -> Vec<Key> {
    let keys: [(f64, f64, [f64; 2], [f64; 2]); 5] = [(-16.0, 10.0, [-18.333333, 10.0], [-13.666667, 10.0]), (-9.0, 60.0, [-11.333333, 60.0], [-6.0, 60.0]), (0.0, 0.0, [-3.0, 30.0], [1.0, 2.2078141]), (3.0, 6.0, [2.0, 4.4156282], [5.333333, 9.6969786]), (10.0, 10.0, [7.666667, 10.0], [12.333333, 10.0])];
    keys.iter().map(|&(frame, value, left, right)| Key::with_handles(frame / FPS, value, Interpolation::Bezier, [left[0] / FPS, left[1]], [right[0] / FPS, right[1]])).collect()
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
            phases: None,
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
fn keys_stay_far_enough_apart_for_blender_to_keep_them() {
    // blender merges keys less than a hundredth of a frame apart when it sorts a curve, so a key next to a hit would
    // take the hit with it. 24 fps here, the slowest common rate, where a hundredth of a frame is longest
    for (name, reference) in references() {
        let (lead, tail) = span(&reference);
        let mut frames = 2.0;
        while frames <= (lead + tail) * FPS + 4.0 {
            let keys = combined(&reference, &hits_for(&reference, frames / FPS), 0.0, true);
            for pair in keys.windows(2) {
                let apart = (pair[1].time - pair[0].time) * FPS;
                assert!(apart > 0.02, "{name}, {frames} frames apart: keys at {:.4} and {:.4} are {apart:.4} frames apart", pair[0].time * FPS, pair[1].time * FPS);
            }
            frames += 0.5;
        }
    }
}

#[test]
fn notes_with_room_play_exactly_as_keyed() {
    for (name, reference) in references() {
        let (lead, tail) = span(&reference);
        // room for the whole note, and to sit at rest for two windups before the next one
        let hits = [1.0, 1.0 + 3.0 * lead + tail + 0.1];
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
    // rings with no windup (spring, pluck, baked spring) are left out: where the old ring is on the far side when the
    // new stroke starts, keeping it would make the stroke faster than keyed, so the ring goes like the doc's recoil
    for (name, reference) in references().into_iter().filter(|(name, _)| !["spring", "pluck", "baked spring"].contains(name)) {
        let (lead, tail) = span(&reference);
        let range = motion(&reference, -lead, tail);
        let range = range.hi - range.lo;
        let (mut biggest, mut frames) = (0.0f64, 1.0);
        while frames <= (lead + tail) * FPS + 4.0 {
            let hits = [1.0, 1.0 + frames / FPS];
            let m = motion(&combined(&reference, &hits, 0.0, true), hits[0], hits[1]);
            let size = m.hi - m.lo;
            // a ripple of a few percent where the join changes
            assert!(size >= biggest - 0.03 * range, "{name}: {frames} frames apart moves {size:.3}, less than {biggest:.3} at a smaller gap");
            biggest = biggest.max(size);
            frames += 0.25;
        }
    }
}

// the slowest the curve moves between two times
fn slowest(keys: &[Key], from: f64, to: f64) -> f64 {
    let mut slowest = f64::INFINITY;
    let mut t = from;
    while t < to {
        slowest = slowest.min(((evaluate(keys, t + DT).unwrap() - evaluate(keys, t).unwrap()) / DT).abs());
        t += DT;
    }
    slowest
}

#[test]
fn a_hammer_coming_back_up_doesnt_stop_at_rest_before_a_close_note() {
    let hammer = hammer();
    // the hammer's whole stroke is 26 frames, it rests at 10 on the way up from the hit (0) to the windup (60, 9 frames
    // before the hit). as keyed it would stop dead at rest when the notes touch (26 frames) or nearly do, its recoil
    // is its rest so it goes straight from the hit up to the next windup instead
    for frames in [20.0, 23.0, 26.0] {
        let hits = [1.0, 1.0 + frames / FPS];
        let (from, to) = (hits[0] + 3.0 / FPS, hits[1] - 10.0 / FPS);
        let carried = combined(&hammer, &hits, 0.0, true);
        let moving = slowest(&carried, from, to);
        assert!(moving > 0.5 * 4.0 / (7.0 / FPS), "{frames} frames apart still stops: {moving:.2} a second");
    }
}

#[test]
fn a_carried_hammer_only_slows_down_at_its_rest_height() {
    let hammer = hammer();
    // at every gap it goes one way from the hit to the next windup, and wherever it's nearly still it's at rest (10),
    // never hovering above it
    let mut frames = 18.0;
    while frames <= 60.0 {
        let hits = [1.0, 1.0 + frames / FPS];
        let keys = combined(&hammer, &hits, 0.0, true);
        let (from, to) = (hits[0] + DT, hits[1] - 10.0 / FPS);
        let mut t = from;
        while t < to {
            let (v, next) = (evaluate(&keys, t).unwrap(), evaluate(&keys, t + DT).unwrap());
            let speed = (next - v) / DT;
            assert!(speed > -0.06, "{frames} frames apart goes back down at {:.2} frames: {speed:.3} a second", (t - hits[0]) * FPS);
            if speed < 0.1 * 4.0 / (7.0 / FPS) {
                assert!((v - 10.0).abs() < 0.6, "{frames} frames apart nearly stops at {v:.2}, not at rest, {:.2} frames after the hit", (t - hits[0]) * FPS);
            }
            t += DT;
        }
        frames += 0.5;
    }
}

// notes at different velocities: each note's whole curve scaled by velocity / 127, like the generator's velocity
// intensity, so each rests at its own height
fn loud_and_soft(reference: &[Key], hits: &[f64], velocities: &[f64]) -> Vec<Key> {
    let notes = hits
        .iter()
        .zip(velocities)
        .map(|(&hit, &velocity)| MomentumNote {
            hit,
            keys: at_hit(reference, hit)
                .into_iter()
                .map(|mut key| {
                    let scale = velocity / 127.0;
                    key.value *= scale;
                    key.left[1] *= scale;
                    key.right[1] *= scale;
                    key
                })
                .collect(),
            dampen: 0.0,
            use_anchor_value: true,
            phases: None,
        })
        .collect();
    let mut keys = momentum_keys(notes);
    finish_keys(&mut keys);
    keys
}

#[test]
fn notes_with_room_ease_between_their_rest_heights() {
    // a drumstick at velocity 96 then 104 rests at 15.1 then 16.4, with half a second between its rests: it eases
    // across the whole pause instead of hopping at one point in it
    let (_, drumstick) = references().into_iter().next().unwrap();
    let (lead, tail) = span(&drumstick);
    let pause = 0.5;
    let hits = [1.0, 1.0 + lead + tail + pause];
    let keys = loud_and_soft(&drumstick, &hits, &[96.0, 104.0]);
    let (from, to) = (hits[0] + tail, hits[1] - lead);
    let rise = 20.0 * (104.0 - 96.0) / 127.0;
    let fastest = motion(&keys, from, to).speed;
    assert!(fastest <= 1.6 * rise / pause, "hops between rests at {fastest:.2} a second, an ease is about {:.2}", rise / pause);
    for hit in hits {
        assert!(evaluate(&keys, hit).unwrap().abs() < 1e-9);
    }
}

#[test]
fn a_hammer_carries_through_rest_at_different_velocities() {
    // loud and soft notes rest at different heights, the hammer still doesn't stop on its way up between them, and
    // nearly still it's only between the two rest heights
    let hammer = hammer();
    for (frames, velocities) in [(26.0, [96.0, 104.0]), (26.0, [110.0, 40.0]), (32.0, [40.0, 110.0]), (20.0, [100.0, 60.0])] {
        let hits = [1.0, 1.0 + frames / FPS];
        let keys = loud_and_soft(&hammer, &hits, &velocities);
        let rests = [10.0 * velocities[0] / 127.0, 10.0 * velocities[1] / 127.0];
        let (lo, hi) = (rests[0].min(rests[1]), rests[0].max(rests[1]));
        let mut t = hits[0] + 4.0 / FPS;
        while t < hits[1] - 10.0 / FPS {
            let (v, next) = (evaluate(&keys, t).unwrap(), evaluate(&keys, t + DT).unwrap());
            let speed = (next - v) / DT;
            if speed.abs() < 0.05 * 4.0 / (7.0 / FPS) {
                assert!(v > lo - 0.3 && v < hi + 0.3, "{frames} frames apart at {velocities:?} nearly stops at {v:.2}, rests are {lo:.2} to {hi:.2}");
            }
            t += DT;
        }
        for hit in hits {
            assert!(evaluate(&keys, hit).unwrap().abs() < 1e-9);
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
        phases: None,
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

#[test]
fn phases_are_found_where_the_tangent_is_flat() {
    let all = references();
    let get = |name: &str| all.iter().find(|(n, _)| *n == name).unwrap().1.clone();
    let frames = |keys: Vec<Key>| phases(&keys, 0.0).map(|t| (t * FPS * 1000.0).round() / 1000.0);
    // the windup's peak and the recoil's top
    assert_eq!(frames(get("drumstick")), [-4.0, 5.0]);
    // a hammer that comes back up through rest: its recoil is its rest
    assert_eq!(frames(get("hammer")), [-9.0, 10.0]);
    // no windup (it starts at the hit's value): the first key, the recoil its first swing
    assert_eq!(frames(get("antenna")), [-1.0, 2.0]);
    // an anchor point a little off the drumstick's hit still finds the same windup and recoil
    let off = |by: f64| phases(&get("drumstick"), by / FPS).map(|t| (t * FPS * 1000.0).round() / 1000.0);
    assert_eq!(off(-0.4), [-4.0, 5.0]);
    assert_eq!(off(0.4), [-4.0, 5.0]);
    // the hit at the top: the windup is the rest it rises from, and it comes down without a bounce, so the recoil is
    // where it's back at rest
    assert_eq!(frames(get("laser tilt")), [-16.0, 10.0]);
}

#[test]
fn phases_set_on_the_note_are_used() {
    let (_, drumstick) = references().into_iter().next().unwrap();
    let hits = [1.0, 1.0 + 20.0 / FPS];
    // found: the recoil is its top (5 frames), the next windup starts during the rest after it, so it comes back down
    // a little before winding up
    let found = combined(&drumstick, &hits, 0.0, true);
    // the recoil set at the rest (13 frames): no room for it, so it goes straight up into the windup
    let mut set = notes(&drumstick, &hits, 0.0, true);
    for note in &mut set {
        note.phases = Some([note.hit - 4.0 / FPS, note.hit + 13.0 / FPS]);
    }
    let mut set = momentum_keys(set);
    finish_keys(&mut set);
    let peak_time = hits[1] - 4.0 / FPS;
    let dips = |keys: &[Key]| {
        let mut t = hits[0] + DT;
        let mut dip = 0.0f64;
        while t < peak_time {
            dip = dip.max(evaluate(keys, t).unwrap() - evaluate(keys, t + DT).unwrap());
            t += DT;
        }
        dip
    };
    assert!(dips(&found) > 0.0, "found phases go straight up");
    assert!(dips(&set) < 1e-9, "the recoil set at rest still comes back down");
}

#[test]
fn an_accent_after_a_ghost_note_keeps_its_whole_windup() {
    // a soft note right before a loud one: the loud note's windup starts during the soft note's rest and still goes
    // all the way up
    let (_, drumstick) = references().into_iter().next().unwrap();
    let hits = [1.0, 1.0 + 20.0 / FPS];
    let keys = loud_and_soft(&drumstick, &hits, &[38.0, 112.0]);
    let top = peak(&keys, hits[0], hits[1]);
    assert!(top > 0.999 * 60.0 * 112.0 / 127.0, "peaks at {top:.2}");
}

#[test]
fn the_generator_finds_its_windup_and_recoil_on_the_first_note_on_curve() {
    // the drumstick keyed from frame 0, its hit at frame 11, the anchor point puts the hit on the note
    let (_, drumstick) = references().into_iter().next().unwrap();
    let keys: Vec<Key> = drumstick.iter().map(|key| key.moved(11.0 / FPS)).collect();
    let curve = json!([serde_json::to_value(AnimCurve {
        channel: "rotation_euler[0]".into(),
        keys
    })
    .unwrap()]);
    let inputs = |extra: &[(&str, serde_json::Value)]| {
        let mut inputs: HashMap<String, serde_json::Value> = HashMap::from([("animation_overlap".into(), json!("momentum")), ("note_on_keyframes".into(), curve.clone()), ("note_on_anchor_point".into(), json!(-11.0))]);
        inputs.extend(extra.iter().map(|(k, v)| (k.to_string(), v.clone())));
        Inputs::from(inputs)
    };
    let frames = |v: &serde_json::Value| (v.as_f64().unwrap() * 1000.0).round() / 1000.0;
    let outputs = animation_generator(&inputs(&[])).unwrap().to_json();
    assert_eq!(frames(&outputs["generator"]["overlap_windup"]), -4.0);
    assert_eq!(frames(&outputs["generator"]["overlap_recoil"]), 5.0);
    // set ones are kept
    let outputs = animation_generator(&inputs(&[("overlap_windup", json!(-6.0)), ("overlap_recoil", json!(9.0))])).unwrap().to_json();
    assert_eq!(outputs["generator"]["overlap_windup"], json!(-6.0));
    assert_eq!(outputs["generator"]["overlap_recoil"], json!(9.0));
    // the windup comes before the hit, the recoil after it
    assert!(animation_generator(&inputs(&[("overlap_windup", json!(2.0))])).unwrap_err().contains("overlap windup"));
    assert!(animation_generator(&inputs(&[("overlap_recoil", json!(-1.0))])).unwrap_err().contains("overlap recoil"));
    // other modes don't look for them
    let outputs = animation_generator(&inputs(&[("animation_overlap", json!("add"))])).unwrap().to_json();
    assert!(outputs["generator"].get("overlap_windup").is_none());
}
