use std::collections::{HashMap, HashSet};
use std::io::Read;
use serde_json::Map;

use crate::state::{STATE, update_state};
use crate::command::javascript::evaluate_js;

#[tauri::command]
pub async fn execute_graph(handle: tauri::AppHandle, realtime: bool) {
    let now = std::time::Instant::now();

    let state = tokio::task::spawn_blocking(move || {
        let state = STATE.lock().unwrap();
        return state.clone();
    }).await.map_err(|e| e.to_string()).unwrap();

    if state.connected {
        println!("CONNECTED TO 3D SOFTWARE {}", state.connected_application);
    }

    // get current nodes & edges
    let rf_instance: HashMap<String, serde_json::Value> = state.rf_instance.clone();

    // get the default nodes
    let resource_path = handle.path_resolver().resolve_resource("src/configs/default_nodes.json").unwrap();
    let mut file = std::fs::File::open(resource_path).unwrap();
    let mut data = String::new();

    file.read_to_string(&mut data).unwrap();
    let default_nodes: HashMap<String, serde_json::Value> = serde_json::from_str(&data).unwrap();

    #[async_recursion::async_recursion]
    async fn execute_dfs(node_id: String, visited: &mut HashSet<String>, results: &mut HashMap<String, serde_json::Value>, inputs: &mut HashMap<String, serde_json::Value>, rf_instance: &HashMap<String, serde_json::Value>, default_nodes: &HashMap<String, serde_json::Value>, realtime: &bool) {
        // println!("EXECUTING NODE {:?}", node_id);

        if visited.contains(&node_id) {
            // println!("ALREADY EXECUTED {:?}", node_id);
            return;
        }
    
        let node = rf_instance["nodes"].as_array().unwrap().iter().find(|node| node["id"] == node_id).unwrap();
        let node_no_uuid = node_id.split("-").collect::<Vec<&str>>()[0];
        
        let default_node = default_nodes["nodes"].as_array().unwrap().iter().find(|node| node["id"] == node_no_uuid).unwrap();

        let node_data = node["data"].as_object().unwrap().clone();
        
        let incoming_edges = rf_instance["edges"].as_array().unwrap().iter().filter(|edge| edge["source"] == node_id);

        // println!("INCOMING EDGES: {:#?} FOR {:?}", incoming_edges, node_id);
        
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
                // println!("NOT EXECUTED YET, executing on {:?} while on {:?}", edge["target"], node_id);
                
                // return early  as we don't want to continue execution
                execute_dfs(edge["target"].to_string(), visited, results, inputs, rf_instance, default_nodes, realtime).await;
                return;
            }
        }

        // this comes from the front end (user input)
        if node_data.contains_key("inputs") {
            // println!("FOUND COMPUTED INPUT DATA {:?}", node_id);
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
            // println!("REALTIME MODE, NODE NOT REALTIME, skipping {:?}", node_id);
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
            // println!("{}", code);
            
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

    // find the root nodes
    let nodes: HashSet<String> = rf_instance["nodes"].as_array().unwrap().iter().map(|node| node["id"].as_str().unwrap().to_string()).collect();

    let mut target_nodes: HashSet<String> = HashSet::new();

    // find nodes with incoming edges
    for edge in rf_instance["edges"].as_array().unwrap() {
        target_nodes.insert(edge["source"].as_str().unwrap().to_string());
    }

    let root_nodes = nodes.difference(&target_nodes).map(|node| node.clone()).collect::<Vec<String>>();

    // println!("ROOT NODES: {:#?}", root_nodes);

    for node_id in root_nodes {
        execute_dfs(node_id, &mut visited, &mut results, &mut inputs, &rf_instance, &default_nodes, &realtime).await;
    }

    // println!("FINAL RESULTS: {:#?}", results);



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