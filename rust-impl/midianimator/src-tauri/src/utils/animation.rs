use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use crate::midi::MIDINote;
use crate::scene_generics::KeyframePoint;

pub fn sec_to_frames(seconds: f64, fps: f64) -> f64 {
    seconds * fps
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BlendKeyframe {
    pub time: f64,
    pub value: f64,
    pub data_path: String,
    pub array_index: u32,
}

impl BlendKeyframe {
    pub fn new(time: f64, value: f64, data_path: &str, array_index: u32) -> Self {
        Self {
            time,
            value,
            data_path: data_path.to_string(),
            array_index,
        }
    }

    pub fn bare(time: f64, value: f64) -> Self {
        Self {
            time,
            value,
            data_path: String::new(),
            array_index: 0,
        }
    }
}

/// the output of the animation_generator node
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AnimationGenerator {
    pub name: String,
    pub note_on_keyframes: Vec<KeyframePoint>,
    pub note_on_anchor_point: f64,
    pub note_off_keyframes: Vec<KeyframePoint>,
    pub note_off_anchor_point: f64,
    // mappers are passed through but not used yet
    pub time_mapper: String,
    pub amplitude_mapper: String,
    pub velocity_intensity: f64,
    pub animation_overlap: String,
    /// crossfade only: seconds to ease from the old animation into the note
    #[serde(default = "default_overlap_blend")]
    pub overlap_blend: f64,
    pub animation_property: String,
}

fn default_overlap_blend() -> f64 {
    DEFAULT_OVERLAP_BLEND
}

/// one object's animations, each with the notes that trigger it, e.g. {"crash": [49], "ride": [51]}
/// the names key into object_map.animations
pub type ObjectMapEntry = HashMap<String, Vec<u8>>;

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ObjectMap {
    pub animations: HashMap<String, AnimationGenerator>,
    pub objects: HashMap<String, ObjectMapEntry>,
}

impl ObjectMap {
    /// merges another map into this one, objects in both get the animations of both, each keeping its own notes
    pub fn merge(&mut self, other: ObjectMap) -> Result<(), String> {
        // generators are keyed by name, the same generator twice is fine but two different ones can't share a name
        for (name, generator) in other.animations {
            match self.animations.get(&name) {
                Some(existing) if *existing != generator => return Err(format!("two different animation generators are named '{}', rename one of them", name)),
                Some(_) => {}
                None => {
                    self.animations.insert(name, generator);
                }
            }
        }

        for (obj_name, entry) in other.objects {
            let merged = self.objects.entry(obj_name).or_default();
            for (anim_name, notes) in entry {
                let merged_notes = merged.entry(anim_name).or_default();
                for note in notes {
                    if !merged_notes.contains(&note) {
                        merged_notes.push(note);
                    }
                }
            }
        }
        Ok(())
    }
}

pub fn get_value(k1: &BlendKeyframe, k2: &BlendKeyframe, time: f64) -> f64 {
    let (x1, y1) = (k1.time, k1.value);
    let (x2, y2) = (k2.time, k2.value);
    let m = if (x2 - x1).abs() < f64::EPSILON {
        0.0
    } else {
        (y2 - y1) / (x2 - x1)
    };
    m * time + (y1 - m * x1)
}

pub fn interval<'a>(key_list: &'a [BlendKeyframe], time: f64) -> (Option<&'a BlendKeyframe>, Option<&'a BlendKeyframe>) {
    if key_list.is_empty() {
        return (None, None);
    }
    if key_list[0].time > time {
        return (Some(&key_list[0]), Some(&key_list[0]));
    }
    let last = &key_list[key_list.len() - 1];
    if last.time < time {
        return (Some(last), Some(last));
    }
    for i in 0..key_list.len() - 1 {
        if key_list[i].time <= time && time <= key_list[i + 1].time {
            return (Some(&key_list[i]), Some(&key_list[i + 1]));
        }
    }
    (None, None)
}

