use std::cell::RefCell;
use std::collections::HashMap;

use super::io::{NodeResult, Outputs};
use crate::blender::scene_data::write_scene_data;
use crate::state::STATE;
use crate::utils::log::log;
use crate::scene_generics::{ObjectGroup, Scene};
use crate::utils::animation::BlendKeyframe;

/// a scene writer's error while Blender isn't connected, it goes away when Blender connects (`clear_not_connected`)
pub const NOT_CONNECTED: &str = "Blender isn't connected";

thread_local! {
    // the scene of the tab being run, see `with_scene`
    static SCENE: RefCell<Option<Scene>> = const { RefCell::new(None) };
}

/// runs `f` with `scene` as the scene `scene_link` gives, a run of a tab runs on that tab's scene
pub fn with_scene<T>(scene: Option<Scene>, f: impl FnOnce() -> T) -> T {
    let before = SCENE.with(|current| current.replace(scene));
    let result = f();
    SCENE.with(|current| *current.borrow_mut() = before);
    result
}

// Node: scene_link
///
/// inputs:
/// none
///
/// outputs:
/// "name": `String`
/// "object_groups": `Array<ObjectGroup>`
#[node_registry::node]
pub fn scene_link() -> NodeResult {
    let mut outputs = Outputs::new();

    // the scene of the tab being run
    let scene = SCENE.with(|scene| scene.borrow().clone());

    // no scene yet (Blender hasn't sent one), empty outputs
    let Some(scene) = scene else {
        println!("NO SCENE DATA");
        outputs.set("name", String::new());
        outputs.set("object_groups", Vec::<ObjectGroup>::new());
        return Ok(outputs);
    };

    outputs.set("name", scene.name);
    outputs.set("object_groups", scene.object_groups);
    Ok(outputs)
}

/// Node: scene_writer
///
/// inputs:
/// "keyframes": `HashMap<String, Array<BlendKeyframe>>`
/// "clean_keyframes": `bool`, clears each object's existing animation before writing, on when unset
///
/// outputs:
/// None. it fails with Blender's error when the write doesn't go through, or with the objects it skipped and the
/// errors Blender gave for single objects. a node that ran without an error wrote everything
#[node_registry::node]
pub fn scene_writer(keyframes: &HashMap<String, Vec<BlendKeyframe>>, clean_keyframes: Option<bool>) -> NodeResult {
    // every write and how it went is in the log file, with Blender's error when it failed
    log(format!("writing keyframes for {} object(s) to Blender", keyframes.len()));
    let outcome = write(keyframes, clean_keyframes.unwrap_or(true));
    match &outcome {
        Ok(_) => log("wrote keyframes to Blender"),
        Err(error) => log(format!("writing keyframes to Blender failed: {error}")),
    }
    outcome
}

// the scene writer's write, what Blender says ends up on the node
fn write(keyframes: &HashMap<String, Vec<BlendKeyframe>>, clean_keyframes: bool) -> NodeResult {
    // the Blender side reads JSON
    let keyframes = serde_json::to_value(keyframes).map_err(|e| e.to_string())?;

    // with nothing connected the write would wait out its timeout
    if !STATE.lock().unwrap_or_else(std::sync::PoisonError::into_inner).connected {
        return Err(NOT_CONNECTED.to_string());
    }

    // the write finishes before the node does
    let report = block_on(write_scene_data(keyframes, clean_keyframes)).map_err(|e| e.to_string())?;

    let mut problems = Vec::new();
    if !report.missing_objects.is_empty() {
        problems.push(format!("not in the Blender scene, skipped: {}", report.missing_objects.join(", ")));
    }
    problems.extend(report.errors);
    if !problems.is_empty() {
        return Err(problems.join("\n"));
    }
    Ok(Outputs::new())
}

// waits for `future` from a node function (they're sync). runs run on the async runtime's worker threads,
// block_in_place lets the runtime move its other tasks off this one while it waits
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future)),
        Err(_) => tauri::async_runtime::block_on(future),
    }
}
