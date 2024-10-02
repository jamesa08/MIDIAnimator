use std::collections::HashMap;

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
pub fn animation_generator(inputs: HashMap<String, serde_json::Value>) -> HashMap<String, serde_json::Value> {
    let mut outputs: HashMap<String, serde_json::Value> = HashMap::new();
    
    // if !inputs.contains_key("note_on_keyframes") || !inputs.contains_key("note_on_anchor_point") || !inputs.contains_key("note_off_keyframes") || !inputs.contains_key("note_off_anchor_point") || !inputs.contains_key("time_mapper") || !inputs.contains_key("amplitude_mapper") || !inputs.contains_key("velocity_intensity") || !inputs.contains_key("animation_overlap") || !inputs.contains_key("animation_property") {
    //     outputs.insert("generator".to_string(), serde_json::Value::Object(serde_json::Map::new()));
    //     return outputs;
    // }

    let empty_vec: Vec<serde_json::Value> = Vec::new();
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