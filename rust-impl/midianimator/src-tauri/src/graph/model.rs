// plain data model for the node graph, this doesn't depend on tauri or the MCP server
//
// the graph is stored the same way React Flow's `toObject()` gives it to us (`{nodes, edges, viewport}`).
// these structs only name the fields the backend cares about, everything else
// react flow writes (measured, selected, viewport, ...) gets kept in `extra` so going
// through `Graph` and back doesn't lose anything
//
// EDGE DIRECTION: source and target are backwards from how data flows. in stored edges `source`/`sourceHandle`
// is the node that CONSUMES the data (and its input handle), `target`/`targetHandle` is the node that PRODUCES it (and its output handle).
// use the data-flow accessors on `RfEdge` (`from_node`, `from_output`, `to_node`, `to_input`)
// instead of the raw fields so we don't get it mixed up

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet, VecDeque};

// MARK: - Node Specs (default_nodes.json)

/// one input or output handle on a node, as described in default_nodes.json
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HandleSpec {
    pub id: String,
    pub name: String,
    pub data_type: String,
    #[serde(default)]
    pub description: String,
    /// hidden in the node UI (edited with a widget, or shown inside the node).
    /// hidden handles have no position on the canvas, so they must never be connected
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
}

/// the inputs and outputs of a node spec
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct HandleSpecs {
    #[serde(default)]
    pub inputs: Vec<HandleSpec>,
    #[serde(default)]
    pub outputs: Vec<HandleSpec>,
}

/// a node type, as described in default_nodes.json
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NodeSpec {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub executor: String,
    #[serde(default)]
    pub realtime: bool,
    #[serde(default)]
    pub handles: HandleSpecs,
}

impl NodeSpec {
    /// finds an input handle on this node spec by id
    pub fn input(&self, id: &str) -> Option<&HandleSpec> {
        self.handles.inputs.iter().find(|h| h.id == id)
    }

    /// finds an output handle on this node spec by id
    pub fn output(&self, id: &str) -> Option<&HandleSpec> {
        self.handles.outputs.iter().find(|h| h.id == id)
    }
}

/// parses the `default_nodes` map held in `AppState` (the contents of default_nodes.json)
pub fn node_specs(default_nodes: &HashMap<String, Value>) -> Vec<NodeSpec> {
    default_nodes.get("nodes").and_then(|nodes| serde_json::from_value(nodes.clone()).ok()).unwrap_or_default()
}

/// finds the spec for a node type (e.g. `get_midi_file`)
pub fn find_spec<'a>(specs: &'a [NodeSpec], node_type: &str) -> Option<&'a NodeSpec> {
    specs.iter().find(|spec| spec.id == node_type)
}

/// inputs whose handle is hidden in the UI are parameters: set with a widget (graph_set_inputs), never connected
pub fn is_param(spec: &NodeSpec, input_id: &str) -> bool {
    spec.input(input_id).is_some_and(|h| h.hidden)
}

/// all outputs of a node: the declared ones plus dynamic ones found in its executed results
///
/// a declared `Dyn<T>` output (keyframes_from_object's `dyn_output`) holds a map whose keys
/// become extra outputs of type `T` once the node has executed (e.g. `location_z`).
pub fn node_outputs(spec: &NodeSpec, node_results: Option<&Value>) -> Vec<HandleSpec> {
    // start with the declared outputs
    let mut outputs = spec.handles.outputs.clone();
    // then look for any `Dyn<T>` outputs, those are the only ones that can add more
    for handle in &spec.handles.outputs {
        let Some(inner) = handle.data_type.strip_prefix("Dyn<").and_then(|t| t.strip_suffix('>')) else {
            continue;
        };
        // dynamic outputs only exist once the node has executed and returned a map for that handle
        let Some(dynamic) = node_results.and_then(|r| r.get(&handle.id)).and_then(|v| v.as_object()) else {
            continue;
        };
        // each key in the map becomes its own output, e.g. `location_z` -> "Location Z"
        for key in dynamic.keys() {
            outputs.push(HandleSpec {
                id: key.clone(),
                name: key.split('_').map(capitalize).collect::<Vec<_>>().join(" "),
                data_type: inner.to_string(),
                description: format!("Dynamic output of {}.", handle.id),
                hidden: false,
            });
        }
    }
    outputs
}

/// uppercases the first letter of a word
fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// whether an output of type `out_ty` can feed an input of type `in_ty`
pub fn compatible(out_ty: &str, in_ty: &str) -> bool {
    out_ty == in_ty || in_ty == "Any" || out_ty.starts_with("Dyn<")
}

// MARK: - Type Schemas

