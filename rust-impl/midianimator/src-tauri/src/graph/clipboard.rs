// the node clipboard. copied nodes are a JSON `Payload`, put on the system clipboard under MotionKeys' own type
// (state/graph.rs) so they can be pasted into any project, in any running MotionKeys. pasting checks everything in it

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use super::builtin::{all_groups, migrate};
use super::model::{Graph, GroupDef, NodeSpec, Position, RfEdge, RfNode, Specs};
use super::ops::{graph_in, id_prefix, select, target, with_zone_partners, zone_of, Added, UI_KEYS};
use super::run::{GROUP, GROUP_INPUT, GROUP_OUTPUT};
use super::tags;

/// what copied nodes say they are
pub const FORMAT: &str = "motionkeys/nodes";
/// the newest payload this version reads and the one it writes
pub const VERSION: u64 = 1;
/// limits for pasted text, it can come from anywhere
const MAX_TEXT: usize = 5_000_000;
const MAX_NODES: usize = 2000;
/// edge fields only the UI uses
const UI_EDGE_KEYS: &[&str] = &["selected"];

/// copied nodes with the connections between them, and the project's own groups they run (and the groups those run).
/// built-in groups are used by id
#[derive(Serialize, Deserialize, Debug)]
struct Payload {
    format: String,
    version: u64,
    nodes: Vec<RfNode>,
    #[serde(default)]
    edges: Vec<RfEdge>,
    #[serde(default)]
    groups: BTreeMap<String, GroupDef>,
}

// MARK: - Copy

/// a graph without the fields only the UI uses, for the clipboard and for comparing groups
fn clean_graph(graph: &Graph) -> Graph {
    let mut graph = graph.clone();
    graph.extra.remove("viewport");
    for node in &mut graph.nodes {
        node.extra.retain(|k, _| !UI_KEYS.contains(&k.as_str()));
    }
    for edge in &mut graph.edges {
        edge.extra.retain(|k, _| !UI_EDGE_KEYS.contains(&k.as_str()));
    }
    graph
}

fn clean_def(def: &GroupDef) -> GroupDef {
    GroupDef {
        graph: clean_graph(&def.graph),
        ..def.clone()
    }
}

/// the group a group node runs
fn group_of(node: &RfNode) -> Option<&str> {
    (node.node_type == GROUP).then(|| node.data.get("group_id").and_then(Value::as_str)).flatten()
}

/// copies nodes of the graph `scope` (a zone as a pair) with the connections between them, as text
pub fn copy(project: &Graph, scope: Option<&str>, nodes: &[String]) -> Result<String, String> {
    let graph = graph_in(project, scope)?;
    let ids = with_zone_partners(graph, nodes);
    let copied = clean_graph(&Graph {
        nodes: graph.nodes.iter().filter(|n| ids.contains(&n.id)).cloned().collect(),
        edges: graph.edges.iter().filter(|e| ids.contains(e.from_node()) && ids.contains(e.to_node())).cloned().collect(),
        ..Default::default()
    });
    if copied.nodes.is_empty() {
        return Err("nothing to copy".to_string());
    }

    // the project's own groups the nodes run, and the groups those run
    let mut groups = BTreeMap::new();
    let mut queue: VecDeque<&str> = copied.nodes.iter().filter_map(group_of).collect();
    while let Some(id) = queue.pop_front() {
        let Some(def) = project.groups.get(id) else {
            continue;
        };
        if groups.contains_key(id) {
            continue;
        }
        groups.insert(id.to_string(), clean_def(def));
        queue.extend(def.graph.nodes.iter().filter_map(group_of));
    }

    let payload = Payload {
        format: FORMAT.to_string(),
        version: VERSION,
        nodes: copied.nodes,
        edges: copied.edges,
        groups,
    };
    serde_json::to_string(&payload).map_err(|e| format!("could not copy: {}", e))
}

// MARK: - Paste

/// true if `group` is `inner` or runs it somewhere inside (a group node of it, at any depth)
fn runs(groups: &BTreeMap<String, GroupDef>, group: &str, inner: &str) -> bool {
    let mut seen = HashSet::new();
    let mut queue = VecDeque::from([group]);
    while let Some(id) = queue.pop_front() {
        if id == inner {
            return true;
        }
        if !seen.insert(id) {
            continue;
        }
        if let Some(def) = groups.get(id) {
            queue.extend(def.graph.nodes.iter().filter_map(group_of));
        }
    }
    false
}

