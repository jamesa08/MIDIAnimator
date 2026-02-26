use std::collections::HashMap;

use crate::midi::MIDINote;

/// Node: keyframes_from_object
/// 
/// inputs: 
/// "object_groups": `Array<ObjectGroup>`,
/// "object_group_name": `String`,
/// "object_name": `String`
/// 
/// outputs:
/// "dyn_output": `Dyn<Array<Keyframe>>`
#[tauri::command]
#[node_registry::node]
pub fn keyframes_from_object(inputs: HashMap<String, serde_json::Value>) -> HashMap<String, serde_json::Value> {
    let mut outputs: HashMap<String, serde_json::Value> = HashMap::new();
    
    if !inputs.contains_key("object_groups") || !inputs.contains_key("object_group_name") || !inputs.contains_key("object_name") {
        outputs.insert("dyn_output".to_string(), serde_json::Value::Object(serde_json::Map::new()));
        return outputs;
    }

    let object_groups_unwrapped = inputs["object_groups"].as_array().expect("in keyframes_from_object, object_groups is not an array").clone();
    let object_group_name = inputs["object_group_name"].as_str().unwrap();
    let object_name = inputs["object_name"].as_str().unwrap();
    let xyz = ["x", "y", "z"];

    for object_group in object_groups_unwrapped {
        let object_group_unwrapped = object_group.as_object().unwrap();
        if object_group_unwrapped.get_key_value("name").unwrap().1.as_str().unwrap() == object_group_name {
            let objects = object_group_unwrapped.get("objects").unwrap().as_array().unwrap();
            for object in objects {
                let object_unwrapped = object.as_object().unwrap();
                if object_unwrapped.get_key_value("name").unwrap().1.as_str().unwrap() == object_name {
                    // get all the anim curves
                    let anim_curves_unwrapped = object_unwrapped.get("anim_curves").unwrap().as_array().unwrap();

                        // add the keyframe points to the outputs
                        for anim_curve in anim_curves_unwrapped {
                            let anim_curve_unwrapped = anim_curve.as_object().unwrap();
                            let data_path = anim_curve_unwrapped.get("data_path").unwrap().as_str().unwrap();
                            let array_index = anim_curve_unwrapped.get("array_index").unwrap().as_u64().unwrap();
                            let keyframe_points = anim_curve_unwrapped.get("keyframe_points").unwrap().as_array().unwrap();

                            if vec!("location", "rotation", "scale").contains(&data_path) {
                                // convert to x, y, z
                                let anim_curve_name = format!("{}_{}", data_path, xyz[array_index as usize]);
                                outputs.insert(anim_curve_name, serde_json::to_value(keyframe_points).unwrap());
                            } else {
                                let anim_curve_name = format!("{}_{}", data_path, array_index);
                                outputs.insert(anim_curve_name, serde_json::to_value(keyframe_points).unwrap());
                            }
                    }
                    // also add the anim curves to the dyn_output
                    outputs.insert("dyn_output".to_string(), serde_json::to_value(anim_curves_unwrapped).unwrap());
                    break;
                }
            }
            break;
        }
    }

    return outputs;
}


