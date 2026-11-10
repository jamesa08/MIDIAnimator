use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::io::{Inputs, NodeResult, Outputs, Val};
use crate::midi::MIDINote;
use crate::scene_generics::{find_group, find_object, AnimCurve, Object, ObjectGroup};
use crate::utils::note_to_name;

use super::scene::scene_fps;
use crate::utils::animation::{combine_curve_keys, note_curve_keys, AnimationGenerator, CurveKeys, NoteTarget, ObjectMap, ANIMATION_OVERLAPS, DEFAULT_OVERLAP_BLEND};
use crate::utils::console::{self, Level, Source};
use crate::utils::mapper::{NoteContext, DEFAULT_FPS};
use crate::utils::momentum::phases;
use crate::utils::note_table::{self, NoteTable};
use crate::utils::wave::{Cymbal, Drumstick, Spring};
use crate::graph::model::mode_default;

/// Node: keyframes_from_object
///
/// inputs:
/// "object_groups": `Array<ObjectGroup>`,
/// "object_group_name": `String`,
/// "object_name": `String`,
/// "channels": `Array<String>`
///
/// outputs:
/// "dyn_output": `Dyn<Array<Keyframe>>`,
/// "available_channels": `Array<Any>`
#[node_registry::node]
pub fn keyframes_from_object(object_groups: Option<&Vec<ObjectGroup>>, object_group_name: Option<&String>, object_name: Option<&String>, channels: Option<&Vec<String>>) -> NodeResult {
    let mut outputs = Outputs::new();
    let object_groups: &[ObjectGroup] = object_groups.map(Vec::as_slice).unwrap_or(&[]);
    let object_group_name = object_group_name.map(String::as_str).unwrap_or("");
    let object_name = object_name.map(String::as_str).unwrap_or("");
    let channels: &[String] = channels.map(Vec::as_slice).unwrap_or(&[]);

    // nothing picked yet, no dynamic outputs
    let mut dyn_output = serde_json::Map::new();
    if object_groups.is_empty() || object_group_name.is_empty() || object_name.is_empty() {
        outputs.set("dyn_output", dyn_output);
        outputs.set("available_channels", serde_json::Value::Array(vec![]));
        return Ok(outputs);
    }

    // find the object, a name that isn't in the scene is an error
    let object_group = find_group(object_groups, object_group_name).ok_or_else(|| format!("object group '{}' does not exist", object_group_name))?;
    let object = find_object(&object_group.objects, object_name).ok_or_else(|| format!("object '{}' does not exist in '{}'", object_name, object_group.name))?;

    /*
    example, with `location[2]` and a bone's `pose.bones["Arm"].location[0]` picked:
        {
            "dyn_output": {
                "location[2]": "Location Z",
                "pose.bones[\"Arm\"].location[0]": "Arm › Location X"
            },
            "location[2]": FCurveData,
            "pose.bones[\"Arm\"].location[0]": FCurveData,
            "available_channels": [{ "id": "location[2]", "group": "Object", "name": "Location Z", "label": "Location Z" }, ...]
        }
    */

    // every channel of the object with a curve, for the dropdowns
    let available = object_channels(object);
    outputs.set("available_channels", serde_json::to_value(&available).unwrap_or_default());

    // one output per picked channel, flat and inside dyn_output (see nodes_and_backend.md)
    for id in channels {
        let channel = available.iter().find(|c| &c.id == id).ok_or_else(|| format!("'{}' has no keyframes on '{}'", id, object.name))?;
        let anim_curve = object.anim_curves.iter().find(|c| channel_id(c) == *id).unwrap();
        dyn_output.insert(id.clone(), serde_json::Value::String(channel.label.clone()));
        outputs.set(id, anim_curve.clone());
    }

    outputs.set("dyn_output", dyn_output);
    Ok(outputs)
}

// MARK: - Channels

/// a channel in Keyframes From Object's dropdowns: its id, the group it's listed under, its name in the group (with the
/// axis) and its label as an output's name
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ChannelOption {
    pub id: String,
    pub group: String,
    pub name: String,
    pub label: String,
}

/// the id of an anim curve's channel, the bridge's
pub fn channel_id(anim_curve: &AnimCurve) -> String {
    anim_curve.channel.clone()
}

/// the channels of an object that have curves: in the order the bridge lists them, then curves on channels it doesn't
/// list (scene data saved before bridges listed channels) by their ids
pub fn object_channels(object: &Object) -> Vec<ChannelOption> {
    let has_curve = |id: &str| object.anim_curves.iter().any(|curve| curve.channel == id);
    let listed = object.channels.iter().filter(|channel| has_curve(&channel.id)).map(|channel| ChannelOption {
        id: channel.id.clone(),
        group: channel.group.clone(),
        name: channel.title(),
        label: object.channel_label(&channel.id),
    });
    let unlisted = object.anim_curves.iter().filter(|curve| object.channel(&curve.channel).is_none()).map(|curve| ChannelOption {
        id: curve.channel.clone(),
        group: String::new(),
        name: curve.channel.clone(),
        label: curve.channel.clone(),
    });
    listed.chain(unlisted).collect()
}