/// the keys at the end of `key_list1` that come after `key_list2` starts, plus the last one before it
pub fn find_overlap(key_list1: &[BlendKeyframe], key_list2: &[BlendKeyframe]) -> Vec<BlendKeyframe> {
    if key_list1.is_empty() || key_list2.is_empty() {
        return vec![];
    }
    // key_list1 can start after key_list2 when two generators with different timing share a curve, then all of it overlaps
    let first_next = key_list2[0].time;
    let mut result = vec![];
    let mut found = false;
    for key in key_list1.iter().rev() {
        if key.time > first_next {
            found = true;
            result.push(key.clone());
        } else {
            if found {
                result.push(key.clone());
            }
            break;
        }
    }
    result.reverse();
    result
}

pub fn add_keyframes(inserted_keys: &mut Vec<BlendKeyframe>, next_keys: &mut Vec<BlendKeyframe>) {
    let overlapping = find_overlap(inserted_keys, next_keys);
    if overlapping.is_empty() {
        inserted_keys.append(next_keys);
        inserted_keys.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
        return;
    }

    // the inserted keys get the note's values from before they're changed below
    let original_next_keys = next_keys.clone();

    // Add interpolated overlap values into next_keys
    for key in next_keys.iter_mut() {
        let (i1, i2) = interval(&overlapping, key.time);
        if let (Some(i1), Some(i2)) = (i1, i2) {
            key.value += get_value(i1, i2, key.time);
        }
    }

    // Add interpolated next_keys values into the overlapping region of inserted_keys
    let overlapping_times: Vec<f64> = overlapping.iter().map(|k| k.time).collect();
    for key in inserted_keys.iter_mut() {
        if overlapping_times.contains(&key.time) {
            let (i1, i2) = interval(&original_next_keys, key.time);
            if let (Some(i1), Some(i2)) = (i1, i2) {
                key.value += get_value(i1, i2, key.time);
            }
        }
    }

    inserted_keys.append(next_keys);
    inserted_keys.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
    inserted_keys.dedup_by(|a, b| (a.time - b.time).abs() < f64::EPSILON);
}

/// one object a note animates and the name of the animation it plays (a key into `ObjectMap::animations`),
/// the Note Targets node lists these per note number
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NoteTarget {
    pub object: String,
    pub animation: String,
}

/// the keyframes one note adds to one curve of an object, before overlapping notes are combined
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CurveKeys {
    pub object: String,
    pub data_path: String,
    pub array_index: u32,
    pub animation_overlap: String,
    pub overlap_blend: f64,
    pub keyframes: Vec<BlendKeyframe>,
}

impl ObjectMap {
    /// which objects (and with which animation) each note number triggers, sorted by object then animation name
    pub fn note_targets(&self) -> Result<BTreeMap<u8, Vec<NoteTarget>>, String> {
        let mut targets: BTreeMap<u8, Vec<NoteTarget>> = BTreeMap::new();
        // sorted so notes on the same curve always combine in the same order
        let objects: BTreeMap<&String, &ObjectMapEntry> = self.objects.iter().collect();
        for (obj_name, entry) in objects {
            let animations: BTreeMap<&String, &Vec<u8>> = entry.iter().collect();
            for (anim_name, notes) in animations {
                self.generator(anim_name, obj_name)?;
                for &note_number in notes {
                    targets.entry(note_number).or_default().push(NoteTarget {
                        object: obj_name.clone(),
                        animation: anim_name.clone(),
                    });
                }
            }
        }
        Ok(targets)
    }

    /// the generator an object's animation name points to
    pub fn generator(&self, animation: &str, object: &str) -> Result<&AnimationGenerator, String> {
        self.animations.get(animation).ok_or_else(|| format!("object '{}' uses animation '{}', which isn't in the object map", object, animation))
    }
}

/// the keyframes a note plays on an object, offset to the note's time and scaled by its velocity. `None` if the generator has no keys
pub fn note_curve_keys(object: &str, gen: &AnimationGenerator, note: &MIDINote) -> Option<CurveKeys> {
    // parse data_path and array_index from animation_property e.g. "location[0]"
    let (data_path, array_index) = parse_animation_property(&gen.animation_property);

    // use seconds instead of frames, Blender converts to frames with the scene's frame rate. this keeps timing based on the music rather than frame numbers
    let mut keyframes = note_keyframes(&gen.note_on_keyframes, note.time_on + gen.note_on_anchor_point, note.velocity, gen.velocity_intensity, &data_path, array_index);
    keyframes.extend(note_keyframes(&gen.note_off_keyframes, note.time_off + gen.note_off_anchor_point, note.velocity, gen.velocity_intensity, &data_path, array_index));
    keyframes.sort_by(|a, b| a.time.total_cmp(&b.time));

    if keyframes.is_empty() {
        return None;
    }
    Some(CurveKeys {
        object: object.to_string(),
        data_path,
        array_index,
        animation_overlap: gen.animation_overlap.clone(),
        overlap_blend: gen.overlap_blend,
        keyframes,
    })
}

