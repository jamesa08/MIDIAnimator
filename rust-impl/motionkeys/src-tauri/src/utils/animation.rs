use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Serialize};

use crate::midi::MIDINote;
use crate::scene_generics::AnimCurve;
use crate::utils::curve::{auto_handles, evaluate, slope, sort_keys, Key};
use crate::utils::mapper::{Mappers, NoteContext, NoteVars};
use crate::utils::momentum::{momentum_keys, MomentumNote};

pub fn sec_to_frames(seconds: f64, fps: f64) -> f64 {
    seconds * fps
}

/// a key the scene writer writes, with the channel it goes on
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BlendKeyframe {
    /// the bridge's channel id, `location[2]` in Blender
    pub channel: String,
    /// `[time, value, interpolation, left_time, left_value, right_time, right_value]`
    #[serde(with = "crate::utils::curve::wire_key")]
    #[schemars(with = "Vec<serde_json::Value>")]
    pub key: Key,
}

/// the output of the animation_generator node. every curve plays on its own channel
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AnimationGenerator {
    pub name: String,
    pub note_on_keyframes: Vec<AnimCurve>,
    pub note_on_anchor_point: f64,
    pub note_off_keyframes: Vec<AnimCurve>,
    pub note_off_anchor_point: f64,
    /// an expression giving each key's new time in frames, before the note's offset is added. empty changes nothing
    #[serde(default)]
    pub time_mapper: String,
    /// an expression giving each key's new value, before velocity intensity. empty changes nothing
    #[serde(default)]
    pub amplitude_mapper: String,
    pub velocity_intensity: f64,
    pub animation_overlap: String,
    /// crossfade only: seconds to ease from the old animation into the note
    #[serde(default = "default_overlap_blend")]
    pub overlap_blend: f64,
    /// momentum only: 0 to 1, how much crowded notes shrink
    #[serde(default)]
    pub overlap_dampen: f64,
    /// momentum only: dampening shrinks toward the hit's value so the hit still lands, off shrinks toward rest
    #[serde(default = "default_use_anchor_value")]
    pub overlap_use_anchor_value: bool,
    /// the mappers compiled, once for every copy of the generator
    #[serde(skip)]
    #[schemars(skip)]
    pub compiled: CompiledMappers,
}

fn default_overlap_blend() -> f64 {
    DEFAULT_OVERLAP_BLEND
}

fn default_use_anchor_value() -> bool {
    true
}

/// a generator's compiled mappers, shared by its copies. made the first time they're needed, a generator read from JSON
/// compiles them again. copies are always equal, the mappers they come from are compared
#[derive(Clone, Default)]
pub struct CompiledMappers(Arc<OnceLock<Result<Mappers, String>>>);

impl PartialEq for CompiledMappers {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl fmt::Debug for CompiledMappers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CompiledMappers")
    }
}

impl AnimationGenerator {
    /// the time and amplitude mappers, compiled the first time. an error names the mapper
    pub fn mappers(&self) -> Result<&Mappers, String> {
        self.compiled.0.get_or_init(|| Mappers::compile(&self.time_mapper, &self.amplitude_mapper)).as_ref().map_err(String::clone)
    }
}