/// Node: animation_generator
/// these may change in the future
///
/// inputs:
/// "name": `String`,
/// "note_on_keyframes": `Array<FCurveData>`, multi, each curve plays on its own channel,
/// "note_on_anchor_point": `f64`, frames
/// "note_off_keyframes": `Array<FCurveData>`, multi,
/// "note_off_anchor_point": `f64`, frames
/// "time_mapper": `String`, an expression (utils/mapper.rs), typed in or from a Note Table,
/// "amplitude_mapper": `String`,
/// "velocity_intensity": `f64`,
/// "animation_overlap": `String`,
/// "overlap_blend": `f64`,
/// "overlap_dampen": `f64`, momentum, 0 to 1,
/// "overlap_use_anchor_value": `bool`, momentum
/// "overlap_windup": `f64`, momentum, frames from the hit to the windup's peak, negative
/// "overlap_recoil": `f64`, momentum, frames from the hit to the recoil, positive
///
/// outputs:
/// "generator": `AnimationGenerator`
#[node_registry::node]
pub fn animation_generator(name: Option<String>, note_on_keyframes: Option<&Vec<AnimCurve>>, note_on_anchor_point: Option<f64>, note_off_keyframes: Option<&Vec<AnimCurve>>, note_off_anchor_point: Option<f64>, time_mapper: Option<&Value>, amplitude_mapper: Option<&Value>, velocity_intensity: Option<f64>, animation_overlap: Option<String>, overlap_blend: Option<f64>, overlap_dampen: Option<f64>, overlap_use_anchor_value: Option<bool>, overlap_windup: Option<f64>, overlap_recoil: Option<f64>) -> NodeResult {
    // the keyframe curves are optional, an unconnected one is just no keyframes
    let note_on_keyframes = note_on_keyframes.cloned().unwrap_or_default();
    let note_off_keyframes = note_off_keyframes.cloned().unwrap_or_default();

    // two curves on one channel would play over each other, the curve says which channel it's on
    for (curves, side) in [(&note_on_keyframes, "note on"), (&note_off_keyframes, "note off")] {
        let mut seen: Vec<String> = Vec::new();
        for curve in curves {
            let id = channel_id(curve);
            if seen.contains(&id) {
                return Err(format!("two {} curves are on {}, use Set Channel to move one", side, id));
            }
            seen.push(id);
        }
    }

    // unset is the default, "add"
    let mut animation_overlap = animation_overlap.unwrap_or_default();
    if animation_overlap.is_empty() {
        animation_overlap = ANIMATION_OVERLAPS[0].to_string();
    }
    if !ANIMATION_OVERLAPS.contains(&animation_overlap.as_str()) {
        return Err(format!("unknown animation overlap '{}', expected one of: {}", animation_overlap, ANIMATION_OVERLAPS.join(", ")));
    }

    // unset uses the default, a crossfade can't go backwards in time
    let overlap_blend = overlap_blend.unwrap_or(DEFAULT_OVERLAP_BLEND);
    if overlap_blend < 0.0 {
        return Err(format!("overlap blend can't be negative, got {}", overlap_blend));
    }

    // momentum's dampening, unset is none
    let overlap_dampen = overlap_dampen.unwrap_or_default();
    if !(0.0..=1.0).contains(&overlap_dampen) {
        return Err(format!("overlap dampen goes from 0 to 1, got {}", overlap_dampen));
    }

    // momentum's windup comes before the hit and its recoil after it. one on the wrong side is found again rather than
    // failing the node, which would take its phase editor with it and leave nothing to move it back with
    let name = name.unwrap_or_default();
    let side = |value: Option<f64>, before: bool, input: &str| match value {
        Some(frames) if frames.is_finite() && (frames < 0.0) == before && frames != 0.0 => Some(frames),
        Some(frames) => {
            let wanted = if before {
                "before"
            } else {
                "after"
            };
            console::log(Level::Warn, Source::Node, format!("Animation Generator '{}': the {} is {} frames from the hit, it has to be {} it, found it on the curve instead", name, input, frames, wanted));
            None
        }
        None => None,
    };
    let overlap_windup = side(overlap_windup, true, "windup");
    let overlap_recoil = side(overlap_recoil, false, "recoil");

    // a mapper is an expression, nothing set changes nothing. one saved with the expression inside an object (how the
    // mappers were saved for a while) reads its expression
    let mapper = |value: Option<&Value>, name: &str| -> Result<String, String> {
        match value {
            None | Some(Value::Null) => Ok(String::new()),
            Some(Value::String(text)) => Ok(text.clone()),
            Some(Value::Object(saved)) => Ok(saved.get("expression").and_then(Value::as_str).unwrap_or_default().to_string()),
            Some(other) => Err(format!("{} must be text, got {}", name, other)),
        }
    };
    let time_mapper = mapper(time_mapper, "Time Mapper")?;
    let amplitude_mapper = mapper(amplitude_mapper, "Amplitude Mapper")?;

    let mut generator = AnimationGenerator {
        name,
        note_on_keyframes,
        note_on_anchor_point: note_on_anchor_point.unwrap_or_default(),
        note_off_keyframes,
        note_off_anchor_point: note_off_anchor_point.unwrap_or_default(),
        time_mapper,
        amplitude_mapper,
        velocity_intensity: velocity_intensity.unwrap_or_default(),
        animation_overlap,
        overlap_blend,
        overlap_dampen,
        overlap_use_anchor_value: overlap_use_anchor_value.unwrap_or(true),
        overlap_windup,
        overlap_recoil,
        compiled: Default::default(),
    };

    // momentum's windup and recoil that aren't set are found on the first note on curve, around its hit
    let momentum = generator.animation_overlap == "momentum";
    if let (true, Some(curve)) = (momentum, generator.note_on_keyframes.iter().find(|curve| !curve.keys.is_empty())) {
        let fps = scene_fps().unwrap_or(DEFAULT_FPS);
        let hit = -generator.note_on_anchor_point / fps;
        let [windup, recoil] = phases(&curve.keys, hit);
        // a point found at the hit (a curve that starts or ends there) is a frame from it
        generator.overlap_windup = generator.overlap_windup.or(Some(((windup - hit) * fps).min(-0.5)));
        generator.overlap_recoil = generator.overlap_recoil.or(Some(((recoil - hit) * fps).max(0.5)));
    }

    // the mappers compile once here, a mistake in one fails this node
    let mappers = generator.mappers()?;
    // anchor points are frames too
    let anchored = generator.note_on_anchor_point != 0.0 || generator.note_off_anchor_point != 0.0;
    if (mappers.needs_fps() || anchored) && scene_fps().is_none() {
        console::log(Level::Warn, Source::Node, format!("Animation Generator '{}': no scene frame rate, the mappers and anchor points assume {} fps", generator.name, DEFAULT_FPS));
    }

    let mut outputs = Outputs::new();
    outputs.set("generator", generator);
    Ok(outputs)
}

