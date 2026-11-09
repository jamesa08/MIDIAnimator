// the keyframes of the objects a graph reads (Keyframes From Object). the bridge only sends the curves of those objects,
// every object's animation would make each message huge. picking an object fetches its curves (the watch request), the
// bridge then watches it and sends them again in a curves message when its keys change. an object with more keyframes
// than the limit setting asks first

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::bridge::scene_data::on_screen;
use crate::graph::execute::run_instance;
use crate::graph::model::Graph;
use crate::ipc;
use crate::scene_generics::{AnimCurve, Object, Scene};
use crate::state::{lock, update_state, STATE};
use crate::utils::curve::check_keys;
use crate::utils::log::log;

// the limit when the setting is missing or not a number
const DEFAULT_KEYFRAME_IMPORT_LIMIT: u64 = 5000;

// the arrival number (`ipc::request_numbered`) of the last curves message each object's curves came in, by id. a watch
// reply that came before it has older curves for the object. only changed with the state locked, so a reply and a
// curves message are applied in the order they came
static CURVES_ARRIVED: Mutex<BTreeMap<String, u64>> = Mutex::new(BTreeMap::new());

fn curves_arrived() -> MutexGuard<'static, BTreeMap<String, u64>> {
    CURVES_ARRIVED.lock().unwrap_or_else(PoisonError::into_inner)
}

/// the most keyframes an object can have before importing it asks first
pub fn keyframe_import_limit() -> u64 {
    crate::settings::get_setting("bridge.keyframe_import_limit").as_u64().unwrap_or(DEFAULT_KEYFRAME_IMPORT_LIMIT)
}

/// the objects a graph reads keyframes from, by id: every Keyframes From Object's object, in the graph and its groups.
/// a pick typed as a name is the id of the object in `scene` with that name
pub fn curve_sources(graph: &Graph, scene: Option<&Scene>) -> BTreeSet<String> {
    let nodes = graph.nodes.iter().chain(graph.groups.values().flat_map(|def| def.graph.nodes.iter()));
    let picks = nodes.filter(|node| node.resolved_node_type() == "keyframes_from_object").filter_map(|node| node.input_value("object_name")?.as_str()).filter(|pick| !pick.is_empty());
    let objects: Vec<&Object> = scene.into_iter().flat_map(Scene::objects).collect();
    let id = |pick: &str| objects.iter().find(|object| object.id == pick).or_else(|| objects.iter().find(|object| object.name == pick)).map_or(pick, |object| object.id.as_str()).to_string();
    picks.map(id).collect()
}

/// sets the curves of the objects in `curves`, in every group they're in
pub fn merge_curves(scene: &mut Option<Scene>, curves: &HashMap<String, Vec<AnimCurve>>) {
    for object in scene.iter_mut().flat_map(Scene::objects_mut) {
        if let Some(object_curves) = curves.get(&object.id) {
            object.anim_curves = object_curves.clone();
        }
    }
}

/// a new scene from Blender only has the curves of the objects its tracker watches. the others keep the ones they had
/// (until the project is saved, see `prune_curves`)
pub fn carry_over_curves(old: &Option<Scene>, new: &mut Scene, watched: &BTreeSet<String>) {
    let kept: HashMap<String, Vec<AnimCurve>> = old.iter().flat_map(Scene::objects).filter(|object| !object.anim_curves.is_empty() && !watched.contains(&object.id)).map(|object| (object.id.clone(), object.anim_curves.clone())).collect();
    for object in new.objects_mut() {
        if object.anim_curves.is_empty() {
            if let Some(curves) = kept.get(&object.id) {
                object.anim_curves = curves.clone();
            }
        }
    }
}

/// drops the curves of objects the graph doesn't read anymore, done when the project is saved
pub fn prune_curves(scene: &mut Option<Scene>, keep: &BTreeSet<String>) {
    for object in scene.iter_mut().flat_map(Scene::objects_mut) {
        if !keep.contains(&object.id) {
            object.anim_curves.clear();
        }
    }
}

// the watch request and its curves reply
#[derive(Serialize)]
struct CurveRequest {
    objects: BTreeSet<String>,
    fetch: BTreeSet<String>,
    limit: u64,
    approved: BTreeSet<String>,
}

#[derive(Deserialize)]
struct CurveResponse {
    /// read curve by curve, a bad one is left out on its own
    #[serde(default)]
    objects: HashMap<String, Vec<Value>>,
    #[serde(default)]
    over_limit: BTreeMap<String, u64>,
    #[serde(default)]
    missing_objects: Vec<String>,
}

/// each object's curves from a curves message, a curve that isn't valid is logged and left out
pub fn read_curves(objects: HashMap<String, Vec<Value>>) -> HashMap<String, Vec<AnimCurve>> {
    objects
        .into_iter()
        .map(|(object, curves)| {
            let curves = curves
                .into_iter()
                .filter_map(|curve| match serde_json::from_value::<AnimCurve>(curve) {
                    Ok(curve) => match check_keys(&curve.keys) {
                        None => Some(curve),
                        Some(problem) => {
                            log(format!("left out the curve '{}' of '{object}' the bridge sent: {problem}", curve.channel));
                            None
                        }
                    },
                    Err(e) => {
                        log(format!("left out a curve of '{object}' the bridge sent: {e}"));
                        None
                    }
                })
                .collect();
            (object, curves)
        })
        .collect()
}

