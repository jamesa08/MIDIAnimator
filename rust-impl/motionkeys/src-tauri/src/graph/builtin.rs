// node groups that ship with the app. they're shared by reference: a project only stores a built-in group once it's
// made local (edited), and the local copy (same id) then replaces the built-in for every group node using it

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde_json::Value;

use super::model::{GroupDef, Graph, Position, RfEdge, RfNode};
use super::run::GROUP;

/// (group id, definition json)
const BUILTIN_GROUPS: [(&str, &str); 1] = [("evaluate_instrument", include_str!("../configs/groups/evaluate_instrument.json"))];

/// every built-in group by id
pub fn builtin_groups() -> &'static BTreeMap<String, GroupDef> {
    static GROUPS: OnceLock<BTreeMap<String, GroupDef>> = OnceLock::new();
    GROUPS.get_or_init(|| BUILTIN_GROUPS.iter().map(|(id, json)| (id.to_string(), serde_json::from_str(json).unwrap_or_else(|e| panic!("invalid built-in group '{}': {}", id, e)))).collect())
}

/// the built-in groups, for the frontend's add menu and to show a built-in group's inside
#[tauri::command]
pub fn get_builtin_groups() -> BTreeMap<String, GroupDef> {
    builtin_groups().clone()
}

/// the groups a graph can use: the built-ins, replaced by the project's own copies
pub fn all_groups(graph: &Graph) -> BTreeMap<String, GroupDef> {
    let mut groups = builtin_groups().clone();
    groups.extend(graph.groups.iter().map(|(id, def)| (id.clone(), def.clone())));
    groups
}

/// updates graphs saved by older versions, in the project and in its own groups:
/// - before node groups: a node whose type is now a built-in group becomes a group node running it. the node keeps its
///   id, connections and values
/// - before keyframes from object had picked channels: it gets the channels it had connected, under their new ids
/// - before animation generators took several curves: a generator's animation property becomes a set channel node in
///   front of each curve it plays
pub fn migrate(graph: &mut Graph) -> bool {
    let mut changed = false;
    for node in &mut graph.nodes {
        let node_type = node.resolved_node_type().to_string();
        if builtin_groups().contains_key(&node_type) {
            node.node_type = GROUP.to_string();
            node.data.insert("group_id".to_string(), Value::String(node_type));
            changed = true;
        }
    }
    changed |= migrate_keyframes_from_object(graph);
    changed |= migrate_animation_property(graph);
    for def in graph.groups.values_mut() {
        changed |= migrate_keyframes_from_object(&mut def.graph);
        changed |= migrate_animation_property(&mut def.graph);
    }
    changed
}

/// where a set channel node goes, left of the generator it feeds, a row for each one
const SET_CHANNEL_DX: f64 = 260.0;
const SET_CHANNEL_DY: f64 = 90.0;

