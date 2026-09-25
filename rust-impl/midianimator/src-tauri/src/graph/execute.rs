use serde_json::{json, Map};
use std::collections::{HashMap, HashSet};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::PoisonError;

use crate::graph::executors::io::{node_error, NodeFunction, ERROR_KEY};
use crate::graph::model::Graph;
use crate::node_registry::get_node_registry;
use crate::state::{update_state, STATE};

#[tauri::command]
pub async fn execute_graph(realtime: bool) {
    let now = std::time::Instant::now();

    // copy the state, a poisoned lock (a panic somewhere else) still has a usable graph
    let state = STATE.lock().unwrap_or_else(PoisonError::into_inner).clone();

    if state.connected {
        println!("CONNECTED TO 3D SOFTWARE {}", state.connected_application);
    }

    // get current nodes & edges, a graph that can't be read doesn't run at all
    // note: an empty rf_instance (nothing pushed from the frontend yet) parses as an empty graph
    let graph = match Graph::from_rf(&state.rf_instance) {
        Ok(graph) => graph,
        Err(e) => {
            eprintln!("ERROR: not executing, {}", e);
            return;
        }
    };

    // get default nodes from state:
    let default_nodes = state.default_nodes.clone();

    let node_registry = get_node_registry();

    #[async_recursion::async_recursion]
    async fn execute_dfs(node_id: String, visited: &mut HashSet<String>, results: &mut HashMap<String, serde_json::Value>, inputs: &mut HashMap<String, serde_json::Value>, graph: &Graph, default_nodes: &HashMap<String, serde_json::Value>, realtime: &bool, node_registry: &HashMap<String, NodeFunction>) {
        // println!("EXECUTING NODE {:?}", node_id);

        if visited.contains(&node_id) {
            // println!("ALREADY EXECUTED {:?}", node_id);
            return;
        }

        // find the node, an edge to a node that doesn't exist is skipped
        let Some(node) = graph.node(&node_id) else {
            eprintln!("ERROR: could not find node '{}', skipping it", node_id);
            return;
        };

        let node_no_uuid = node_id.split("-").collect::<Vec<&str>>()[0];

        // an unknown node type can't run, show it on the node
        let Some(default_node) = default_nodes.get("nodes").and_then(|nodes| nodes.as_array()).and_then(|nodes| nodes.iter().find(|node| node["id"] == node_no_uuid)) else {
            visited.insert(node_id.clone());
            results.insert(node_id.clone(), json!({ ERROR_KEY: format!("unknown node type '{}'", node_no_uuid) }));
            return;
        };

        // edges coming into this node, in data flow terms: from_node › from_output -> this node › to_input
        let incoming_edges = graph.edges.iter().filter(|edge| edge.to_node() == node_id);

        // println!("INCOMING EDGES: {:#?} FOR {:?}", incoming_edges, node_id);

        // add the node id to inputs
        inputs.insert(node_id.clone(), serde_json::Value::Object(Map::new()));

        // note: the stored edge fields are reversed (source is the consuming node), the accessors hide that
        for edge in incoming_edges {
            if let Some(node_results) = results.get(edge.from_node()).cloned() {
                // an upstream node failed, don't run this one. only the failed node gets the error
                if node_error(&node_results).is_some() {
                    visited.insert(node_id.clone());
                    return;
                }

                // add computed results to inputs
                // to_input: the "input" for the node
                // the value: the stored result of the upstream output
                inputs.get_mut(&node_id).and_then(|input_map| {
                    input_map.as_object_mut().unwrap().insert(edge.to_input().to_string(), node_results[edge.from_output()].clone());
                    Some(())
                });
            } else {
                // println!("NOT EXECUTED YET, executing on {:?} while on {:?}", edge.from_node(), node_id);

                // return early  as we don't want to continue execution
                execute_dfs(edge.from_node().to_string(), visited, results, inputs, graph, default_nodes, realtime, node_registry).await;
                return;
            }
        }

        // this comes from the front end (user input)
        // note: inputs that aren't an object (a broken save file) are ignored
        if let Some(node_inputs) = node.data.get("inputs").and_then(|v| v.as_object()) {
            // println!("FOUND COMPUTED INPUT DATA {:?}", node_id);
            for (handle_name, handle_value) in node_inputs {
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
        if (*realtime) && default_node["realtime"].as_bool().unwrap_or(false) == false {
            return;
        }

        // execute the node
        let mut exec_result: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();

        if default_node["executor"] == "rust" {
            // let node_name = default_node["id"].as_str().unwrap();
            let input_value = inputs[&node_id].as_object().unwrap_or(&serde_json::Map::new()).clone();

            if let Some(node_func) = node_registry.get(node_no_uuid) {
                let input_hashmap: HashMap<String, serde_json::Value> = input_value.into_iter().collect();

                // run it, a panic that slipped through is turned into an error too so it can't take the app down
                let outcome = catch_unwind(AssertUnwindSafe(|| node_func(input_hashmap.into()))).unwrap_or_else(|panic| Err(format!("crashed: {}", panic_message(&panic))));

                match outcome {
                    Ok(outputs) => exec_result.extend(outputs.into_map()),
                    Err(message) => {
                        // store the error on the node and stop here, the nodes after it don't run
                        eprintln!("ERROR: node '{}' failed: {}", node_id, message);
                        results.insert(node_id.clone(), json!({ ERROR_KEY: message }));
                        return;
                    }
                }
            } else {
                println!("ERROR: Node '{}' not found in registry", node_id);
            }
        } else {
            println!("ERROR: javascript execution not implemented");
        }

        // after successful execution, add to results
        results.insert(node_id.clone(), serde_json::Value::Object(exec_result.clone()));

        for edge in &graph.edges {
            if edge.from_node() == node_id {
                execute_dfs(edge.to_node().to_string(), visited, results, inputs, graph, default_nodes, realtime, node_registry).await;
            }
        }
    }

    // execute the graph starting from the root node
    let mut visited: HashSet<String> = HashSet::new();
    let mut results: HashMap<String, serde_json::Value> = HashMap::new();
    let mut inputs: HashMap<String, serde_json::Value> = HashMap::new();

    // find the root nodes
    let nodes: HashSet<String> = graph.nodes.iter().map(|node| node.id.clone()).collect();

    let mut target_nodes: HashSet<String> = HashSet::new();

    // find nodes with incoming edges
    for edge in &graph.edges {
        target_nodes.insert(edge.to_node().to_string());
    }

    let root_nodes = nodes.difference(&target_nodes).map(|node| node.clone()).collect::<Vec<String>>();

    // println!("ROOT NODES: {:#?}", root_nodes);

    for node_id in root_nodes {
        execute_dfs(node_id, &mut visited, &mut results, &mut inputs, &graph, &default_nodes, &realtime, &node_registry).await;
    }

    // println!("FINAL RESULTS: {:#?}", results);

    let elapsed = now.elapsed();
    println!("took {} ms to execute", elapsed.as_nanos() as f32 / 1_000_000.0);

    let mut state = STATE.lock().unwrap_or_else(PoisonError::into_inner);
    state.executed_results = results.clone();
    state.executed_inputs = inputs.clone();
    drop(state);
    update_state();

    // now we do something useful with the results

    // ....
}

/// the message of a caught panic, it can be a &str or a String
pub fn panic_message(panic: &Box<dyn std::any::Any + Send>) -> String {
    panic.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| panic.downcast_ref::<String>().cloned()).unwrap_or_else(|| "unknown panic".to_string())
}
