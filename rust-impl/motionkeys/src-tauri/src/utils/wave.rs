// the Preset Animation node's shapes: functions of time with their parameters, baked into bezier keys that follow them
// within a tolerance. times are seconds from the note, the hit is at 0

use std::f64::consts::PI;

use crate::utils::curve::{auto_handles, segment_value, Interpolation, Key};

/// how far a baked curve may be from its function, a share of how far the function moves (the bridges' tolerance, §14)
pub const TOLERANCE: f64 = 0.001;

// how many samples look for turns in each piece of a function, and check each baked segment
const SEARCH_SAMPLES: usize = 512;
const CHECK_SAMPLES: usize = 24;
// how many times a segment can be split in half to follow its function
const MAX_SPLITS: u32 = 12;

// MARK: - Spring

/// a damped sine: still at `rest` until `start`, swings back and forth once every `period` seconds while it dies down,
/// and is back at rest exactly at `end`. the first swing goes up, `flip` sends it down
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    pub start: f64,
    pub end: f64,
    pub amplitude: f64,
    /// seconds for one swing there and back
    pub period: f64,
    /// 0 to 1: 0 dies down in a straight line, 1 is gone after the first few swings
    pub dampen: f64,
    pub rest: f64,
    pub flip: bool,
}

// how quickly the dampening at 1 dies down, over the spring's length
const MAX_DECAY: f64 = 10.0;

impl Spring {
    fn length(&self) -> f64 {
        self.end - self.start
    }

    /// the swing's speed in radians per second
    fn omega(&self) -> f64 {
        2.0 * PI / self.period
    }

    fn scale(&self) -> f64 {
        if self.flip {
            -self.amplitude
        } else {
            self.amplitude
        }
    }

    /// the envelope over the spring, 1 at its start down to 0 at its end, and its slope per second. an exponential
    /// decay moved and scaled to land on 0, a straight line without dampening
    fn envelope(&self, tau: f64) -> (f64, f64) {
        let length = self.length();
        let u = tau / length;
        let d = self.dampen * MAX_DECAY;
        if d < 1e-9 {
            return (1.0 - u, -1.0 / length);
        }
        let floor = (-d).exp();
        let span = 1.0 - floor;
        (((-d * u).exp() - floor) / span, -d * (-d * u).exp() / span / length)
    }

    /// how far from rest it is at `t`, without its amplitude
    fn unit(&self, t: f64) -> f64 {
        let tau = t - self.start;
        if tau <= 0.0 || tau >= self.length() {
            return 0.0;
        }
        self.envelope(tau).0 * (self.omega() * tau).sin()
    }

    /// the slope of `unit`, on the side of `t` after it when `after`. 0 where it's at rest
    fn unit_slope(&self, t: f64, after: bool) -> f64 {
        let tau = t - self.start;
        let outside = tau < 0.0 || tau > self.length() || (tau == 0.0 && !after) || (tau == self.length() && after);
        if outside {
            return 0.0;
        }
        let (env, env_slope) = self.envelope(tau);
        let w = self.omega();
        env_slope * (w * tau).sin() + env * w * (w * tau).cos()
    }

    pub fn value(&self, t: f64) -> f64 {
        self.rest + self.scale() * self.unit(t)
    }

    pub fn slope(&self, t: f64, after: bool) -> f64 {
        self.scale() * self.unit_slope(t, after)
    }

    /// the times its slope jumps at
    fn breaks(&self) -> [f64; 2] {
        [self.start, self.end]
    }

    pub fn keys(&self) -> Vec<Key> {
        bake(|t| self.value(t), |t, after| self.slope(t, after), &self.breaks())
    }
}

// MARK: - Cymbal

/// a cymbal struck at its edge. it rocks two ways at once, like a cymbal does: once a `period` and a little slower,
/// `detune` (a share, 0.06 is 6%) longer. both start together, so it first rocks toward where it's struck, then they
/// drift apart and the rocking opens into a wobble around the stand. worked out in the strike's own direction and turned
/// to `angle` (radians), so the whole crash turns with it. the two axes are the cymbal's tilt on each of its two curves,
/// at 45 degrees each axis is one of the two swings
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cymbal {
    /// the faster swing, its amplitude is the whole tilt. its rest and flip are ignored (each axis has its own rest)
    pub spring: Spring,
    pub angle: f64,
    pub detune: f64,
    pub rest: [f64; 2],
}

