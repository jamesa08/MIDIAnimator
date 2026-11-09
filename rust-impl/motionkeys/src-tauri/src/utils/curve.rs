// curves as MotionKeys and every bridge has them (docs/bridge_protocol.md §8.6, §8.7): keys in time order, each with
// its interpolation to the next key and its two Bezier handles as absolute (time, value) points, held past both ends.
// nothing here is any 3D app's, a bridge converts its app's curves into these and writes them back as they are

use serde::de::{self, SeqAccess, Visitor};
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// how a curve gets from a key to the next one
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Interpolation {
    #[default]
    Bezier,
    Linear,
    Constant,
}

impl Interpolation {
    pub fn name(self) -> &'static str {
        match self {
            Interpolation::Bezier => "bezier",
            Interpolation::Linear => "linear",
            Interpolation::Constant => "constant",
        }
    }

    fn parse(name: &str) -> Option<Self> {
        match name {
            "bezier" => Some(Interpolation::Bezier),
            "linear" => Some(Interpolation::Linear),
            "constant" => Some(Interpolation::Constant),
            _ => None,
        }
    }
}

/// one key. `interpolation` is for the segment after it, `left` shapes the segment before it and `right` the one after
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, schemars::JsonSchema)]
pub struct Key {
    /// seconds
    pub time: f64,
    pub value: f64,
    #[serde(default)]
    pub interpolation: Interpolation,
    /// the left handle, an absolute [time, value] point
    pub left: [f64; 2],
    /// the right handle, an absolute [time, value] point
    pub right: [f64; 2],
    /// made by MotionKeys (where notes overlap), `auto_handles` gives it handles once its neighbors are known
    #[serde(skip)]
    #[schemars(skip)]
    pub auto: bool,
}

impl Key {
    /// a key with both handles on it. as a bezier that's a straight line to a key whose handle is on it too
    pub fn new(time: f64, value: f64, interpolation: Interpolation) -> Self {
        Self {
            time,
            value,
            interpolation,
            left: [time, value],
            right: [time, value],
            auto: false,
        }
    }

    pub fn linear(time: f64, value: f64) -> Self {
        Self::new(time, value, Interpolation::Linear)
    }

    /// a key MotionKeys made, smooth handles come later (`auto_handles`)
    pub fn auto(time: f64, value: f64) -> Self {
        Self {
            auto: true,
            ..Self::new(time, value, Interpolation::Bezier)
        }
    }

    pub fn with_handles(time: f64, value: f64, interpolation: Interpolation, left: [f64; 2], right: [f64; 2]) -> Self {
        Self {
            time,
            value,
            interpolation,
            left,
            right,
            auto: false,
        }
    }

    /// the key moved by `dt` seconds, its handles with it
    pub fn moved(&self, dt: f64) -> Self {
        Self {
            time: self.time + dt,
            left: [self.left[0] + dt, self.left[1]],
            right: [self.right[0] + dt, self.right[1]],
            ..self.clone()
        }
    }
}

// MARK: - Evaluation

/// a bezier segment's control points from key `a` to key `b`, each handle kept on its own side of its key and no longer
/// in time than the segment (§8.7 steps 1 and 2), so the curve never goes back in time
pub fn segment_points(a: &Key, b: &Key) -> [[f64; 2]; 4] {
    let p0 = [a.time, a.value];
    let p3 = [b.time, b.value];
    let mut p1 = [a.right[0].max(a.time), a.right[1]];
    let mut p2 = [b.left[0].min(b.time), b.left[1]];
    let d = b.time - a.time;
    let l1 = p1[0] - p0[0];
    if l1 > d && l1 > 0.0 {
        let f = d / l1;
        p1 = [p0[0] + (p1[0] - p0[0]) * f, p0[1] + (p1[1] - p0[1]) * f];
    }
    let l2 = p3[0] - p2[0];
    if l2 > d && l2 > 0.0 {
        let f = d / l2;
        p2 = [p3[0] + (p2[0] - p3[0]) * f, p3[1] + (p2[1] - p3[1]) * f];
    }
    [p0, p1, p2, p3]
}