/// offsets a generator's keyframes to a note's time and scales them by its velocity
fn note_keyframes(keyframes: &[KeyframePoint], offset: f64, velocity: u8, velocity_intensity: f64, data_path: &str, array_index: u32) -> Vec<BlendKeyframe> {
    keyframes
        .iter()
        .filter_map(|kf| {
            let (time, mut value) = co_of(kf)?;
            if velocity_intensity != 0.0 {
                value *= velocity as f64 / 127.0 * velocity_intensity;
            }
            Some(BlendKeyframe::new(time + offset, value, data_path, array_index))
        })
        .collect()
}

/// combines each note's keys into its curve in order with the curve's overlap mode, then flattens to one list per object.
/// every object in `objects` is in the result, ones with no keys stay empty so the writer still clears them
pub fn combine_curve_keys<'a>(objects: impl Iterator<Item = &'a String>, chunks: impl IntoIterator<Item = CurveKeys>) -> Result<HashMap<String, Vec<BlendKeyframe>>, String> {
    // keyframes per object, then per curve. overlap only combines keys on the same curve
    let mut obj_curves: HashMap<String, BTreeMap<(String, u32), Vec<BlendKeyframe>>> = objects.map(|name| (name.clone(), BTreeMap::new())).collect();
    for mut chunk in chunks {
        let inserted = obj_curves.entry(chunk.object).or_default().entry((chunk.data_path, chunk.array_index)).or_default();
        let settings = OverlapSettings {
            blend: chunk.overlap_blend,
        };
        combine_keyframes(&chunk.animation_overlap, &settings, inserted, &mut chunk.keyframes)?;
    }
    Ok(obj_curves.into_iter().map(|(name, curves)| (name, curves.into_values().flatten().collect())).collect())
}

/// the ways a note's keyframes can combine with the keyframes already on its curve, the first is the default
pub const ANIMATION_OVERLAPS: [&str; 8] = ["add", "min", "max", "prev", "next", "rvc", "prune", "crossfade"];

/// seconds a crossfade eases over when the generator doesn't set one
pub const DEFAULT_OVERLAP_BLEND: f64 = 0.1;

/// settings some overlap modes use, each mode ignores the ones that aren't its own
#[derive(Debug, Clone, PartialEq)]
pub struct OverlapSettings {
    /// crossfade: seconds to ease from the old animation into the note
    pub blend: f64,
}

impl Default for OverlapSettings {
    fn default() -> Self {
        Self {
            blend: DEFAULT_OVERLAP_BLEND,
        }
    }
}

// values this close to 0 count as the rest value
const REST_EPSILON: f64 = 1e-9;

// keys this close in time count as the same time, notes on a grid often land right on an old key
const TIME_EPSILON: f64 = 1e-6;

// how many keys a crossfade's ease is sampled into
const CROSSFADE_STEPS: usize = 8;

/// combines a note's keyframes into the keyframes already on the curve with the given overlap mode
/// both lists must be sorted by time, `next_keys` is used up
pub fn combine_keyframes(mode: &str, settings: &OverlapSettings, inserted_keys: &mut Vec<BlendKeyframe>, next_keys: &mut Vec<BlendKeyframe>) -> Result<(), String> {
    match mode {
        "add" | "" => add_keyframes(inserted_keys, next_keys),
        "min" => extreme_keyframes(inserted_keys, next_keys, f64::min),
        "max" => extreme_keyframes(inserted_keys, next_keys, f64::max),
        "prev" => prev_keyframes(inserted_keys, next_keys),
        "next" => next_keyframes(inserted_keys, next_keys),
        "rvc" => rest_value_crossing_keyframes(inserted_keys, next_keys),
        "prune" => prune_keyframes(inserted_keys, next_keys),
        "crossfade" => crossfade_keyframes(inserted_keys, next_keys, settings.blend),
        other => return Err(format!("unknown animation overlap '{}', expected one of: {}", other, ANIMATION_OVERLAPS.join(", "))),
    }
    Ok(())
}

