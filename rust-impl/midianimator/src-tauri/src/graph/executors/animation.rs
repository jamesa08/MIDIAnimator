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