/// Node: note_table
///
/// inputs:
/// "note_map": `HashMap<String, Array<u8>>`, each object's notes, from Assign Notes to Objects' Note Map. without one the
/// table is a range, whatever its source says,
/// "table": `Any`, the table's settings (utils/note_table.rs `NoteTable`)
///
/// outputs:
/// "expression": `String`, the expression a mapper runs,
/// "preview": `Any`, the rows and the curve the node shows, hidden
#[node_registry::node]
pub fn note_table(note_map: Option<&BTreeMap<String, Vec<u8>>>, table: Option<&Value>) -> NodeResult {
    // nothing set is the defaults
    let table: NoteTable = match table {
        None | Some(Value::Null) => NoteTable::default(),
        Some(value) => serde_json::from_value(value.clone()).map_err(|e| format!("the table can't be read: {}", e))?,
    };
    // the note map is only a source while one is connected, without one the table is a range
    let mut table = table;
    if note_map.is_none() {
        table.source = note_table::Source::Range;
    }
    let empty = BTreeMap::new();
    let preview = note_table::preview(&table, note_map.unwrap_or(&empty))?;

    let mut outputs = Outputs::new();
    outputs.set("expression", note_table::expression(&table, &preview));
    outputs.set("preview", serde_json::to_value(&preview).unwrap_or_default());
    Ok(outputs)
}

/// Node: set_channel
///
/// inputs:
/// "keyframes": `FCurveData`,
/// "channel": `String`, a channel id of the connected app like Blender's `rotation_euler[1]`, empty leaves the curve where
/// it is
///
/// outputs:
/// "keyframes": `FCurveData`, the same keys on the new channel
#[node_registry::node]
pub fn set_channel(keyframes: &AnimCurve, channel: Option<&String>) -> NodeResult {
    let mut curve = keyframes.clone();
    let channel = channel.map(|c| c.trim()).unwrap_or("");
    if !channel.is_empty() {
        curve.channel = channel.to_string();
    }

    let mut outputs = Outputs::new();
    outputs.set("keyframes", curve);
    Ok(outputs)
}

// MARK: - Preset Animation

/// the Preset Animation node's modes, same ids as `modes` in its UI (src/nodes/preset_animation.tsx)
pub const PRESET_ANIMATION_MODES: [&str; 3] = ["spring", "drumstick", "cymbal"];