fn is_rest(value: f64) -> bool {
    value.abs() < REST_EPSILON
}

/// the value of a curve at `time`, linear between keys and held past either end
pub fn value_at(keys: &[BlendKeyframe], time: f64) -> f64 {
    match interval(keys, time) {
        (Some(k1), Some(k2)) => get_value(k1, k2, time),
        _ => 0.0,
    }
}

// whether the segment just before (or just after) `time` moves, a segment moves unless both its keys are at rest
fn moving_near(keys: &[BlendKeyframe], time: f64, before: bool) -> bool {
    keys.windows(2).any(|w| {
        let inside = if before {
            w[0].time < time && time <= w[1].time
        } else {
            w[0].time <= time && time < w[1].time
        };
        inside && !(is_rest(w[0].value) && is_rest(w[1].value))
    })
}

/// whether a curve is in motion at `time`
/// false where it sits at rest, and on the key where it leaves or comes back to rest
/// a key at rest in the middle of a motion (a zero crossing) is still in motion
fn in_motion(keys: &[BlendKeyframe], time: f64) -> bool {
    !is_rest(value_at(keys, time)) || (moving_near(keys, time, true) && moving_near(keys, time, false))
}

// index of the first key at or after `start`, keys before it are never touched by a note starting at `start`
fn split_at(keys: &[BlendKeyframe], start: f64) -> usize {
    keys.partition_point(|k| k.time < start)
}

/// min and max: where both curves are in motion keep the smaller (or bigger) one, elsewhere whichever one is moving
/// a curve at rest doesn't count, so a note can't be flattened by the rest value of the one before it
fn extreme_keyframes(inserted_keys: &mut Vec<BlendKeyframe>, next_keys: &mut Vec<BlendKeyframe>, pick: fn(f64, f64) -> f64) {
    let Some(first) = next_keys.first().cloned() else {
        return;
    };
    let start = first.time;
    let split = split_at(inserted_keys, start);
    // the old curve from its last key before the note, that's all that can overlap
    let old: Vec<BlendKeyframe> = inserted_keys[split.saturating_sub(1)..].to_vec();

    // evaluate both curves at every key time from the start of the note
    let mut times: Vec<f64> = old.iter().map(|k| k.time).filter(|&t| t >= start).chain(next_keys.iter().map(|k| k.time)).collect();
    times.sort_by(f64::total_cmp);
    times.dedup_by(|a, b| (*a - *b).abs() < f64::EPSILON);

    // where both are moving and cross between two times the result switches curves, so the crossing gets a key too
    let mut crossings = vec![];
    for w in times.windows(2) {
        let mid = (w[0] + w[1]) / 2.0;
        if !(in_motion(&old, mid) && in_motion(next_keys, mid)) {
            continue;
        }
        let d0 = value_at(&old, w[0]) - value_at(next_keys, w[0]);
        let d1 = value_at(&old, w[1]) - value_at(next_keys, w[1]);
        if d0 * d1 < 0.0 {
            crossings.push(w[0] + (w[1] - w[0]) * d0 / (d0 - d1));
        }
    }
    times.extend(crossings);
    times.sort_by(f64::total_cmp);
    times.dedup_by(|a, b| (*a - *b).abs() < f64::EPSILON);

    let combined: Vec<BlendKeyframe> = times
        .iter()
        .map(|&time| {
            let (old_value, next_value) = (value_at(&old, time), value_at(next_keys, time));
            let value = match (in_motion(&old, time), in_motion(next_keys, time)) {
                (true, true) => pick(old_value, next_value),
                (true, false) => old_value,
                (false, true) => next_value,
                (false, false) => 0.0,
            };
            BlendKeyframe {
                time,
                value,
                ..first.clone()
            }
        })
        .collect();

    inserted_keys.truncate(split);
    inserted_keys.extend(combined);
    next_keys.clear();
}