fn cubic(p0: f64, p1: f64, p2: f64, p3: f64, u: f64) -> f64 {
    let s = 1.0 - u;
    s * s * s * p0 + 3.0 * s * s * u * p1 + 3.0 * s * u * u * p2 + u * u * u * p3
}

fn cubic_slope(p0: f64, p1: f64, p2: f64, p3: f64, u: f64) -> f64 {
    let s = 1.0 - u;
    3.0 * s * s * (p1 - p0) + 6.0 * s * u * (p2 - p1) + 3.0 * u * u * (p3 - p2)
}

// how close the solved time has to be, seconds
const TIME_TOLERANCE: f64 = 1e-9;

/// the curve parameter where a segment's time reaches `t`. the time never goes back after `segment_points`, so
/// bisection always works, Newton's method gets there first
fn solve_u(p: &[[f64; 2]; 4], t: f64) -> f64 {
    let (t0, t1, t2, t3) = (p[0][0], p[1][0], p[2][0], p[3][0]);
    let d = t3 - t0;
    if d <= 0.0 {
        return 0.0;
    }
    let mut u = ((t - t0) / d).clamp(0.0, 1.0);
    // handles at a third and two thirds: time runs straight along the segment
    if (t1 - (t0 + d / 3.0)).abs() <= TIME_TOLERANCE && (t2 - (t3 - d / 3.0)).abs() <= TIME_TOLERANCE {
        return u;
    }
    for _ in 0..8 {
        let error = cubic(t0, t1, t2, t3, u) - t;
        if error.abs() <= TIME_TOLERANCE {
            return u;
        }
        let slope = cubic_slope(t0, t1, t2, t3, u);
        if slope.abs() < 1e-12 {
            break;
        }
        let next = u - error / slope;
        if !(0.0..=1.0).contains(&next) {
            break;
        }
        u = next;
    }
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..100 {
        u = 0.5 * (lo + hi);
        let x = cubic(t0, t1, t2, t3, u);
        if (x - t).abs() <= TIME_TOLERANCE {
            break;
        }
        if x < t {
            lo = u;
        } else {
            hi = u;
        }
    }
    u
}

/// the index of the key that starts the segment `t` is in, for a time inside the keys
fn segment_index(keys: &[Key], t: f64) -> usize {
    keys.partition_point(|key| key.time <= t).saturating_sub(1).min(keys.len().saturating_sub(2))
}

/// the curve's value at `t`, held before the first key and after the last. none without keys
pub fn evaluate(keys: &[Key], t: f64) -> Option<f64> {
    let first = keys.first()?;
    let last = keys.last()?;
    if t <= first.time {
        return Some(first.value);
    }
    if t >= last.time {
        return Some(last.value);
    }
    let i = segment_index(keys, t);
    Some(segment_value(&keys[i], &keys[i + 1], t))
}

/// the value at `t` on the segment from `a` to `b`
pub fn segment_value(a: &Key, b: &Key, t: f64) -> f64 {
    match a.interpolation {
        Interpolation::Constant => a.value,
        Interpolation::Linear => {
            let d = b.time - a.time;
            if d <= 0.0 {
                a.value
            } else {
                a.value + (b.value - a.value) * (t - a.time) / d
            }
        }
        Interpolation::Bezier => {
            let p = segment_points(a, b);
            let u = solve_u(&p, t);
            cubic(p[0][1], p[1][1], p[2][1], p[3][1], u)
        }
    }
}