/// Node: preset_animation
///
/// inputs:
/// "mode": `String`, spring, drumstick or cymbal
/// "channel", "channel_y": `String`, the channels of the curve and the cymbal's second curve
/// spring and cymbal: "start", "end", "period" (frames), "amplitude", "dampen" (0 to 1), "rest". unset end, period,
/// amplitude and dampen are the mode's, `mode_defaults` in the spec
/// spring: "flip" (`bool`, the first swing goes down)
/// cymbal: "angle" (degrees), "detune" (percent), "rest_y"
/// drumstick: its phases' lengths in frames, "windup", "hit", "recoil" and "rest_length", and the values they end at,
/// "windup_value", "hit_value", "recoil_value" and "rest" (where it starts too). the hit lands on the note
///
/// outputs:
/// "keyframes": `FCurveData`, and "keyframes_y" for a cymbal
#[node_registry::node]
pub fn preset_animation(mode: Option<String>, channel: Option<String>, channel_y: Option<String>, start: Option<f64>, end: Option<f64>, period: Option<f64>, amplitude: Option<f64>, dampen: Option<f64>, flip: Option<bool>, rest: Option<f64>, angle: Option<f64>, detune: Option<f64>, rest_y: Option<f64>, windup: Option<f64>, windup_value: Option<f64>, hit: Option<f64>, hit_value: Option<f64>, recoil: Option<f64>, recoil_value: Option<f64>, rest_length: Option<f64>) -> NodeResult {
    let mode = mode.filter(|m| !m.is_empty()).unwrap_or_else(|| PRESET_ANIMATION_MODES[0].to_string());
    if !PRESET_ANIMATION_MODES.contains(&mode.as_str()) {
        return Err(format!("unknown mode '{}', expected one of: {}", mode, PRESET_ANIMATION_MODES.join(", ")));
    }
    // times are typed in frames, keys are seconds
    let fps = scene_fps().unwrap_or(DEFAULT_FPS);
    let seconds = |frames: f64| frames / fps;
    let default = |input: &str| mode_default("preset_animation", input, &mode).unwrap_or(0.0);
    let finite = |value: Option<f64>, default: f64, name: &str| -> Result<f64, String> {
        let value = value.unwrap_or(default);
        if value.is_finite() {
            Ok(value)
        } else {
            Err(format!("{} has to be a number, got {}", name, value))
        }
    };
    let rest = finite(rest, 0.0, "rest")?;
    // unset or emptied is the default, the first rotation axes. channel ids are the bridge's, any goes as typed
    let channel_or = |channel: Option<String>, default: &str| channel.map(|c| c.trim().to_string()).filter(|c| !c.is_empty()).unwrap_or_else(|| default.to_string());
    let channel = channel_or(channel, "rotation_euler[0]");

    let spring = || -> Result<Spring, String> {
        let (start, end) = (finite(start, 0.0, "start")?, finite(end, default("end"), "end")?);
        if end <= start {
            return Err(format!("the end ({}) has to come after the start ({})", end, start));
        }
        let period = finite(period, default("period"), "period")?;
        if period <= 0.0 {
            return Err(format!("the period has to be more than 0, got {}", period));
        }
        let dampen = finite(dampen, default("dampen"), "dampen")?;
        if !(0.0..=1.0).contains(&dampen) {
            return Err(format!("dampen goes from 0 to 1, got {}", dampen));
        }
        Ok(Spring {
            start: seconds(start),
            end: seconds(end),
            amplitude: finite(amplitude, default("amplitude"), "amplitude")?,
            period: seconds(period),
            dampen,
            rest,
            flip: flip.unwrap_or(false),
        })
    };

    let mut outputs = Outputs::new();
    match mode.as_str() {
        "spring" => outputs.set(
            "keyframes",
            AnimCurve {
                channel,
                keys: spring()?.keys(),
            },
        ),
        "drumstick" => {
            let stick = Drumstick {
                windup: seconds(finite(windup, 8.0, "windup")?),
                windup_value: finite(windup_value, 0.6, "windup value")?,
                hit: seconds(finite(hit, 5.0, "hit")?),
                hit_value: finite(hit_value, -0.3, "hit value")?,
                recoil: seconds(finite(recoil, 6.0, "recoil")?),
                recoil_value: finite(recoil_value, 0.15, "recoil value")?,
                settle: seconds(finite(rest_length, 8.0, "rest length")?),
                rest,
            };
            if let Some(phase) = stick.check() {
                return Err(format!("the {} has to be longer than 0 frames", phase));
            }
            outputs.set(
                "keyframes",
                AnimCurve {
                    channel,
                    keys: stick.keys(),
                },
            );
        }
        _ => {
            let channel_y = channel_or(channel_y, "rotation_euler[1]");
            if channel_y == channel {
                return Err(format!("both curves are on {}, give the second one its own channel", channel));
            }
            let detune = finite(detune, 6.1, "detune")?;
            if detune <= -100.0 {
                return Err(format!("the detune has to be more than -100%, got {}", detune));
            }
            let cymbal = Cymbal {
                spring: Spring {
                    flip: false,
                    ..spring()?
                },
                angle: finite(angle, 45.0, "angle")?.to_radians(),
                detune: detune / 100.0,
                rest: [rest, finite(rest_y, 0.0, "rest y")?],
            };
            let [x, y] = cymbal.keys();
            outputs.set(
                "keyframes",
                AnimCurve {
                    channel,
                    keys: x,
                },
            );
            outputs.set(
                "keyframes_y",
                AnimCurve {
                    channel: channel_y,
                    keys: y,
                },
            );
        }
    }
    Ok(outputs)
}

pub fn pad_nums(mut nums: Vec<u8>, pad_amount: usize) -> Vec<u8> {
    if nums.is_empty() {
        return Vec::new();
    }

    // remove duplicates and sort
    nums.sort();
    nums.dedup();
    let mut result = nums.clone();

    if result.len() >= pad_amount {
        return result[..pad_amount].to_vec();
    }

    // pad within bounds first
    let mut i = 0;
    while result.len() < pad_amount && i < nums.len() - 1 {
        let current = nums[i];
        let next_num = nums[i + 1];
        let gap = next_num - current - 1;

        if gap > 0 {
            let to_add = (pad_amount - result.len()).min(gap as usize);
            // spread evenly with integer math: to_add <= gap keeps the offsets at least 1 apart, after current and
            // before next_num, so they never collide (float rounding did when to_add == gap)
            for j in 1..=to_add {
                let offset = (j * (gap as usize + 1)) / (to_add + 1);
                result.push(current + offset as u8);
            }
        }
        i += 1;
    }

    // if we still need more numbers, add them below and above alternately
    while result.len() < pad_amount {
        // the next free note below and above, staying inside the MIDI range 0-127
        let min = *result.iter().min().unwrap_or(&0);
        let max = *result.iter().max().unwrap_or(&127);
        let below = min.checked_sub(1);
        let above = max.checked_add(1).filter(|n| *n <= 127);

        // alternate below and above, use the other side when one runs out, stop when both do
        let next = if result.len() % 2 == 0 {
            below.or(above)
        } else {
            above.or(below)
        };
        let Some(next) = next else {
            break;
        };
        result.push(next);
    }

    result.sort();
    return result;
}

