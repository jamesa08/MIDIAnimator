use std::collections::{BTreeMap, HashMap};

use super::io::{Inputs, NodeResult, Outputs};
use crate::midi::MIDINote;
use crate::scene_generics::{AnimCurve, KeyframePoint, ObjectGroup};
use crate::utils::animation::{add_keyframes, co_of, parse_animation_property, AnimationGenerator, BlendKeyframe, ObjectMap, ObjectMapEntry};

/// Node: keyframes_from_object
///
/// inputs:
/// "object_groups": `Array<ObjectGroup>`,
/// "object_group_name": `String`,
/// "object_name": `String`
///
/// outputs:
/// "dyn_output": `Dyn<Array<Keyframe>>`
#[node_registry::node]
pub fn keyframes_from_object(inputs: Inputs) -> NodeResult {
    let mut outputs = Outputs::new();
    let object_groups: Vec<ObjectGroup> = inputs.or_default("object_groups")?;
    let object_group_name: String = inputs.or_default("object_group_name")?;
    let object_name: String = inputs.or_default("object_name")?;

    // nothing picked yet, no dynamic outputs
    let mut dyn_output = serde_json::Map::new();
    if object_groups.is_empty() || object_group_name.is_empty() || object_name.is_empty() {
        outputs.set("dyn_output", &dyn_output)?;
        return Ok(outputs);
    }

    // find the object, a name that isn't in the scene is an error
    let object_group = object_groups.iter().find(|g| g.name == object_group_name).ok_or_else(|| format!("object group '{}' does not exist", object_group_name))?;
    let object = object_group.objects.iter().find(|o| o.name == object_name).ok_or_else(|| format!("object '{}' does not exist in '{}'", object_name, object_group_name))?;

    /*
    example:
        {
            "dyn_output": {
                "location_x": FCurveData,
                "location_y": FCurveData,
                "location_z": FCurveData
            }
            "location_x": FCurveData
            "location_y": FCurveData,
            "location_z": FCurveData

        }
    */

    // one output per anim curve, flat and inside dyn_output (see nodes_and_backend.md)
    for anim_curve in &object.anim_curves {
        let name = anim_curve_name(anim_curve);
        outputs.set(&name, anim_curve)?;
        dyn_output.insert(name.clone(), outputs[&name].clone());
    }

    outputs.set("dyn_output", &dyn_output)?;
    Ok(outputs)
}

