// the momentum overlap mode. notes keep their keyed timing, so nothing moves faster than it was keyed. each note's curve
// has three phase points: its windup's peak, its hit (the note on) and its recoil (where it's back from the hit, at
// rest or at the top of a bounce). they're set on the generator or found on the curve where its tangent is flat.
// where two notes crowd each other they're joined the way the procedural drumstick does it, on the keyed shapes:
//   as keyed     room for everything, a level ease between two rests at different heights
//   rest cut     the new windup starts during the old rest: the rest is cut there and the windup rises from it
//   full windup  no room for the old recoil: straight from the old hit up to the new windup's peak, then its stroke
//   shrunk       no room for the whole stroke: the new stroke shrunk (same speed) until the way up to it fits
// dampening shrinks crowded notes further on purpose. works on any keyed curve: hits are each note's note on, the
// anchor point already puts the keyed hit there

use crate::utils::curve::{evaluate, slope, split_at, Interpolation, Key};

/// one note's keys on a curve, when it hits and the dampening its generator asks for
#[derive(Debug, Clone)]
pub struct MomentumNote {
    /// seconds, the note on
    pub hit: f64,
    /// in time order
    pub keys: Vec<Key>,
    /// 0 to 1, how much a crowded note shrinks
    pub dampen: f64,
    /// dampening shrinks toward the hit's value so the hit still lands, else toward rest
    pub use_anchor_value: bool,
    /// seconds, its windup's peak and its recoil. unset finds them on the keys (`phases`)
    pub phases: Option<[f64; 2]>,
}

// how close two keys can be, seconds. blender merges keys less than a hundredth of a frame apart when it sorts a curve,
// which would take one's handles with it, so this stays well clear of that at any frame rate from 5 up
const EDGE: f64 = 0.05 / 24.0;

// how close two hits are to count as one (a chord on one curve), seconds
const SAME_HIT: f64 = 2.0 * EDGE;

// keys this close are one key when the curve is written (`finish_keys`), seconds
const SAME_KEY: f64 = 1e-6;

// sampling step for finding the phases, seconds
const STEP: f64 = 1.0 / 2400.0;

// a phase point has moved away from the hit's value when it's this share of the curve's range from it
const AWAY: f64 = 0.02;

// a turn this close to the hit is the hit's own (an anchor point a little off the curve's turn), seconds
const HIT_TURN: f64 = 2.0 / 24.0;

// a curve that leaves a key and arrives level at the next (`rise`) moves at most this many times its average speed
const RISE: f64 = 1.5;

// MARK: - phases

/// times where a curve's tangent is flat: keys with level handles and every place its slope changes direction
fn flat_times(keys: &[Key]) -> Vec<f64> {
    let mut out: Vec<f64> = keys.iter().filter(|key| (key.left[1] - key.value).abs() <= 1e-9 * (1.0 + key.value.abs()) && (key.right[1] - key.value).abs() <= 1e-9 * (1.0 + key.value.abs())).map(|key| key.time).collect();
    let (first, last) = (keys[0].time, keys[keys.len() - 1].time);
    let n = ((last - first) / STEP).ceil().max(1.0) as usize;
    let h = (last - first) / n as f64;
    let values: Vec<f64> = (0..=n).map(|i| evaluate(keys, first + h * i as f64).unwrap_or(0.0)).collect();
    let vmax = values.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0, f64::max);
    let mut sign = 0.0;
    for (i, w) in values.windows(2).enumerate() {
        let d = w[1] - w[0];
        if d.abs() < 1e-4 * vmax {
            continue;
        }
        if sign != 0.0 && d.signum() != sign {
            // a turn next to a key is that key
            let t = first + h * i as f64;
            let snapped = keys.iter().map(|key| key.time).find(|&k| (k - t).abs() < 0.5 / 60.0);
            out.push(snapped.unwrap_or(t));
        }
        sign = d.signum();
    }
    out.sort_by(f64::total_cmp);
    out.dedup_by(|later, earlier| *later - *earlier <= 0.25 / 60.0);
    out
}

