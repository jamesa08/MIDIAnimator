// the momentum overlap mode. notes keep their keyed timing, so nothing moves faster than it was keyed: where two notes
// crowd each other, the end of the old note's tail and the start of the new note's lead are trimmed off, and the two
// are joined by one bezier segment (a "bridge") that matches value and slope on both sides. a tight gap gives a low
// bridge on its own, the motion shrinks instead of speeding up. dampening shrinks crowded notes further on purpose.
// works on any keyed curve: hits are each note's note on, the anchor point already puts the keyed hit there

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
}

// sampling step, seconds: a twentieth of a frame at 24 fps
const STEP: f64 = 0.05 / 24.0;

// how close a bridge comes to a hit, seconds
const EDGE: f64 = 0.01 / 24.0;

// how close two hits are to count as one (a chord on one curve), seconds
const SAME_HIT: f64 = 2.0 * EDGE;

// keys this close are one key when the curve is written (`finish_keys`), seconds. a cut this close to a key uses it
const SAME_KEY: f64 = 1e-6;

// a bridge never moves faster than the notes ever do, and turns at most this many times harder. the turn limit is what
// keeps a bridge from snapping across a gap in a slow part of the curve
const TURN_LIMIT: f64 = 1.5;

// sharp keys (vector handles, a drumstick's hit) are left out of how hard a note turns, this far around them
const SHARP_WINDOW: f64 = 0.25 / 24.0;

/// how a note's curve moves, measured once on its keys as keyed (before dampening)
#[derive(Debug, Clone, Copy)]
struct Shape {
    /// seconds of keys before and after the hit
    lead: f64,
    tail: f64,
    /// fastest it moves (value per second) and hardest it turns (value per second squared)
    vmax: f64,
    amax: f64,
    lo: f64,
    hi: f64,
}

fn shape(keys: &[Key], hit: f64) -> Shape {
    let (first, last) = (keys[0].time, keys[keys.len() - 1].time);
    let sharp: Vec<f64> = keys.iter().filter(|key| (slope(keys, key.time, true) - slope(keys, key.time, false)).abs() > 1e-6 * (1.0 + slope(keys, key.time, true).abs())).map(|key| key.time).collect();
    let n = ((last - first) / STEP).ceil().max(1.0) as usize;
    let values: Vec<f64> = (0..=n).map(|i| evaluate(keys, first + (last - first) * i as f64 / n as f64).unwrap_or(0.0)).collect();
    let h = (last - first) / n as f64;
    let (mut vmax, mut amax, mut lo, mut hi) = (0.0f64, 0.0f64, f64::INFINITY, f64::NEG_INFINITY);
    for (i, &v) in values.iter().enumerate() {
        lo = lo.min(v);
        hi = hi.max(v);
        if i > 0 && h > 0.0 {
            vmax = vmax.max((v - values[i - 1]).abs() / h);
        }
        if i > 0 && i < n && h > 0.0 {
            let t = first + h * i as f64;
            if !sharp.iter().any(|s| (t - s).abs() < SHARP_WINDOW.max(2.0 * h)) {
                amax = amax.max((values[i + 1] - 2.0 * v + values[i - 1]).abs() / (h * h));
            }
        }
    }
    Shape {
        lead: (hit - first).max(0.0),
        tail: (last - hit).max(0.0),
        vmax,
        amax,
        lo,
        hi,
    }
}

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

/// a key at `t` for cutting a curve there, a key already this close is used instead, or writing the curve would merge
/// the two and drop one's handles
fn cut(keys: &mut Vec<Key>, t: f64) -> usize {
    match keys.iter().position(|key| (key.time - t).abs() <= SAME_KEY) {
        Some(i) => i,
        None => split_at(keys, t),
    }
}

/// a side's dampening from the gap to its neighbor: 1 with room for the whole note, down to 1 - dampen as it crowds
fn side_amount(gap: Option<f64>, span: f64, dampen: f64) -> f64 {
    match gap {
        Some(gap) if span > 0.0 => 1.0 - dampen * (1.0 - (gap / span).min(1.0)),
        _ => 1.0,
    }
}

/// a bridge from `t0` on the old note's curve to `t1` on the new one's, a cubic hermite (as a bezier segment: handles a
/// third of the way along at each end's slope)
#[derive(Debug, Clone, Copy)]
struct Bridge {
    t0: f64,
    v0: f64,
    m0: f64,
    t1: f64,
    v1: f64,
    m1: f64,
}