/// the output id for an anim curve, `location_x` for vectors, `data_path_0` for anything else
fn anim_curve_name(anim_curve: &AnimCurve) -> String {
    let xyz = ["x", "y", "z"];
    match xyz.get(anim_curve.array_index as usize) {
        Some(axis) if ["location", "rotation", "scale"].contains(&anim_curve.data_path.as_str()) => format!("{}_{}", anim_curve.data_path, axis),
        _ => format!("{}_{}", anim_curve.data_path, anim_curve.array_index),
    }
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
/// "animation_property": `String`
///
/// outputs:
/// "generator": `AnimationGenerator`
#[node_registry::node]
pub fn animation_generator(inputs: Inputs) -> NodeResult {
    // the keyframe curves are optional, an unconnected one is just no keyframes
    let note_on_curve: Option<AnimCurve> = inputs.opt("note_on_keyframes")?;
    let note_off_curve: Option<AnimCurve> = inputs.opt("note_off_keyframes")?;

    // inherit the property from the note on curve if none is given, e.g. "location[0]"
    let mut animation_property: String = inputs.or_default("animation_property")?;
    if animation_property.is_empty() {
        let (data_path, array_index) = note_on_curve.as_ref().map_or(("", 0), |c| (c.data_path.as_str(), c.array_index));
        animation_property = format!("{}[{}]", data_path, array_index);
    }

    let generator = AnimationGenerator {
        name: inputs.or_default("name")?,
        note_on_keyframes: note_on_curve.map(|c| c.keyframe_points).unwrap_or_default(),
        note_on_anchor_point: inputs.or_default("note_on_anchor_point")?,
        note_off_keyframes: note_off_curve.map(|c| c.keyframe_points).unwrap_or_default(),
        note_off_anchor_point: inputs.or_default("note_off_anchor_point")?,
        time_mapper: inputs.or_default("time_mapper")?,
        amplitude_mapper: inputs.or_default("amplitude_mapper")?,
        velocity_intensity: inputs.or_default("velocity_intensity")?,
        // FIXME: only "add" is supported until the overlap modes are ported
        animation_overlap: "add".to_string(),
        animation_property,
    };

    let mut outputs = Outputs::new();
    outputs.set("generator", &generator)?;
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
            let step = gap as f32 / (to_add as f32 + 1.0);
            for j in 1..=to_add {
                let padded_num = (current as f32 + j as f32 * step).round() as u8;
                if !result.contains(&padded_num) {
                    result.push(padded_num);
                }
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

/// Node: assign_notes_to_objects
/// read assign_midi_notes_to_objects.md for more information
///
/// inputs:
/// "object_groups": `Array<ObjectGroup>`,
/// "object_group_name": `String`,
/// "midi_notes": `Array<MIDINote>`,
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

**Note:** This input is hidden by default and needs to be enabled in the node properties.

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
pub fn assign_notes_to_objects(inputs: Inputs) -> NodeResult {
    let midi_notes: Vec<MIDINote> = inputs.or_default("midi_notes")?;
    let object_groups: Vec<ObjectGroup> = inputs.or_default("object_groups")?;
    let object_group_name: String = inputs.or_default("object_group_name")?;
    let generator: Option<AnimationGenerator> = inputs.opt("generator")?;

    /*  ObjectMap example:
       {
       "animations": {
           "ANIM_test": AnimationGenerator
       },
       "objects": {
           "object1": {
               note_number: 45, 46,
               animations: "ANIM_test"
           },
           ...
           }
       }
    */
    let mut object_map = ObjectMap::default();
    let mut outputs = Outputs::new();

    // nothing picked yet, empty object map
    if object_groups.is_empty() || object_group_name.is_empty() {
        outputs.set("object_map", &object_map)?;
        return Ok(outputs);
    }

    // find the object group, a name that isn't in the scene is an error
    let object_group = object_groups.iter().find(|g| g.name == object_group_name).ok_or_else(|| format!("object group '{}' does not exist", object_group_name))?;
    println!("object group name: {}", object_group_name);

    // every object gets the generator's animation, if one is connected
    let animations: Vec<String> = generator.iter().map(|g| g.name.clone()).collect();
    if let Some(generator) = generator {
        object_map.animations.insert(generator.name.clone(), generator);
    }

    // get all used notes from midi notes
    let used_notes = all_used_notes_from_array(&midi_notes);

    // case 2 when the object count is the same as the note count, otherwise case 3
    let notes = if object_group.objects.len() == used_notes.len() {
        println!("case 2: direct assignment from MIDI track");
        used_notes
    } else {
        println!("case 3: flexible assignment with padding");
        pad_nums(used_notes, object_group.objects.len())
    };

    // pair objects with notes in order, extra objects (not enough notes) are left out
    for (object, note_number) in object_group.objects.iter().zip(notes) {
        let entry = object_map.objects.entry(object.name.clone()).or_insert_with(|| ObjectMapEntry {
            note_number: vec![],
            animations: animations.clone(),
        });
        entry.note_number.push(note_number);
    }

    outputs.set("object_map", &object_map)?;
    Ok(outputs)
}

/// Node: evaluate_instrument
///
/// inputs:
/// "object_map": `ObjectMap`,
/// "midi_notes": `Array<MIDINote>`,`
///
/// outputs:
/// "keyframes": `HashMap<String, Array<BlendKeyframe>>`, keyframes per object, written to Blender by scene_writer
#[node_registry::node]
pub fn evaluate_instrument(inputs: Inputs) -> NodeResult {
    let object_map: ObjectMap = inputs.get("object_map")?;
    let midi_notes: Vec<MIDINote> = inputs.get("midi_notes")?;

    // look up which objects (and with which animation) each note triggers
    let mut note_to_objects: HashMap<u8, Vec<(String, &AnimationGenerator)>> = HashMap::new();
    for (obj_name, entry) in &object_map.objects {
        for anim_name in &entry.animations {
            let gen = object_map.animations.get(anim_name).ok_or_else(|| format!("object '{}' uses animation '{}', which isn't in the object map", obj_name, anim_name))?;
            for &note_num in &entry.note_number {
                note_to_objects.entry(note_num).or_default().push((obj_name.clone(), gen));
            }
        }
    }

    // keyframes per object, then per curve. overlap only combines keys on the same curve
    let mut obj_curves: HashMap<String, BTreeMap<(String, u32), Vec<BlendKeyframe>>> = object_map.objects.keys().map(|name| (name.clone(), BTreeMap::new())).collect();

    for note in &midi_notes {
        let Some(targets) = note_to_objects.get(&note.note_number) else {
            continue;
        };

        for (obj_name, gen) in targets {
            // Parse data_path and array_index from animation_property e.g. "location[0]"
            let (data_path, array_index) = parse_animation_property(&gen.animation_property);

            // use seconds instead of frames, Blender converts to frames with the scene's frame rate. this keeps timing based on the music rather than frame numbers
            let mut next_keys: Vec<BlendKeyframe> = note_keyframes(&gen.note_on_keyframes, note.time_on + gen.note_on_anchor_point, note.velocity, gen.velocity_intensity, &data_path, array_index);
            let mut note_off_keys: Vec<BlendKeyframe> = note_keyframes(&gen.note_off_keyframes, note.time_off + gen.note_off_anchor_point, note.velocity, gen.velocity_intensity, &data_path, array_index);

            next_keys.append(&mut note_off_keys);
            next_keys.sort_by(|a, b| a.time.total_cmp(&b.time));

            if next_keys.is_empty() {
                continue;
            }

            // combine with the keyframes already on this curve
            let inserted = obj_curves.entry(obj_name.clone()).or_default().entry((data_path, array_index)).or_default();
            match gen.animation_overlap.as_str() {
                "add" | "" => add_keyframes(inserted, &mut next_keys),
                other => return Err(format!("animation overlap '{}' is not supported yet", other)),
            }
        }
    }

    // flatten back to one list per object, objects with no keys stay so the writer still clears them
    let obj_blend_keyframes: HashMap<String, Vec<BlendKeyframe>> = obj_curves.into_iter().map(|(name, curves)| (name, curves.into_values().flatten().collect())).collect();

    let mut outputs = Outputs::new();
    outputs.set("keyframes", &obj_blend_keyframes)?;
    Ok(outputs)
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