/// how fast the curve changes at `t`, in value per second: on the segment before `t` when `before`, else the one after.
/// 0 past the ends, where it holds
pub fn slope(keys: &[Key], t: f64, before: bool) -> f64 {
    let (Some(first), Some(last)) = (keys.first(), keys.last()) else {
        return 0.0;
    };
    if (before && t <= first.time) || (!before && t >= last.time) || t < first.time || t > last.time {
        return 0.0;
    }
    // the segment that holds t, the one ending at t when looking before a key
    let i = if before {
        keys.partition_point(|key| key.time < t).saturating_sub(1)
    } else {
        keys.partition_point(|key| key.time <= t).saturating_sub(1)
    }
    .min(keys.len() - 2);
    let (a, b) = (&keys[i], &keys[i + 1]);
    match a.interpolation {
        Interpolation::Constant => 0.0,
        Interpolation::Linear => {
            let d = b.time - a.time;
            if d <= 0.0 {
                0.0
            } else {
                (b.value - a.value) / d
            }
        }
        Interpolation::Bezier => {
            let p = segment_points(a, b);
            let mut u = solve_u(&p, t);
            // a handle on its key has no direction there, a step inside shows where the curve heads
            let mut dt = cubic_slope(p[0][0], p[1][0], p[2][0], p[3][0], u);
            if dt.abs() < 1e-12 {
                u = if u < 0.5 {
                    1e-6
                } else {
                    1.0 - 1e-6
                };
                dt = cubic_slope(p[0][0], p[1][0], p[2][0], p[3][0], u);
            }
            if dt.abs() < 1e-12 {
                return 0.0;
            }
            cubic_slope(p[0][1], p[1][1], p[2][1], p[3][1], u) / dt
        }
    }
}

// MARK: - Making keys

/// handles for the keys MotionKeys made (`auto`): level at a peak, a valley and the curve's ends, otherwise along the
/// line between the neighbors, a third of the way to each and never past a neighbor's value. the keys become beziers
pub fn auto_handles(keys: &mut [Key]) {
    for i in 0..keys.len() {
        if !keys[i].auto {
            continue;
        }
        let (time, value) = (keys[i].time, keys[i].value);
        let prev = (i > 0).then(|| (keys[i - 1].time, keys[i - 1].value));
        let next = keys.get(i + 1).map(|key| (key.time, key.value));
        let slope = match (prev, next) {
            (Some((tp, vp)), Some((tn, vn))) if (value - vp) * (vn - value) > 0.0 && tn > tp => (vn - vp) / (tn - tp),
            _ => 0.0,
        };
        let key = &mut keys[i];
        key.interpolation = Interpolation::Bezier;
        key.auto = false;
        key.left = match prev {
            Some((tp, vp)) => {
                let dt = (time - tp) / 3.0;
                [time - dt, clamp_between(value - slope * dt, vp, value)]
            }
            None => [time, value],
        };
        key.right = match next {
            Some((tn, vn)) => {
                let dt = (tn - time) / 3.0;
                [time + dt, clamp_between(value + slope * dt, value, vn)]
            }
            None => [time, value],
        };
    }
}

fn clamp_between(value: f64, a: f64, b: f64) -> f64 {
    value.clamp(a.min(b), a.max(b))
}

/// puts a key at `t` without changing the curve and returns its index, or the index of a key already there. a bezier
/// segment is split in two (de Casteljau), the halves keep its exact shape. past either end the held value gets a key,
/// joined to the end with a straight (flat) segment
pub fn split_at(keys: &mut Vec<Key>, t: f64) -> usize {
    if let Some(i) = keys.iter().position(|key| (key.time - t).abs() <= TIME_TOLERANCE) {
        return i;
    }
    let Some(first) = keys.first() else {
        keys.push(Key::linear(t, 0.0));
        return 0;
    };
    if t < first.time {
        let value = first.value;
        keys.insert(0, Key::linear(t, value));
        return 0;
    }
    let last = keys.len() - 1;
    if t > keys[last].time {
        let value = keys[last].value;
        keys[last].interpolation = Interpolation::Linear;
        keys.push(Key::linear(t, value));
        return last + 1;
    }
    let i = segment_index(keys, t);
    let (a, b) = (&keys[i], &keys[i + 1]);
    let key = match a.interpolation {
        Interpolation::Constant => Key::new(t, a.value, Interpolation::Constant),
        Interpolation::Linear => Key::linear(t, segment_value(a, b, t)),
        Interpolation::Bezier => {
            let p = segment_points(a, b);
            let u = solve_u(&p, t);
            let lerp = |p: [f64; 2], q: [f64; 2]| [p[0] + (q[0] - p[0]) * u, p[1] + (q[1] - p[1]) * u];
            let (p01, p12, p23) = (lerp(p[0], p[1]), lerp(p[1], p[2]), lerp(p[2], p[3]));
            let (p012, p123) = (lerp(p01, p12), lerp(p12, p23));
            let mid = lerp(p012, p123);
            keys[i].right = p01;
            keys[i + 1].left = p23;
            Key::with_handles(t, mid[1], Interpolation::Bezier, p012, p123)
        }
    };
    keys.insert(i + 1, key);
    i + 1
}