/// previous: the animation already playing wins, a note that starts while it's still moving is dropped
fn prev_keyframes(inserted_keys: &mut Vec<BlendKeyframe>, next_keys: &mut Vec<BlendKeyframe>) {
    let Some(start) = next_keys.first().map(|k| k.time) else {
        return;
    };
    let old = &inserted_keys[split_at(inserted_keys, start).saturating_sub(1)..];
    let still_playing = in_motion(old, start) || old.iter().any(|k| k.time > start && !is_rest(k.value));
    if still_playing {
        next_keys.clear();
        return;
    }
    // nothing is playing, the rest keys left after the start would only zigzag with the note
    next_keyframes(inserted_keys, next_keys);
}

// index to cut the old keys at for a note starting at `start`
// an old key right on the start stays when the old curve is still moving there, like a peak landing on the next hit
fn cut_index(keys: &[BlendKeyframe], start: f64) -> usize {
    let split = split_at(keys, start - TIME_EPSILON);
    let on_start = keys.get(split).is_some_and(|k| k.time <= start + TIME_EPSILON);
    if on_start && in_motion(&keys[split.saturating_sub(1)..], start) {
        split + 1
    } else {
        split
    }
}

/// next: the new note wins, the old animation is cut off where the note starts
fn next_keyframes(inserted_keys: &mut Vec<BlendKeyframe>, next_keys: &mut Vec<BlendKeyframe>) {
    let Some(start) = next_keys.first().map(|k| k.time) else {
        return;
    };
    let cut = cut_index(inserted_keys, start);
    inserted_keys.truncate(cut);
    // the note's keys up to an old key that stayed would only duplicate it
    if let Some(last) = inserted_keys.last().map(|k| k.time) {
        next_keys.retain(|k| k.time > last + TIME_EPSILON);
    }
    inserted_keys.append(next_keys);
}

/// crossfade: the note takes over right away, but starts from where the old animation was and eases into its own curve over `blend` seconds
/// only the gap between the two fades out, so the note keeps its own shape and timing and the value never jumps
fn crossfade_keyframes(inserted_keys: &mut Vec<BlendKeyframe>, next_keys: &mut Vec<BlendKeyframe>, blend: f64) {
    let Some(first) = next_keys.first().cloned() else {
        return;
    };
    let start = first.time;
    let split = split_at(inserted_keys, start);
    let old = &inserted_keys[split.saturating_sub(1)..];
    if blend <= 0.0 || !in_motion(old, start) {
        return next_keyframes(inserted_keys, next_keys);
    }
    let gap = value_at(old, start) - first.value;
    let end = start + blend;

    // sample the ease evenly, plus the note's own keys inside it so its shape isn't cut short
    let mut times: Vec<f64> = (0..=CROSSFADE_STEPS).map(|i| start + blend * i as f64 / CROSSFADE_STEPS as f64).chain(next_keys.iter().map(|k| k.time).filter(|&t| t > start && t < end)).collect();
    times.sort_by(f64::total_cmp);
    times.dedup_by(|a, b| (*a - *b).abs() < TIME_EPSILON);

    let faded: Vec<BlendKeyframe> = times
        .iter()
        .map(|&time| {
            // smoothstep from the full gap down to nothing
            let u = (time - start) / blend;
            let fade = 2.0 * u.powi(3) - 3.0 * u.powi(2) + 1.0;
            BlendKeyframe {
                time,
                value: value_at(next_keys, time) + gap * fade,
                ..first.clone()
            }
        })
        .collect();

    inserted_keys.truncate(split);
    inserted_keys.extend(faded);
    inserted_keys.extend(next_keys.drain(..).filter(|k| k.time > end + TIME_EPSILON));
}

