// f-curves drawn the way Blender draws them: the handles Blender gives keys the scene writer adds, the curve between keys
// for every interpolation, and how a curve carries on past its ends. ported from Blender's curve.cc (auto handles and
// continuous acceleration smoothing), fcurve.cc (evaluation, sorting, duplicate keys) and easing.cc.
// times are seconds, like everywhere else in the graph

use serde::Serialize;
use std::f64::consts::PI;

use crate::scene_generics::{AnimCurve, KeyframePoint};

/// Blender merges keys closer than 0.01 frames when a curve is updated. the fps isn't known here, so it's 0.01 frames at 24 fps
const DUPLICATE_SECONDS: f64 = 0.01 / 24.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Interpolation {
    Constant,
    Linear,
    Bezier,
    Sine,
    Quad,
    Cubic,
    Quart,
    Quint,
    Expo,
    Circ,
    Back,
    Bounce,
    Elastic,
}

impl Interpolation {
    fn from_blender(name: &str) -> Self {
        match name {
            "CONSTANT" => Self::Constant,
            "LINEAR" => Self::Linear,
            "SINE" => Self::Sine,
            "QUAD" => Self::Quad,
            "CUBIC" => Self::Cubic,
            "QUART" => Self::Quart,
            "QUINT" => Self::Quint,
            "EXPO" => Self::Expo,
            "CIRC" => Self::Circ,
            "BACK" => Self::Back,
            "BOUNCE" => Self::Bounce,
            "ELASTIC" => Self::Elastic,
            _ => Self::Bezier,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Easing {
    Auto,
    In,
    Out,
    InOut,
}

impl Easing {
    fn from_blender(name: &str) -> Self {
        match name {
            "EASE_IN" => Self::In,
            "EASE_OUT" => Self::Out,
            "EASE_IN_OUT" => Self::InOut,
            _ => Self::Auto,
        }
    }
}

/// a keyframe with its handles. the interpolation and easing are for the segment after it
#[derive(Clone, Debug, PartialEq)]
pub struct Key {
    pub co: [f64; 2],
    pub left: [f64; 2],
    pub right: [f64; 2],
    pub interpolation: Interpolation,
    pub easing: Easing,
    pub back: f64,
    pub amplitude: f64,
    pub period: f64,
    /// auto clamp flattened a handle, the smoothing pass leaves the key alone (HD_AUTOTYPE_LOCKED_FINAL)
    locked: bool,
}

impl Key {
    fn from_point(point: &KeyframePoint) -> Option<Self> {
        let pair = |v: &[f32]| -> Option<[f64; 2]> {
            match v {
                [x, y, ..] => Some([*x as f64, *y as f64]),
                _ => None,
            }
        };
        let co = pair(&point.co)?;
        Some(Self {
            co,
            left: pair(&point.handle_left).unwrap_or(co),
            right: pair(&point.handle_right).unwrap_or(co),
            interpolation: Interpolation::from_blender(&point.interpolation),
            easing: Easing::from_blender(&point.easing),
            back: point.back as f64,
            amplitude: point.amplitude as f64,
            period: point.period as f64,
            locked: false,
        })
    }

    /// a key the way `keyframe_points.add` makes it: bezier, auto clamped handles (Blender's defaults for new keys)
    fn added(time: f64, value: f64) -> Self {
        Self {
            co: [time, value],
            left: [time, value],
            right: [time, value],
            interpolation: Interpolation::Bezier,
            easing: Easing::Auto,
            back: 1.70158,
            amplitude: 0.8,
            period: 4.1,
            locked: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FCurve {
    pub keys: Vec<Key>,
    /// false: the curve stays flat past its ends (constant), true: it carries on in the direction it ends in (linear)
    pub linear_extrapolation: bool,
}

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
    /// eased interpolations, sampled
    Points {
        points: Vec<[f64; 2]>,
    },
}

/// a curve ready to draw: its keys, the segments between them and the slopes it carries on with past its ends
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Drawing {
    pub keys: Vec<[f64; 2]>,
    /// each key's left and right handle, none on a side that isn't a bezier (like Blender only shows those)
    pub handles: Vec<[Option<[f64; 2]>; 2]>,
    pub segments: Vec<Segment>,
    pub slope_before: f64,
    pub slope_after: f64,
}

impl FCurve {
    /// a curve from Blender, with the handles Blender gave it
    pub fn from_anim_curve(curve: &AnimCurve) -> Self {
        Self {
            keys: curve.keyframe_points.iter().filter_map(Key::from_point).collect(),
            linear_extrapolation: curve.extrapolation == "LINEAR",
        }
    }

    /// keys taken from a Blender curve (an animation generator's note on and off keys)
    pub fn from_points(points: &[KeyframePoint]) -> Self {
        Self {
            keys: points.iter().filter_map(Key::from_point).collect(),
            linear_extrapolation: false,
        }
    }

    /// the curve the scene writer makes from (time, value) keys: Blender sorts them, keeps the last of keys at the same
    /// time and gives them auto clamped handles smoothed for continuous acceleration (the default for new curves)
    pub fn written(keys: impl IntoIterator<Item = (f64, f64)>) -> Self {
        let mut keys: Vec<Key> = keys.into_iter().map(|(time, value)| Key::added(time, value)).collect();
        // stable, like Blender's bubble sort
        keys.sort_by(|a, b| a.co[0].total_cmp(&b.co[0]));
        let mut curve = Self {
            keys: dedupe(keys),
            linear_extrapolation: false,
        };
        curve.auto_handles();
        curve
    }

    /// BKE_fcurve_handles_recalc for a curve of only auto clamped keys, constant extrapolation and continuous acceleration
    /// smoothing
    fn auto_handles(&mut self) {
        let count = self.keys.len();
        if count < 2 {
            return;
        }
        for i in 0..count {
            let prev = (i > 0).then(|| self.keys[i - 1].co);
            let next = (i + 1 < count).then(|| self.keys[i + 1].co);
            let key = &mut self.keys[i];
            auto_clamped_handles(key, prev, next);
            // constant extrapolation eases in and out of the ends
            if i == 0 || i == count - 1 {
                key.left[1] = key.co[1];
                key.right[1] = key.co[1];
                key.locked = true;
            }
        }
        smooth_handles(&mut self.keys);
    }

    pub fn drawing(&self) -> Drawing {
        let mut segments = Vec::with_capacity(self.keys.len().saturating_sub(1));
        for pair in self.keys.windows(2) {
            segments.push(segment(&pair[0], &pair[1]));
        }
        let (slope_before, slope_after) = match (self.keys.first(), self.keys.last()) {
            (Some(first), Some(last)) => (self.slope(first, self.keys.get(1), true), self.slope(last, self.keys.len().checked_sub(2).map(|i| &self.keys[i]), false)),
            _ => (0.0, 0.0),
        };
        // a key's left handle shapes the segment before it, the right one the segment after it
        let bezier = |i: usize| self.keys[i].interpolation == Interpolation::Bezier;
        let handles = (0..self.keys.len())
            .map(|i| {
                let key = &self.keys[i];
                let left = if i == 0 {
                    bezier(i)
                } else {
                    bezier(i - 1)
                };
                [left.then_some(key.left), bezier(i).then_some(key.right)]
            })
            .collect();
        Drawing {
            keys: self.keys.iter().map(|k| k.co).collect(),
            handles,
            segments,
            slope_before,
            slope_after,
        }
    }

    /// how the curve carries on past an end key, `neighbor` is the key next to it (fcurve_eval_keyframes_extrapolate)
    fn slope(&self, end: &Key, neighbor: Option<&Key>, first: bool) -> f64 {
        if !self.linear_extrapolation || end.interpolation == Interpolation::Constant {
            return 0.0;
        }
        let (from, to) = if end.interpolation == Interpolation::Linear {
            match neighbor {
                Some(neighbor) => (end.co, neighbor.co),
                None => return 0.0,
            }
        } else {
            // the end key's outer handle
            (
                end.co,
                if first {
                    end.left
                } else {
                    end.right
                },
            )
        };
        let dx = to[0] - from[0];
        if dx == 0.0 {
            0.0
        } else {
            (to[1] - from[1]) / dx
        }
    }
}

/// sorted keys with the last of keys at the same time kept, at the first one's time (BKE_fcurve_deduplicate_keys)
fn dedupe(keys: Vec<Key>) -> Vec<Key> {
    let mut out: Vec<Key> = Vec::with_capacity(keys.len());
    for key in keys {
        match out.last_mut() {
            Some(prev) if key.co[0] - prev.co[0] <= DUPLICATE_SECONDS => {
                let time = prev.co[0];
                *prev = key;
                prev.co[0] = time;
            }
            _ => out.push(key),
        }
    }
    out
}

// MARK: - Auto handles

/// calchandleNurb_intern for an auto clamped key of an f-curve with smoothing, only the neighbors' positions matter
fn auto_clamped_handles(key: &mut Key, prev: Option<[f64; 2]>, next: Option<[f64; 2]>) {
    key.locked = false;
    let p2 = key.co;
    let (p1, p3) = match (prev, next) {
        (None, None) => return,
        (Some(p1), Some(p3)) => (p1, p3),
        (None, Some(p3)) => ([2.0 * p2[0] - p3[0], 2.0 * p2[1] - p3[1]], p3),
        (Some(p1), None) => (p1, [2.0 * p2[0] - p1[0], 2.0 * p2[1] - p1[1]]),
    };
    let dvec_a = [p2[0] - p1[0], p2[1] - p1[1]];
    let dvec_b = [p3[0] - p2[0], p3[1] - p2[1]];
    let len_a = if dvec_a[0] == 0.0 {
        1.0
    } else {
        dvec_a[0]
    };
    let len_b = if dvec_b[0] == 0.0 {
        1.0
    } else {
        dvec_b[0]
    };
    let tvec = [dvec_b[0] / len_b + dvec_a[0] / len_a, dvec_b[1] / len_b + dvec_a[1] / len_a];
    // with smoothing the handles are 1/3 of the time to the next key (tvec's time is 2), so time runs linearly along the bezier
    let len = 6.0;

    let (mut left_violate, mut right_violate) = (false, false);
    let len_a = len_a / len;
    key.left = [p2[0] - tvec[0] * len_a, p2[1] - tvec[1] * len_a];
    let len_b = len_b / len;
    key.right = [p2[0] + tvec[0] * len_b, p2[1] + tvec[1] * len_b];

    if let (Some(prev), Some(next)) = (prev, next) {
        let ydiff1 = prev[1] - p2[1];
        let ydiff2 = next[1] - p2[1];
        if (ydiff1 <= 0.0 && ydiff2 <= 0.0) || (ydiff1 >= 0.0 && ydiff2 >= 0.0) {
            // an extreme stays flat
            key.left[1] = p2[1];
            key.right[1] = p2[1];
            key.locked = true;
        } else {
            // handles don't go past the neighbors' values
            if (ydiff1 <= 0.0 && prev[1] > key.left[1]) || (ydiff1 > 0.0 && prev[1] < key.left[1]) {
                key.left[1] = prev[1];
                left_violate = true;
            }
            if (ydiff1 <= 0.0 && next[1] < key.right[1]) || (ydiff1 > 0.0 && next[1] > key.right[1]) {
                key.right[1] = next[1];
                right_violate = true;
            }
        }
    }

    // the clamped handle keeps the other one in line with it
    if left_violate || right_violate {
        let h1_x = key.left[0] - p2[0];
        let h2_x = p2[0] - key.right[0];
        if left_violate {
            key.right[1] = p2[1] + ((p2[1] - key.left[1]) / h1_x) * h2_x;
        } else {
            key.left[1] = p2[1] + ((p2[1] - key.right[1]) / h2_x) * h1_x;
        }
    }
}

// MARK: - Continuous acceleration

/// BKE_nurb_handle_smooth_fcurve (not cyclic): each run of keys between locked ones is smoothed with the locked keys' handles
/// held where they are
fn smooth_handles(keys: &mut [Key]) {
    let total = keys.len();
    let (mut start, mut count) = (0, 1);
    for j in 1..total {
        if keys[j].locked {
            smooth_run(keys, start, count + 1);
            start = j;
            count = 1;
        } else {
            count += 1;
        }
    }
    if count > 1 {
        smooth_run(keys, start, count);
    }
}

/// bezier_handle_calc_smooth_fcurve: solves for the handles that keep acceleration continuous through the keys
/// `start..start + count`, limited so auto clamp still holds. h[i] is the right handle's height above key i
fn smooth_run(keys: &mut [Key], start: usize, count: usize) {
    if count < 2 {
        return;
    }
    let total = keys.len();
    let first = start;
    let last = start + count - 1;
    let solve_first = start == 0 && !keys[first].locked;
    let solve_last = start + count == total && !keys[last].locked;
    if count == 2 && solve_first == solve_last {
        return;
    }

    // key spacing
    let mut dx = vec![f64::NAN; count];
    let mut dy = vec![f64::NAN; count];
    for i in 1..count {
        dx[i] = keys[start + i].co[0] - keys[start + i - 1].co[0];
        dy[i] = keys[start + i].co[1] - keys[start + i - 1].co[1];
    }
    let mut l = vec![1.0; count];
    for i in 1..count - 1 {
        l[i] = dx[i + 1] / dx[i];
    }

    // auto clamp: no handle reverses its key's direction or overshoots the next key
    let mut hmax = vec![f64::MAX; count];
    let mut hmin = vec![-f64::MAX; count];
    for i in 1..count {
        clamp(&mut hmax, &mut hmin, i - 1, dy[i]);
        clamp(&mut hmax, &mut hmin, i, dy[i] * l[i]);
    }

    let (mut a, mut b, mut c, mut d) = (vec![0.0; count], vec![0.0; count], vec![0.0; count], vec![0.0; count]);
    let (mut first_adj, mut last_adj) = (0.0, 0.0);

    // ends: the locked key's handle as it is, or no acceleration
    if !solve_first {
        let mut size = [keys[first].right[0] - keys[first].co[0], keys[first].right[1] - keys[first].co[1]];
        first_adj = handle_adj(&mut size, dx[1]);
        lock(&mut a, &mut b, &mut c, &mut d, 0, size[1]);
    } else {
        a[0] = 0.0;
        b[0] = 2.0;
        c[0] = 1.0 / l[1];
        d[0] = dy[1];
    }
    if !solve_last {
        let mut size = [keys[last].co[0] - keys[last].left[0], keys[last].co[1] - keys[last].left[1]];
        last_adj = handle_adj(&mut size, dx[count - 1]);
        lock(&mut a, &mut b, &mut c, &mut d, count - 1, size[1]);
    } else {
        let i = count - 1;
        a[i] = l[i] * l[i];
        b[i] = 2.0 * l[i];
        c[i] = 0.0;
        d[i] = dy[i] * l[i] * l[i];
    }

    // continuous acceleration at every key between
    for i in 1..count - 1 {
        a[i] = l[i] * l[i];
        b[i] = 2.0 * (l[i] + 1.0);
        c[i] = 1.0 / l[i + 1];
        d[i] = dy[i] * l[i] * l[i] + dy[i + 1];
    }

    // locked handles that aren't 1/3 of the way
    if count > 2 || solve_last {
        b[1] += l[1] * first_adj;
    }
    if count > 2 || solve_first {
        b[count - 2] += last_adj;
    }

    let Some(h) = solve_with_limits(&mut a, &mut b, &mut c, &mut d, &hmin, &hmax) else {
        return;
    };
    for i in 1..count - 1 {
        let key = &mut keys[start + i];
        key.left[1] = key.co[1] - h[i] / l[i];
        key.right[1] = key.co[1] + h[i];
    }
    // an end of the whole curve mirrors its other handle
    if solve_first {
        let key = &mut keys[first];
        key.right[1] = key.co[1] + h[0];
        if start == 0 {
            key.left = [2.0 * key.co[0] - key.right[0], 2.0 * key.co[1] - key.right[1]];
        }
    }
    if solve_last {
        let key = &mut keys[last];
        key.left[1] = key.co[1] - h[count - 1] / l[count - 1];
        if start + count == total {
            key.right = [2.0 * key.co[0] - key.left[0], 2.0 * key.co[1] - key.left[1]];
        }
    }
}

/// bezier_clamp for an auto clamped key
fn clamp(hmax: &mut [f64], hmin: &mut [f64], i: usize, dy: f64) {
    if dy > 0.0 {
        hmax[i] = hmax[i].min(dy);
        hmin[i] = 0.0;
    } else if dy < 0.0 {
        hmax[i] = 0.0;
        hmin[i] = hmin[i].max(dy);
    } else {
        hmax[i] = 0.0;
        hmin[i] = 0.0;
    }
}

/// bezier_calc_handle_adj: handles that overlap in time are scaled to fit
fn handle_adj(size: &mut [f64; 2], dx: f64) -> f64 {
    let fac = dx / (size[0] + dx / 3.0);
    if fac < 1.0 {
        size[0] *= fac;
        size[1] *= fac;
    }
    1.0 - 3.0 * size[0] / dx
}

fn lock(a: &mut [f64], b: &mut [f64], c: &mut [f64], d: &mut [f64], i: usize, value: f64) {
    a[i] = 0.0;
    c[i] = 0.0;
    b[i] = 1.0;
    d[i] = value;
}

/// tridiagonal_solve_with_limits: solves, locks handles past their limits to the limit and solves again, and lets locked
/// ones go again (at most twice each) when they'd move back inside
fn solve_with_limits(a: &mut [f64], b: &mut [f64], c: &mut [f64], d: &mut [f64], hmin: &[f64], hmax: &[f64]) -> Option<Vec<f64>> {
    let count = a.len();
    let (a0, b0, c0, d0) = (a.to_vec(), b.to_vec(), c.to_vec(), d.to_vec());
    let mut is_locked = vec![false; count];
    let mut unlocks = vec![0u8; count];

    loop {
        let h = solve_tridiagonal(a, b, c, d)?;

        // lock the handles past their limits, the ones going the right way first if there are any
        let mut overshoot = false;
        let mut locked = false;
        let mut all = false;
        loop {
            for i in 0..count {
                if h[i] >= hmin[i] && h[i] <= hmax[i] {
                    continue;
                }
                overshoot = true;
                let target = if h[i] > hmax[i] {
                    hmax[i]
                } else {
                    hmin[i]
                };
                if target != 0.0 || all {
                    is_locked[i] = true;
                    lock(a, b, c, d, i, target);
                    locked = true;
                }
            }
            all = true;
            if !(overshoot && !locked) {
                break;
            }
        }

        // nothing new locked, let go of locked handles that want to move back inside
        let mut unlocked = false;
        if !locked {
            for i in 0..count {
                if !is_locked[i] || unlocks[i] >= 2 {
                    continue;
                }
                let state = a0[i] * h[(i + count - 1) % count] + b0[i] * h[i] + c0[i] * h[(i + 1) % count] - d0[i];
                let relax = -state * b0[i];
                if (relax > 0.0 && h[i] < hmax[i]) || (relax < 0.0 && h[i] > hmin[i]) {
                    a[i] = a0[i];
                    b[i] = b0[i];
                    c[i] = c0[i];
                    d[i] = d0[i];
                    is_locked[i] = false;
                    unlocks[i] += 1;
                    unlocked = true;
                }
            }
        }

        if !(overshoot || unlocked) {
            return Some(h);
        }
    }
}

/// BLI_tridiagonal_solve (the ends never wrap around here, so the cyclic version is the same)
fn solve_tridiagonal(a: &[f64], b: &[f64], c: &[f64], d: &[f64]) -> Option<Vec<f64>> {
    let count = a.len();
    if count == 0 {
        return None;
    }
    let mut c1 = vec![0.0; count];
    let mut d1 = vec![0.0; count];
    c1[0] = c[0] / b[0];
    d1[0] = d[0] / b[0];
    for i in 1..count {
        let denum = b[i] - a[i] * c1[i - 1];
        c1[i] = c[i] / denum;
        d1[i] = (d[i] - a[i] * d1[i - 1]) / denum;
    }
    let mut x = vec![0.0; count];
    x[count - 1] = d1[count - 1];
    for i in (0..count - 1).rev() {
        x[i] = d1[i] - c1[i] * x[i + 1];
    }
    x.iter().all(|v| v.is_finite()).then_some(x)
}

// MARK: - Segments

/// samples drawn for an eased segment, more for the ones that wiggle
fn samples(interpolation: Interpolation) -> usize {
    match interpolation {
        Interpolation::Bounce | Interpolation::Elastic => 96,
        _ => 32,
    }
}

/// the curve from `prev` to `next` (fcurve_eval_keyframes_interpolate)
fn segment(prev: &Key, next: &Key) -> Segment {
    let duration = next.co[0] - prev.co[0];
    match prev.interpolation {
        _ if duration == 0.0 => Segment::Step {
            to: next.co,
        },
        Interpolation::Constant => Segment::Step {
            to: next.co,
        },
        Interpolation::Linear => Segment::Line {
            to: next.co,
        },
        Interpolation::Bezier => {
            let (c1, c2) = correct_bezpart(prev.co, prev.right, next.left, next.co);
            Segment::Bezier {
                c1,
                c2,
                to: next.co,
            }
        }
        eased => {
            let count = samples(eased);
            let points = (1..=count)
                .map(|i| {
                    let time = duration * i as f64 / count as f64;
                    [prev.co[0] + time, ease(prev, time, prev.co[1], next.co[1] - prev.co[1], duration)]
                })
                .collect();
            Segment::Points {
                points,
            }
        }
    }
}

/// BKE_fcurve_correct_bezpart: handles can't reach past the other key, so the curve can't loop back in time
fn correct_bezpart(v1: [f64; 2], mut v2: [f64; 2], mut v3: [f64; 2], v4: [f64; 2]) -> ([f64; 2], [f64; 2]) {
    let h1 = [v1[0] - v2[0], v1[1] - v2[1]];
    let h2 = [v4[0] - v3[0], v4[1] - v3[1]];
    let len = v4[0] - v1[0];
    let len1 = h1[0].abs();
    let len2 = h2[0].abs();
    if len1 + len2 == 0.0 {
        return (v2, v3);
    }
    if len1 > len {
        let fac = len / len1;
        v2 = [v1[0] - fac * h1[0], v1[1] - fac * h1[1]];
    }
    if len2 > len {
        let fac = len / len2;
        v3 = [v4[0] - fac * h2[0], v4[1] - fac * h2[1]];
    }
    (v2, v3)
}

/// the eased value `time` into a segment, Blender's easing functions with its defaults for auto easing
fn ease(key: &Key, time: f64, begin: f64, change: f64, duration: f64) -> f64 {
    use Easing::*;
    use Interpolation::*;
    // auto eases in, except back, bounce and elastic, which ease out
    let easing = match (key.easing, key.interpolation) {
        (Auto, Back | Bounce | Elastic) => Out,
        (Auto, _) => In,
        (easing, _) => easing,
    };
    let (t, b, c, d) = (time, begin, change, duration);
    match (key.interpolation, easing) {
        (Back, In) => back_in(t, b, c, d, key.back),
        (Back, Out) => back_out(t, b, c, d, key.back),
        (Back, _) => back_in_out(t, b, c, d, key.back),
        (Bounce, In) => bounce_in(t, b, c, d),
        (Bounce, Out) => bounce_out(t, b, c, d),
        (Bounce, _) => bounce_in_out(t, b, c, d),
        (Circ, In) => -c * ((1.0 - (t / d).powi(2)).sqrt() - 1.0) + b,
        (Circ, Out) => c * (1.0 - (t / d - 1.0).powi(2)).sqrt() + b,
        (Circ, _) => {
            let t = t / (d / 2.0);
            if t < 1.0 {
                -c / 2.0 * ((1.0 - t * t).sqrt() - 1.0) + b
            } else {
                c / 2.0 * ((1.0 - (t - 2.0).powi(2)).sqrt() + 1.0) + b
            }
        }
        (Cubic, _) => power(t, b, c, d, 3, easing),
        (Quad, _) => power(t, b, c, d, 2, easing),
        (Quart, _) => power(t, b, c, d, 4, easing),
        (Quint, _) => power(t, b, c, d, 5, easing),
        (Elastic, In) => elastic_in(t, b, c, d, key.amplitude, key.period),
        (Elastic, Out) => elastic_out(t, b, c, d, key.amplitude, key.period),
        (Elastic, _) => elastic_in_out(t, b, c, d, key.amplitude, key.period),
        (Expo, In) => expo_in(t, b, c, d),
        (Expo, Out) => expo_out(t, b, c, d),
        (Expo, _) => {
            if t <= d / 2.0 {
                expo_in(t, b, c / 2.0, d / 2.0)
            } else {
                expo_out(t - d / 2.0, b + c / 2.0, c / 2.0, d / 2.0)
            }
        }
        (Sine, In) => -c * (t / d * PI / 2.0).cos() + c + b,
        (Sine, Out) => c * (t / d * PI / 2.0).sin() + b,
        (Sine, _) => -c / 2.0 * ((PI * t / d).cos() - 1.0) + b,
        (Constant | Linear | Bezier, _) => c * t / d + b,
    }
}

/// quad, cubic, quart and quint easing, the out half is the in half turned around
fn power(t: f64, b: f64, c: f64, d: f64, n: i32, easing: Easing) -> f64 {
    match easing {
        Easing::Out => c * (1.0 - (1.0 - t / d).powi(n)) + b,
        Easing::InOut => {
            let t = t / (d / 2.0);
            if t < 1.0 {
                c / 2.0 * t.powi(n) + b
            } else {
                c / 2.0 * (2.0 - (2.0 - t).powi(n)) + b
            }
        }
        _ => c * (t / d).powi(n) + b,
    }
}

fn back_in(t: f64, b: f64, c: f64, d: f64, overshoot: f64) -> f64 {
    let t = t / d;
    c * t * t * ((overshoot + 1.0) * t - overshoot) + b
}

fn back_out(t: f64, b: f64, c: f64, d: f64, overshoot: f64) -> f64 {
    let t = t / d - 1.0;
    c * (t * t * ((overshoot + 1.0) * t + overshoot) + 1.0) + b
}

fn back_in_out(t: f64, b: f64, c: f64, d: f64, overshoot: f64) -> f64 {
    let s = overshoot * 1.525;
    let t = t / (d / 2.0);
    if t < 1.0 {
        c / 2.0 * (t * t * ((s + 1.0) * t - s)) + b
    } else {
        let t = t - 2.0;
        c / 2.0 * (t * t * ((s + 1.0) * t + s) + 2.0) + b
    }
}

fn bounce_out(t: f64, b: f64, c: f64, d: f64) -> f64 {
    let t = t / d;
    if t < 1.0 / 2.75 {
        c * (7.5625 * t * t) + b
    } else if t < 2.0 / 2.75 {
        let t = t - 1.5 / 2.75;
        c * (7.5625 * t * t + 0.75) + b
    } else if t < 2.5 / 2.75 {
        let t = t - 2.25 / 2.75;
        c * (7.5625 * t * t + 0.9375) + b
    } else {
        let t = t - 2.625 / 2.75;
        c * (7.5625 * t * t + 0.984375) + b
    }
}

fn bounce_in(t: f64, b: f64, c: f64, d: f64) -> f64 {
    c - bounce_out(d - t, 0.0, c, d) + b
}

fn bounce_in_out(t: f64, b: f64, c: f64, d: f64) -> f64 {
    if t < d / 2.0 {
        bounce_in(t * 2.0, 0.0, c, d) * 0.5 + b
    } else {
        bounce_out(t * 2.0 - d, 0.0, c, d) * 0.5 + c * 0.5 + b
    }
}

/// 2^-10, expo easing is scaled so it starts and ends exactly
const POW_MIN: f64 = 0.0009765625;
const POW_SCALE: f64 = 1.0 / (1.0 - POW_MIN);

fn expo_in(t: f64, b: f64, c: f64, d: f64) -> f64 {
    if t == 0.0 {
        return b;
    }
    c * (2f64.powf(10.0 * (t / d - 1.0)) - POW_MIN) * POW_SCALE + b
}

fn expo_out(t: f64, b: f64, c: f64, d: f64) -> f64 {
    if t == 0.0 {
        return b;
    }
    c * (1.0 - (2f64.powf(-10.0 * t / d) - POW_MIN) * POW_SCALE) + b
}

/// elastic easing blends in from the start of the sine when the amplitude is smaller than the change (USE_ELASTIC_BLEND)
fn elastic_blend(time: f64, change: f64, duration: f64, amplitude: f64, s: f64, f: f64) -> f64 {
    if change == 0.0 {
        return f;
    }
    let t = s.abs();
    let mut f = if amplitude != 0.0 {
        f * amplitude / change.abs()
    } else {
        0.0
    };
    if (time * duration).abs() < t {
        let l = (time * duration).abs() / t;
        f = f * l + (1.0 - l);
    }
    f
}

/// the phase shift and amplitude elastic easing uses, and the blend factor
fn elastic_shape(time: f64, change: f64, duration: f64, amplitude: f64, period: f64) -> (f64, f64, f64) {
    if amplitude == 0.0 || amplitude < change.abs() {
        let s = period / 4.0;
        (s, change, elastic_blend(time, change, duration, amplitude, s, 1.0))
    } else {
        (period / (2.0 * PI) * (change / amplitude).asin(), amplitude, 1.0)
    }
}

fn elastic_in(t: f64, b: f64, c: f64, d: f64, amplitude: f64, period: f64) -> f64 {
    if t == 0.0 {
        return b;
    }
    let t = t / d;
    if t == 1.0 {
        return b + c;
    }
    let t = t - 1.0;
    let period = if period == 0.0 {
        d * 0.3
    } else {
        period
    };
    let (s, amplitude, f) = elastic_shape(t, c, d, amplitude, period);
    -f * (amplitude * 2f64.powf(10.0 * t) * ((t * d - s) * (2.0 * PI) / period).sin()) + b
}

fn elastic_out(t: f64, b: f64, c: f64, d: f64, amplitude: f64, period: f64) -> f64 {
    if t == 0.0 {
        return b;
    }
    let t = t / d;
    if t == 1.0 {
        return b + c;
    }
    let t = -t;
    let period = if period == 0.0 {
        d * 0.3
    } else {
        period
    };
    let (s, amplitude, f) = elastic_shape(t, c, d, amplitude, period);
    f * (amplitude * 2f64.powf(10.0 * t) * ((t * d - s) * (2.0 * PI) / period).sin()) + c + b
}

fn elastic_in_out(t: f64, b: f64, c: f64, d: f64, amplitude: f64, period: f64) -> f64 {
    if t == 0.0 {
        return b;
    }
    let t = t / (d / 2.0);
    if t == 2.0 {
        return b + c;
    }
    let t = t - 1.0;
    let period = if period == 0.0 {
        d * 0.3 * 1.5
    } else {
        period
    };
    let (s, amplitude, f) = elastic_shape(t, c, d, amplitude, period);
    if t < 0.0 {
        -0.5 * f * (amplitude * 2f64.powf(10.0 * t) * ((t * d - s) * (2.0 * PI) / period).sin()) + b
    } else {
        let t = -t;
        0.5 * f * (amplitude * 2f64.powf(10.0 * t) * ((t * d - s) * (2.0 * PI) / period).sin()) + c + b
    }
}
