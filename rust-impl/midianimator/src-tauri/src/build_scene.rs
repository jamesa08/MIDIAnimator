use crate::scene_generics;
use crate::structures::ipc;
use serde_json::{self, Value};
use std::collections::HashMap;

static SCENE_BUILDER_PY: &str = include_str!("blender_python/blender_scene_builder.py");
static SCENE_SENDER_PY: &str = include_str!("blender_python/blender_scene_sender.py");

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
                let object_groups: Vec<scene_generics::ObjectGroup> = object_groups_map
                    .iter()
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
                                            keys: blend_shapes["keys"]
                                                .as_array()?
                                                .iter()
                                                .filter_map(|key| {
                                                    key.as_str().map(|s| s.to_string())
                                                })
                                                .collect(),
                                            reference: blend_shapes["reference"]
                                                .as_str()
                                                .map(|s| s.to_string()),
                                        },
                                        anim_curves: anim_curves
                                            .iter()
                                            .filter_map(|curve| {
                                                let array_index =
                                                    curve["array_index"].as_u64()? as u32;
                                                let auto_smoothing =
                                                    curve["auto_smoothing"].as_str()?.to_string();
                                                let data_path =
                                                    curve["data_path"].as_str()?.to_string();
                                                let extrapolation =
                                                    curve["extrapolation"].as_str()?.to_string();
                                                let range = curve["range"]
                                                    .as_array()?
                                                    .iter()
                                                    .filter_map(|r| r.as_f64().map(|v| v as f32))
                                                    .collect::<Vec<_>>();
                                                let keyframe_points = curve["keyframe_points"]
                                                    .as_array()?
                                                    .iter()
                                                    .filter_map(|kf| {
                                                        Some(scene_generics::KeyframePoint {
                                                            amplitude: kf["amplitude"].as_f64()?
                                                                as f32,
                                                            back: kf["back"].as_f64()? as f32,
                                                            easing: kf["easing"]
                                                                .as_str()?
                                                                .to_string(),
                                                            handle_left: vec![
                                                                kf["handle_left"][0].as_f64()?
                                                                    as f32,
                                                                kf["handle_left"][1].as_f64()?
                                                                    as f32,
                                                            ],
                                                            handle_left_type: kf
                                                                ["handle_left_type"]
                                                                .as_str()?
                                                                .to_string(),
                                                            handle_right: vec![
                                                                kf["handle_right"][0].as_f64()?
                                                                    as f32,
                                                                kf["handle_right"][1].as_f64()?
                                                                    as f32,
                                                            ],
                                                            handle_right_type: kf
                                                                ["handle_right_type"]
                                                                .as_str()?
                                                                .to_string(),
                                                            interpolation: kf["interpolation"]
                                                                .as_str()?
                                                                .to_string(),
                                                            co: vec![
                                                                kf["co"][0].as_f64()? as f32,
                                                                kf["co"][1].as_f64()? as f32,
                                                            ],
                                                            period: kf["period"].as_f64()? as f32,
                                                        })
                                                    })
                                                    .collect();

                                                Some(scene_generics::AnimCurve {
                                                    array_index,
                                                    auto_smoothing,
                                                    data_path,
                                                    extrapolation,
                                                    keyframe_points,
                                                    range,
                                                })
                                            })
                                            .collect(),
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

    return scenes;
}

pub async fn send_scene_data(
    scenes: HashMap<String, scene_generics::Scene>,
) -> std::io::Result<()> {
    let json_string = serde_json::to_string(&scenes).unwrap();

    let injected_script = SCENE_SENDER_PY.replace(
        "JSON_DATA = r\"\"\"\"\"\"",
        &format!("JSON_DATA = r\"\"\"{}\"\"\"", json_string),
    );

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