/// animation generators used to play every curve on one animation property, set on the node or connected, and empty
/// played them on the note on curve's channel. now each curve plays on its own channel, so a set property moves each
/// curve there with a set channel node in between, unless it's already on it. an empty one changes nothing.
/// note: an empty property with note on and off curves on different channels used to put both on the note on channel,
/// they now stay on their own (old and new graphs can't be told apart there)
fn migrate_animation_property(graph: &mut Graph) -> bool {
    let mut changed = false;
    let generators: Vec<String> = graph.nodes.iter().filter(|n| n.resolved_node_type() == "animation_generator").map(|n| n.id.clone()).collect();
    for id in generators {
        // the property as it was set, and the connection feeding it
        let node = graph.node_mut(&id).unwrap();
        let value = node.inputs_mut().remove("animation_property");
        let property = value.as_ref().and_then(|v| v.as_str()).map(str::trim).unwrap_or("").to_string();
        let feed = graph.edges.iter().position(|e| e.to_node() == id && e.to_input() == "animation_property").map(|i| graph.edges.remove(i));
        changed |= value.is_some() || feed.is_some();
        if property.is_empty() && feed.is_none() {
            continue;
        }

        // a set channel node between each curve and the generator
        let position = graph.node(&id).unwrap().position.clone();
        let curves: Vec<usize> = graph.edges.iter().enumerate().filter(|(_, e)| e.to_node() == id && (e.to_input() == "note_on_keyframes" || e.to_input() == "note_off_keyframes")).map(|(i, _)| i).collect();
        // keyframes from object's outputs are their channels, a curve already on the property stays as it is
        let curves: Vec<usize> = curves.into_iter().filter(|i| feed.is_some() || source_channel(graph, &graph.edges[*i]).as_deref() != Some(property.as_str())).collect();
        for (row, index) in curves.into_iter().enumerate() {
            let set_id = graph.next_node_id("set_channel");
            let mut set = RfNode {
                id: set_id.clone(),
                node_type: "set_channel".to_string(),
                position: Position {
                    x: position.x - SET_CHANNEL_DX,
                    y: position.y + row as f64 * SET_CHANNEL_DY,
                },
                data: Default::default(),
                extra: Default::default(),
            };
            if !property.is_empty() {
                set.inputs_mut().insert("channel".to_string(), Value::String(property.clone()));
            }
            graph.nodes.push(set);

            // the curve goes through it, and what fed the property now feeds its channel
            let edge = graph.edges[index].clone();
            let into = RfEdge::new(&set_id, "keyframes", &id, edge.to_input());
            graph.edges[index] = RfEdge {
                extra: edge.extra.clone(),
                ..RfEdge::new(edge.from_node(), edge.from_output(), &set_id, "keyframes")
            };
            graph.edges.push(into);
            if let Some(feed) = &feed {
                graph.edges.push(RfEdge {
                    extra: feed.extra.clone(),
                    ..RfEdge::new(feed.from_node(), feed.from_output(), &set_id, "channel")
                });
            }
        }
    }
    changed
}

/// keyframes from object used to output every curve as `location_z` or `{data_path}_{index}`, now only the picked
/// channels as `location[2]`. the curves it had connected become its channels and the connections move to them
fn migrate_keyframes_from_object(graph: &mut Graph) -> bool {
    let mut changed = false;
    for node in &mut graph.nodes {
        if node.resolved_node_type() != "keyframes_from_object" || node.input_value("channels").is_some() {
            continue;
        }
        let mut channels: Vec<String> = Vec::new();
        for edge in graph.edges.iter_mut().filter(|e| e.from_node() == node.id) {
            let Some(channel) = old_channel(edge.from_output()) else {
                continue;
            };
            if !channels.contains(&channel) {
                channels.push(channel.clone());
            }
            let moved = RfEdge::new(&node.id, &channel, edge.to_node(), edge.to_input());
            *edge = RfEdge {
                extra: std::mem::take(&mut edge.extra),
                ..moved
            };
        }
        node.inputs_mut().insert("channels".to_string(), Value::from(channels));
        changed = true;
    }
    changed
}

/// the channel the curve on an edge is on, when it comes straight from keyframes from object (its output id is the channel)
fn source_channel(graph: &Graph, edge: &RfEdge) -> Option<String> {
    let from = graph.node(edge.from_node())?;
    (from.resolved_node_type() == "keyframes_from_object").then(|| edge.from_output().to_string())
}

/// the channel of an old output id, `location_z` -> `location[2]`, `color_1` -> `color[1]`
fn old_channel(output: &str) -> Option<String> {
    let (data_path, suffix) = output.rsplit_once('_')?;
    let index = match (data_path, suffix) {
        ("location" | "rotation" | "scale", axis) => ["x", "y", "z"].iter().position(|a| *a == axis)?,
        (_, index) => index.parse().ok()?,
    };
    Some(format!("{}[{}]", data_path, index))
}