/// rest value crossing: the old animation keeps playing until it crosses its rest value, then the note plays from there
/// switching at rest means the value never jumps and the note keeps its whole shape, but it starts late while the old animation finishes
fn rest_value_crossing_keyframes(inserted_keys: &mut Vec<BlendKeyframe>, next_keys: &mut Vec<BlendKeyframe>) {
    let Some(first) = next_keys.first().cloned() else {
        return;
    };
    let start = first.time;
    let split = split_at(inserted_keys, start);
    let old = &inserted_keys[split.saturating_sub(1)..];
    if !in_motion(old, start) {
        return next_keyframes(inserted_keys, next_keys);
    }

    // walk the old curve from the note's start to where it next reaches or crosses rest
    let (mut prev_time, mut prev_value) = (start, value_at(old, start));
    let mut switch = None;
    if is_rest(prev_value) {
        // already crossing rest right at the start
        switch = Some(start);
    }
    for key in old.iter().filter(|k| k.time > start) {
        if switch.is_some() {
            break;
        }
        if is_rest(key.value) {
            switch = Some(key.time);
        } else if prev_value * key.value < 0.0 {
            switch = Some(prev_time + (key.time - prev_time) * prev_value / (prev_value - key.value));
        }
        (prev_time, prev_value) = (key.time, key.value);
    }
    // an old curve that never gets back to rest (it holds a value) has nothing to wait for, the note cuts in like next
    let Some(switch_time) = switch else {
        return next_keyframes(inserted_keys, next_keys);
    };

    // the note waits for the switch and plays whole from there
    let delay = switch_time - start;
    inserted_keys.retain(|k| k.time < switch_time - TIME_EPSILON);
    inserted_keys.extend(next_keys.drain(..).map(|k| BlendKeyframe {
        time: k.time + delay,
        ..k
    }));
}

/// keyframe pruning: the old animation is cut off where the note starts, and the note's keys leading up to its first peak are dropped
/// when they'd make the curve change direction on the way there. what's left flows straight from the old animation into the note's
/// first peak, so with Blender's smooth handles the curve's slope doesn't jump
fn prune_keyframes(inserted_keys: &mut Vec<BlendKeyframe>, next_keys: &mut Vec<BlendKeyframe>) {
    let Some(start) = next_keys.first().map(|k| k.time) else {
        return;
    };
    let split = split_at(inserted_keys, start);
    let old = &inserted_keys[split.saturating_sub(1)..];
    // nothing to smooth when the old curve is resting, or the note starts before it
    if !in_motion(old, start) || split == 0 {
        return next_keyframes(inserted_keys, next_keys);
    }
    // an old key right on the note's start (like a peak landing on the next hit) is kept as where the old curve left off
    let split = inserted_keys.partition_point(|k| k.time <= start + TIME_EPSILON);
    let from = inserted_keys[split - 1].value;
    let from_time = inserted_keys[split - 1].time;
    next_keys.retain(|k| k.time > from_time);

    // the note's first peak: the first key away from rest where the curve heads back towards rest (or its last key)
    let peak = (0..next_keys.len()).find(|&i| !is_rest(next_keys[i].value) && next_keys.get(i + 1).map_or(true, |after| (after.value - next_keys[i].value) * next_keys[i].value < 0.0));
    if let Some(peak) = peak {
        let to = next_keys[peak].value;
        let (low, high) = (from.min(to), from.max(to));
        // keep a key before the peak only if it lies strictly between where the old curve left off and the peak
        let mut index = 0;
        next_keys.retain(|k| {
            let keep = index >= peak || (low < k.value && k.value < high);
            index += 1;
            keep
        });
    }
    // cut where the note starts, not at its first key left after pruning
    inserted_keys.truncate(split);
    inserted_keys.append(next_keys);
}

// helper to parse properties like "rotation[0]" into ("rotation", 0). the index is the last `[n]`, so paths with keys
// of their own work too: `pose.bones["Arm"].location[1]`, `["prop"][0]`
pub fn parse_animation_property(prop: &str) -> (String, u32) {
    let indexed = prop.strip_suffix(']').and_then(|p| p.rsplit_once('[')).and_then(|(data_path, index)| index.parse::<u32>().ok().map(|index| (data_path.to_string(), index)));
    indexed.unwrap_or_else(|| (prop.to_string(), 0))
}

/// the (time, value) of a keyframe point, `None` if `co` doesn't have both
pub fn co_of(kf: &KeyframePoint) -> Option<(f64, f64)> {
    match kf.co.as_slice() {
        [time, value, ..] => Some((*time as f64, *value as f64)),
        _ => None,
    }
}