// FIXME make static from midi/mod.rs
pub fn all_used_notes_from_array(notes: &[MIDINote]) -> Vec<u8> {
    let mut used_notes: Vec<u8> = notes.iter().map(|note| note.note_number).collect();
    used_notes.sort_unstable();
    used_notes.dedup();
    used_notes
}

/// the entries of the note list on Assign Notes to Objects as given: a list from a connection, or text typed in
/// ("60, 61 62", with or without surrounding brackets like "[60, 61]"). empty when there's nothing
pub fn note_list_entries(value: Option<&Value>) -> Vec<String> {
    match value {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::String(text)) => text.trim().trim_start_matches('[').trim_end_matches(']').split(|c: char| c == ',' || c.is_whitespace()).filter(|s| !s.is_empty()).map(String::from).collect(),
        Some(Value::Array(items)) => items.iter().map(|v| v.as_str().map(String::from).unwrap_or_else(|| v.to_string())).collect(),
        Some(other) => vec![other.to_string()],
    }
}

/// a note list entry as a note number 0-127
fn note_number(entry: &str) -> Option<u8> {
    entry.parse::<u8>().ok().filter(|n| *n <= 127)
}

/// the note in an object's name after its last underscore, a number or a name like the old add-on: Cube_60 and Cube_C3
/// are both 60 (C3 = 60, sharps like Cube_F#3). None when there isn't one
pub fn note_in_name(name: &str) -> Option<u8> {
    let (_, suffix) = name.rsplit_once('_')?;
    if let Some(note) = note_number(suffix) {
        return Some(note);
    }

    // a note name: letter, optional sharp, octave from -2
    let mut chars = suffix.chars();
    let base = match chars.next()?.to_ascii_uppercase() {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let rest = chars.as_str();
    let (sharp, octave) = match rest.strip_prefix('#') {
        Some(octave) => (1, octave),
        None => (0, rest),
    };
    let note = base + sharp + (octave.parse::<i32>().ok()? + 2) * 12;
    (0..=127).contains(&note).then_some(note as u8)
}

/// compares names with the numbers in them as numbers, so Cube_2 comes before Cube_10. names that only differ in
/// leading zeros (Cube_02, Cube_2) fall back to plain text order so the order never depends on the scene's
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let (whole_a, whole_b) = (a, b);
    let (mut a, mut b) = (a, b);
    loop {
        match (a.chars().next(), b.chars().next()) {
            (None, None) => return whole_a.cmp(whole_b),
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            // a run of digits on both sides compares by value: fewer digits (leading zeros left off) is smaller
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let a_end = a.find(|c: char| !c.is_ascii_digit()).unwrap_or(a.len());
                let b_end = b.find(|c: char| !c.is_ascii_digit()).unwrap_or(b.len());
                let (a_num, b_num) = (a[..a_end].trim_start_matches('0'), b[..b_end].trim_start_matches('0'));
                let order = a_num.len().cmp(&b_num.len()).then_with(|| a_num.cmp(b_num));
                if order != Ordering::Equal {
                    return order;
                }
                a = &a[a_end..];
                b = &b[b_end..];
            }
            (Some(x), Some(y)) => {
                if x != y {
                    return x.cmp(&y);
                }
                a = &a[x.len_utf8()..];
                b = &b[y.len_utf8()..];
            }
        }
    }
}

/// why a note list can't be used, then what it gives each object the way the old add-on's Assign Notes to Objects
/// dialog showed it ("Object: Cube_60 => Note: 60/C3"), with what's wrong in place of a note
fn note_list_error(objects: &[&Object], group_name: &str, entries: &[String]) -> String {
    let mut lines = Vec::new();
    if entries.len() != objects.len() {
        lines.push(format!("got {} note numbers for {} objects in '{}', give one per object", entries.len(), objects.len(), group_name));
    }
    let bad: Vec<String> = entries.iter().filter(|e| note_number(e).is_none()).map(|e| format!("'{}'", e)).collect();
    match bad.len() {
        0 => {}
        1 => lines.push(format!("{} isn't a note number (0-127)", bad[0])),
        _ => lines.push(format!("{} aren't note numbers (0-127)", bad.join(", "))),
    }
    lines.push(String::new());

    // one line per object or entry, whichever there are more of
    for i in 0..objects.len().max(entries.len()) {
        let object = objects.get(i).map_or("none", |o| o.name.as_str());
        let note = match entries.get(i) {
            None => "missing".to_string(),
            Some(entry) => match note_number(entry) {
                Some(n) => format!("{}/{}", n, note_to_name(n as i32)),
                None => format!("'{}'?", entry),
            },
        };
        lines.push(format!("Object: {} => Note: {}", object, note));
    }
    lines.join("\n")
}

