use std::collections::HashSet;
use std::io::Read;
use std::{collections::HashMap, sync::Mutex};
use serde::{Deserialize, Serialize};
use serde_json::Map;
use std::sync::Arc;
use lazy_static::lazy_static;

use crate::api::javascript::evaluate_js;
use crate::scene_generics::Scene;



lazy_static! {
    pub static ref STATE: Mutex<AppState> = Mutex::new(AppState::default());
    pub static ref WINDOW: Arc<Mutex<Option<tauri::Window>>> = Arc::new(Mutex::new(None));
}

/// state struct for the application
/// 
/// this is a global state that is shared between the front end and the backend.
/// 
/// front end also has its own state, but this is the global state that is shared between the two.
/// 
/// note: the only way to update this state is through the backend, and the front end can only read from it.
///       if you wanted to change a variable, you will have to create a command in the backend that will update the state.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppState {
    pub ready: bool,
    pub connected: bool,
    pub connected_application: String,
    pub connected_version: String,
    pub connected_file_name: String,
    pub scene_data: HashMap<String, Scene>,
    pub rf_instance: HashMap<String, serde_json::Value>,
    pub executed_results: HashMap<String, serde_json::Value>,
    pub executed_inputs: HashMap<String, serde_json::Value>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            ready: false,
            connected: false,
            connected_application: "".to_string(),
            connected_version: "".to_string(),
            connected_file_name: "".to_string(),
            scene_data: HashMap::new(),
            rf_instance: HashMap::new(),
            executed_results: HashMap::new(),
            executed_inputs: HashMap::new(),
        }
    }
}

/// this commmand is called when the front end is loaded and ready to receive commands
#[tauri::command]
pub fn ready() -> AppState {
    println!("READY");
    let mut state = STATE.lock().unwrap();
    state.ready = true;
    return state.clone();
}

/// this command can be called from the front end to update changes to the state from the front end
/// you can use this by calling `window.tauri.invoke('js_update_state', {state: JSON.stringify(your_new_state_objet)})`
/// also use setBackendState() in the front end to update React's state with the new state
/// this function will replace the entire state, so make sure to include all the necessary data in the new state
/// note you cannot add new fields to the state, only update the existing fields
#[allow(unused_must_use)]
#[tauri::command]
pub fn js_update_state(state: String) {
    println!("FRONTEND STATE UPDATE");
    println!("{:#?}", state);
    let mut cur_state = STATE.lock().unwrap();

    // re-serealize the state
    let new_state: AppState = serde_json::from_str(&state).expect("Failed to deserialize state");

    // replace the current state with the new state
    std::mem::replace(&mut *cur_state, new_state);
    drop(cur_state);
}

#[tauri::command]
pub fn log(message: String) {
    println!("{}", message);
}


/// sends state to front end
/// emits via command "update_state"
pub fn update_state() {
    println!("BACKEND STATE UPDATE");
    loop {
        let state = STATE.lock().unwrap();
        
        if state.ready { // if the app is ready, we can send the state
            break;
        }
        drop(state);
    }
    let state = STATE.lock().unwrap();
    let window = WINDOW.lock().unwrap();
    window.as_ref().unwrap().emit("update_state", state.clone()).unwrap();
    drop(window);
    drop(state);
}