/// strips container types down to the element type: `Array<T>`, `Dyn<T>` -> `T`, `HashMap<K, V>` -> `V`
pub fn base_type(data_type: &str) -> &str {
    let mut data_type = data_type.trim();
    loop {
        if let Some(inner) = data_type.strip_prefix("Array<").or_else(|| data_type.strip_prefix("Dyn<")).and_then(|t| t.strip_suffix('>')) {
            data_type = inner.trim();
        } else if let Some(inner) = data_type.strip_prefix("HashMap<").and_then(|t| t.strip_suffix('>')) {
            data_type = inner.split_once(',').map_or(inner, |(_, value)| value).trim();
        } else {
            return data_type;
        }
    }
}

/// JSON schema (pretty printed) for a node handle type name, e.g. `Array<MIDINote>`
pub fn describe_type(data_type: &str) -> Result<String, String> {
    use crate::midi::{MIDIEvent, MIDINote, MIDITrack};
    use crate::scene_generics::{AnimCurve, Keyframe, Object, ObjectGroup, Scene};
    use crate::utils::animation::{AnimationGenerator, BlendKeyframe, ObjectMap};

    // strip the wrappers so we can look up the struct
    let base = base_type(data_type);
    // pick the schema for the base type, some types come with an extra note
    let (schema, note) = match base {
        "MIDINote" => (schemars::schema_for!(MIDINote), None),
        "MIDIEvent" => (schemars::schema_for!(MIDIEvent), None),
        "MIDITrack" => (schemars::schema_for!(MIDITrack), None),
        "ObjectGroup" => (schemars::schema_for!(ObjectGroup), None),
        "Object" => (schemars::schema_for!(Object), None),
        "Scene" => (schemars::schema_for!(Scene), None),
        "AnimCurve" => (schemars::schema_for!(AnimCurve), None),
        // handles typed Array<Keyframe> currently carry one Blender F-curve, not a list of `Keyframe`s
        "Keyframe" => (schemars::schema_for!(AnimCurve), Some(format!("note: values typed {} are currently a single Blender F-curve (AnimCurve) object; its keyframe_points[].co is [frame, value]. The plain Keyframe struct is: {}", data_type, serde_json::to_string(&schemars::schema_for!(Keyframe)).unwrap_or_default()))),
        "BlendKeyframe" => (schemars::schema_for!(BlendKeyframe), None),
        "AnimationGenerator" => (schemars::schema_for!(AnimationGenerator), None),
        "ObjectMap" => (schemars::schema_for!(ObjectMap), None),
        // primitives don't have a schema, just describe them
        "String" | "f64" | "u8" | "Any" => {
            return Ok(format!(
                "{} is a plain JSON value ({})",
                data_type,
                if base == "Any" {
                    "any type"
                } else {
                    base
                }
            ))
        }
        // unknown type, list the ones we do know
        _ => return Err(format!("unknown type '{}'; known types: MIDINote, MIDIEvent, MIDITrack, ObjectGroup, Object, Scene, Keyframe, AnimCurve, BlendKeyframe, AnimationGenerator, ObjectMap (optionally wrapped in Array<...>, Dyn<...> or HashMap<K, ...>)", data_type)),
    };

    // build the text, mention the wrapper type if there was one
    let mut text = String::new();
    if base != data_type {
        text.push_str(&format!("{} wraps {}; schema of {}:\n", data_type, base, base));
    }
    text.push_str(&serde_json::to_string_pretty(&schema).unwrap_or_default());
    // add the extra note at the end
    if let Some(note) = note {
        text.push_str("\n");
        text.push_str(&note);
    }
    Ok(text)
}

// MARK: - Graph

// a node's position on the canvas (plain comment so it doesn't end up in the MCP tool schema)
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, schemars::JsonSchema)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

