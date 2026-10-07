use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::io::{Inputs, NodeResult, Outputs, Val};
use crate::midi::MIDINote;
use crate::scene_generics::{AnimCurve, Object, ObjectGroup};
use crate::utils::note_to_name;

use crate::utils::animation::{combine_curve_keys, note_curve_keys, AnimationGenerator, CurveKeys, NoteTarget, ObjectMap, ANIMATION_OVERLAPS, DEFAULT_OVERLAP_BLEND};

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
    let object_group = object_groups.iter().find(|g| g.name == object_group_name).ok_or_else(|| format!("object group '{}' does not exist", object_group_name))?;
    let object = object_group.objects.iter().find(|o| o.name == object_name).ok_or_else(|| format!("object '{}' does not exist in '{}'", object_name, object_group_name))?;

    /*
    example, with `location[2]` and a bone's `pose.bones["Arm"].location[0]` picked:
        {
            "dyn_output": {
                "location[2]": "Location Z",
                "pose.bones[\"Arm\"].location[0]": "Arm › Location X"
            },
            "location[2]": FCurveData,
            "pose.bones[\"Arm\"].location[0]": FCurveData,
            "available_channels": [{ "id": "location[2]", "group": "Object", "name": "Location Z" }, ...]
        }
    */

    // every animated channel on the object, for the dropdowns
    let available = object_channels(object);
    outputs.set("available_channels", serde_json::to_value(&available).unwrap_or_default());

    // one output per picked channel, flat and inside dyn_output (see nodes_and_backend.md)
    for id in channels {
        let channel = available.iter().find(|c| &c.id == id).ok_or_else(|| format!("'{}' has no keyframes on '{}'", id, object_name))?;
        let anim_curve = object.anim_curves.iter().find(|c| channel_id(c) == *id).unwrap();
        dyn_output.insert(id.clone(), serde_json::Value::String(channel.label()));
        outputs.set(id, anim_curve.clone());
    }

    outputs.set("dyn_output", dyn_output);
    Ok(outputs)
}

// MARK: - Channels

/// an animated property of an object, one F-curve: its output id, the group it's listed under and its name in the group
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Channel {
    pub id: String,
    pub group: String,
    pub name: String,
}

impl Channel {
    /// the group and name together, e.g. `Arm › Location X`. object channels are just their name
    pub fn label(&self) -> String {
        if self.group == OBJECT_GROUP {
            self.name.clone()
        } else {
            format!("{} › {}", self.group, self.name)
        }
    }
}

const OBJECT_GROUP: &str = "Object";
const SHAPE_KEYS_GROUP: &str = "Shape Keys";
const CUSTOM_PROPERTIES_GROUP: &str = "Custom Properties";

/// the id of an anim curve's channel, its Blender path with the index: `location[2]`, `pose.bones["Arm"].location[0]`
pub fn channel_id(anim_curve: &AnimCurve) -> String {
    format!("{}[{}]", anim_curve.data_path, anim_curve.array_index)
}

/// every animated channel of an object, in the order Blender has them
pub fn object_channels(object: &Object) -> Vec<Channel> {
    let all: Vec<(&str, u32)> = object.anim_curves.iter().map(|c| (c.data_path.as_str(), c.array_index)).collect();
    object.anim_curves.iter().map(|c| curve_channel(&c.data_path, c.array_index, &all)).collect()
}

/// the channel of the curve at a Blender path and index, `all` is every curve's path and index on the same object.
/// bones, shape keys and custom properties get groups of their own, the rest is the object's
pub fn curve_channel(path: &str, array_index: u32, all: &[(&str, u32)]) -> Channel {
    let id = format!("{}[{}]", path, array_index);
    let indexed = |name: String| with_index(name, path, array_index, all);
    let (group, property) = if let Some((bone, rest)) = path.strip_prefix("pose.bones").and_then(quoted) {
        (bone, rest.strip_prefix('.').unwrap_or(rest))
    } else if let Some((key, rest)) = path.strip_prefix("key_blocks").and_then(quoted) {
        // a shape key's value is the shape key itself
        let rest = rest.strip_prefix('.').unwrap_or(rest);
        return Channel {
            id,
            group: SHAPE_KEYS_GROUP.to_string(),
            name: if rest == "value" {
                key
            } else {
                format!("{} {}", key, words(rest))
            },
        };
    } else if let Some((property, "")) = quoted(path) {
        return Channel {
            id,
            group: CUSTOM_PROPERTIES_GROUP.to_string(),
            name: indexed(property),
        };
    } else {
        (OBJECT_GROUP.to_string(), path)
    };

    // a bone's custom property is `["name"]` after the bone
    let name = match quoted(property) {
        Some((custom, "")) => indexed(custom),
        _ => match axes(property).and_then(|axes| axes.get(array_index as usize)) {
            Some(axis) => format!("{} {}", words(property), axis),
            None => indexed(words(property)),
        },
    };
    Channel {
        id,
        group,
        name,
    }
}