#[tauri::command]
pub async fn execute_graph(handle: tauri::AppHandle, realtime: bool) {    
    let now = std::time::Instant::now();

    // get current nodes & edges
    let rf_instance: HashMap<String, serde_json::Value> = tokio::task::spawn_blocking(move || {
        let state = STATE.lock().map_err(|e| e.to_string()).unwrap();
        return state.rf_instance.clone()
    }).await.map_err(|e| e.to_string()).unwrap();

    // get the default nodes
    let resource_path = handle.path_resolver().resolve_resource("src/configs/default_nodes.json").unwrap();
    let mut file = std::fs::File::open(resource_path).unwrap();
    let mut data = String::new();

    file.read_to_string(&mut data).unwrap();
    let default_nodes: HashMap<String, serde_json::Value> = serde_json::from_str(&data).unwrap();

    #[async_recursion::async_recursion]
    async fn execute_dfs(node_id: String, visited: &mut HashSet<String>, results: &mut HashMap<String, serde_json::Value>, inputs: &mut HashMap<String, serde_json::Value>, rf_instance: &HashMap<String, serde_json::Value>, default_nodes: &HashMap<String, serde_json::Value>, realtime: &bool) {
        println!("EXECUTING NODE {:?}", node_id);

        if visited.contains(&node_id) {
            println!("ALREADY EXECUTED {:?}", node_id);
            return;
        }
    
        let node = rf_instance["nodes"].as_array().unwrap().iter().find(|node| node["id"] == node_id).unwrap();
        let default_node = default_nodes["nodes"].as_array().unwrap().iter().find(|node| node["id"] == node_id).unwrap();

        let node_data = node["data"].as_object().unwrap().clone();
        
        let incoming_edges = rf_instance["edges"].as_array().unwrap().iter().filter(|edge| edge["source"] == node_id);

        println!("INCOMING EDGES: {:#?} FOR {:?}", incoming_edges, node_id);
        
        // add the node id to inputs
        inputs.insert(node_id.clone(), serde_json::Value::Object(Map::new()));

        // source and target are backwards in my brain for some reason
        // TARGET: left side of the node
        // SOURCE: right side of the node
        
        for edge in incoming_edges {
            if results.contains_key(edge["target"].as_str().unwrap()) {
                // get node_results via looking up the source node
                let node_results = results[edge["target"].as_str().unwrap()].clone();

                // add computed results to inputs
                // target handle: the "input" for the node
                // the value: the stored result
                // result["inputs"][edge["sourceHandle"].as_str().unwrap()] = node_results[edge["targetHandle"].as_str().unwrap()].clone();
                inputs.get_mut(&node_id).and_then(|input_map| {
                    input_map.as_object_mut().unwrap().insert(edge["sourceHandle"].as_str().unwrap().to_string(), node_results[edge["targetHandle"].as_str().unwrap()].clone());
                    Some(())
                });
            } else {
                println!("NOT EXECUTED YET, executing on {:?} while on {:?}", edge["target"], node_id);
                
                // return early  as we don't want to continue execution
                execute_dfs(edge["target"].to_string(), visited, results, inputs, rf_instance, default_nodes, realtime).await;
                return;
            }
        }

        // this comes from the front end (user input)
        if node_data.contains_key("inputs") {
            println!("FOUND COMPUTED INPUT DATA {:?}", node_id);
            for (handle_name, handle_value) in node_data["inputs"].as_object().unwrap() {
                // we don't want to insert if a handle is connected to a socket, whether its computed or not (the handle should be hidden if you want to use computed data)
                if !inputs[&node_id].as_object().unwrap().contains_key(handle_name) {
                    inputs.get_mut(&node_id).and_then(|input_map| {
                        input_map.as_object_mut().unwrap().insert(handle_name.clone(), handle_value.clone());
                        Some(())
                    });
                }
            }
        }

        // add the node to visited after we have computed the inputs 
        visited.insert(node_id.clone());

        // before executing the node, check if the node is realtime
        if (*realtime) && node["realtime"] == false {
            println!("REALTIME MODE, NODE NOT REALTIME, skipping {:?}", node_id);
            return;
        }

        // execute the node
        let mut exec_result: serde_json::Map<String, serde_json::Value> = serde_json::Map::new(); 
        
        if default_node["executor"] == "rust" {
            let code = format!(r#"
function execute() {{
    return window.__TAURI__.invoke({0}, {{
        inputs: {1}
    }}).then((result) => {{
        console.log(result); 
        return result;
    }});
}}"#, default_node["id"], inputs[&node_id].to_string());
            println!("{}", code);
            
            let executor_result = evaluate_js(code.to_string()).await;
            let wrapped_map: serde_json::Value = serde_json::from_str(&executor_result).unwrap();
            if let serde_json::Value::Object(actual_result) = &wrapped_map["result"] {
                exec_result.extend(actual_result.clone());
            } else {
                println!("ERROR while deserializing:, expected result to be an object but was not :(");
            }
        }

        // after successful execution, add to results
        results.insert(node_id.clone(), serde_json::Value::Object(exec_result.clone()));

        for edge in rf_instance["edges"].as_array().unwrap() {
            if edge["target"] == node_id {
                execute_dfs(edge["source"].as_str().unwrap().to_string(), visited, results, inputs, rf_instance, default_nodes, realtime).await;
            }
        }
    }

    // execute the graph starting from the root node
    let mut visited: HashSet<String> = HashSet::new();
    let mut results: HashMap<String, serde_json::Value> = HashMap::new();
    let mut inputs: HashMap<String, serde_json::Value> = HashMap::new();
    execute_dfs("get_midi_file".to_string(), &mut visited, &mut results, &mut inputs, &rf_instance, &default_nodes, &realtime).await;
    
    println!("FINAL RESULTS: {:#?}", results);



    let elapsed = now.elapsed();
    println!("took {} ms to execute", elapsed.as_nanos() as f32/1_000_000.0);

    let mut state = STATE.lock().unwrap();
    state.executed_results = results.clone();
    state.executed_inputs = inputs.clone();
    drop(state);
    update_state();
    
    // now we do something useful with the results

    // ....
}