/// keys sorted by time with the last of keys closer than `tolerance` seconds kept, the wire needs strictly increasing times
pub fn sort_keys(keys: &mut Vec<Key>, tolerance: f64) {
    keys.sort_by(|a, b| a.time.total_cmp(&b.time));
    let mut out: Vec<Key> = Vec::with_capacity(keys.len());
    for key in keys.drain(..) {
        match out.last_mut() {
            Some(last) if key.time - last.time <= tolerance => *last = key,
            _ => out.push(key),
        }
    }
    *keys = out;
}

/// what's wrong with a curve's keys for the wire, none when they're fine: times strictly increasing, every number finite
pub fn check_keys(keys: &[Key]) -> Option<String> {
    for (i, key) in keys.iter().enumerate() {
        if ![key.time, key.value, key.left[0], key.left[1], key.right[0], key.right[1]].iter().all(|n| n.is_finite()) {
            return Some(format!("key {i} has a number that isn't finite"));
        }
        if i > 0 && key.time <= keys[i - 1].time {
            return Some(format!("key {i} isn't after the key before it"));
        }
    }
    None
}

// MARK: - Wire form

/// keys as the bridge protocol sends them: `[time, value, interpolation]`, or with the handles
/// `[time, value, interpolation, left_time, left_value, right_time, right_value]` when a bezier segment uses them
pub mod wire_keys {
    use super::*;

    pub fn serialize<S: Serializer>(keys: &[Key], serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(keys.len()))?;
        let mut prev_bezier = false;
        for key in keys {
            let bezier = key.interpolation == Interpolation::Bezier;
            if bezier || prev_bezier {
                seq.serialize_element(&(key.time, key.value, key.interpolation.name(), key.left[0], key.left[1], key.right[0], key.right[1]))?;
            } else {
                seq.serialize_element(&(key.time, key.value, key.interpolation.name()))?;
            }
            prev_bezier = bezier;
        }
        seq.end()
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<Key>, D::Error> {
        deserializer.deserialize_seq(KeysVisitor)
    }

    struct KeysVisitor;

    impl<'de> Visitor<'de> for KeysVisitor {
        type Value = Vec<Key>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("an array of keys")
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<Key>, A::Error> {
            let mut keys: Vec<Key> = Vec::with_capacity(seq.size_hint().unwrap_or(0));
            while let Some(raw) = seq.next_element::<Vec<serde_json::Value>>()? {
                let i = keys.len();
                let number = |n: usize| raw[n].as_f64().ok_or_else(|| de::Error::custom(format!("key {i}: element {n} isn't a number")));
                if raw.len() != 3 && raw.len() != 7 {
                    return Err(de::Error::custom(format!("key {i} has {} elements, a key has 3 or 7", raw.len())));
                }
                let (time, value) = (number(0)?, number(1)?);
                let name = raw[2].as_str().unwrap_or_default();
                let interpolation = Interpolation::parse(name).ok_or_else(|| de::Error::custom(format!("key {i}: unknown interpolation '{name}'")))?;
                let prev_bezier = keys.last().is_some_and(|prev| prev.interpolation == Interpolation::Bezier);
                let key = if raw.len() == 7 {
                    Key::with_handles(time, value, interpolation, [number(3)?, number(4)?], [number(5)?, number(6)?])
                } else if interpolation == Interpolation::Bezier || prev_bezier {
                    return Err(de::Error::custom(format!("key {i} is next to a bezier segment and has no handles")));
                } else {
                    Key::new(time, value, interpolation)
                };
                keys.push(key);
            }
            Ok(keys)
        }
    }
}

