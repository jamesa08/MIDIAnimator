// the curves in a node's last run, for the graph window (src/windows/Graph.tsx): the bridge's curves, an animation
// generator's note on and off keys and the keyframes notes add, drawn as they're evaluated and written (utils/curve.rs)

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};

use crate::graph::executors::io::ERROR_KEY;
use crate::scene_generics::{find_object, AnimCurve, Object, Scene};
use crate::utils::animation::{finish_keys, AnimationGenerator, BlendKeyframe, CurveKeys};
use crate::utils::curve::{drawing, Drawing, Key};

/// a curve in the graph window's list: one curve, or one curve's keys from separate notes
#[derive(Serialize, Clone, Debug)]
pub struct CurveChannel {
    /// unique in its node
    pub id: String,
    /// what it's listed under (an object, a generator), empty for nothing
    pub group: String,
    pub name: String,
    /// the axis of a vector's component (`X`, `Y`, `Z`, `W`), for its color
    pub axis: Option<String>,
    /// what it moves (`position.x`, `rotation.z`), for the 3d preview, and the euler order of its object's rotation
    pub semantic: Option<String>,
    pub rotation_order: Option<String>,
    /// drawn on past its ends. false for keys that aren't a whole curve yet (one note's keys)
    pub extend: bool,
    pub pieces: Vec<Drawing>,
}

#[derive(Serialize, Clone, Debug)]
pub struct NodeCurves {
    /// the node's key in the run's results, `group-1/node-2` for a node inside a group
    pub node: String,
    pub channels: Vec<CurveChannel>,
}

/// the scene's objects with their channels, to name curves by. each object once, without its curves
struct SceneChannels {
    objects: Vec<Object>,
}

/// what a curve's channel is called and what it moves
struct Described {
    name: String,
    axis: Option<String>,
    semantic: Option<String>,
    rotation_order: Option<String>,
}

impl SceneChannels {
    fn of(scene: Option<&Scene>) -> SceneChannels {
        let mut seen = HashSet::new();
        let objects = scene
            .into_iter()
            .flat_map(Scene::objects)
            .filter(|object| seen.insert(object.id.clone()))
            .map(|object| Object {
                id: object.id.clone(),
                name: object.name.clone(),
                channels: object.channels.clone(),
                transform: None,
                rotation_order: object.rotation_order.clone(),
                anim_curves: Vec::new(),
            })
            .collect();
        SceneChannels {
            objects,
        }
    }

    /// the object an object id (or name) picks, with its name for listing curves under
    fn object_name(&self, object: &str) -> String {
        find_object(&self.objects, object).map_or_else(|| object.to_string(), |o| o.name.clone())
    }

    /// a channel on `object`, or when it's not one of the object's (or there's no object) on the first object that has
    /// it. a channel no object has is its id
    fn describe(&self, object: Option<&str>, channel: &str) -> Described {
        let on_object = object.and_then(|pick| find_object(&self.objects, pick)).filter(|o| o.channel(channel).is_some());
        match on_object.or_else(|| self.objects.iter().find(|o| o.channel(channel).is_some())) {
            Some(object) => {
                let listed = object.channel(channel).unwrap();
                Described {
                    name: object.channel_label(channel),
                    axis: listed.axis.clone(),
                    semantic: listed.semantic.clone(),
                    rotation_order: object.rotation_order.clone(),
                }
            }
            None => Described {
                name: channel.to_string(),
                axis: None,
                semantic: None,
                rotation_order: None,
            },
        }
    }

    fn channel(&self, id: String, group: String, name: Option<String>, object: Option<&str>, channel: &str, extend: bool, pieces: Vec<Drawing>) -> CurveChannel {
        let described = self.describe(object, channel);
        CurveChannel {
            id,
            group,
            name: name.unwrap_or(described.name),
            axis: described.axis,
            semantic: described.semantic,
            rotation_order: described.rotation_order,
            extend,
            pieces,
        }
    }
}

