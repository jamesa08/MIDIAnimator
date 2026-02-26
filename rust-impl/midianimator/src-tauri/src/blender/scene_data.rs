use crate::{graph::execute::execute_graph, scene_generics};
use crate::scene_generics::Scene;
use crate::ipc;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use serde_json::{self, Value};
use crate::state::{STATE, update_state};

static SCENE_BUILDER_PY: &str = include_str!("../blender/python/blender_scene_builder.py");
static SCENE_SENDER_PY: &str = include_str!("../blender/python/blender_scene_sender.py");

#[derive(Debug, Serialize, Deserialize)]
pub struct SceneUpdate {
    pub r#type: String,  // Use 'r#type' to handle 'type' keyword in Rust 
    pub change_type: Option<String>,
    pub changed_data: Option<HashMap<String, scene_generics::Scene>>,
    pub scene_data: HashMap<String, scene_generics::Scene>,
}


pub async fn get_scene_data() -> HashMap<String, scene_generics::Scene> {
    let scene_data = match ipc::send_message(SCENE_BUILDER_PY.to_string()).await {
        Some(data) => data,
        None => {
            panic!("Error building scene data");
        }
    };

    let mut scenes: HashMap<String, scene_generics::Scene> = HashMap::new();
    let json_data: Value = serde_json::from_str(&scene_data).unwrap_or(Value::Null);

    if let Some(scene_map) = json_data.as_object() {
        for (scene_name, scene_data) in scene_map {
            if let Some(object_groups_map) = scene_data["object_group"].as_object() {
                let object_groups: Vec<scene_generics::ObjectGroup> = object_groups_map.iter()
                    .map(|(group_name, object_group_data)| {
                        let objects: Vec<scene_generics::Object> = object_group_data["objects"]
                            .as_array()
                            .unwrap_or(&Vec::new())
                            .iter()
                            .filter_map(|object_data| {
                                let name = object_data["name"].as_str()?.to_string();
                                let position = object_data["location"].as_array()?;
                                let rotation = object_data["rotation"].as_array()?;
                                let scale = object_data["scale"].as_array()?;
                                let blend_shapes = object_data["blend_shapes"].as_object()?;
                                let anim_curves = object_data["anim_curves"].as_array()?;

                                if position.len() == 3 && rotation.len() == 3 && scale.len() == 3 {
                                    Some(scene_generics::Object {
                                        name,
                                        position: scene_generics::Vector3 {
                                            x: position[0].as_f64()? as f32,
                                            y: position[1].as_f64()? as f32,
                                            z: position[2].as_f64()? as f32,
                                        },
                                        rotation: scene_generics::Vector3 {
                                            x: rotation[0].as_f64()? as f32,
                                            y: rotation[1].as_f64()? as f32,
                                            z: rotation[2].as_f64()? as f32,
                                        },
                                        scale: scene_generics::Vector3 {
                                            x: scale[0].as_f64()? as f32,
                                            y: scale[1].as_f64()? as f32,
                                            z: scale[2].as_f64()? as f32,
                                        },
                                        blend_shapes: scene_generics::BlendShapes {
                                            keys: blend_shapes["keys"].as_array()?.iter().filter_map(|key| key.as_str().map(|s| s.to_string())).collect(),
                                            reference: blend_shapes["reference"].as_str().map(|s| s.to_string()),
                                        },
                                        anim_curves: anim_curves.iter().filter_map(|curve| {
                                            let array_index = curve["array_index"].as_u64()? as u32;
                                            let auto_smoothing = curve["auto_smoothing"].as_str()?.to_string();
                                            let data_path = curve["data_path"].as_str()?.to_string();
                                            let extrapolation = curve["extrapolation"].as_str()?.to_string();
                                            let range = curve["range"].as_array()?.iter().filter_map(|r| r.as_f64().map(|v| v as f32)).collect::<Vec<_>>();
                                            let keyframe_points = curve["keyframe_points"].as_array()?.iter().filter_map(|kf| {
                                                Some(scene_generics::KeyframePoint {
                                                    amplitude: kf["amplitude"].as_f64()? as f32,
                                                    back: kf["back"].as_f64()? as f32,
                                                    easing: kf["easing"].as_str()?.to_string(),
                                                    handle_left: vec! {
                                                        kf["handle_left"][0].as_f64()? as f32,
                                                        kf["handle_left"][1].as_f64()? as f32,
                                                    },
                                                    handle_left_type: kf["handle_left_type"].as_str()?.to_string(),
                                                    handle_right: vec! {
                                                        kf["handle_right"][0].as_f64()? as f32,
                                                        kf["handle_right"][1].as_f64()? as f32,
                                                    },
                                                    handle_right_type: kf["handle_right_type"].as_str()?.to_string(),
                                                    interpolation: kf["interpolation"].as_str()?.to_string(),
                                                    co: vec! {
                                                        kf["co"][0].as_f64()? as f32,
                                                        kf["co"][1].as_f64()? as f32,
                                                    },
                                                    period: kf["period"].as_f64()? as f32,
                                                })
                                            }).collect();

                                            Some(scene_generics::AnimCurve {
                                                array_index,
                                                auto_smoothing,
                                                data_path,
                                                extrapolation,
                                                keyframe_points,
                                                range,
                                            })
                                        }).collect(),
                                    })
                                } else {
                                    None
                                }
                            })
                            .collect();
                        scene_generics::ObjectGroup {
                            name: group_name.to_string(),
                            objects,
                        }
                    })
                    .collect();

                scenes.insert(
                    scene_name.to_string(),
                    scene_generics::Scene {
                        name: scene_name.to_string(),
                        object_groups,
                    },
                );
            }
        }
    } else {
        panic!("Invalid scene data format");
    }

    return scenes
}


