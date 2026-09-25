use std::sync::PoisonError;

use super::io::{Inputs, NodeResult, Outputs};
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