/// Node: assign_notes_to_objects
/// read assign_midi_notes_to_objects.md for more information
///
/// inputs:
/// "object_groups": `Array<ObjectGroup>`,
/// "object_group_name": `String`,
/// "midi_notes": `Array<MIDINote>`,
/// "note_numbers": `Array<u8>` (or text typed in, see `note_list_entries`),
/// "generator": `Array<AnimationGenerator>`, multi, keyed by name in the object map and the note map,
/// "mode": `String`, "rules" or "map",
/// "note_map": `Any`, the map drawn in the app (`NoteMap`)
///
/// outputs:
/// "object_map": `ObjectMap`,
/// "object_notes": `HashMap<String, Array<u8>>`, each object's notes, it doesn't read the generators

/*
# Methods for Assigning MIDI Note Numbers to Objects

There are five main ways to assign MIDI note numbers to objects. Each method has its own advantages and use cases.

Methods 1-4 use the `Assign Notes to Object` node while method 5 uses the `Visual Note Map` node.

## 1. Object Name with Embedded Note Number

In this method, the note number is directly embedded in the object's name, separated by an underscore.

It's tried first (after a note list, method 4). The part after the last underscore can also be a note name like the old
add-on took (Cube_C3 => Note 60). Objects without one in the same group share the MIDI notes no named object plays, as
in methods 2 and 3. In every method objects are taken in name order, numbers compared as numbers (Cube_2 before Cube_10).

- **Format**: ObjectName_NoteNumber
- **Example**:

```other
Cube_60 => Note 60
Cube_61 => Note 61
Cube_62 => Note 62
Cube_63 => Note 63
```

- **Pros**: Simple and straightforward
- **Cons**: Inflexible, requires consistent naming convention

## 2. Direct Assignment from MIDI Track

This method assigns note numbers based on the order of unique notes in a MIDI track.

- **Input**: MIDI track with unique note numbers [60, 61, 62, 63]
- **Assignment**:

```other
Cube.000 => Note 60
Cube.001 => Note 61
Cube.002 => Note 62
Cube.003 => Note 63
```

- **Pros**: Flexible, allows for dynamic assignment
- **Cons**: Requires exact match between number of objects and unique MIDI notes

## 3. Flexible Assignment with Padding

This method is similar to the second, but adds flexibility when the number of objects doesn't match the number of unique MIDI notes.

### Example A: Fewer MIDI notes than objects

- **Input**: MIDI track with unique note numbers [60, 63]
- **Padding**: Function expands to [60, 61, 62, 63]
- **Assignment**:

```other
Cube.000 => Note 60
Cube.001 => Note 61
Cube.002 => Note 62
Cube.003 => Note 63
```

### Example B: More MIDI notes than objects

- **Input**: MIDI track with unique note numbers [60, 61, 62, 63, 64, 65]
- **Assignment**: Use available notes until objects are exhausted

```other
Cube.000 => Note 60
Cube.001 => Note 61
Cube.002 => Note 62
Cube.003 => Note 63
```

- **Pros**: Most flexible, handles mismatches between object count and note count
- **Cons**: May require additional logic for padding or truncation

## 4. User-Provided Object List

This method allows users to directly input note numbers, but requires that the number of notes matches the number of objects.

**User inputs:** [60, 61, 62, 63]

```other
Cube.000 => Note 60
Cube.001 => Note 61
Cube.002 => Note 62
Cube.003 => Note 63
```

**Note:** Connect the list to the Note Numbers input (e.g. from Get MIDI Track Data's Unique Note Numbers), or type it in
the node's properties. Leave it empty to use methods 2 and 3 instead. A list that doesn't match the object count is an
error on the node.

## 5. Visual Mapping Method

This method offers the most flexibility but requires more setup. Unlike the previous methods which assume a 1:1 mapping and one animation curve, the visual mapping approach allows for:

- Assigning multiple notes to objects
- Applying multiple animations to objects

### Key Features:

- Highly flexible assignment process
- Can create complex relationships between notes, objects, and animations
- Ideal for advanced cases which need precise control over the setup

### Use Cases:

- When objects need to respond to multiple MIDI notes
- For creating layered or complex animations triggered by different notes
- In scenarios where different parts of an object should animate based on different MIDI inputs

While this method requires more initial setup, it provides the greatest degree of creative control and can be used to create more sophisticated MIDI-driven animations and interactions.


*/
#[node_registry::node]
pub fn assign_notes_to_objects(object_groups: Option<&Vec<ObjectGroup>>, object_group_name: Option<&String>, midi_notes: Option<&Vec<MIDINote>>, note_numbers: Option<&Value>, generator: Option<&Vec<AnimationGenerator>>, mode: Option<&String>, note_map: Option<&Value>) -> NodeResult {
    let midi_notes: &[MIDINote] = midi_notes.map(Vec::as_slice).unwrap_or(&[]);
    let object_groups: &[ObjectGroup] = object_groups.map(Vec::as_slice).unwrap_or(&[]);
    let object_group_name = object_group_name.map(String::as_str).unwrap_or("");
    let map_mode = mode.map(String::as_str) == Some("map");

    /*  ObjectMap example:
       {
       "animations": {
           "ANIM_test": AnimationGenerator
       },
       "objects": {
           "object1": {
               "ANIM_test": [45, 46]
           },
           ...
           }
       }
    */
    let mut object_map = ObjectMap::default();
    let mut outputs = Outputs::new();

    // nothing picked yet, empty object map
    if object_groups.is_empty() || object_group_name.is_empty() {
        outputs.set("object_map", object_map);
        outputs.set("object_notes", BTreeMap::<String, Vec<u8>>::new());
        return Ok(outputs);
    }

    // find the object group, a name that isn't in the scene is an error
    let object_group = find_group(object_groups, object_group_name).ok_or_else(|| format!("object group '{}' does not exist", object_group_name))?;

    // the generators connected, by name in connect order. the same one twice is fine, two different ones can't share
    // a name
    let mut animations: Vec<&str> = Vec::new();
    for generator in generator.map(Vec::as_slice).unwrap_or(&[]) {
        match object_map.animations.get(&generator.name) {
            Some(existing) if existing != generator => {
                return Err(if generator.name.is_empty() {
                    "two different animation generators have no name, name them so the note map can tell them apart".to_string()
                } else {
                    format!("two different animation generators are named '{}', rename one of them", generator.name)
                });
            }
            Some(_) => {}
            None => {
                object_map.animations.insert(generator.name.clone(), generator.clone());
                animations.push(&generator.name);
            }
        }
    }

    // objects pair with notes in name order (Cube_2 before Cube_10), not the order the scene lists them in
    let mut objects: Vec<&Object> = object_group.objects.iter().collect();
    objects.sort_by(|a, b| natural_cmp(&a.name, &b.name));

    // map mode takes each object's notes and animations from the note map drawn in the app, rules mode works the notes
    // out (methods 1-4) and every object plays every animation
    let saved = if map_mode {
        Some(parse_note_map(note_map)?)
    } else {
        None
    };
    let pairs: Vec<(&Object, u8)> = match &saved {
        Some(saved) => objects.iter().flat_map(|object| saved.objects.get(&object.id).into_iter().flatten().map(move |note| (*object, *note))).collect(),
        None => rule_pairs(&objects, object_group_name, &all_used_notes_from_array(midi_notes), &note_list_entries(note_numbers))?,
    };

    // each object's notes without the animations, the Note Map output. it doesn't read the generators (`reads` in
    // default_nodes.json), so it can feed a Note Table that feeds one of them
    let mut object_notes: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for (object, note_number) in &pairs {
        add_note(object_notes.entry(object.id.clone()).or_default(), *note_number);
    }
    outputs.set("object_notes", object_notes);

    // extra objects (not enough notes) are left out. in map mode an object plays the animations wired to it, a map from
    // before they were wired plays every animation on every object
    for (object, note_number) in &pairs {
        let entry = object_map.objects.entry(object.id.clone()).or_default();
        for name in &animations {
            let wired = saved.as_ref().and_then(|s| s.animations.as_ref()).map_or(true, |wired| wired.get(*name).is_some_and(|objects| objects.contains(&object.id)));
            if wired {
                add_note(entry.entry(name.to_string()).or_default(), *note_number);
            }
        }
    }

    outputs.set("object_map", object_map);
    Ok(outputs)
}

