use std::collections::HashMap;
/// Node: viewer
/// 
/// inputs:
/// "data": `Any`
/// 
/// outputs: 
/// None
#[tauri::command]
#[node_registry::node]
pub fn viewer(_inputs: HashMap<String, serde_json::Value>) {
    // :) 
    return HashMap::new();
}