/// a curve's windup and recoil around its hit at `hit`, seconds. the windup is the flat point before the hit furthest
/// from the hit's value (the first key when nothing moves away), the recoil the first flat point after the hit that has
/// moved away from it (the last key when none has). a turn right next to the hit is the hit's own, both look past it
pub fn phases(keys: &[Key], hit: f64) -> [f64; 2] {
    let (first, last) = (keys[0].time, keys[keys.len() - 1].time);
    let at = |t: f64| evaluate(keys, t).unwrap_or(0.0);
    let flats = flat_times(keys);
    let own = flats.iter().copied().filter(|&t| (t - hit).abs() <= HIT_TURN).min_by(|a, b| (a - hit).abs().total_cmp(&(b - hit).abs()));
    let (before, after) = own.map_or((hit, hit), |t| (hit.min(t), hit.max(t)));
    let hit_value = at(own.unwrap_or(hit));
    let (lo, hi) = keys.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), key| (lo.min(key.value), hi.max(key.value)));
    let range = (hi - lo).max(1e-9);
    let mut windup = first;
    let mut best = -1.0;
    for &t in flats.iter().filter(|&&t| t < before - EDGE) {
        let d = (at(t) - hit_value).abs();
        if d > best + 1e-9 * range {
            best = d;
            windup = t;
        }
    }
    if best < AWAY * range {
        windup = first;
    }
    let recoil = flats.iter().copied().find(|&t| t > after + EDGE && (at(t) - hit_value).abs() > AWAY * range).unwrap_or(last);
    [windup, recoil]
}

// MARK: - measuring

/// what's measured on a note's keys, relative to its hit and its range, so notes that only differ in time and in how
/// loud they are (velocity scales the whole curve) are measured once
struct Measured {
    /// the keys' times from the hit and values over the range, and the handles'
    shape: Vec<f64>,
    range: f64,
    /// seconds from the hit
    phases: [f64; 2],
    /// the fastest the curve moves, value per second
    speed: f64,
}

fn shape_of(keys: &[Key], hit: f64) -> (Vec<f64>, f64) {
    let (lo, hi) = keys.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), key| (lo.min(key.value), hi.max(key.value)));
    let range = hi - lo;
    let scale = if range > 0.0 {
        1.0 / range
    } else {
        1.0
    };
    let base = keys[0].value;
    let shape = keys.iter().flat_map(|key| [key.time - hit, (key.value - base) * scale, key.left[0] - hit, (key.left[1] - base) * scale, key.right[0] - hit, (key.right[1] - base) * scale]).collect();
    (shape, range)
}

fn top_speed(keys: &[Key]) -> f64 {
    let (first, last) = (keys[0].time, keys[keys.len() - 1].time);
    let n = ((last - first) / STEP).ceil().max(1.0) as usize;
    let h = (last - first) / n as f64;
    let values: Vec<f64> = (0..=n).map(|i| evaluate(keys, first + h * i as f64).unwrap_or(0.0)).collect();
    values.windows(2).map(|w| (w[1] - w[0]).abs() / h).fold(0.0, f64::max)
}

/// a note's phases (seconds from its hit) and top speed, measured or taken from one measured before
fn measured(cache: &mut Vec<Measured>, keys: &[Key], hit: f64) -> ([f64; 2], f64) {
    let (shape, range) = shape_of(keys, hit);
    if let Some(m) = cache.iter().find(|m| m.shape.len() == shape.len() && m.shape.iter().zip(&shape).all(|(a, b)| (a - b).abs() <= 1e-9)) {
        let ratio = if m.range > 0.0 {
            range / m.range
        } else {
            1.0
        };
        return (m.phases, m.speed * ratio);
    }
    let [windup, recoil] = phases(keys, hit);
    let m = Measured {
        shape,
        range,
        phases: [windup - hit, recoil - hit],
        speed: top_speed(keys),
    };
    let out = (m.phases, m.speed);
    cache.push(m);
    out
}

// MARK: - notes

/// a note's keys with keys at its windup, hit and recoil, and where those are
struct Phased {
    hit: f64,
    keys: Vec<Key>,
    windup: usize,
    hit_index: usize,
    start: f64,
    end: f64,
    windup_time: f64,
    recoil_time: f64,
    /// the fastest it moves as keyed (before dampening), value per second
    speed: f64,
}

/// a key at `t` for cutting a curve there, a key already closer than blender keeps apart is used instead
fn cut(keys: &mut Vec<Key>, t: f64) -> usize {
    match keys.iter().position(|key| (key.time - t).abs() < EDGE) {
        Some(i) => i,
        None => split_at(keys, t),
    }
}