/// the axis (`X`, `Y`, `Z`, `W`) the curve at a Blender path and index animates, `None` if it isn't a vector's
pub fn curve_axis(path: &str, array_index: u32) -> Option<&'static str> {
    let property = match path.strip_prefix("pose.bones").and_then(quoted) {
        Some((_, rest)) => rest.strip_prefix('.').unwrap_or(rest),
        None => path,
    };
    axes(property)?.get(array_index as usize).copied()
}

/// the axis names of vector properties, `None` for anything else
fn axes(property: &str) -> Option<&'static [&'static str]> {
    match property {
        "location" | "scale" | "rotation_euler" | "delta_location" | "delta_scale" | "delta_rotation_euler" => Some(&["X", "Y", "Z"]),
        "rotation_quaternion" | "delta_rotation_quaternion" | "rotation_axis_angle" => Some(&["W", "X", "Y", "Z"]),
        _ => None,
    }
}

/// `rotation_euler` -> `Rotation`, `delta_location` -> `Delta Location`
fn words(property: &str) -> String {
    let property = property.strip_suffix("_euler").unwrap_or(property);
    property.split('_').filter(|w| !w.is_empty()).map(capitalize).collect::<Vec<_>>().join(" ")
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map_or(String::new(), |first| first.to_uppercase().chain(chars).collect())
}

/// the name with the curve's index when the property has more than one curve, `Color 2`
fn with_index(name: String, path: &str, array_index: u32, all: &[(&str, u32)]) -> String {
    if all.iter().any(|&(p, i)| p == path && i != array_index) {
        format!("{} {}", name, array_index)
    } else {
        name
    }
}

/// splits `["name"].rest` into the unescaped name and `.rest`, `None` if the path doesn't start with a quoted key
fn quoted(path: &str) -> Option<(String, &str)> {
    let inner = path.strip_prefix("[\"")?;
    let mut name = String::new();
    let mut chars = inner.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' => name.push(chars.next()?.1),
            '"' => return inner[i + 1..].strip_prefix(']').map(|rest| (name, rest)),
            _ => name.push(c),
        }
    }
    None
}