/// outputs that are only there for the node's own UI
const SKIPPED: [&str; 3] = ["dyn_output", "available_channels", ERROR_KEY];

/// the curves of nodes in a tab's last run, by their keys in the results
#[tauri::command]
pub async fn graph_curves(tab: String, nodes: Vec<String>) -> Result<Vec<NodeCurves>, String> {
    // copied out so the state isn't held while the curves are worked out
    let (scene, values): (SceneChannels, Vec<(String, Option<Value>, Option<Value>)>) = {
        let state = crate::state::lock();
        let instance = state.instance(&tab).ok_or_else(|| format!("no tab '{}'", tab))?;
        let values = nodes
            .into_iter()
            .map(|node| {
                let outputs = instance.executed_results.get(&node).cloned();
                let inputs = instance.executed_inputs.get(&node).cloned();
                (node, outputs, inputs)
            })
            .collect();
        (SceneChannels::of(instance.scene.as_ref()), values)
    };
    Ok(values
        .into_iter()
        .map(|(node, outputs, inputs)| NodeCurves {
            channels: curves_of(&scene, outputs.as_ref(), inputs.as_ref()),
            node,
        })
        .collect())
}

/// the curves in a node's outputs as the node has them (its executed results), for a preview on the node itself. nodes
/// are on the tab on screen, its scene names the channels
#[tauri::command]
pub async fn graph_value_curves(outputs: Value) -> Vec<CurveChannel> {
    let scene = SceneChannels::of(crate::state::lock().active().scene.as_ref());
    curves_of(&scene, Some(&outputs), None)
}

/// the curves in a node's outputs, or in its inputs when it has none (a viewer, the scene writer), named by `scene`'s
/// channels
pub fn node_curves(scene: Option<&Scene>, outputs: Option<&Value>, inputs: Option<&Value>) -> Vec<CurveChannel> {
    curves_of(&SceneChannels::of(scene), outputs, inputs)
}

fn curves_of(scene: &SceneChannels, outputs: Option<&Value>, inputs: Option<&Value>) -> Vec<CurveChannel> {
    let channels = outputs.map(|values| handle_curves(scene, values)).unwrap_or_default();
    if !channels.is_empty() {
        return channels;
    }
    inputs.map(|values| handle_curves(scene, values)).unwrap_or_default()
}

/// the curves in each handle's value. Keyframes From Object names its channels in `dyn_output`
fn handle_curves(scene: &SceneChannels, values: &Value) -> Vec<CurveChannel> {
    let Some(values) = values.as_object() else {
        return Vec::new();
    };
    let labels = values.get("dyn_output").and_then(Value::as_object);
    let mut channels = Vec::new();
    for (handle, value) in values {
        if SKIPPED.contains(&handle.as_str()) {
            continue;
        }
        let label = labels.and_then(|labels| labels.get(handle)).and_then(Value::as_str);
        curves_in(scene, value, handle, label, &mut channels);
    }
    // a curve with no keys has nothing to show (a generator's note off keys that aren't set)
    channels.retain(|channel| channel.pieces.iter().any(|piece| !piece.keys.is_empty()));
    channels
}

fn parse<'a, T: Deserialize<'a>>(value: &'a Value) -> Option<T> {
    T::deserialize(value).ok()
}