/// adds a note to a list of notes, kept in order and without repeats
fn add_note(notes: &mut Vec<u8>, note: u8) {
    if let Err(at) = notes.binary_search(&note) {
        notes.insert(at, note);
    }
}

/// the objects' notes by the rules (methods 1-4 above), in name order. `objects` are sorted by name, `used_notes` are
/// the MIDI's unique notes
pub fn rule_pairs<'a>(objects: &[&'a Object], object_group_name: &str, used_notes: &[u8], note_entries: &[String]) -> Result<Vec<(&'a Object, u8)>, String> {
    // case 4 when a note list is given, it needs one note number per object
    if !note_entries.is_empty() {
        println!("case 4: user-provided note list");
        let numbers: Option<Vec<u8>> = note_entries.iter().map(|e| note_number(e)).collect();
        return match numbers {
            Some(numbers) if numbers.len() == objects.len() => Ok(objects.iter().copied().zip(numbers).collect()),
            _ => Err(note_list_error(objects, object_group_name, note_entries)),
        };
    }

    // case 1 for objects with a note in their name (Cube_60, Cube_C3)
    let (named, unnamed): (Vec<_>, Vec<_>) = objects.iter().map(|object| (*object, note_in_name(&object.name))).partition(|(_, note)| note.is_some());
    let mut pairs: Vec<(&Object, u8)> = named.into_iter().filter_map(|(object, note)| Some((object, note?))).collect();
    if !pairs.is_empty() {
        println!("case 1: note numbers from object names");
    }

    // the other objects share the MIDI's notes that no named object plays
    let unnamed: Vec<&Object> = unnamed.into_iter().map(|(object, _)| object).collect();
    let used_notes: Vec<u8> = used_notes.iter().copied().filter(|n| !pairs.iter().any(|(_, named)| named == n)).collect();

    // case 2 when the object count is the same as the note count, otherwise case 3
    let notes = if unnamed.len() == used_notes.len() {
        println!("case 2: direct assignment from MIDI track");
        used_notes
    } else {
        println!("case 3: flexible assignment with padding");
        pad_nums(used_notes, unnamed.len())
    };
    pairs.extend(unnamed.into_iter().zip(notes));
    Ok(pairs)
}

/// the note map drawn in the app (src/components/nodegraph/NoteMapView.tsx): each object's notes, the objects each
/// animation is wired to (by the generator's name), and notes added that nothing is connected to yet. a map without
/// animations is from before they were wired in the map, every animation plays on every object
#[derive(Deserialize, Default, Debug)]
pub struct NoteMap {
    #[serde(default)]
    pub objects: BTreeMap<String, Vec<u8>>,
    #[serde(default)]
    pub notes: Vec<u8>,
    #[serde(default)]
    pub animations: Option<BTreeMap<String, Vec<String>>>,
}