/// a node as React Flow stores it
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RfNode {
    pub id: String,
    #[serde(rename = "type", default)]
    pub node_type: String,
    #[serde(default)]
    pub position: Position,
    #[serde(default)]
    pub data: Map<String, Value>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl RfNode {
    /// node type, from the `type` field or the id prefix (`{type}-{something}`)
    pub fn resolved_node_type(&self) -> &str {
        if self.node_type.is_empty() {
            self.id.split('-').next().unwrap_or("")
        } else {
            &self.node_type
        }
    }

    /// values set on the node itself (not coming from a connection)
    pub fn inputs(&self) -> Option<&Map<String, Value>> {
        self.data.get("inputs").and_then(|v| v.as_object())
    }

    /// a single value set on the node itself (not coming from a connection)
    pub fn input_value(&self, id: &str) -> Option<&Value> {
        self.inputs().and_then(|inputs| inputs.get(id))
    }

    /// the node's `inputs` map, created if it's missing (or replaced if it isn't an object)
    pub fn inputs_mut(&mut self) -> &mut Map<String, Value> {
        // make sure `inputs` exists and is an object before we hand it back
        let inputs = self.data.entry("inputs").or_insert_with(|| Value::Object(Map::new()));
        if !inputs.is_object() {
            *inputs = Value::Object(Map::new());
        }
        inputs.as_object_mut().unwrap()
    }
}

/// an edge as React Flow stores it
///
/// note: `source`/`target` are reversed from the data flow, see EDGE DIRECTION at the top of the file
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RfEdge {
    #[serde(default)]
    pub id: String,
    pub source: String,
    #[serde(rename = "sourceHandle", default)]
    pub source_handle: Option<String>,
    pub target: String,
    #[serde(rename = "targetHandle", default)]
    pub target_handle: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl RfEdge {
    /// builds an edge in data-flow terms, translating to the stored (reversed) direction
    pub fn new(from_node: &str, from_output: &str, to_node: &str, to_input: &str) -> Self {
        // the id matches the format React Flow uses when you connect handles in the UI
        Self {
            id: format!("xy-edge__{}{}-{}{}", to_node, to_input, from_node, from_output),
            source: to_node.to_string(),
            source_handle: Some(to_input.to_string()),
            target: from_node.to_string(),
            target_handle: Some(from_output.to_string()),
            extra: Map::new(),
        }
    }

    /// producing node
    pub fn from_node(&self) -> &str {
        &self.target
    }

    /// output handle on the producing node
    pub fn from_output(&self) -> &str {
        self.target_handle.as_deref().unwrap_or("")
    }

    /// consuming node
    pub fn to_node(&self) -> &str {
        &self.source
    }

    /// input handle on the consuming node
    pub fn to_input(&self) -> &str {
        self.source_handle.as_deref().unwrap_or("")
    }
}

/// the whole node graph, `extra` keeps the viewport and anything else React Flow stores
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Graph {
    #[serde(default)]
    pub nodes: Vec<RfNode>,
    #[serde(default)]
    pub edges: Vec<RfEdge>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Graph {
    /// reads the graph out of the `rf_instance` map in `AppState`
    pub fn from_rf(rf_instance: &HashMap<String, Value>) -> Result<Self, String> {
        let value = Value::Object(rf_instance.clone().into_iter().collect());
        serde_json::from_value(value).map_err(|e| format!("could not read the node graph: {}", e))
    }

    /// turns the graph back into an `rf_instance` map for `AppState`
    pub fn to_rf(&self) -> HashMap<String, Value> {
        match serde_json::to_value(self) {
            Ok(Value::Object(map)) => map.into_iter().collect(),
            _ => HashMap::new(),
        }
    }

    /// finds a node by its exact id
    pub fn node(&self, id: &str) -> Option<&RfNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// finds a node by its exact id (mutable)
    pub fn node_mut(&mut self, id: &str) -> Option<&mut RfNode> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    /// the edge feeding `to_input` on `to_node`, if any
    pub fn edge_into(&self, to_node: &str, to_input: &str) -> Option<&RfEdge> {
        self.edges.iter().find(|e| e.to_node() == to_node && e.to_input() == to_input)
    }

    /// edges leaving `from_node` (optionally only from one output)
    pub fn edges_from<'a>(&'a self, from_node: &'a str, from_output: Option<&'a str>) -> impl Iterator<Item = &'a RfEdge> + 'a {
        self.edges.iter().filter(move |e| e.from_node() == from_node && from_output.map_or(true, |o| e.from_output() == o))
    }

    /// resolves a node id or a unique prefix of one
    pub fn resolve(&self, query: &str) -> Result<String, String> {
        // an exact match always wins
        let query = query.trim();
        if self.node(query).is_some() {
            return Ok(query.to_string());
        }
        // otherwise look for ids that start with the query
        let matches: Vec<&str> = self.nodes.iter().map(|n| n.id.as_str()).filter(|id| id.starts_with(query)).collect();
        match matches.len() {
            // only one match, use it
            1 => Ok(matches[0].to_string()),
            // no match, list the node ids so the caller can pick one
            0 => {
                let ids: Vec<&str> = self.nodes.iter().map(|n| n.id.as_str()).collect();
                if ids.is_empty() {
                    Err(format!("no node '{}'; the graph is empty", query))
                } else {
                    Err(format!("no node '{}'; nodes are: {}", query, ids.join(", ")))
                }
            }
            // more than one match, the prefix is too short
            _ => Err(format!("'{}' matches several nodes ({}); use a longer prefix", query, matches.join(", "))),
        }
    }

    /// next short id for a node type: `{type}-{N}` with N = highest existing N + 1
    pub fn next_node_id(&self, node_type: &str) -> String {
        // find the highest N already used for this type, ignoring old `{type}-{uuid}` ids
        let prefix = format!("{}-", node_type);
        let max = self.nodes.iter().filter_map(|n| n.id.strip_prefix(&prefix)).filter_map(|rest| rest.parse::<u64>().ok()).max().unwrap_or(0);
        format!("{}{}", prefix, max + 1)
    }

    /// true if data can flow from `start` to `goal` along existing edges
    pub fn reaches(&self, start: &str, goal: &str) -> bool {
        // breadth first search along the data flow
        let mut seen = HashSet::new();
        let mut queue = VecDeque::from([start.to_string()]);
        while let Some(id) = queue.pop_front() {
            if id == goal {
                return true;
            }
            // skip nodes we have already visited (this also stops cycles from looping forever)
            if !seen.insert(id.clone()) {
                continue;
            }
            // queue up every node this one feeds into
            for edge in self.edges_from(&id, None) {
                queue.push_back(edge.to_node().to_string());
            }
        }
        false
    }

    /// node ids ordered so producers come before consumers; ties keep the stored order
    pub fn topo_order(&self) -> Vec<String> {
        // count how many incoming edges each node has, ignoring edges to nodes that don't exist
        let ids: Vec<&str> = self.nodes.iter().map(|n| n.id.as_str()).collect();
        let known: HashSet<&str> = ids.iter().copied().collect();
        let mut indegree: HashMap<&str, usize> = ids.iter().map(|id| (*id, 0)).collect();
        for edge in &self.edges {
            if known.contains(edge.from_node()) && known.contains(edge.to_node()) {
                *indegree.get_mut(edge.to_node()).unwrap() += 1;
            }
        }

        // keep picking nodes until every node is in the order
        let mut order: Vec<String> = Vec::new();
        let mut done: HashSet<&str> = HashSet::new();
        while order.len() < ids.len() {
            // first node (in stored order) with no pending inputs, fall back to any remaining node if there is a cycle
            let next = ids.iter().find(|id| !done.contains(**id) && indegree[**id] == 0).or_else(|| ids.iter().find(|id| !done.contains(**id))).copied().unwrap();
            done.insert(next);
            order.push(next.to_string());
            // this node is done, so the nodes it feeds have one less input to wait on
            for edge in self.edges_from(next, None) {
                if let Some(d) = indegree.get_mut(edge.to_node()) {
                    *d = d.saturating_sub(1);
                }
            }
        }
        order
    }
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;

    // checks which output types can connect to which input types
    #[test]
    fn compatible_types() {
        assert!(compatible("Array<MIDINote>", "Array<MIDINote>"));
        assert!(compatible("Array<MIDINote>", "Any"));
        assert!(compatible("Dyn<Array<Keyframe>>", "Array<Keyframe>"));
        assert!(!compatible("Array<MIDITrack>", "Array<MIDINote>"));
        assert!(!compatible("Any", "String"));
    }

    // checks that wrapper types get stripped and that schemas are found (or errors for unknown types)
    #[test]
    fn base_types_and_schemas() {
        assert_eq!(base_type("Array<MIDINote>"), "MIDINote");
        assert_eq!(base_type("Dyn<Array<Keyframe>>"), "Keyframe");
        assert_eq!(base_type("HashMap<String, Array<BlendKeyframe>>"), "BlendKeyframe");
        assert!(describe_type("Array<MIDINote>").unwrap().contains("note_number"));
        assert!(describe_type("Array<Keyframe>").unwrap().contains("keyframe_points"));
        assert!(describe_type("Nope").unwrap_err().contains("known types"));
    }

    // checks that data-flow terms get stored in the reversed source/target direction
    #[test]
    fn edge_direction_accessors() {
        // get_midi_file's tracks output feeds get_midi_track_data's tracks input
        let edge = RfEdge::new("get_midi_file-1", "tracks", "get_midi_track_data-1", "tracks");
        assert_eq!(edge.source, "get_midi_track_data-1");
        assert_eq!(edge.target, "get_midi_file-1");
        assert_eq!(edge.from_node(), "get_midi_file-1");
        assert_eq!(edge.to_input(), "tracks");
        assert_eq!(edge.id, "xy-edge__get_midi_track_data-1tracks-get_midi_file-1tracks");
    }

    #[test]
    // checks that the next id counts up from existing numbered ids and skips uuid ids
    fn next_id_ignores_uuids() {
        let mut graph = Graph::default();
        // empty graph starts at 1
        assert_eq!(graph.next_node_id("viewer"), "viewer-1");
        // only `viewer-3` counts, the uuid id and the `viewer_x` type are ignored
        for id in ["viewer-3", "viewer-14bc399f-f4e2-48ec-9103-7bae2a0ca62f", "viewer_x-9"] {
            graph.nodes.push(RfNode {
                id: id.to_string(),
                node_type: "viewer".to_string(),
                position: Position::default(),
                data: Map::new(),
                extra: Map::new(),
            });
        }
        assert_eq!(graph.next_node_id("viewer"), "viewer-4");
    }
}