/// Node: animation_generator
/// these may change in the future
///
/// inputs:
/// "name": `String`,
/// "note_on_keyframes": `FCurveData`,
/// "note_on_anchor_point": `f64`,
/// "note_off_keyframes": `FCurveData`,
/// "note_off_anchor_point": `f64`,
/// "time_mapper": `String`,
/// "amplitude_mapper": `String`,
/// "velocity_intensity": `f64`,
/// "animation_overlap": `String`,
/// "overlap_blend": `f64`,
/// "animation_property": `String`
///
/// outputs:
/// "generator": `AnimationGenerator`
#[node_registry::node]
pub fn animation_generator(name: Option<String>, note_on_keyframes: Option<&AnimCurve>, note_on_anchor_point: Option<f64>, note_off_keyframes: Option<&AnimCurve>, note_off_anchor_point: Option<f64>, time_mapper: Option<String>, amplitude_mapper: Option<String>, velocity_intensity: Option<f64>, animation_overlap: Option<String>, overlap_blend: Option<f64>, animation_property: Option<String>) -> NodeResult {
    // the keyframe curves are optional, an unconnected one is just no keyframes

    // inherit the property from the note on curve if none is given, e.g. "location[0]"
    let mut animation_property = animation_property.unwrap_or_default();
    if animation_property.is_empty() {
        let (data_path, array_index) = note_on_keyframes.map_or(("", 0), |c| (c.data_path.as_str(), c.array_index));
        animation_property = format!("{}[{}]", data_path, array_index);
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

    let generator = AnimationGenerator {
        name: name.unwrap_or_default(),
        note_on_keyframes: note_on_keyframes.map(|c| c.keyframe_points.clone()).unwrap_or_default(),
        note_on_anchor_point: note_on_anchor_point.unwrap_or_default(),
        note_off_keyframes: note_off_keyframes.map(|c| c.keyframe_points.clone()).unwrap_or_default(),
        note_off_anchor_point: note_off_anchor_point.unwrap_or_default(),
        time_mapper: time_mapper.unwrap_or_default(),
        amplitude_mapper: amplitude_mapper.unwrap_or_default(),
        velocity_intensity: velocity_intensity.unwrap_or_default(),
        animation_overlap,
        overlap_blend,
        animation_property,
    };

    let mut outputs = Outputs::new();
    outputs.set("generator", generator);
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
/// "generator": `AnimationGenerator`
///
/// outputs:
/// "object_map": `ObjectMap`

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
pub fn assign_notes_to_objects(object_groups: Option<&Vec<ObjectGroup>>, object_group_name: Option<&String>, midi_notes: Option<&Vec<MIDINote>>, note_numbers: Option<&Value>, generator: Option<&AnimationGenerator>) -> NodeResult {
    let midi_notes: &[MIDINote] = midi_notes.map(Vec::as_slice).unwrap_or(&[]);
    let note_entries = note_list_entries(note_numbers);
    let object_groups: &[ObjectGroup] = object_groups.map(Vec::as_slice).unwrap_or(&[]);
    let object_group_name = object_group_name.map(String::as_str).unwrap_or("");

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
        return Ok(outputs);
    }

    // find the object group, a name that isn't in the scene is an error
    let object_group = object_groups.iter().find(|g| g.name == object_group_name).ok_or_else(|| format!("object group '{}' does not exist", object_group_name))?;
    println!("object group name: {}", object_group_name);

    // every object gets the generator's animation, if one is connected
    let anim_name = generator.map(|g| g.name.clone());
    if let Some(generator) = generator {
        object_map.animations.insert(generator.name.clone(), generator.clone());
    }

    // objects pair with notes in name order (Cube_2 before Cube_10), not the order the scene lists them in
    let mut objects: Vec<&Object> = object_group.objects.iter().collect();
    objects.sort_by(|a, b| natural_cmp(&a.name, &b.name));

    // case 4 when a note list is given, it needs one note number per object
    let pairs: Vec<(&Object, u8)> = if !note_entries.is_empty() {
        println!("case 4: user-provided note list");
        let numbers: Option<Vec<u8>> = note_entries.iter().map(|e| note_number(e)).collect();
        match numbers {
            Some(numbers) if numbers.len() == objects.len() => objects.iter().copied().zip(numbers).collect(),
            _ => return Err(note_list_error(&objects, object_group_name, &note_entries)),
        }
    } else {
        // case 1 for objects with a note in their name (Cube_60, Cube_C3)
        let (named, unnamed): (Vec<_>, Vec<_>) = objects.iter().map(|object| (*object, note_in_name(&object.name))).partition(|(_, note)| note.is_some());
        let mut pairs: Vec<(&Object, u8)> = named.into_iter().filter_map(|(object, note)| Some((object, note?))).collect();
        if !pairs.is_empty() {
            println!("case 1: note numbers from object names");
        }

        // the other objects share the MIDI's notes that no named object plays
        let unnamed: Vec<&Object> = unnamed.into_iter().map(|(object, _)| object).collect();
        let used_notes: Vec<u8> = all_used_notes_from_array(midi_notes).into_iter().filter(|n| !pairs.iter().any(|(_, named)| named == n)).collect();

        // case 2 when the object count is the same as the note count, otherwise case 3
        let notes = if unnamed.len() == used_notes.len() {
            println!("case 2: direct assignment from MIDI track");
            used_notes
        } else {
            println!("case 3: flexible assignment with padding");
            pad_nums(used_notes, unnamed.len())
        };
        pairs.extend(unnamed.into_iter().zip(notes));
        pairs
    };

    // extra objects (not enough notes) are left out
    for (object, note_number) in pairs {
        let entry = object_map.objects.entry(object.name.clone()).or_default();
        if let Some(anim_name) = &anim_name {
            entry.entry(anim_name.clone()).or_default().push(note_number);
        }
    }

    outputs.set("object_map", object_map);
    Ok(outputs)
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
/// "note": `MIDINote`
///
/// outputs:
/// "keys": `CurveKeys`, null when the generator has no keyframes
#[node_registry::node]
pub fn note_keyframes(object_map: &ObjectMap, target: &NoteTarget, note: &MIDINote) -> NodeResult {
    let generator = object_map.generator(&target.animation, &target.object)?;
    let mut outputs = Outputs::new();
    outputs.set("keys", note_curve_keys(&target.object, generator, note));
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

/// collects `CurveKeys` out of nested lists in order, nulls (notes with no keys) are skipped
fn flatten_curve_keys(value: &Val, out: &mut Vec<CurveKeys>) -> Result<(), String> {
    if let Some(keys) = value.downcast_ref::<Option<CurveKeys>>() {
        out.extend(keys.clone());
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
