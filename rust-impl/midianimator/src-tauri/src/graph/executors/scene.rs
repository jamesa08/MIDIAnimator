use std::collections::HashMap;

use crate::state::STATE;

// Node: scene_link
///
/// inputs:
/// none
/// 
/// outputs:
/// "name": `String`
/// "object_groups": `Array<ObjectGroup>`
#[tauri::command]
pub fn scene_link(_inputs: HashMap<String, serde_json::Value>) -> HashMap<String, serde_json::Value> {
    let mut outputs: HashMap<String, serde_json::Value> = HashMap::new();
    
    let state = STATE.lock().unwrap();

    if !state.scene_data.contains_key("Scene") {
        println!("NO SCENE DATA");
        outputs.insert("name".to_string(), serde_json::Value::String("".to_string()));
        outputs.insert("object_groups".to_string(), serde_json::Value::Array(vec![]));
        return outputs;
    }
    
    let scene = state.scene_data["Scene"].clone();
    drop(state);

    outputs.insert("name".to_string(), serde_json::to_value(scene.name).unwrap());
    outputs.insert("object_groups".to_string(), serde_json::to_value(scene.object_groups).unwrap());
    return outputs;
}