/// Node: animation_generator
/// these may change in the future
/// 
/// inputs: 
/// "name": `String`,
/// "note_on_keyframes": `Array<Keyframe>`,
/// "note_on_anchor_point": `f64`,
/// "note_off_keyframes": `Array<Keyframe>`,
/// "note_off_anchor_point": `f64`,
/// "time_mapper": `String`,
/// "amplitude_mapper": `String`,
/// "velocity_intensity": `f64`,
/// "animation_overlap": `String`,
/// "animation_property": `String`
/// 
/// outputs:
/// "generator": `AnimationGenerator`
#[tauri::command]
#[node_registry::node]
pub fn animation_generator(inputs: HashMap<String, serde_json::Value>) -> HashMap<String, serde_json::Value> {
    let mut outputs: HashMap<String, serde_json::Value> = HashMap::new();
    
    // if !inputs.contains_key("note_on_keyframes") || !inputs.contains_key("note_on_anchor_point") || !inputs.contains_key("note_off_keyframes") || !inputs.contains_key("note_off_anchor_point") || !inputs.contains_key("time_mapper") || !inputs.contains_key("amplitude_mapper") || !inputs.contains_key("velocity_intensity") || !inputs.contains_key("animation_overlap") || !inputs.contains_key("animation_property") {
    //     outputs.insert("generator".to_string(), serde_json::Value::Object(serde_json::Map::new()));
    //     return outputs;
    // }

    let empty_vec: Vec<serde_json::Value> = Vec::new();
    let name = inputs.get("name").and_then(|v| v.as_str()).unwrap_or_default();
    let note_on_keyframes = inputs.get("note_on_keyframes").and_then(|v| v.as_array()).unwrap_or(&empty_vec);
    let note_on_anchor_point = inputs.get("note_on_anchor_point").and_then(|v| v.as_f64()).unwrap_or_default();
    let note_off_keyframes = inputs.get("note_off_keyframes").and_then(|v| v.as_array()).unwrap_or(&empty_vec);
    let note_off_anchor_point = inputs.get("note_off_anchor_point").and_then(|v| v.as_f64()).unwrap_or_default();
    let time_mapper = inputs.get("time_mapper").and_then(|v| v.as_str()).unwrap_or_default();
    let amplitude_mapper = inputs.get("amplitude_mapper").and_then(|v| v.as_str()).unwrap_or_default();
    let velocity_intensity = inputs.get("velocity_intensity").and_then(|v| v.as_f64()).unwrap_or_default();
    let animation_overlap = inputs.get("animation_overlap").and_then(|v| v.as_str()).unwrap_or_default();
    let animation_property = inputs.get("animation_property").and_then(|v| v.as_str()).unwrap_or_default();

    let mut generator = HashMap::new();
    generator.insert("name".to_string(), serde_json::to_value(name).unwrap());
    generator.insert("note_on_keyframes".to_string(), serde_json::to_value(note_on_keyframes).unwrap());
    generator.insert("note_on_anchor_point".to_string(), serde_json::to_value(note_on_anchor_point).unwrap());
    generator.insert("note_off_keyframes".to_string(), serde_json::to_value(note_off_keyframes).unwrap());
    generator.insert("note_off_anchor_point".to_string(), serde_json::to_value(note_off_anchor_point).unwrap());
    generator.insert("time_mapper".to_string(), serde_json::to_value(time_mapper).unwrap());
    generator.insert("amplitude_mapper".to_string(), serde_json::to_value(amplitude_mapper).unwrap());
    generator.insert("velocity_intensity".to_string(), serde_json::to_value(velocity_intensity).unwrap());
    generator.insert("animation_overlap".to_string(), serde_json::to_value(animation_overlap).unwrap());
    generator.insert("animation_property".to_string(), serde_json::to_value(animation_property).unwrap());

    outputs.insert("generator".to_string(), serde_json::to_value(generator).unwrap());
    return outputs;
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
        if result.len() % 2 == 0 {
            // add below
            let mut new_num = *result.iter().min().unwrap() - 1;
            while result.contains(&new_num) {
                new_num -= 1;
            }
            result.push(new_num);
        } else {
            // add above
            let mut new_num = *result.iter().max().unwrap() + 1;
            while result.contains(&new_num) {
                new_num += 1;
            }
            result.push(new_num);
        }
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
#[tauri::command]
#[node_registry::node]
pub fn assign_notes_to_objects(inputs: HashMap<String, serde_json::Value>) -> HashMap<String, serde_json::Value> {
    let mut outputs: HashMap<String, serde_json::Value> = HashMap::new();
    
    let mut object_map: HashMap<String, serde_json::Value> = HashMap::new();
    object_map.insert("animations".to_string(), serde_json::json!({}));
    object_map.insert("objects".to_string(), serde_json::json!({}));
    
    // get all used notes from midi notes
    let midi_notes: Vec<MIDINote> = match inputs.get("midi_notes") {
        Some(value) => serde_json::from_value(value.clone()).unwrap_or_else(|err| {
            eprintln!("Failed to deserialize midi_notes: {}", err);
            Vec::new()
        }),
        None => Vec::new(),
    };
    let used_notes = all_used_notes_from_array(&midi_notes);

    // get the object groups
    let empty_vec = Vec::new();
    let object_groups_unwrapped = inputs.get("object_groups").and_then(|v| v.as_array()).unwrap_or(&empty_vec);
    let object_group_name = inputs.get("object_group_name").and_then(|v| v.as_str()).unwrap_or_default();
    let object_group = object_groups_unwrapped.iter().find(|object_group| {
        object_group.get("name").and_then(|v| v.as_str()).unwrap_or_default() == object_group_name
    }).unwrap_or(&serde_json::Value::Null);

    println!("object group name: {}", object_group_name);

    // get the generator
    let empty_map = serde_json::Map::new();
    let generator = inputs.get("generator").and_then(|v| v.as_object()).unwrap_or(&empty_map);
    
    // check for case 2 if the object count is the same as the note count
    let object_count = object_group.get("objects").and_then(|v| v.as_array()).unwrap_or(&Vec::new()).len();
    let note_count = used_notes.len();
    
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

    if object_count == note_count {
        // case 2: direct assignment from MIDI track
        println!("case 2: direct assignment from MIDI track");
        for (i, object) in object_group.get("objects").and_then(|v| v.as_array()).unwrap_or(&Vec::new()).iter().enumerate() {
            let object_name = object.get("name").and_then(|v| v.as_str()).unwrap_or_default();
            let note_number = used_notes[i];
            let object_map_lookup = object_map.get_mut("objects").unwrap();
            object_map_lookup.as_object_mut().unwrap().insert(object_name.to_string(), serde_json::json!(note_number));
        }
    } else {
        // case 3: flexible assignment with padding
        println!("case 3: flexible assignment with padding");
        let padded_notes = pad_nums(used_notes, object_count);
        for (obj, note_number) in object_group.get("objects").and_then(|v| v.as_array()).unwrap_or(&Vec::new()).iter().zip(padded_notes.iter()) {        
            let object_name = obj.get("name").and_then(|v| v.as_str()).unwrap_or_default();
            let object_map_lookup = object_map.get_mut("objects").unwrap();
            object_map_lookup.as_object_mut().unwrap().insert(object_name.to_string(), serde_json::json!(note_number));
        }        
    }
    let animation_generator_name = generator.get("name").and_then(|v| v.as_str()).unwrap_or_default();
    object_map.get_mut("animations").unwrap().as_object_mut().unwrap().insert(animation_generator_name.to_string(), serde_json::json!(generator.clone()));
    outputs.insert("object_map".to_string(), serde_json::to_value(object_map).unwrap());
    // println!("object map: {:?}", serde_json::to_string_pretty(&outputs).unwrap());
    return outputs;
    
}
