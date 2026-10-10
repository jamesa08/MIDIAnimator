// the keyframes of the objects a graph reads (Keyframes From Object). Blender only sends the curves of those objects, every
// object's animation would make each scene update huge. picking an object fetches its curves, Blender's tracker then
// watches it and sends them again when its keys change. an object with more keyframes than the limit setting asks first

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::graph::execute::run_instance;
use crate::graph::model::Graph;
use crate::ipc;
use crate::scene_generics::{AnimCurve, Scene};
use crate::state::{lock, update_state};
use crate::utils::log::log;

static OBJECT_CURVES_PY: &str = include_str!("python/blender_object_curves.py");

// how long Blender gets to send curves, an object with a lot of keyframes takes a while
const CURVES_TIMEOUT: Duration = Duration::from_secs(30);

// the limit when the setting is missing or not a number
const DEFAULT_KEYFRAME_IMPORT_LIMIT: u64 = 5000;

/// the most keyframes an object can have before importing it asks first
pub fn keyframe_import_limit() -> u64 {
    crate::settings::get_setting("blender.keyframe_import_limit").as_u64().unwrap_or(DEFAULT_KEYFRAME_IMPORT_LIMIT)
}

/// the objects a graph reads keyframes from: every Keyframes From Object's object, in the graph and its groups
pub fn curve_sources(graph: &Graph) -> BTreeSet<String> {
    let nodes = graph.nodes.iter().chain(graph.groups.values().flat_map(|def| def.graph.nodes.iter()));
    nodes.filter(|node| node.resolved_node_type() == "keyframes_from_object").filter_map(|node| node.input_value("object_name")?.as_str()).filter(|name| !name.is_empty()).map(str::to_string).collect()
}

/// sets the curves of the objects in `curves`, in every scene they're in
pub fn merge_curves(scenes: &mut HashMap<String, Scene>, curves: &HashMap<String, Vec<AnimCurve>>) {
    for object in scenes.values_mut().flat_map(|scene| scene.object_groups.iter_mut()).flat_map(|group| group.objects.iter_mut()) {
        if let Some(object_curves) = curves.get(&object.name) {
            object.anim_curves = object_curves.clone();
        }
    }
}

/// a new scene from Blender only has the curves of the objects its tracker watches. the others keep the ones they had
/// (until the project is saved, see `prune_curves`)
pub fn carry_over_curves(old: &HashMap<String, Scene>, new: &mut HashMap<String, Scene>, watched: &BTreeSet<String>) {
    let kept: HashMap<String, Vec<AnimCurve>> = old.values().flat_map(|scene| scene.object_groups.iter()).flat_map(|group| group.objects.iter()).filter(|object| !object.anim_curves.is_empty() && !watched.contains(&object.name)).map(|object| (object.name.clone(), object.anim_curves.clone())).collect();
    for object in new.values_mut().flat_map(|scene| scene.object_groups.iter_mut()).flat_map(|group| group.objects.iter_mut()) {
        if object.anim_curves.is_empty() {
            if let Some(curves) = kept.get(&object.name) {
                object.anim_curves = curves.clone();
            }
        }
    }
}

/// drops the curves of objects the graph doesn't read anymore, done when the project is saved
pub fn prune_curves(scenes: &mut HashMap<String, Scene>, keep: &BTreeSet<String>) {
    for object in scenes.values_mut().flat_map(|scene| scene.object_groups.iter_mut()).flat_map(|group| group.objects.iter_mut()) {
        if !keep.contains(&object.name) {
            object.anim_curves.clear();
        }
    }
}

// what blender_object_curves.py gets and gives
#[derive(Serialize)]
struct CurveRequest {
    watch: BTreeSet<String>,
    fetch: BTreeSet<String>,
    limit: u64,
    approved: BTreeSet<String>,
}

#[derive(Deserialize)]
struct CurveResponse {
    curves: HashMap<String, Vec<AnimCurve>>,
    large: BTreeMap<String, u64>,
}