/// one object's animations, each with the notes that trigger it, e.g. {"crash": [49], "ride": [51]}
/// the names key into object_map.animations
pub type ObjectMapEntry = HashMap<String, Vec<u8>>;

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ObjectMap {
    pub animations: HashMap<String, AnimationGenerator>,
    /// each object's animations and their notes, by the object's id
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

/// the keys at the end of `key_list1` that come after `key_list2` starts, plus the last one before it
pub fn find_overlap(key_list1: &[Key], key_list2: &[Key]) -> Vec<Key> {
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

/// add: where the note overlaps the curve, both are added together. the handles of each key follow the other curve's
/// slope, so a curve at rest leaves the other's keys as they are
pub fn add_keyframes(inserted_keys: &mut Vec<Key>, next_keys: &mut Vec<Key>) {
    let overlapping = find_overlap(inserted_keys, next_keys);
    if overlapping.is_empty() {
        inserted_keys.append(next_keys);
        inserted_keys.sort_by(|a, b| a.time.total_cmp(&b.time));
        return;
    }

    // the inserted keys get the note's values from before they're changed below
    let original_next_keys = next_keys.clone();

    // the old curve's values into the note's keys
    for key in next_keys.iter_mut() {
        add_curve(key, &overlapping);
    }

    // the note's values into the old keys it overlaps
    let overlapping_times: Vec<f64> = overlapping.iter().map(|k| k.time).collect();
    for key in inserted_keys.iter_mut() {
        if overlapping_times.contains(&key.time) {
            add_curve(key, &original_next_keys);
        }
    }

    inserted_keys.append(next_keys);
    inserted_keys.sort_by(|a, b| a.time.total_cmp(&b.time));
    inserted_keys.dedup_by(|a, b| (a.time - b.time).abs() < f64::EPSILON);
}

/// adds `other`'s value at a key to it, its handles tilt by `other`'s slope on each side
fn add_curve(key: &mut Key, other: &[Key]) {
    let at = value_at(other, key.time);
    let (before, after) = (slope(other, key.time, true), slope(other, key.time, false));
    key.value += at;
    key.left[1] += at + before * (key.left[0] - key.time);
    key.right[1] += at + after * (key.right[0] - key.time);
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
    /// the bridge's channel id
    pub channel: String,
    pub animation_overlap: String,
    pub overlap_blend: f64,
    /// momentum: when the note hits (its note on), seconds. unset uses the first key
    #[serde(default)]
    pub hit: Option<f64>,
    /// momentum: the generator's dampening and anchor point value
    #[serde(default)]
    pub overlap_dampen: f64,
    #[serde(default = "default_use_anchor_value")]
    pub overlap_use_anchor_value: bool,
    /// in the wire form, `[time, value, interpolation]` or with the handles
    #[serde(with = "crate::utils::curve::wire_keys")]
    #[schemars(with = "Vec<Vec<serde_json::Value>>")]
    pub keyframes: Vec<Key>,
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

/// the keyframes a note plays on an object, through the generator's mappers, offset to the note's time and scaled by its
/// velocity. one per channel the generator's curves are on, in channel order, channels with no keys are left out.
/// `ctx` is the track around the note and the fps, for the mappers
pub fn note_curve_keys(object: &str, gen: &AnimationGenerator, note: &MIDINote, ctx: &NoteContext) -> Result<Vec<CurveKeys>, String> {
    let mappers = gen.mappers()?;
    // the note's mapper variables, held looks at the whole track so it's only counted when it's used
    let mut vars = (!mappers.is_identity()).then(|| NoteVars::new(note, ctx, mappers.uses("held")));

    // note on and note off curves on the same channel play on the same curve
    let mut channels: BTreeMap<&str, Vec<Key>> = BTreeMap::new();
    for (curves, offset) in [(&gen.note_on_keyframes, note.time_on + gen.note_on_anchor_point), (&gen.note_off_keyframes, note.time_off + gen.note_off_anchor_point)] {
        for curve in curves {
            // seconds, not frames, so timing follows the music rather than frame numbers
            let keys = note_keyframes(curve, offset, note.velocity, gen.velocity_intensity, mappers, vars.as_mut(), ctx.fps())?;
            channels.entry(curve.channel.as_str()).or_default().extend(keys);
        }
    }

    // a time mapper can put keys out of order, sorting puts them back
    Ok(channels
        .into_iter()
        .filter(|(_, keyframes)| !keyframes.is_empty())
        .map(|(channel, mut keyframes)| {
            keyframes.sort_by(|a, b| a.time.total_cmp(&b.time));
            CurveKeys {
                object: object.to_string(),
                channel: channel.to_string(),
                animation_overlap: gen.animation_overlap.clone(),
                overlap_blend: gen.overlap_blend,
                hit: Some(note.time_on),
                overlap_dampen: gen.overlap_dampen,
                overlap_use_anchor_value: gen.overlap_use_anchor_value,
                keyframes,
            }
        })
        .collect())
}

/// runs a generator curve's keys through its mappers, offsets them to a note's time and scales them by its velocity, in
/// that order like the old add-on. the handles go through the same, so they keep their shape. `vars` is `None` when
/// there are no mappers
fn note_keyframes(curve: &AnimCurve, offset: f64, velocity: u8, velocity_intensity: f64, mappers: &Mappers, mut vars: Option<&mut NoteVars>, fps: f64) -> Result<Vec<Key>, String> {
    let scale = if velocity_intensity != 0.0 {
        velocity as f64 / 127.0 * velocity_intensity
    } else {
        1.0
    };
    let mut map = |[time, value]: [f64; 2]| -> Result<[f64; 2], String> {
        let (time, value) = match vars.as_deref_mut() {
            Some(vars) => mappers.map(vars, time, value, fps)?,
            None => (time, value),
        };
        Ok([time + offset, value * scale])
    };
    let mut keys = Vec::with_capacity(curve.keys.len());
    for key in &curve.keys {
        let [time, value] = map([key.time, key.value])?;
        keys.push(Key::with_handles(time, value, key.interpolation, map(key.left)?, map(key.right)?));
    }
    Ok(keys)
}

/// combines each note's keys into its curve in order with the curve's overlap mode, then flattens to one list per object.
/// every object in `objects` is in the result, ones with no keys stay empty so the writer still clears them.
/// momentum looks at a note's neighbors on both sides, so a curve's momentum notes in a row are collected and combined
/// together, then added to the curve like one note
pub fn combine_curve_keys<'a>(objects: impl Iterator<Item = &'a String>, chunks: impl IntoIterator<Item = CurveKeys>) -> Result<HashMap<String, Vec<BlendKeyframe>>, String> {
    // keyframes per object, then per channel. overlap only combines keys on the same curve
    let mut obj_curves: HashMap<String, BTreeMap<String, Vec<Key>>> = objects.map(|name| (name.clone(), BTreeMap::new())).collect();
    // each curve's momentum notes waiting to be combined
    let mut runs: HashMap<(String, String), Vec<MomentumNote>> = HashMap::new();
    let flush = |obj_curves: &mut HashMap<String, BTreeMap<String, Vec<Key>>>, (object, channel): (String, String), run: Vec<MomentumNote>| {
        let inserted = obj_curves.entry(object).or_default().entry(channel).or_default();
        add_keyframes(inserted, &mut momentum_keys(run));
    };
    for mut chunk in chunks {
        let curve = (chunk.object, chunk.channel);
        if chunk.animation_overlap == "momentum" {
            let hit = chunk.hit.or(chunk.keyframes.first().map(|key| key.time)).unwrap_or_default();
            runs.entry(curve).or_default().push(MomentumNote {
                hit,
                keys: chunk.keyframes,
                dampen: chunk.overlap_dampen,
                use_anchor_value: chunk.overlap_use_anchor_value,
            });
            continue;
        }
        if let Some(run) = runs.remove(&curve) {
            flush(&mut obj_curves, curve.clone(), run);
        }
        let inserted = obj_curves.entry(curve.0).or_default().entry(curve.1).or_default();
        let settings = OverlapSettings {
            blend: chunk.overlap_blend,
        };
        combine_keyframes(&chunk.animation_overlap, &settings, inserted, &mut chunk.keyframes)?;
    }
    for (curve, run) in runs {
        flush(&mut obj_curves, curve, run);
    }
    Ok(obj_curves
        .into_iter()
        .map(|(name, curves)| {
            let keys = curves.into_iter().flat_map(|(channel, mut keys)| {
                finish_keys(&mut keys);
                keys.into_iter().map(move |key| BlendKeyframe {
                    channel: channel.clone(),
                    key,
                })
            });
            (name, keys.collect())
        })
        .collect())
}

/// a combined curve ready to write: in time order with no two keys at the same time, and handles on the keys made
/// where notes overlap
pub fn finish_keys(keys: &mut Vec<Key>) {
    sort_keys(keys, TIME_EPSILON);
    auto_handles(keys);
}

/// the ways a note's keyframes can combine with the keyframes already on its curve, the first is the default
pub const ANIMATION_OVERLAPS: [&str; 9] = ["add", "min", "max", "prev", "next", "rvc", "prune", "crossfade", "momentum"];

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
pub fn combine_keyframes(mode: &str, settings: &OverlapSettings, inserted_keys: &mut Vec<Key>, next_keys: &mut Vec<Key>) -> Result<(), String> {
    match mode {
        "add" | "" => add_keyframes(inserted_keys, next_keys),
        "min" => extreme_keyframes(inserted_keys, next_keys, f64::min),
        "max" => extreme_keyframes(inserted_keys, next_keys, f64::max),
        "prev" => prev_keyframes(inserted_keys, next_keys),
        "next" => next_keyframes(inserted_keys, next_keys),
        "rvc" => rest_value_crossing_keyframes(inserted_keys, next_keys),
        "prune" => prune_keyframes(inserted_keys, next_keys),
        "crossfade" => crossfade_keyframes(inserted_keys, next_keys, settings.blend),
        // momentum needs every note's hit and both neighbors, combine_curve_keys does it (utils/momentum.rs). two bare key
        // lists have neither, they add
        "momentum" => add_keyframes(inserted_keys, next_keys),
        other => return Err(format!("unknown animation overlap '{}', expected one of: {}", other, ANIMATION_OVERLAPS.join(", "))),
    }
    Ok(())
}

fn is_rest(value: f64) -> bool {
    value.abs() < REST_EPSILON
}

/// the value of a curve at `time`, held past either end, 0 without keys
pub fn value_at(keys: &[Key], time: f64) -> f64 {
    evaluate(keys, time).unwrap_or(0.0)
}

// whether the segment just before (or just after) `time` moves, a segment moves unless both its keys are at rest
fn moving_near(keys: &[Key], time: f64, before: bool) -> bool {
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
fn in_motion(keys: &[Key], time: f64) -> bool {
    !is_rest(value_at(keys, time)) || (moving_near(keys, time, true) && moving_near(keys, time, false))
}

// index of the first key at or after `start`, keys before it are never touched by a note starting at `start`
fn split_at(keys: &[Key], start: f64) -> usize {
    keys.partition_point(|k| k.time < start)
}

/// min and max: where both curves are in motion keep the smaller (or bigger) one, elsewhere whichever one is moving
/// a curve at rest doesn't count, so a note can't be flattened by the rest value of the one before it
fn extreme_keyframes(inserted_keys: &mut Vec<Key>, next_keys: &mut Vec<Key>, pick: fn(f64, f64) -> f64) {
    let Some(start) = next_keys.first().map(|k| k.time) else {
        return;
    };
    let split = split_at(inserted_keys, start);
    // the old curve from its last key before the note, that's all that can overlap
    let old: Vec<Key> = inserted_keys[split.saturating_sub(1)..].to_vec();

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

    let combined: Vec<Key> = times
        .iter()
        .map(|&time| {
            let (old_value, next_value) = (value_at(&old, time), value_at(next_keys, time));
            let (value, from): (f64, Option<&[Key]>) = match (in_motion(&old, time), in_motion(next_keys, time)) {
                (true, true) => {
                    let value = pick(old_value, next_value);
                    (
                        value,
                        Some(if value == old_value {
                            &old
                        } else {
                            next_keys.as_slice()
                        }),
                    )
                }
                (true, false) => (old_value, Some(&old)),
                (false, true) => (next_value, Some(next_keys.as_slice())),
                (false, false) => (0.0, None),
            };
            // a key of the curve that wins there stays as it is, the keys in between are made
            match from.and_then(|keys| keys.iter().find(|k| (k.time - time).abs() < f64::EPSILON)) {
                Some(key) => key.clone(),
                None => Key::auto(time, value),
            }
        })
        .collect();

    inserted_keys.truncate(split);
    inserted_keys.extend(combined);
    next_keys.clear();
}

/// previous: the animation already playing wins, a note that starts while it's still moving is dropped
fn prev_keyframes(inserted_keys: &mut Vec<Key>, next_keys: &mut Vec<Key>) {
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
fn cut_index(keys: &[Key], start: f64) -> usize {
    let split = split_at(keys, start - TIME_EPSILON);
    let on_start = keys.get(split).is_some_and(|k| k.time <= start + TIME_EPSILON);
    if on_start && in_motion(&keys[split.saturating_sub(1)..], start) {
        split + 1
    } else {
        split
    }
}

/// next: the new note wins, the old animation is cut off where the note starts
fn next_keyframes(inserted_keys: &mut Vec<Key>, next_keys: &mut Vec<Key>) {
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
fn crossfade_keyframes(inserted_keys: &mut Vec<Key>, next_keys: &mut Vec<Key>, blend: f64) {
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

    let faded: Vec<Key> = times
        .iter()
        .map(|&time| {
            // smoothstep from the full gap down to nothing
            let u = (time - start) / blend;
            let fade = 2.0 * u.powi(3) - 3.0 * u.powi(2) + 1.0;
            Key::auto(time, value_at(next_keys, time) + gap * fade)
        })
        .collect();

    inserted_keys.truncate(split);
    inserted_keys.extend(faded);
    inserted_keys.extend(next_keys.drain(..).filter(|k| k.time > end + TIME_EPSILON));
}

/// rest value crossing: the old animation keeps playing until it crosses its rest value, then the note plays from there
/// switching at rest means the value never jumps and the note keeps its whole shape, but it starts late while the old animation finishes
fn rest_value_crossing_keyframes(inserted_keys: &mut Vec<Key>, next_keys: &mut Vec<Key>) {
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
    inserted_keys.extend(next_keys.drain(..).map(|k| k.moved(delay)));
}

/// keyframe pruning: the old animation is cut off where the note starts, and the note's keys leading up to its first peak are dropped
/// when they'd make the curve change direction on the way there. what's left flows straight from the old animation into the note's
/// first peak, so with Blender's smooth handles the curve's slope doesn't jump
fn prune_keyframes(inserted_keys: &mut Vec<Key>, next_keys: &mut Vec<Key>) {
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
