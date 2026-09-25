use std::sync::PoisonError;

use super::io::{Inputs, NodeResult, Outputs};
use crate::blender::scene_data::write_scene_data;
use crate::scene_generics::ObjectGroup;
use crate::state::STATE;

// Node: scene_link
///
/// inputs:
/// none
///
/// outputs:
/// "name": `String`
/// "object_groups": `Array<ObjectGroup>`
#[node_registry::node]
pub fn scene_link(_inputs: Inputs) -> NodeResult {
    let mut outputs = Outputs::new();

    // copy the scene out, a poisoned lock (a panic somewhere else) still has usable scene data
    let scene = STATE.lock().unwrap_or_else(PoisonError::into_inner).scene_data.get("Scene").cloned();

    // no scene yet (Blender hasn't sent one), empty outputs
    let Some(scene) = scene else {
        println!("NO SCENE DATA");
        outputs.set("name", "")?;
        outputs.set("object_groups", &Vec::<ObjectGroup>::new())?;
        return Ok(outputs);
    };

    outputs.set("name", &scene.name)?;
    outputs.set("object_groups", &scene.object_groups)?;
    Ok(outputs)
}

/// Node: scene_writer
///
/// inputs:
/// "keyframes": `HashMap<String, Array<BlendKeyframe>>`
///
/// outputs:
/// None, the result of the write is logged to the console
#[node_registry::node]
pub fn scene_writer(inputs: Inputs) -> NodeResult {
    let keyframes: serde_json::Value = inputs.get("keyframes")?;

    // node functions are sync, so the write runs in the background
    println!("writing keyframes to Blender...");
    tauri::async_runtime::spawn(async move {
        match write_scene_data(keyframes).await {
            Ok(report) => {
                for name in &report.missing_objects {
                    eprintln!("scene writer: object '{}' isn't in the Blender scene, skipped it", name);
                }
                for error in &report.errors {
                    eprintln!("scene writer: {}", error);
                }
                if report.missing_objects.is_empty() && report.errors.is_empty() {
                    println!("done writing keyframes to Blender");
                } else {
                    eprintln!("wrote keyframes to Blender with {} missing object(s) and {} error(s)", report.missing_objects.len(), report.errors.len());
                }
            }
            Err(e) => eprintln!("failed to write keyframes to Blender: {}", e),
        }
    });

    Ok(Outputs::new())
}