pub async fn send_scene_data(scenes: HashMap<String, scene_generics::Scene>) -> std::io::Result<()> {
    let json_string = serde_json::to_string(&scenes).unwrap();
    
    let injected_script = SCENE_SENDER_PY.replace("JSON_DATA = r\"\"\"\"\"\"", &format!("JSON_DATA = r\"\"\"{}\"\"\"", json_string));

    let result = match ipc::send_message(injected_script.to_string()).await {
        Some(data) => data,
        None => {
            panic!("Error sending scene data");
        }
    };
    
    println!("{:?}", result);

    if result != "OK" {
        panic!("Error from Python file: {:?}", result);
    }
    Ok(())
}


pub fn process_scene_update(json_data: &str) {
    match serde_json::from_str::<SceneUpdate>(json_data) {
        Ok(update) => {
            if update.r#type == "scene_update" {
                println!("Received scene update: {:?}", update.change_type);
                
                let mut state = STATE.lock().unwrap();
                state.scene_data = update.scene_data;
                drop(state);
                update_state();
            }
        }
        Err(e) => {
            println!("Error processing scene update: {}", e);
        }
    }
}

pub async fn reconnect_with_validation() -> Result<SceneDiff, String> {
    let state = STATE.lock().unwrap();
    let saved_scene_data = state.scene_data.clone();
    drop(state);
    
    // Fetch fresh scene data from Blender
    let fresh_scene_data = get_scene_data().await;
    
    // Diff the data
    let diff = compare_scene_data(&saved_scene_data, &fresh_scene_data);
    
    if diff.has_changes() {
        // Return diff to frontend, let user decide
        Ok(diff)
    } else {
        // No changes, just update and connect
        let mut state = STATE.lock().unwrap();
        state.scene_data = fresh_scene_data;
        state.connected = true;
        drop(state);
        update_state();
        Ok(SceneDiff::empty())
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SceneDiff {
    pub missing_objects: Vec<String>,      // Objects in saved but not in fresh
    pub new_objects: Vec<String>,           // Objects in fresh but not in saved
    pub missing_collections: Vec<String>,
    pub new_collections: Vec<String>,
}

impl SceneDiff {
    pub fn has_changes(&self) -> bool {
        !self.missing_objects.is_empty() || 
        !self.new_objects.is_empty() ||
        !self.missing_collections.is_empty() ||
        !self.new_collections.is_empty()
    }
    
    pub fn empty() -> Self {
        Self {
            missing_objects: vec![],
            new_objects: vec![],
            missing_collections: vec![],
            new_collections: vec![],
        }
    }
}

pub fn compare_scene_data(saved: &HashMap<String, Scene>, fresh: &HashMap<String, Scene>) -> SceneDiff {
    let mut diff = SceneDiff::empty();
    
    // Compare collections and objects
    for (scene_name, saved_scene) in saved {
        if let Some(fresh_scene) = fresh.get(scene_name) {
            let saved_collections: std::collections::HashSet<_> = 
                saved_scene.object_groups.iter().map(|g| &g.name).collect();
            let fresh_collections: std::collections::HashSet<_> = 
                fresh_scene.object_groups.iter().map(|g| &g.name).collect();
            
            // Missing collections
            for coll in saved_collections.difference(&fresh_collections) {
                diff.missing_collections.push(format!("{}/{}", scene_name, coll));
            }
            
            // New collections
            for coll in fresh_collections.difference(&saved_collections) {
                diff.new_collections.push(format!("{}/{}", scene_name, coll));
            }
            
            // Compare objects within matching collections
            for saved_group in &saved_scene.object_groups {
                if let Some(fresh_group) = fresh_scene.object_groups.iter()
                    .find(|g| g.name == saved_group.name) {
                    
                    let saved_objects: std::collections::HashSet<_> = 
                        saved_group.objects.iter().map(|o| &o.name).collect();
                    let fresh_objects: std::collections::HashSet<_> = 
                        fresh_group.objects.iter().map(|o| &o.name).collect();
                    
                    for obj in saved_objects.difference(&fresh_objects) {
                        diff.missing_objects.push(
                            format!("{}/{}/{}", scene_name, saved_group.name, obj)
                        );
                    }
                    
                    for obj in fresh_objects.difference(&saved_objects) {
                        diff.new_objects.push(
                            format!("{}/{}/{}", scene_name, fresh_group.name, obj)
                        );
                    }
                }
            }
        }
    }
    
    diff
}

#[tauri::command]
pub async fn check_scene_changes() -> Result<SceneDiff, String> {
    let (saved_scene_data, pending_scene_data) = {
        let mut state = STATE.lock().unwrap();
        
        if !state.execution_paused {
            drop(state);
            return Err("No validation pending".to_string());
        }
        
        let pending = match &state.pending_scene_data {
            Some(data) => data.clone(),
            None => {
                state.execution_paused = false;
                drop(state);
                update_state();
                tauri::async_runtime::spawn(async move {
                    execute_graph(true).await;
                });
                return Err("No pending scene data".to_string());
            }
        };
        
        (state.scene_data.clone(), pending)
    };
    
    let diff = compare_scene_data(&saved_scene_data, &pending_scene_data);
    Ok(diff)
}

#[tauri::command]
pub async fn accept_scene_changes() -> Result<(), String> {
    let mut state = STATE.lock().unwrap();
    
    let fresh_scene_data = match state.pending_scene_data.take() {
        Some(data) => data,
        None => {
            state.execution_paused = false;
            drop(state);
            update_state();
            tauri::async_runtime::spawn(async move {
                    execute_graph(true).await;
            });
            return Err("No pending scene data".to_string());
        }
    };
    
    state.scene_data = fresh_scene_data;
    state.execution_paused = false;
    drop(state);
    
    update_state();
    Ok(())
}

#[tauri::command]
pub fn reject_scene_changes() -> Result<(), String> {
    let mut state = STATE.lock().unwrap();
    state.pending_scene_data = None;
    // Stay paused
    drop(state);
    
    update_state();
    Ok(())
}