/// takes a curves message the bridge sent when watched objects' keys changed: their curves go into the linked tab's
/// scene (its scene waiting for review too). the tab if it should run, like `ipc::apply_scene_update`. `arrival` is
/// the message's arrival number
pub fn apply_curves_update(body: Value, arrival: u64) -> Option<String> {
    let objects: HashMap<String, Vec<Value>> = body.get("objects").cloned().and_then(|objects| serde_json::from_value(objects).ok()).unwrap_or_default();
    let curves = read_curves(objects);
    let mut state = STATE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let id = state.connected_instance_id.clone()?;
    let shown = state.active_instance_id == id;
    // only what's watched, curves for an object the tab stopped reading are left out
    let watched = state.watched_curves.clone().unwrap_or_default();
    let curves: HashMap<String, Vec<AnimCurve>> = curves.into_iter().filter(|(object, _)| watched.contains(object)).collect();
    if curves.is_empty() {
        return None;
    }
    let instance = state.instance_mut(&id)?;
    merge_curves(&mut instance.scene, &curves);
    merge_curves(&mut instance.pending_scene, &curves);
    // a watch reply that came before this message doesn't put its older curves back
    curves_arrived().extend(curves.into_keys().map(|object| (object, arrival)));
    (shown && !instance.execution_paused).then_some(id)
}

/// makes Blender's tracker watch the objects the live tab's graph reads, and fetches the curves of the ones it didn't
/// watch yet. objects over the keyframe limit wait for `accept_curve_import`. does nothing for a tab that isn't live
pub async fn sync_curves(id: &str) {
    // a bridge that doesn't watch has no curves to send
    if !ipc::bridge_watches() {
        return;
    }
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
        let wanted = curve_sources(&graph, instance.scene.as_ref());
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
        approved.extend(instance.scene.iter().flat_map(Scene::objects).filter(|object| fetch.contains(&object.id) && !object.anim_curves.is_empty()).map(|object| object.id.clone()));
        let request = CurveRequest {
            objects: watch.clone(),
            fetch,
            limit: keyframe_import_limit(),
            approved,
        };
        // set now so a run starting meanwhile doesn't fetch the same objects
        state.watched_curves = Some(watch);
        (request, before)
    };

    let (arrival, response) = match send_request(&request).await {
        Ok(response) => response,
        Err(e) => {
            // the next run tries again
            log(format!("the bridge couldn't send keyframes: {e}"));
            lock().watched_curves = before;
            return;
        }
    };

    let mut state = lock();
    // objects over the limit aren't watched until they're accepted
    if let Some(watched) = state.watched_curves.as_mut() {
        for name in response.over_limit.keys() {
            watched.remove(name);
        }
    }
    if !response.missing_objects.is_empty() {
        log(format!("not in the scene, no keyframes: {}", response.missing_objects.join(", ")));
    }
    let Some(instance) = state.instance_mut(id) else {
        return;
    };
    // an object whose curves came in a curves message after the reply has newer ones already
    let mut curves = read_curves(response.objects);
    let arrived = curves_arrived();
    curves.retain(|object, _| arrived.get(object).is_none_or(|newer| *newer < arrival));
    drop(arrived);
    merge_curves(&mut instance.scene, &curves);
    if !response.over_limit.is_empty() {
        instance.pending_curve_import = Some(response.over_limit);
    }
}

/// makes Blender's tracker stop sending keyframes, no tab is live
pub async fn unwatch_curves() {
    if !ipc::bridge_watches() {
        return;
    }
    {
        let mut state = lock();
        if !state.connected {
            return;
        }
        state.watched_curves = None;
    }
    let request = CurveRequest {
        objects: BTreeSet::new(),
        fetch: BTreeSet::new(),
        limit: 0,
        approved: BTreeSet::new(),
    };
    if let Err(e) = send_request(&request).await {
        log(format!("the bridge didn't stop sending keyframes: {e}"));
    }
}

// the bridge's curves for the request, with the reply's arrival number
async fn send_request(request: &CurveRequest) -> Result<(u64, CurveResponse), String> {
    let body = serde_json::to_value(request).map_err(|e| e.to_string())?;
    let (arrival, reply) = ipc::request_numbered("watch", body).await?;
    let response = serde_json::from_value(reply).map_err(|e| format!("couldn't read the keyframes {} sent: {e}", crate::ipc::app_name()))?;
    Ok((arrival, response))
}

/// imports the objects over the keyframe limit waiting on the tab on screen (`tab`), and runs it
#[tauri::command]
pub async fn accept_curve_import(tab: Option<String>) -> Result<(), String> {
    let id = {
        let mut state = lock();
        on_screen(&state, tab.as_deref())?;
        let instance = state.active_mut();
        let pending = instance.pending_curve_import.take().ok_or("No keyframes waiting to be imported")?;
        instance.approved_curves.extend(pending.into_keys());
        instance.id.clone()
    };
    update_state();
    run_instance(id, true).await;
    Ok(())
}

/// leaves out the objects over the keyframe limit waiting on the tab on screen (`tab`), until they're picked again
#[tauri::command]
pub fn reject_curve_import(tab: Option<String>) -> Result<(), String> {
    {
        let mut state = lock();
        on_screen(&state, tab.as_deref())?;
        let instance = state.active_mut();
        if let Some(pending) = instance.pending_curve_import.take() {
            instance.declined_curves.extend(pending.into_keys());
        }
    }
    update_state();
    Ok(())
}
