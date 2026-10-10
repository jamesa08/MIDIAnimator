// the curves in a node's last run, for the graph window (src/windows/Graph.tsx): Blender's curves, an animation generator's
// note on and off keys and the keyframes notes add, drawn the way Blender would draw them (utils/fcurve.rs)

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

use crate::graph::executors::animation::{curve_axis, curve_channel};
use crate::graph::executors::io::ERROR_KEY;
use crate::scene_generics::AnimCurve;
use crate::utils::animation::{AnimationGenerator, BlendKeyframe, CurveKeys};
use crate::utils::fcurve::{Drawing, FCurve};

/// a curve in the graph window's list: one curve, or one curve's keys from separate notes
#[derive(Serialize, Clone, Debug)]
pub struct CurveChannel {
    /// unique in its node
    pub id: String,
    /// what it's listed under (an object, a generator), empty for nothing
    pub group: String,
    pub name: String,
    /// the axis of a vector property (`X`, `Y`, `Z`, `W`), for its color
    pub axis: Option<&'static str>,
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

/// outputs that are only there for the node's own UI
const SKIPPED: [&str; 3] = ["dyn_output", "available_channels", ERROR_KEY];

/// the curves of nodes in a tab's last run, by their keys in the results
#[tauri::command]
pub async fn graph_curves(tab: String, nodes: Vec<String>) -> Result<Vec<NodeCurves>, String> {
    // copied out so the state isn't held while the curves are worked out
    let values: Vec<(String, Option<Value>, Option<Value>)> = {
        let state = crate::state::lock();
        let instance = state.instance(&tab).ok_or_else(|| format!("no tab '{}'", tab))?;
        nodes
            .into_iter()
            .map(|node| {
                let outputs = instance.executed_results.get(&node).cloned();
                let inputs = instance.executed_inputs.get(&node).cloned();
                (node, outputs, inputs)
            })
            .collect()
    };
    Ok(values
        .into_iter()
        .map(|(node, outputs, inputs)| NodeCurves {
            channels: node_curves(outputs.as_ref(), inputs.as_ref()),
            node,
        })
        .collect())
}

/// the curves in a node's outputs as the node has them (its executed results), for a preview on the node itself
#[tauri::command]
pub async fn graph_value_curves(outputs: Value) -> Vec<CurveChannel> {
    node_curves(Some(&outputs), None)
}

/// the curves in a node's outputs, or in its inputs when it has none (a viewer, the scene writer)
pub fn node_curves(outputs: Option<&Value>, inputs: Option<&Value>) -> Vec<CurveChannel> {
    let channels = outputs.map(handle_curves).unwrap_or_default();
    if !channels.is_empty() {
        return channels;
    }
    inputs.map(handle_curves).unwrap_or_default()
}

/// the curves in each handle's value. Keyframes From Object names its channels in `dyn_output`
fn handle_curves(values: &Value) -> Vec<CurveChannel> {
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
        curves_in(value, handle, label, &mut channels);
    }
    // a curve with no keys has nothing to show (a generator's note off keys that aren't set)
    channels.retain(|channel| channel.pieces.iter().any(|piece| !piece.keys.is_empty()));
    channels
}

fn parse<'a, T: Deserialize<'a>>(value: &'a Value) -> Option<T> {
    T::deserialize(value).ok()
}