fn phased(mut keys: Vec<Key>, hit: f64, [windup, recoil]: [f64; 2], speed: f64) -> Phased {
    let (start, end) = (keys[0].time, keys[keys.len() - 1].time);
    // the windup before the hit and the recoil after it, both on the curve
    let windup = windup.clamp(start, hit.max(start));
    let recoil = recoil.clamp(hit.min(end), end);
    for t in [hit, windup, recoil] {
        if t > start && t < end {
            cut(&mut keys, t);
        }
    }
    // the key nearest each point, a cut close to a key used that key
    let nearest = |t: f64| (0..keys.len()).min_by(|&i, &j| (keys[i].time - t).abs().total_cmp(&(keys[j].time - t).abs())).unwrap_or(0);
    let hit_index = nearest(hit);
    let windup_index = nearest(windup).min(hit_index);
    Phased {
        hit: keys[hit_index].time,
        windup: windup_index,
        hit_index,
        start,
        end,
        windup_time: keys[windup_index].time,
        recoil_time: recoil.max(keys[hit_index].time),
        speed,
        keys,
    }
}

// MARK: - dampening

/// the note's keys shrunk by its dampening: `lead_amount` before the hit and `tail_amount` after it, toward `pivot`.
/// a straight multiply of values and handles, so the curve keeps its shape and can only get slower
fn dampened(keys: &[Key], hit: f64, pivot: f64, lead_amount: f64, tail_amount: f64) -> Vec<Key> {
    let mut keys = keys.to_vec();
    if lead_amount == 1.0 && tail_amount == 1.0 {
        return keys;
    }
    // two amounts meet at the hit, a segment across it is split there so each half takes its own
    if lead_amount != tail_amount && keys[0].time < hit && hit < keys[keys.len() - 1].time {
        cut(&mut keys, hit);
    }
    let shrink = |v: f64, amount: f64| pivot + (v - pivot) * amount;
    for key in &mut keys {
        let (before, after) = if key.time < hit - SAME_KEY {
            (lead_amount, lead_amount)
        } else if key.time > hit + SAME_KEY {
            (tail_amount, tail_amount)
        } else {
            (lead_amount, tail_amount)
        };
        // a key on the hit sits on the pivot (anchor point value) or has one amount on both sides, either amount is the same
        key.value = shrink(key.value, before);
        key.left[1] = shrink(key.left[1], before);
        key.right[1] = shrink(key.right[1], after);
    }
    keys
}

/// a side's dampening from the gap to its neighbor: 1 with room for the whole note, down to 1 - dampen as it crowds
fn side_amount(gap: Option<f64>, span: f64, dampen: f64) -> f64 {
    match gap {
        Some(gap) if span > 0.0 => 1.0 - dampen * (1.0 - (gap / span).min(1.0)),
        _ => 1.0,
    }
}

// MARK: - joins

/// blender's auto clamped slope at `key` between its neighbors: level at a turn, otherwise along the weighted
/// direction, never past a neighbor's value a third of the way there
fn auto_slope(prev: &Key, key: &Key, next: &Key) -> f64 {
    if (key.value - prev.value) * (next.value - key.value) <= 0.0 {
        return 0.0;
    }
    let (ax, ay, bx, by) = (key.time - prev.time, key.value - prev.value, next.time - key.time, next.value - key.value);
    let (la, lb) = (ax.hypot(ay), bx.hypot(by));
    let (tx, ty) = (bx / lb + ax / la, by / lb + ay / la);
    let s = if tx.abs() < 1e-14 {
        0.0
    } else {
        ty / tx
    };
    let cap = |dv: f64, dt: f64| (dv / (dt / 3.0)).abs();
    s.signum() * s.abs().min(cap(ay, ax)).min(cap(by, bx))
}

/// a run's values moved so its first key lands on `target` and its last stays put. a run that starts and ends at the
/// same height fades the offset out along it instead
fn rebased(run: &[Key], target: f64) -> Vec<Key> {
    let (a, b) = (&run[0], &run[run.len() - 1]);
    let mut out = run.to_vec();
    if (a.value - b.value).abs() > 1e-9 * (1.0 + b.value.abs()) {
        let f = (target - b.value) / (a.value - b.value);
        let pin = b.value;
        let map = |v: f64| pin + (v - pin) * f;
        for key in &mut out {
            key.value = map(key.value);
            key.left[1] = map(key.left[1]);
            key.right[1] = map(key.right[1]);
        }
    } else {
        let (d, t0, span) = (target - a.value, a.time, (b.time - a.time).max(SAME_KEY));
        let fade = |t: f64| d * (1.0 - (t - t0) / span).max(0.0);
        for key in &mut out {
            let o = fade(key.time);
            key.value += o;
            key.left[1] += o;
            key.right[1] += o;
        }
    }
    out
}