impl Bridge {
    fn at(&self, t: f64) -> f64 {
        let h = self.t1 - self.t0;
        let u = (t - self.t0) / h;
        let (u2, u3) = (u * u, u * u * u);
        (2.0 * u3 - 3.0 * u2 + 1.0) * self.v0 + (u3 - 2.0 * u2 + u) * h * self.m0 + (-2.0 * u3 + 3.0 * u2) * self.v1 + (u3 - u2) * h * self.m1
    }
}

/// what both notes allow a bridge between them
struct Limits {
    vmax: f64,
    amax: f64,
    lo: f64,
    hi: f64,
}

/// how a bridge from `t0` to `t1` does against the limits: whether it passes, and a score to pick the closest one when
/// none does. its speed m0 + c x + d x^2 is a quadratic, so the checks are exact, no sampling
fn check(a: &[Key], b: &[Key], t0: f64, t1: f64, limits: &Limits) -> (Bridge, bool, f64) {
    let bridge = Bridge {
        t0,
        v0: evaluate(a, t0).unwrap_or(0.0),
        m0: slope(a, t0, true),
        t1,
        v1: evaluate(b, t1).unwrap_or(0.0),
        m1: slope(b, t1, false),
    };
    let Bridge {
        v0,
        m0,
        v1,
        m1,
        ..
    } = bridge;
    let h = t1 - t0;
    let c = 6.0 * (v1 - v0) / (h * h) - (4.0 * m0 + 2.0 * m1) / h;
    let d = -6.0 * (v1 - v0) / (h * h * h) + 3.0 * (m0 + m1) / (h * h);
    let speed = |x: f64| (m0 + c * x + d * x * x).abs();
    let mut top = speed(0.0).max(speed(h));
    if d.abs() > 1e-12 {
        let x = -c / (2.0 * d);
        if x > 0.0 && x < h {
            top = top.max(speed(x));
        }
    }
    // the range: the ends and wherever the speed is zero
    let (mut lo, mut hi) = (v0.min(v1), v0.max(v1));
    let disc = c * c - 4.0 * d * m0;
    if disc >= 0.0 && d.abs() > 1e-12 {
        for x in [(-c + disc.sqrt()) / (2.0 * d), (-c - disc.sqrt()) / (2.0 * d)] {
            if x > 0.0 && x < h {
                let v = bridge.at(t0 + x);
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
    }
    let turn = c.abs().max((c + 2.0 * d * h).abs());
    let tolerance = 1e-3 * (limits.hi - limits.lo);
    let speed_limit = limits.vmax.max(1e-12);
    let turn_limit = (TURN_LIMIT * limits.amax).max(1e-12);
    let ok = top <= speed_limit * 1.05 && turn <= turn_limit && lo >= limits.lo - tolerance && hi <= limits.hi + tolerance;
    let over = (limits.lo - lo).max(hi - limits.hi).max(0.0) / (limits.hi - limits.lo).max(1e-12);
    (bridge, ok, top / speed_limit + turn / turn_limit + 10.0 * over)
}

/// the bridge between two crowded notes: around the balanced point (the old tail and the new lead trimmed from their
/// far ends, the end further from its own hit first), the narrowest that passes. each width tries a few placements
/// from centered outwards, so a bridge can grow into whichever side has room. none passes: the one that comes closest
fn bridge(a: &[Key], b: &[Key], hit_a: f64, hit_b: f64, shape_a: &Shape, shape_b: &Shape) -> Bridge {
    let gap = hit_b - hit_a;
    let kept_tail = if shape_a.tail < gap / 2.0 {
        shape_a.tail
    } else if shape_b.lead < gap / 2.0 {
        gap - shape_b.lead
    } else {
        gap / 2.0
    };
    let balanced = hit_a + kept_tail;
    let (start, end) = (hit_a + EDGE, hit_b - EDGE);
    let limits = Limits {
        vmax: shape_a.vmax.max(shape_b.vmax),
        amax: shape_a.amax.max(shape_b.amax),
        lo: shape_a.lo.min(shape_b.lo),
        hi: shape_a.hi.max(shape_b.hi),
    };
    let room = end - start;
    let mut best: Option<(Bridge, f64)> = None;
    let mut width = STEP.min(room);
    loop {
        let placements = ((width / STEP).round() as usize).clamp(1, 8);
        // centered, then into the old tail before the new lead: the doc's order, an anticipation outranks a recoil
        let mut offsets = vec![0.0];
        for k in 1..=placements {
            let o = k as f64 / placements as f64 * width / 2.0;
            offsets.push(-o);
            offsets.push(o);
        }
        for offset in offsets {
            let mut t0 = balanced - width / 2.0 + offset;
            let mut t1 = t0 + width;
            if t0 < start {
                t1 += start - t0;
                t0 = start;
            }
            if t1 > end {
                t0 -= t1 - end;
                t1 = end;
            }
            let t0 = t0.max(start);
            if t1 - t0 < 1e-9 {
                continue;
            }
            let (bridge, ok, score) = check(a, b, t0, t1, &limits);
            if ok {
                return bridge;
            }
            if best.as_ref().map_or(true, |(_, s)| score < *s) {
                best = Some((bridge, score));
            }
        }
        if width >= room {
            break;
        }
        // fine steps while narrow, then 5% at a time
        width = (width + STEP.max(width * 0.05)).min(room);
    }
    best.map(|(bridge, _)| bridge).unwrap_or(Bridge {
        t0: balanced,
        v0: evaluate(a, balanced).unwrap_or(0.0),
        m0: 0.0,
        t1: balanced,
        v1: evaluate(b, balanced).unwrap_or(0.0),
        m1: 0.0,
    })
}

/// the notes on one curve combined with momentum, as keys ready to write. notes are put in hit order, a note that hits
/// with the one before it (a chord on one curve) is left out, notes without keys are skipped
pub fn momentum_keys(mut notes: Vec<MomentumNote>) -> Vec<Key> {
    notes.retain(|note| !note.keys.is_empty());
    notes.sort_by(|a, b| a.hit.total_cmp(&b.hit));
    notes.dedup_by(|later, earlier| later.hit - earlier.hit < SAME_HIT);
    if notes.is_empty() {
        return vec![];
    }

    // measured as keyed, then dampened by the room on each side
    let shapes: Vec<Shape> = notes.iter().map(|note| shape(&note.keys, note.hit)).collect();
    let curves: Vec<Vec<Key>> = notes
        .iter()
        .enumerate()
        .map(|(i, note)| {
            let span = shapes[i].lead + shapes[i].tail;
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
            dampened(&note.keys, note.hit, pivot, lead, tail)
        })
        .collect();

    // a bridge between each two notes that crowd each other (or rest at different values), none between ones that don't
    let bridges: Vec<Option<Bridge>> = (0..notes.len().saturating_sub(1))
        .map(|i| {
            let (a, b) = (&curves[i], &curves[i + 1]);
            let (a_end, b_start) = (&a[a.len() - 1], &b[0]);
            if a_end.time <= b_start.time && (a_end.value - b_start.value).abs() < 1e-9 {
                return None;
            }
            Some(bridge(a, b, notes[i].hit, notes[i + 1].hit, &shapes[i], &shapes[i + 1]))
        })
        .collect();

    // each note's curve from the end of the bridge before it to the start of the bridge after it
    let mut out = Vec::new();
    for (i, curve) in curves.into_iter().enumerate() {
        let mut keys = curve;
        if let Some(Some(bridge)) = i.checked_sub(1).map(|p| bridges[p]) {
            let at = cut(&mut keys, bridge.t1);
            keys.drain(..at);
            let third = (bridge.t1 - bridge.t0) / 3.0;
            keys[0].left = [bridge.t1 - third, bridge.v1 - bridge.m1 * third];
        }
        if let Some(Some(bridge)) = bridges.get(i) {
            let at = cut(&mut keys, bridge.t0);
            keys.truncate(at + 1);
            let third = (bridge.t1 - bridge.t0) / 3.0;
            let end = &mut keys[at];
            end.interpolation = Interpolation::Bezier;
            end.right = [bridge.t0 + third, bridge.v0 + bridge.m0 * third];
        }
        // notes that just touch (one ends where the next starts) share that key: the old note's side before it, the
        // new one's after. two keys there would leave one, and one note's shape with it
        if let (Some(end), Some(start)) = (out.last_mut(), keys.first()) {
            let end: &mut Key = end;
            if (start.time - end.time).abs() <= SAME_KEY {
                end.right = start.right;
                end.interpolation = start.interpolation;
                keys.remove(0);
            }
        }
        out.extend(keys);
    }
    out
}
