use std::collections::HashMap;

use crate::blender::scene_data::write_scene_data;
use crate::midi::MIDINote;
use crate::utils::animation::{AnimationGenerator, ObjectMap, BlendKeyframe, parse_animation_property, add_keyframes, co_from_json};

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
                    let anim_curves_unwrapped = object_unwrapped.get("anim_curves").unwrap().as_array().unwrap();

                    // Build the dyn_output map
                    let mut dyn_output_map = serde_json::Map::new();

                    /*
                    example:
                        {
                            "dyn_output": {
                                "location_x": FCurveData,
                                "location_y": FCurveData,
                                "location_z": FCurveData
                            }
                        }
                    */

                    for anim_curve in anim_curves_unwrapped {
                        let anim_curve_unwrapped = anim_curve.as_object().unwrap();
                        let data_path = anim_curve_unwrapped.get("data_path").unwrap().as_str().unwrap();
                        let array_index = anim_curve_unwrapped.get("array_index").unwrap().as_u64().unwrap();

                        let anim_curve_name = if vec!["location", "rotation", "scale"].contains(&data_path) {
                            format!("{}_{}", data_path, xyz[array_index as usize])
                        } else {
                            format!("{}_{}", data_path, array_index)
                        };

                        dyn_output_map.insert(anim_curve_name, anim_curve.clone());
                    }

                    outputs.insert("dyn_output".to_string(), serde_json::Value::Object(dyn_output_map));
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
    // generator.insert("animation_overlap".to_string(), serde_json::to_value(animation_overlap).unwrap());
    generator.insert("animation_overlap".to_string(), serde_json::to_value("add").unwrap());
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
    let animation_generator_name = generator.get("name").and_then(|v| v.as_str()).unwrap_or_default();

    if object_count == note_count {
        // case 2: direct assignment from MIDI track
        println!("case 2: direct assignment from MIDI track");
        for (i, obj) in object_group.get("objects").and_then(|v| v.as_array()).unwrap_or(&Vec::new()).iter().enumerate() {
            let object_name = obj.get("name").and_then(|v| v.as_str()).unwrap_or_default();
            let note_number = used_notes[i];
            let object_map_lookup = object_map.get_mut("objects").unwrap();
            let object_map_obj = object_map_lookup.as_object_mut().unwrap();

            if let Some(existing_entry) = object_map_obj.get_mut(object_name) {
                if let Some(note_array) = existing_entry.get_mut("note_number").and_then(|v| v.as_array_mut()) {
                    note_array.push(serde_json::json!(note_number));
                }
            } else {
                object_map_obj.insert(object_name.to_string(), serde_json::json!({
                    "note_number": [note_number],
                    "animations": [animation_generator_name]
                }));
            }
        }
    } else {
        // case 3: flexible assignment with padding
        println!("case 3: flexible assignment with padding");
        let padded_notes = pad_nums(used_notes, object_count);
        for (obj, note_number) in object_group.get("objects").and_then(|v| v.as_array()).unwrap_or(&Vec::new()).iter().zip(padded_notes.iter()) {        
            let object_name = obj.get("name").and_then(|v| v.as_str()).unwrap_or_default();
            let object_map_lookup = object_map.get_mut("objects").unwrap();
            let object_map_obj = object_map_lookup.as_object_mut().unwrap();

            if let Some(existing_entry) = object_map_obj.get_mut(object_name) {
                if let Some(note_array) = existing_entry.get_mut("note_number").and_then(|v| v.as_array_mut()) {
                    note_array.push(serde_json::json!(note_number));
                }
            } else {
                object_map_obj.insert(object_name.to_string(), serde_json::json!({
                    "note_number": [note_number],
                    "animations": [animation_generator_name]
                }));
            }
        }        
    }
    object_map.get_mut("animations").unwrap().as_object_mut().unwrap().insert(animation_generator_name.to_string(), serde_json::json!(generator.clone()));
    outputs.insert("object_map".to_string(), serde_json::to_value(object_map).unwrap());
    // println!("object map: {:?}", serde_json::to_string_pretty(&outputs).unwrap());
    return outputs;
    
}