/// keys scaled around `pivot` in time and value by `rho`: a smaller copy that moves at the same speeds
fn shrunk(run: &[Key], pivot: &Key, rho: f64) -> Vec<Key> {
    let t = |x: f64| pivot.time + (x - pivot.time) * rho;
    let v = |y: f64| pivot.value + (y - pivot.value) * rho;
    run.iter().map(|key| Key::with_handles(t(key.time), v(key.value), key.interpolation, [t(key.left[0]), v(key.left[1])], [t(key.right[0]), v(key.right[1])])).collect()
}

/// the old hit's way up to a peak: it leaves the hit along its keyed recoil (never past the peak) and arrives level
fn rise(hit: &mut Key, peak: &mut Key, recoil_slope: f64) {
    let d = peak.time - hit.time;
    let average = (peak.value - hit.value) / d;
    // leaving faster than this would make it the fastest part
    let s = if recoil_slope.signum() == average.signum() {
        recoil_slope.abs().min(RISE * average.abs()) * average.signum()
    } else {
        0.0
    };
    let (lo, hi) = (hit.value.min(peak.value), hit.value.max(peak.value));
    hit.right = [hit.time + d / 3.0, (hit.value + s * d / 3.0).clamp(lo, hi)];
    hit.interpolation = Interpolation::Bezier;
    peak.left = [peak.time - d / 3.0, peak.value];
}

/// the keys from `a`'s hit to `b`'s hit, both included
fn join(a: &Phased, b: &Phased) -> Vec<Key> {
    let gap = b.hit - a.hit;
    let stroke_time = b.hit - b.windup_time;
    let stroke = &b.keys[b.windup..=b.hit_index];
    let tail = &a.keys[a.hit_index..];

    // as keyed: the old rest is over before the new windup starts, a level ease between the two rests. notes that
    // touch go on below: a curve that keeps going the same way through rest is carried there instead of stopping
    if b.start - a.end >= EDGE {
        let mut keys: Vec<Key> = tail.to_vec();
        let mut lead = b.keys[..=b.hit_index].to_vec();
        let last = keys.len() - 1;
        let third = (lead[0].time - keys[last].time) / 3.0;
        keys[last].interpolation = Interpolation::Bezier;
        keys[last].right = [keys[last].time + third, keys[last].value];
        lead[0].left = [lead[0].time - third, lead[0].value];
        keys.extend(lead);
        return keys;
    }

    // rest cut: the new windup starts during the old rest, the rest is cut there and the windup rises from it (with no
    // windup, the stroke into the hit). only from the same side, the other would turn it upside down
    let mut old = a.keys.clone();
    let at = cut(&mut old, b.start.min(a.end));
    let pin = if b.windup > 0 {
        b.windup
    } else {
        b.hit_index
    };
    let (base, peak) = (b.keys[0].value, b.keys[pin].value);
    let (from, keyed) = (old[at].value - peak, base - peak);
    let tiny = 1e-9 * (1.0 + base.abs());
    // more to travel in the same time is that much faster, fine while it stays under the notes' top speed
    let fits = || from.abs() <= keyed.abs() + tiny || (keyed.abs() > tiny && pin > 0 && top_speed(&b.keys[..=pin]) * from.abs() / keyed.abs() <= a.speed.max(b.speed));
    if a.recoil_time < b.start - EDGE && pin > 0 && (from * keyed >= 0.0 || from.abs() <= tiny) && fits() {
        let mut keys: Vec<Key> = old[a.hit_index..=at].to_vec();
        let lead = rebased(&b.keys[..=pin], old[at].value);
        // the key where they meet: the old rest's side before it, the new windup's after, smooth across
        let j = keys.len() - 1;
        let mut meet = Key::with_handles(lead[0].time, lead[0].value, lead[0].interpolation, keys[j].left, lead[0].right);
        let (prev, next) = (keys[j - 1].clone(), lead[1].clone());
        let s = auto_slope(&prev, &meet, &next);
        let left = (meet.time - meet.left[0]).max((meet.time - prev.time) / 3.0);
        let right = (meet.right[0] - meet.time).max((next.time - meet.time) / 3.0);
        meet.left = [meet.time - left, meet.value - s * left];
        meet.right = [meet.time + right, meet.value + s * right];
        keys[j] = meet;
        keys.extend(lead.into_iter().skip(1));
        if pin == b.windup {
            keys.extend(stroke.iter().skip(1).cloned());
        }
        return keys;
    }

    let recoil_slope = slope(&a.keys, a.hit, false);
    let mut hit = a.keys[a.hit_index].clone();
    // the way up from the old hit to the stroke shrunk by rho takes this long at the notes' top speed
    let speed = a.speed.max(b.speed).max(1e-12);
    let (b_hit, peak) = (&b.keys[b.hit_index], &stroke[0]);
    let way_up = |rho: f64| RISE * (b_hit.value + (peak.value - b_hit.value) * rho - hit.value).abs() / speed;
    // full windup: room for the new stroke and the way up to it, none for the old recoil
    if gap - stroke_time >= way_up(1.0) || stroke.len() < 2 {
        let mut stroke = stroke.to_vec();
        rise(&mut hit, &mut stroke[0], recoil_slope);
        let mut keys = vec![hit];
        keys.extend(stroke);
        return keys;
    }
    // shrunk: the new stroke shrunk around its hit (same speed) until the way up to it fits in what's left
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..40 {
        let rho = 0.5 * (lo + hi);
        if rho * stroke_time + way_up(rho) <= gap {
            lo = rho;
        } else {
            hi = rho;
        }
    }
    // never closer to the old hit than blender keeps keys apart
    let rho = lo.max(2.0 * EDGE / stroke_time).min((gap - EDGE) / stroke_time);
    let mut stroke = shrunk(stroke, b_hit, rho);
    rise(&mut hit, &mut stroke[0], recoil_slope);
    let mut keys = vec![hit];
    keys.extend(stroke);
    keys
}