impl Cymbal {
    fn slower(&self) -> Spring {
        Spring {
            period: self.spring.period * (1.0 + self.detune),
            ..self.spring
        }
    }

    /// the tilt on both axes from the two swings: along the strike their average, across it half their difference,
    /// turned to the angle
    fn tilt(&self, fast: f64, slow: f64) -> [f64; 2] {
        let (along, across) = (0.5 * (fast + slow), 0.5 * (slow - fast));
        let (sin, cos) = self.angle.sin_cos();
        [cos * along - sin * across, sin * along + cos * across]
    }

    pub fn value(&self, axis: usize, t: f64) -> f64 {
        let (fast, slow) = (self.spring, self.slower());
        self.rest[axis] + fast.amplitude * self.tilt(fast.unit(t), slow.unit(t))[axis]
    }

    pub fn slope(&self, axis: usize, t: f64, after: bool) -> f64 {
        let (fast, slow) = (self.spring, self.slower());
        fast.amplitude * self.tilt(fast.unit_slope(t, after), slow.unit_slope(t, after))[axis]
    }

    /// the keys of each axis
    pub fn keys(&self) -> [Vec<Key>; 2] {
        let breaks = self.spring.breaks();
        [0, 1].map(|axis| bake(|t| self.value(axis, t), |t, after| self.slope(axis, t, after), &breaks))
    }
}

// MARK: - Drumstick

/// a drumstick's stroke in its four phases, each a length in seconds and the value it ends at. it starts at `rest`, the
/// windup lifts it to `windup_value`, the hit falls faster and faster onto `hit_value` at 0 (the note), the recoil
/// knocks it back up to `recoil_value` and the rest settles it back at rest. the hit is a parabola, like a throw, and
/// the recoil out of it slows down the same way. the other keys get MotionKeys' own smooth handles
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drumstick {
    pub windup: f64,
    pub windup_value: f64,
    pub hit: f64,
    pub hit_value: f64,
    pub recoil: f64,
    pub recoil_value: f64,
    /// how long the rest phase is, `rest` is the value it settles at (and starts from)
    pub settle: f64,
    pub rest: f64,
}

impl Drumstick {
    /// the first phase that takes no time, none when each one takes some
    pub fn check(&self) -> Option<&'static str> {
        let phases = [("windup", self.windup), ("hit", self.hit), ("recoil", self.recoil), ("rest", self.settle)];
        phases.iter().find(|(_, length)| *length <= 0.0).map(|(name, _)| *name)
    }

    pub fn keys(&self) -> Vec<Key> {
        let mut keys = vec![Key::auto(-self.hit - self.windup, self.rest), Key::auto(-self.hit, self.windup_value), Key::auto(0.0, self.hit_value), Key::auto(self.recoil, self.recoil_value), Key::auto(self.recoil + self.settle, self.rest)];
        auto_handles(&mut keys);
        // the hit's handles: a parabola's control points are its handle at the still end and one a third of the way
        // back along it from the moving end
        let hit = &mut keys[2];
        hit.left = [-self.hit / 3.0, self.hit_value + (self.windup_value - self.hit_value) * 2.0 / 3.0];
        hit.right = [self.recoil / 3.0, self.hit_value + (self.recoil_value - self.hit_value) * 2.0 / 3.0];
        keys
    }
}

// MARK: - Baking

/// a point a baked curve goes through, with its slope coming in and going out
#[derive(Clone, Copy)]
struct Point {
    time: f64,
    value: f64,
    slope_in: f64,
    slope_out: f64,
}