/// `base`, or `base_2`, `base_3`, ... whichever isn't taken
fn free_id(base: &str, taken: &dyn Fn(&str) -> bool) -> String {
    if !taken(base) {
        return base.to_string();
    }
    (2..).map(|n| format!("{}_{}", base, n)).find(|id| !taken(id)).unwrap()
}

/// `name`, or Blender style `name.001`, `name.002`, ... whichever isn't taken
fn free_name(name: &str, taken: &HashSet<String>) -> String {
    if !taken.contains(name) {
        return name.to_string();
    }
    (1..).map(|n| format!("{}.{:03}", name, n)).find(|n| !taken.contains(n)).unwrap()
}

/// a pasted node's data: only what a node keeps there (its input values checked against its spec, the group it runs,
/// its zone partner)
fn clean_data(node: &RfNode, spec: Option<&NodeSpec>) -> Map<String, Value> {
    let mut data = Map::new();
    let inputs: Map<String, Value> = node
        .inputs()
        .into_iter()
        .flatten()
        .filter(|(key, value)| {
            let Some(handle) = spec.and_then(|s| s.input(key)) else {
                return false;
            };
            match handle.data_type.as_str() {
                "String" => value.is_string(),
                "f64" => value.is_number(),
                _ => !value.is_null(),
            }
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    data.insert("inputs".to_string(), Value::Object(inputs));
    for key in ["group_id", "zone"] {
        if let Some(value) = node.data.get(key).filter(|v| v.is_string()) {
            data.insert(key.to_string(), value.clone());
        }
    }
    data.extend(tags::clean(&node.data));
    data
}

/// adds the nodes in `text` (copied with `copy`) to the graph `scope`, centered on `position`. their groups come along:
/// one the project already has the same is used, one whose id is taken by a different group gets a new id. nodes that
/// can't go here (an unknown type, the group input or output outside a group, a group that would contain itself) are
/// left out. the pasted nodes become the selection
pub fn paste(project: &mut Graph, scope: Option<&str>, specs: &[NodeSpec], text: &str, position: &Position, added: &mut Vec<Added>) -> Result<(), String> {
    if text.len() > MAX_TEXT {
        return Err("the clipboard text is too long to be nodes".to_string());
    }
    let mut payload: Payload = serde_json::from_str(text.trim()).map_err(|_| "the clipboard has no nodes".to_string())?;
    if payload.format != FORMAT {
        return Err("the clipboard has no nodes".to_string());
    }
    if payload.version > VERSION {
        return Err("these nodes were copied from a newer version of MotionKeys".to_string());
    }
    if payload.nodes.len() > MAX_NODES {
        return Err(format!("too many nodes to paste ({}, at most {})", payload.nodes.len(), MAX_NODES));
    }
    // nodes from before node groups are brought up to date, like a project when it's loaded
    let mut copied = Graph {
        nodes: std::mem::take(&mut payload.nodes),
        edges: std::mem::take(&mut payload.edges),
        ..Default::default()
    };
    migrate(&mut copied);
    target(project, scope)?;

    // the groups: the same one the project has is used, one with a taken id comes in under a new id
    let existing = all_groups(project);
    let mut group_ids: HashMap<String, String> = HashMap::new();
    let mut imported: Vec<String> = Vec::new();
    for (id, def) in &payload.groups {
        let same = existing.get(id).is_some_and(|current| serde_json::to_value(clean_def(current)).ok() == serde_json::to_value(clean_def(def)).ok());
        let new_id = if same {
            id.clone()
        } else {
            let taken = |candidate: &str| existing.contains_key(candidate) || imported.iter().any(|i| i == candidate);
            let new_id = free_id(id, &taken);
            imported.push(new_id.clone());
            new_id
        };
        group_ids.insert(id.clone(), new_id);
    }
    let rename = |node: &mut RfNode, group_ids: &HashMap<String, String>| {
        if let Some(new_id) = group_of(node).and_then(|id| group_ids.get(id)).cloned() {
            node.data.insert("group_id".to_string(), json!(new_id));
        }
    };
    let mut names: HashSet<String> = existing.values().map(|g| g.name.clone()).collect();
    for (id, def) in &payload.groups {
        let new_id = &group_ids[id];
        if !imported.contains(new_id) {
            continue;
        }
        let mut def = clean_def(def);
        def.name = free_name(&def.name, &names);
        names.insert(def.name.clone());
        def.graph.nodes.iter_mut().for_each(|node| rename(node, &group_ids));
        project.groups.insert(new_id.clone(), def);
    }
    // a group that runs itself never finishes, refuse the lot
    let groups = all_groups(project);
    if imported.iter().any(|id| groups[id].graph.nodes.iter().filter_map(group_of).any(|inner| runs(&groups, inner, id))) {
        for id in &imported {
            project.groups.remove(id);
        }
        return Err("the copied groups contain themselves".to_string());
    }

    // the nodes, with new ids, around `position`
    let specs = Specs {
        specs,
        groups: &groups,
        scope: scope.and_then(|id| groups.get(id)),
    };
    let xs = copied.nodes.iter().map(|n| n.position.x);
    let ys = copied.nodes.iter().map(|n| n.position.y);
    let cx = (xs.clone().fold(f64::INFINITY, f64::min) + xs.fold(f64::NEG_INFINITY, f64::max)) / 2.0;
    let cy = (ys.clone().fold(f64::INFINITY, f64::min) + ys.fold(f64::NEG_INFINITY, f64::max)) / 2.0;
    let graph = target(project, scope)?;
    let mut ids: HashMap<String, String> = HashMap::new();
    for mut node in copied.nodes {
        rename(&mut node, &group_ids);
        let fits = match node.node_type.as_str() {
            GROUP => group_of(&node).is_some_and(|id| groups.contains_key(id) && !scope.is_some_and(|scope| runs(&groups, id, scope))),
            GROUP_INPUT | GROUP_OUTPUT => scope.is_some(),
            node_type => specs.for_type(node_type).is_some(),
        };
        if !fits || ids.contains_key(&node.id) {
            continue;
        }
        let spec = specs.for_node(&node);
        let pasted = RfNode {
            id: graph.next_node_id(id_prefix(&node)),
            node_type: node.node_type.clone(),
            position: Position {
                x: position.x + node.position.x - cx,
                y: position.y + node.position.y - cy,
            },
            data: clean_data(&node, spec.as_deref()),
            // a resized node keeps its size
            extra: node.extra.iter().filter(|(k, v)| (k.as_str() == "width" || k.as_str() == "height") && v.is_number()).map(|(k, v)| (k.clone(), v.clone())).collect(),
        };
        ids.insert(node.id.clone(), pasted.id.clone());
        graph.nodes.push(pasted);
    }
    if ids.is_empty() {
        return Err("none of the copied nodes can go here".to_string());
    }

    // zone ends point at each other's copies, an end whose partner didn't come along isn't paired
    for new_id in ids.values() {
        let node = graph.node_mut(new_id).unwrap();
        if let Some(partner) = zone_of(node).map(str::to_string) {
            match ids.get(&partner) {
                Some(copy) => node.data.insert("zone".to_string(), json!(copy)),
                None => node.data.remove("zone"),
            };
        }
    }
    // the connections between pasted nodes, never twice and never a cycle. more than one into an input was a multi input
    for edge in &copied.edges {
        let (Some(from), Some(to)) = (ids.get(edge.from_node()), ids.get(edge.to_node())) else {
            continue;
        };
        let twice = graph.edges.iter().any(|e| e.to_node() == to && e.to_input() == edge.to_input() && e.from_node() == from && e.from_output() == edge.from_output());
        if from == to || twice || graph.reaches(to, from) {
            continue;
        }
        graph.edges.push(RfEdge::new(from, edge.from_output(), to, edge.to_input()));
    }

    // pasted outputs get tags of their own, pasted inputs whose output stayed behind use the tags here
    tags::adopt(graph, &ids.values().cloned().collect::<Vec<_>>());

    let pasted: Vec<&str> = ids.values().map(String::as_str).collect();
    select(graph, &pasted.iter().copied().collect(), &HashSet::new());
    // in the order they were copied
    let order: Vec<String> = graph.nodes.iter().filter(|n| pasted.contains(&n.id.as_str())).map(|n| n.id.clone()).collect();
    added.extend(order.into_iter().map(|id| Added {
        position: graph.node(&id).unwrap().position.clone(),
        id,
    }));
    Ok(())
}