/// makes Blender's tracker watch the objects the live tab's graph reads, and fetches the curves of the ones it didn't
/// watch yet. objects over the keyframe limit wait for `accept_curve_import`. does nothing for a tab that isn't live
pub async fn sync_curves(id: &str) {
    let (request, before) = {
        let mut state = lock();
        if !state.connected || !state.is_live(id) {
            return;
        }
        let before = state.watched_curves.clone();
        let Some(instance) = state.instance_mut(id) else {
            return;
        };
        let Ok(graph) = Graph::from_rf(&instance.rf_instance) else {
            return;
        };
        let wanted = curve_sources(&graph);
        // an object picked again asks again
        instance.declined_curves.retain(|name| wanted.contains(name));
        let watch: BTreeSet<String> = wanted.difference(&instance.declined_curves).cloned().collect();
        if before.as_ref() == Some(&watch) {
            return;
        }
        // when it isn't known what Blender watches, every object's curves are fetched again
        let fetch: BTreeSet<String> = before.as_ref().map_or_else(|| watch.clone(), |before| watch.difference(before).cloned().collect());
        // objects with curves were imported before or saved with the project, they don't ask again
        let mut approved = instance.approved_curves.clone();
        approved.extend(instance.scene_data.values().flat_map(|scene| scene.object_groups.iter()).flat_map(|group| group.objects.iter()).filter(|object| fetch.contains(&object.name) && !object.anim_curves.is_empty()).map(|object| object.name.clone()));
        let request = CurveRequest {
            watch: watch.clone(),
            fetch,
            limit: keyframe_import_limit(),
            approved,
        };
        // set now so a run starting meanwhile doesn't fetch the same objects
        state.watched_curves = Some(watch);
        (request, before)
    };

    let response = send_request(&request).await;
    let Some(Ok(response)) = response.as_deref().map(serde_json::from_str::<CurveResponse>) else {
        // the next run tries again
        log(format!("Blender couldn't send keyframes: {}", response.as_deref().unwrap_or("no response")));
        lock().watched_curves = before;
        return;
    };

    let mut state = lock();
    // objects over the limit aren't watched until they're accepted
    if let Some(watched) = state.watched_curves.as_mut() {
        for name in response.large.keys() {
            watched.remove(name);
        }
    }
    let Some(instance) = state.instance_mut(id) else {
        return;
    };
    merge_curves(&mut instance.scene_data, &response.curves);
    if !response.large.is_empty() {
        instance.pending_curve_import = Some(response.large);
    }
}

/// makes Blender's tracker stop sending keyframes, no tab is live
pub async fn unwatch_curves() {
    {
        let mut state = lock();
        if !state.connected {
            return;
        }
        state.watched_curves = None;
    }
    let request = CurveRequest {
        watch: BTreeSet::new(),
        fetch: BTreeSet::new(),
        limit: 0,
        approved: BTreeSet::new(),
    };
    if send_request(&request).await.is_none() {
        log("Blender didn't stop sending keyframes");
    }
}

// runs blender_object_curves.py with the request, Blender's answer
async fn send_request(request: &CurveRequest) -> Option<String> {
    let request = serde_json::to_string(request).unwrap_or_default();
    let script = OBJECT_CURVES_PY.replace("REQUEST = r\"\"\"\"\"\"", &format!("REQUEST = r\"\"\"{}\"\"\"", request));
    ipc::send_message_with_timeout(script, CURVES_TIMEOUT).await
}

/// imports the objects over the keyframe limit waiting on the tab on screen, and runs it
#[tauri::command]
pub async fn accept_curve_import() -> Result<(), String> {
    let id = {
        let mut state = lock();
        let instance = state.active_mut();
        let pending = instance.pending_curve_import.take().ok_or("No keyframes waiting to be imported")?;
        instance.approved_curves.extend(pending.into_keys());
        instance.id.clone()
    };
    update_state();
    run_instance(id, true).await;
    Ok(())
}

/// leaves out the objects over the keyframe limit waiting on the tab on screen, until they're picked again
#[tauri::command]
pub fn reject_curve_import() {
    {
        let mut state = lock();
        let instance = state.active_mut();
        if let Some(pending) = instance.pending_curve_import.take() {
            instance.declined_curves.extend(pending.into_keys());
        }
    }
    update_state();
}