/// bezier keys that follow `value` from the first break to the last within `TOLERANCE`. `slope(t, after)` is the
/// function's slope on either side of `t`, `breaks` are where it jumps (and where the function starts and ends moving,
/// it holds past them). keys go on the breaks and every turn, each with its slope's handles a third of the way to its
/// neighbors, and segments that stray too far get a key in their middle
pub fn bake(value: impl Fn(f64) -> f64, slope: impl Fn(f64, bool) -> f64, breaks: &[f64]) -> Vec<Key> {
    let mut breaks: Vec<f64> = breaks.iter().copied().filter(|t| t.is_finite()).collect();
    breaks.sort_by(f64::total_cmp);
    breaks.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
    let point = |time: f64| Point {
        time,
        value: value(time),
        slope_in: slope(time, false),
        slope_out: slope(time, true),
    };

    // the breaks and every turn between them, where the slope changes sign
    let mut times = breaks.clone();
    for pair in breaks.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let step = (b - a) / SEARCH_SAMPLES as f64;
        let inside = |i: usize| a + step * i as f64;
        for i in 0..SEARCH_SAMPLES {
            let (t0, t1) = (inside(i), inside(i + 1));
            let (s0, s1) = (slope(t0, true), slope(t1, false));
            if s0 != 0.0 && s1 != 0.0 && (s0 > 0.0) != (s1 > 0.0) {
                times.push(turn(&slope, t0, t1));
            }
        }
    }
    times.sort_by(f64::total_cmp);
    times.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    let mut points: Vec<Point> = times.into_iter().map(point).collect();
    // a turn is flat, its slope only rounds to near 0
    for p in points.iter_mut() {
        if !breaks.iter().any(|t| (t - p.time).abs() < 1e-12) {
            p.slope_in = 0.0;
            p.slope_out = 0.0;
        }
    }

    // the tolerance in the function's own units
    let (low, high) = points.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), p| (low.min(p.value), high.max(p.value)));
    let tolerance = (TOLERANCE * (high - low)).max(1e-9);

    let mut out = Vec::with_capacity(points.len() * 2);
    for pair in points.windows(2) {
        follow(&value, &point, pair[0], pair[1], tolerance, MAX_SPLITS, &mut out);
    }
    if let Some(last) = points.last() {
        out.push(*last);
    }
    keys(&out)
}

/// where the slope crosses 0 between `t0` and `t1`, which it has on opposite sides
fn turn(slope: &impl Fn(f64, bool) -> f64, mut t0: f64, mut t1: f64) -> f64 {
    let up = slope(t0, true) > 0.0;
    for _ in 0..60 {
        let mid = 0.5 * (t0 + t1);
        if (slope(mid, true) > 0.0) == up {
            t0 = mid;
        } else {
            t1 = mid;
        }
    }
    0.5 * (t0 + t1)
}

/// the segment from `a` to `b` as it would be baked
fn segment(a: Point, b: Point) -> [Key; 2] {
    let third = (b.time - a.time) / 3.0;
    [Key::with_handles(a.time, a.value, Interpolation::Bezier, [a.time, a.value], [a.time + third, a.value + a.slope_out * third]), Key::with_handles(b.time, b.value, Interpolation::Bezier, [b.time - third, b.value - b.slope_in * third], [b.time, b.value])]
}

/// pushes `a` and the points between it and `b` it takes to follow the function, halving the segment while it strays
fn follow(value: &impl Fn(f64) -> f64, point: &impl Fn(f64) -> Point, a: Point, b: Point, tolerance: f64, splits: u32, out: &mut Vec<Point>) {
    let [ka, kb] = segment(a, b);
    let strays = (1..CHECK_SAMPLES).any(|i| {
        let t = a.time + (b.time - a.time) * i as f64 / CHECK_SAMPLES as f64;
        (segment_value(&ka, &kb, t) - value(t)).abs() > tolerance
    });
    if !strays || splits == 0 {
        out.push(a);
        return;
    }
    let mid = point(0.5 * (a.time + b.time));
    follow(value, point, a, mid, tolerance, splits - 1, out);
    follow(value, point, mid, b, tolerance, splits - 1, out);
}

/// the keys through the points, each handle a third of the way to its neighbor along its slope on that side. the ends'
/// outer handles stay on them
fn keys(points: &[Point]) -> Vec<Key> {
    (0..points.len())
        .map(|i| {
            let p = points[i];
            let left = match i.checked_sub(1).map(|j| points[j]) {
                Some(prev) => {
                    let third = (p.time - prev.time) / 3.0;
                    [p.time - third, p.value - p.slope_in * third]
                }
                None => [p.time, p.value],
            };
            let right = match points.get(i + 1) {
                Some(next) => {
                    let third = (next.time - p.time) / 3.0;
                    [p.time + third, p.value + p.slope_out * third]
                }
                None => [p.time, p.value],
            };
            Key::with_handles(p.time, p.value, Interpolation::Bezier, left, right)
        })
        .collect()
}