/// one key on its own in the wire form, always with its handles since the keys around it aren't known:
/// `[time, value, interpolation, left_time, left_value, right_time, right_value]`. reads the short form too
pub mod wire_key {
    use super::*;

    pub fn serialize<S: Serializer>(key: &Key, serializer: S) -> Result<S::Ok, S::Error> {
        (key.time, key.value, key.interpolation.name(), key.left[0], key.left[1], key.right[0], key.right[1]).serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Key, D::Error> {
        let raw = Vec::<serde_json::Value>::deserialize(deserializer)?;
        let number = |n: usize| raw.get(n).and_then(serde_json::Value::as_f64).ok_or_else(|| de::Error::custom(format!("element {n} of a key isn't a number")));
        let (time, value) = (number(0)?, number(1)?);
        let name = raw.get(2).and_then(serde_json::Value::as_str).unwrap_or_default();
        let interpolation = Interpolation::parse(name).ok_or_else(|| de::Error::custom(format!("unknown interpolation '{name}'")))?;
        match raw.len() {
            3 => Ok(Key::new(time, value, interpolation)),
            7 => Ok(Key::with_handles(time, value, interpolation, [number(3)?, number(4)?], [number(5)?, number(6)?])),
            n => Err(de::Error::custom(format!("a key has 3 or 7 elements, not {n}"))),
        }
    }
}

// MARK: - Drawing

/// one piece of a curve's drawing, from where the last one ended (the first starts at the first key)
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Segment {
    Bezier {
        c1: [f64; 2],
        c2: [f64; 2],
        to: [f64; 2],
    },
    Line {
        to: [f64; 2],
    },
    /// stays at the last value until `to`'s time, then jumps to it
    Step {
        to: [f64; 2],
    },
}

/// a curve ready to draw: its keys, the segments between them and the slopes it carries on with past its ends (none,
/// curves hold)
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Drawing {
    pub keys: Vec<[f64; 2]>,
    /// each key's left and right handle, none on a side that isn't next to a bezier segment
    pub handles: Vec<[Option<[f64; 2]>; 2]>,
    pub segments: Vec<Segment>,
    pub slope_before: f64,
    pub slope_after: f64,
}

/// how a curve's keys are drawn, the same curve `evaluate` gives
pub fn drawing(keys: &[Key]) -> Drawing {
    let segments = keys
        .windows(2)
        .map(|pair| {
            let (a, b) = (&pair[0], &pair[1]);
            let to = [b.time, b.value];
            match a.interpolation {
                Interpolation::Constant => Segment::Step {
                    to,
                },
                Interpolation::Linear => Segment::Line {
                    to,
                },
                Interpolation::Bezier => {
                    let [_, c1, c2, _] = segment_points(a, b);
                    Segment::Bezier {
                        c1,
                        c2,
                        to,
                    }
                }
            }
        })
        .collect();
    let bezier = |i: usize| keys[i].interpolation == Interpolation::Bezier;
    let handles = (0..keys.len())
        .map(|i| {
            let left = if i == 0 {
                bezier(i)
            } else {
                bezier(i - 1)
            };
            [left.then_some(keys[i].left), bezier(i).then_some(keys[i].right)]
        })
        .collect();
    Drawing {
        keys: keys.iter().map(|key| [key.time, key.value]).collect(),
        handles,
        segments,
        slope_before: 0.0,
        slope_after: 0.0,
    }
}