/// the curves in one value, told apart by their shape since an Any input can take anything
fn curves_in(scene: &SceneChannels, value: &Value, handle: &str, label: Option<&str>, out: &mut Vec<CurveChannel>) {
    let has = |key: &str| value.get(key).is_some();

    if has("keys") && has("channel") {
        if let Some(curve) = parse::<AnimCurve>(value) {
            out.push(scene.channel(handle.to_string(), String::new(), label.map(str::to_string), None, &curve.channel, true, vec![drawing(&curve.keys)]));
        }
    } else if has("note_on_keyframes") {
        if let Some(generator) = parse::<AnimationGenerator>(value) {
            // each curve under its side, named after its channel
            for (id, side, curves) in [("note_on", "Note On", &generator.note_on_keyframes), ("note_off", "Note Off", &generator.note_off_keyframes)] {
                for curve in curves {
                    let name = format!("{} › {}", side, scene.describe(None, &curve.channel).name);
                    out.push(scene.channel(format!("{}/{}/{}", handle, id, curve.channel), generator.name.clone(), Some(name), None, &curve.channel, true, vec![drawing(&curve.keys)]));
                }
            }
        }
    } else if has("keyframes") && has("object") {
        if let Some(keys) = parse::<CurveKeys>(value) {
            note_curves(scene, handle, vec![keys], out);
        }
    } else if let Some(items) = value.as_array() {
        // a loop's results: each note's keys, a list per note
        let first = items.iter().find(|item| item.as_array().is_none_or(|keys| !keys.is_empty()));
        let first = first.map(|item| item.as_array().and_then(|keys| keys.first()).unwrap_or(item));
        if first.is_some_and(|item| item.get("keyframes").is_some() && item.get("object").is_some()) {
            let mut keys = Vec::new();
            for item in items {
                match item.as_array() {
                    Some(_) => keys.extend(parse::<Vec<CurveKeys>>(item).unwrap_or_default()),
                    None => keys.extend(parse::<CurveKeys>(item)),
                }
            }
            note_curves(scene, handle, keys, out);
        } else if first.is_some_and(|item| item.get("key").is_some() && item.get("channel").is_some()) {
            if let Some(keys) = parse::<Vec<BlendKeyframe>>(value) {
                written_curves(scene, handle, None, &keys, out);
            }
        }
    } else if let Some(objects) = value.as_object() {
        // keyframes by object, what Combine Keyframes gives the scene writer
        let keyframe_lists = objects.values().all(|keys| keys.as_array().is_some_and(|keys| keys.first().is_none_or(|key| key.get("key").is_some() && key.get("channel").is_some())));
        if keyframe_lists {
            if let Some(objects) = parse::<BTreeMap<String, Vec<BlendKeyframe>>>(value) {
                for (object, keys) in &objects {
                    written_curves(scene, handle, Some(object), keys, out);
                }
            }
        }
    }
}

/// one object's keyframes (by its id), split into the curves the scene writer writes them to
fn written_curves(scene: &SceneChannels, handle: &str, object: Option<&str>, keys: &[BlendKeyframe], out: &mut Vec<CurveChannel>) {
    let mut curves: BTreeMap<&str, Vec<Key>> = BTreeMap::new();
    for key in keys {
        // the writer skips keys without a channel
        if key.channel.is_empty() {
            continue;
        }
        curves.entry(key.channel.as_str()).or_default().push(key.key.clone());
    }
    let group = object.map(|object| scene.object_name(object)).unwrap_or_default();
    for (channel, mut keys) in curves {
        // what the writer sends: in order, with the handles of keys made where notes overlap
        finish_keys(&mut keys);
        out.push(scene.channel(format!("{}/{}/{}", handle, object.unwrap_or(""), channel), group.clone(), None, object, channel, true, vec![drawing(&keys)]));
    }
}

/// notes' keys, one channel per curve with a piece for each note
fn note_curves(scene: &SceneChannels, handle: &str, chunks: Vec<CurveKeys>, out: &mut Vec<CurveChannel>) {
    let mut curves: BTreeMap<(String, String), Vec<Drawing>> = BTreeMap::new();
    for chunk in chunks {
        let mut keys = chunk.keyframes;
        finish_keys(&mut keys);
        curves.entry((chunk.object, chunk.channel)).or_default().push(drawing(&keys));
    }
    for ((object, channel), pieces) in curves {
        out.push(scene.channel(format!("{}/{}/{}", handle, object, channel), scene.object_name(&object), None, Some(&object), &channel, false, pieces));
    }
}