/// `right` added to `left` where it starts: one key there with `left`'s value and side before it, `right`'s after
fn append(left: &mut Vec<Key>, right: Vec<Key>) {
    let mut right = right.into_iter();
    if let (Some(end), Some(start)) = (left.last_mut(), right.next()) {
        end.right = start.right;
        end.interpolation = start.interpolation;
    }
    left.extend(right);
}

// MARK: - combining

/// the notes on one curve combined with momentum, as keys ready to write. notes are put in hit order, a note that hits
/// with the one before it (a chord on one curve) is left out, notes without keys are skipped
pub fn momentum_keys(mut notes: Vec<MomentumNote>) -> Vec<Key> {
    notes.retain(|note| !note.keys.is_empty());
    notes.sort_by(|a, b| a.hit.total_cmp(&b.hit));
    notes.dedup_by(|later, earlier| later.hit - earlier.hit < SAME_HIT);
    if notes.is_empty() {
        return vec![];
    }

    // measured as keyed, dampened by the room on each side, then cut at the phase points
    let mut cache = Vec::new();
    let phased: Vec<Phased> = notes
        .iter()
        .enumerate()
        .map(|(i, note)| {
            let span = note.keys[note.keys.len() - 1].time - note.keys[0].time;
            let prev_gap = (i > 0).then(|| note.hit - notes[i - 1].hit);
            let next_gap = notes.get(i + 1).map(|next| next.hit - note.hit);
            let mut lead = side_amount(prev_gap, span, note.dampen);
            let mut tail = side_amount(next_gap, span, note.dampen);
            let pivot = if note.use_anchor_value {
                evaluate(&note.keys, note.hit).unwrap_or(0.0)
            } else {
                // one amount for the whole note, two would split the curve at the hit
                lead = lead.min(tail);
                tail = lead;
                note.keys[0].value
            };
            let (found, speed) = measured(&mut cache, &note.keys, note.hit);
            // set ones that aren't numbers or are on the wrong side of the hit (a time mapper can turn a note around)
            // are found on the keys
            let points = note.phases.filter(|&[windup, recoil]| windup.is_finite() && recoil.is_finite() && windup < note.hit && recoil > note.hit).unwrap_or([note.hit + found[0], note.hit + found[1]]);
            phased(dampened(&note.keys, note.hit, pivot, lead, tail), note.hit, points, speed)
        })
        .collect();

    let first = &phased[0];
    if phased.len() == 1 {
        return first.keys.clone();
    }
    let mut out: Vec<Key> = first.keys[..=first.hit_index].to_vec();
    for pair in phased.windows(2) {
        append(&mut out, join(&pair[0], &pair[1]));
    }
    let last = &phased[phased.len() - 1];
    append(&mut out, last.keys[last.hit_index..].to_vec());
    out
}