fn parse_note_map(value: Option<&Value>) -> Result<NoteMap, String> {
    match value {
        None | Some(Value::Null) => Ok(NoteMap::default()),
        Some(value) => serde_json::from_value(value.clone()).map_err(|e| format!("the note map can't be read: {}", e)),
    }
}

/// pads note numbers out to `amount` like the rules do (pad_nums), for the note map's Pad Range tool
#[tauri::command]
pub fn pad_note_numbers(notes: Vec<u8>, amount: usize) -> Vec<u8> {
    pad_nums(notes, amount)
}

/// Node: merge_object_maps
///
/// inputs:
/// "object_maps": `Dyn<ObjectMap>`, one input per map, `object_maps_0`, `object_maps_1`, ...
///
/// outputs:
/// "object_map": `ObjectMap`
#[node_registry::node]
pub fn merge_object_maps(inputs: &Inputs) -> NodeResult {
    // merge the maps in input order
    let mut object_map = ObjectMap::default();
    for other in inputs.dynamic::<ObjectMap>("object_maps")? {
        object_map.merge(other.get::<ObjectMap>().clone())?;
    }

    let mut outputs = Outputs::new();
    outputs.set("object_map", object_map);
    Ok(outputs)
}

/// Node: note_targets
///
/// inputs:
/// "object_map": `ObjectMap`
///
/// outputs:
/// "targets": `HashMap<u8, Array<NoteTarget>>`, the objects and animations each note number triggers
#[node_registry::node]
pub fn note_targets(object_map: &ObjectMap) -> NodeResult {
    let mut outputs = Outputs::new();
    outputs.set("targets", object_map.note_targets()?);
    Ok(outputs)
}

/// Node: targets_for_note
///
/// inputs:
/// "targets": `HashMap<u8, Array<NoteTarget>>`,
/// "note": `MIDINote`
///
/// outputs:
/// "targets": `Array<NoteTarget>`, empty when the note triggers nothing
#[node_registry::node]
pub fn targets_for_note(targets: &BTreeMap<u8, Vec<NoteTarget>>, note: &MIDINote) -> NodeResult {
    let mut outputs = Outputs::new();
    outputs.set("targets", targets.get(&note.note_number).cloned().unwrap_or_default());
    Ok(outputs)
}

/// Node: note_keyframes
///
/// inputs:
/// "object_map": `ObjectMap`, where the target's animation is looked up,
/// "target": `NoteTarget`,
/// "note": `MIDINote`,
/// "notes": `Array<MIDINote>`, the track the note is in, for the mappers' index, gap, prev, next and held,
/// "index": `f64`, where the note is in `notes`, looked for when it isn't given
///
/// outputs:
/// "keys": `Array<CurveKeys>`, one per channel the generator plays on, empty when it has no keyframes
#[node_registry::node]
pub fn note_keyframes(object_map: &ObjectMap, target: &NoteTarget, note: &MIDINote, notes: Option<&Vec<MIDINote>>, index: Option<f64>) -> NodeResult {
    let generator = object_map.generator(&target.animation, &target.object)?;
    let ctx = NoteContext {
        notes: notes.map(Vec::as_slice).unwrap_or(&[]),
        index: index.filter(|i| *i >= 0.0).map(|i| i as usize),
        fps: scene_fps(),
    };
    let mut outputs = Outputs::new();
    outputs.set("keys", note_curve_keys(&target.object, generator, note, &ctx)?);
    Ok(outputs)
}

/// Node: combine_keyframes
///
/// inputs:
/// "object_map": `ObjectMap`, every object in it gets an entry, even with no keys,
/// "keys": `Array<Any>`, `CurveKeys` in note order, nested lists (from loops inside loops) are flattened
///
/// outputs:
/// "keyframes": `HashMap<String, Array<BlendKeyframe>>`
#[node_registry::node]
pub fn combine_keyframes(object_map: Option<&ObjectMap>, inputs: &Inputs) -> NodeResult {
    let mut chunks = Vec::new();
    if let Some(keys) = inputs.val("keys") {
        flatten_curve_keys(keys, &mut chunks)?;
    }
    let objects = object_map.map(|m| m.objects.keys().collect::<Vec<_>>()).unwrap_or_default();
    let mut outputs = Outputs::new();
    outputs.set("keyframes", combine_curve_keys(objects.into_iter(), chunks)?);
    Ok(outputs)
}

/// collects `CurveKeys` out of nested lists in order, nulls are skipped
fn flatten_curve_keys(value: &Val, out: &mut Vec<CurveKeys>) -> Result<(), String> {
    if let Some(keys) = value.downcast_ref::<Vec<CurveKeys>>() {
        out.extend(keys.iter().cloned());
    } else if let Some(keys) = value.downcast_ref::<CurveKeys>() {
        out.push(keys.clone());
    } else if value.is_null() {
    } else if let Some(items) = value.items() {
        for item in &items {
            flatten_curve_keys(item, out)?;
        }
    } else {
        // JSON from somewhere else (a test, or a value set on the node)
        let json = value.to_json();
        if !json.is_null() {
            out.push(CurveKeys::deserialize(json).map_err(|e| format!("input 'keys' has the wrong type: {}", e))?);
        }
    }
    Ok(())
}