/// Node: evaluate_instrument
/// 
/// inputs:
/// "object_map": `ObjectMap`,
/// "midi_notes": `Array<MIDINote>`,`
/// 
/// outputs:
/// None for now, this node will directly apply the animations to the objects in Blender, but in the future we may want to have it output some data that can be used by other nodes
#[tauri::command]
#[node_registry::node]
pub fn evaluate_instrument(inputs: HashMap<String, serde_json::Value>) -> HashMap<String, serde_json::Value> {
    let mut outputs: HashMap<String, serde_json::Value> = HashMap::new();

    let object_map: ObjectMap = serde_json::from_value(inputs["object_map"].clone())
        .expect("failed to parse object_map");

    let midi_notes: Vec<MIDINote> = serde_json::from_value(inputs["midi_notes"].clone())
        .expect("failed to parse midi_notes");


    let mut note_to_objects: HashMap<u8, Vec<(String, &AnimationGenerator)>> = HashMap::new();

    for (obj_name, entry) in &object_map.objects {
        for anim_name in &entry.animations {
            let Some(gen) = object_map.animations.get(anim_name) else {
                eprintln!("object '{}' references unknown animation '{}'", obj_name, anim_name);
                continue;
            };
            for &note_num in &entry.note_number {
                note_to_objects
                    .entry(note_num)
                    .or_default()
                    .push((obj_name.clone(), gen));
            }
        }
    }

    let mut obj_BlendKeyframes: HashMap<String, Vec<BlendKeyframe>> = object_map
        .objects
        .keys()
        .map(|name| (name.clone(), vec![]))
        .collect();


        for note in &midi_notes {
        let Some(targets) = note_to_objects.get(&note.note_number) else { continue; };

        for (obj_name, gen) in targets {
            // Parse data_path and array_index from animation_property e.g. "location[0]"
            let (data_path, array_index) = parse_animation_property(&gen.animation_property);
            
            // let data_path = "location"; // FIXME overwrite for now
            // let array_index = 2; // FIXME overwrite for now
            
            let mut next_keys: Vec<BlendKeyframe> = gen
                .note_on_keyframes
                .iter()
                .filter_map(|kf| {
                    let (frame, mut value) = co_from_json(kf)?;
                    let frame = frame + note.time_on + gen.note_on_anchor_point;
                    if gen.velocity_intensity != 0.0 {
                        value *= note.velocity as f64 / 127.0 * gen.velocity_intensity;
                    }
                    Some(BlendKeyframe::new(frame, value, &data_path, array_index))
                })
                .collect();

            let mut note_off_keys: Vec<BlendKeyframe> = gen
                .note_off_keyframes
                .iter()
                .filter_map(|kf| {
                    let (frame, mut value) = co_from_json(kf)?;
                    let frame = frame + note.time_off + gen.note_off_anchor_point;
                    if gen.velocity_intensity != 0.0 {
                        value *= note.velocity as f64 / 127.0 * gen.velocity_intensity;
                    }
                    Some(BlendKeyframe::new(frame, value, &data_path, array_index))
                })
                .collect();

            next_keys.append(&mut note_off_keys);
            next_keys.sort_by(|a, b| a.frame.partial_cmp(&b.frame).unwrap());

            if next_keys.is_empty() { continue; }

            let inserted = obj_BlendKeyframes.get_mut(obj_name).unwrap();
            match gen.animation_overlap.as_str() {
                "add" | "" => add_keyframes(inserted, &mut next_keys),
                other => eprintln!("unsupported animation_overlap '{}', skipping", other),
            }
        }
    }

    println!("writing BlendKeyframes to Blender...");
    let val = serde_json::to_value(obj_BlendKeyframes).expect("failed to serialize BlendKeyframes for writing to Blender");

    let val_for_blender = val.clone();
    tauri::async_runtime::spawn(async move {
        let res = write_scene_data(val_for_blender).await;
        
        if res.is_err() {
            eprintln!("failed to write BlendKeyframes to Blender: {}", res.err().unwrap());
        } else {
            println!("done writing BlendKeyframes to Blender");
        }
    });

    outputs.insert(
        "BlendKeyframes".to_string(),
        serde_json::to_value(val).expect("failed to serialize BlendKeyframes"),
    );

    outputs
}