/// the curves in one value, told apart by their shape since an Any input can take anything
fn curves_in(value: &Value, handle: &str, label: Option<&str>, out: &mut Vec<CurveChannel>) {
    let has = |key: &str| value.get(key).is_some();

    if has("keyframe_points") {
        if let Some(curve) = parse::<AnimCurve>(value) {
            let name = label.map(str::to_string).unwrap_or_else(|| curve_channel(&curve.data_path, curve.array_index, &[]).label());
            out.push(CurveChannel {
                id: handle.to_string(),
                group: String::new(),
                name,
                axis: curve_axis(&curve.data_path, curve.array_index),
                extend: true,
                pieces: vec![FCurve::from_anim_curve(&curve).drawing()],
            });
        }
    } else if has("note_on_keyframes") {
        if let Some(generator) = parse::<AnimationGenerator>(value) {
            // each curve under its side, named after its channel
            for (id, name, curves) in [("note_on", "Note On", &generator.note_on_keyframes), ("note_off", "Note Off", &generator.note_off_keyframes)] {
                let all: Vec<(&str, u32)> = curves.iter().map(|c| (c.data_path.as_str(), c.array_index)).collect();
                for curve in curves {
                    out.push(CurveChannel {
                        id: format!("{}/{}/{}[{}]", handle, id, curve.data_path, curve.array_index),
                        group: generator.name.clone(),
                        name: format!("{} › {}", name, curve_channel(&curve.data_path, curve.array_index, &all).label()),
                        axis: curve_axis(&curve.data_path, curve.array_index),
                        extend: true,
                        pieces: vec![FCurve::from_points(&curve.keyframe_points).drawing()],
                    });
                }
            }
        }
    } else if has("keyframes") && has("object") {
        if let Some(keys) = parse::<CurveKeys>(value) {
            note_curves(handle, vec![keys], out);
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
            note_curves(handle, keys, out);
        } else if first.is_some_and(|item| item.get("time").is_some() && item.get("data_path").is_some()) {
            if let Some(keys) = parse::<Vec<BlendKeyframe>>(value) {
                written_curves(handle, "", &keys, out);
            }
        }
    } else if let Some(objects) = value.as_object() {
        // keyframes by object, what Combine Keyframes gives the scene writer
        let keyframe_lists = objects.values().all(|keys| keys.as_array().is_some_and(|keys| keys.first().is_none_or(|key| key.get("time").is_some() && key.get("data_path").is_some())));
        if keyframe_lists {
            if let Some(objects) = parse::<BTreeMap<String, Vec<BlendKeyframe>>>(value) {
                for (object, keys) in &objects {
                    written_curves(handle, object, keys, out);
                }
            }
        }
    }
}

/// one object's keyframes, split into the curves the scene writer writes them to
fn written_curves(handle: &str, object: &str, keys: &[BlendKeyframe], out: &mut Vec<CurveChannel>) {
    let mut curves: BTreeMap<(&str, u32), Vec<(f64, f64)>> = BTreeMap::new();
    for key in keys {
        // the writer skips keys without a path
        if key.data_path.is_empty() {
            continue;
        }
        curves.entry((key.data_path.as_str(), key.array_index)).or_default().push((key.time, key.value));
    }
    let all: Vec<(&str, u32)> = curves.keys().copied().collect();
    for ((path, index), keys) in curves {
        out.push(CurveChannel {
            id: format!("{}/{}/{}[{}]", handle, object, path, index),
            group: object.to_string(),
            name: curve_channel(path, index, &all).label(),
            axis: curve_axis(path, index),
            extend: true,
            pieces: vec![FCurve::written(keys).drawing()],
        });
    }
}

/// notes' keys, one channel per curve with a piece for each note
fn note_curves(handle: &str, chunks: Vec<CurveKeys>, out: &mut Vec<CurveChannel>) {
    let mut curves: BTreeMap<(String, String, u32), Vec<Drawing>> = BTreeMap::new();
    for chunk in chunks {
        let drawing = FCurve::written(chunk.keyframes.iter().map(|key| (key.time, key.value))).drawing();
        curves.entry((chunk.object, chunk.data_path, chunk.array_index)).or_default().push(drawing);
    }
    let paths: Vec<(String, String, u32)> = curves.keys().cloned().collect();
    for ((object, path, index), pieces) in curves {
        let all: Vec<(&str, u32)> = paths.iter().filter(|(o, _, _)| *o == object).map(|(_, p, i)| (p.as_str(), *i)).collect();
        out.push(CurveChannel {
            id: format!("{}/{}/{}[{}]", handle, object, path, index),
            name: curve_channel(&path, index, &all).label(),
            axis: curve_axis(&path, index),
            group: object,
            extend: false,
            pieces,
        });
    }
}
