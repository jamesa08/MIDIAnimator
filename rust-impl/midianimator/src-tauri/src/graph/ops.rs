// edits from the node editor, applied by `graph_apply` (src/state/graph.rs) to the graph open in the editor: the top
// level or a node group. the ops in one call are applied together, if one fails nothing changes

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap, HashSet};

use super::builtin::{all_groups, builtin_groups};
use super::edit;
use super::model::{Graph, GroupDef, HandleSpec, NodeSpec, Position, RfEdge, RfNode, Specs};
use super::run::{FOR_EACH_INPUT, FOR_EACH_OUTPUT, GROUP, GROUP_INPUT, GROUP_OUTPUT};

/// the empty socket on the group input and output, connecting to it adds a socket to the group. keep in sync with NEW_SOCKET in groups.ts
pub const NEW_SOCKET: &str = "__new__";
/// how far right of a for each input its output is added
const ZONE_WIDTH: f64 = 450.0;
/// width of a node that hasn't been measured, for laying out a new group
const NODE_WIDTH: f64 = 200.0;
/// node fields only the UI uses, left out of nodes moved into a new group
const UI_KEYS: &[&str] = &["selected", "dragging", "measured", "resizing"];

/// a node to add: its type, data and where it goes. a group node is type `group` with `data.group_id`
#[derive(Deserialize, Debug, Clone)]
pub struct NewNode {
    #[serde(rename = "type")]
    pub node_type: String,
    #[serde(default)]
    pub data: Map<String, Value>,
    pub position: Position,
}

/// the sockets of a group: the inputs it takes or the outputs it gives
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Inputs,
    Outputs,
}

/// one edit. node ids are exact, connections are in data-flow terms (from an output to an input)
#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    /// adds nodes, a for each input brings its output along. the new nodes become the selection
    AddNodes {
        nodes: Vec<NewNode>,
    },
    /// removes nodes with their connections (a zone as a pair) and edges
    Delete {
        #[serde(default)]
        nodes: Vec<String>,
        #[serde(default)]
        edges: Vec<String>,
    },
    /// connects an output to an input, replacing what fed the input. `__new__` on the group input or output adds a group socket
    Connect {
        from_node: String,
        from_output: String,
        to_node: String,
        to_input: String,
    },
    /// merges values into a node's inputs, `null` unsets one
    SetInputs {
        node: String,
        inputs: Map<String, Value>,
    },
    Move {
        positions: BTreeMap<String, Position>,
    },
    Resize {
        node: String,
        width: f64,
        height: f64,
        position: Position,
    },
    /// the selected nodes and edges, everything else is deselected
    Select {
        #[serde(default)]
        nodes: Vec<String>,
        #[serde(default)]
        edges: Vec<String>,
    },
    /// copies nodes (a zone as a pair) and the connections between them, the copies become the selection
    Duplicate {
        nodes: Vec<String>,
        offset: Position,
    },
    /// moves nodes into a new group, a group node takes their place. `widths` are the nodes' drawn widths, for the layout
    Group {
        nodes: Vec<String>,
        #[serde(default)]
        widths: HashMap<String, f64>,
    },
    /// replaces group nodes with the nodes inside their groups
    Ungroup {
        nodes: Vec<String>,
    },
    RenameSocket {
        side: Side,
        id: String,
        name: String,
    },
    /// removes a socket of the group, with every connection to it inside and outside
    RemoveSocket {
        side: Side,
        id: String,
    },
    /// copies the built-in group being edited into the project
    MakeLocal,
    /// drops the project's copy of a built-in group
    RevertGroup,
    /// where the graph is looked at, never an undo step
    Viewport {
        viewport: Value,
    },
}

impl Op {
    /// the op's name, used as the name of its undo step
    pub fn name(&self) -> &'static str {
        match self {
            Op::AddNodes {
                ..
            } => "add_nodes",
            Op::Delete {
                ..
            } => "delete",
            Op::Connect {
                ..
            } => "connect",
            Op::SetInputs {
                ..
            } => "set_inputs",
            Op::Move {
                ..
            } => "move",
            Op::Resize {
                ..
            } => "resize",
            Op::Select {
                ..
            } => "select",
            Op::Duplicate {
                ..
            } => "duplicate",
            Op::Group {
                ..
            } => "group",
            Op::Ungroup {
                ..
            } => "ungroup",
            Op::RenameSocket {
                ..
            } => "rename_socket",
            Op::RemoveSocket {
                ..
            } => "remove_socket",
            Op::MakeLocal => "make_local",
            Op::RevertGroup => "revert_group",
            Op::Viewport {
                ..
            } => "viewport",
        }
    }
}

/// a node an op added, so the editor can pick it up (grab it after adding or duplicating)
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Added {
    pub id: String,
    pub position: Position,
}

/// what applying an op needs besides the project: the node specs and the last run's results in the edited graph
/// (for dynamic outputs)
pub struct Ctx<'a> {
    pub specs: &'a [NodeSpec],
    pub results: &'a HashMap<String, Value>,
}

/// applies one op to the graph `scope` (a group id, `None` for the top level). nodes it added go in `added`
pub fn apply(project: &mut Graph, scope: Option<&str>, op: &Op, ctx: &Ctx, added: &mut Vec<Added>) -> Result<(), String> {
    // ops on the group itself
    match op {
        Op::MakeLocal => return make_local(project, scope),
        Op::RevertGroup => return revert_group(project, scope),
        Op::RenameSocket {
            side,
            id,
            name,
        } => return rename_socket(project, scope, *side, id, name),
        Op::RemoveSocket {
            side,
            id,
        } => return remove_socket(project, scope, *side, id),
        // a built-in group's view isn't kept, that would make it the project's own
        Op::Viewport {
            viewport,
        } => {
            if let Ok(graph) = target(project, scope) {
                graph.extra.insert("viewport".to_string(), viewport.clone());
            }
            return Ok(());
        }
        _ => {}
    }

    // the specs of the nodes in the edited graph, group input and output get their sockets from the group
    let groups = all_groups(project);
    target(project, scope)?;
    let specs = Specs {
        specs: ctx.specs,
        groups: &groups,
        scope: scope.and_then(|id| groups.get(id)),
    };

    match op {
        Op::AddNodes {
            nodes,
        } => add_nodes(target(project, scope)?, &specs, scope, nodes, added),
        Op::Delete {
            nodes,
            edges,
        } => {
            delete(target(project, scope)?, nodes, edges);
            Ok(())
        }
        Op::Connect {
            from_node,
            from_output,
            to_node,
            to_input,
        } => connect(project, scope, &specs, from_node, from_output, to_node, to_input),
        Op::SetInputs {
            node,
            inputs,
        } => edit::set_inputs(target(project, scope)?, &specs, node, inputs).map(|_| ()),
        Op::Move {
            positions,
        } => {
            let graph = target(project, scope)?;
            for (id, position) in positions {
                if let Some(node) = graph.node_mut(id) {
                    node.position = position.clone();
                }
            }
            Ok(())
        }
        Op::Resize {
            node,
            width,
            height,
            position,
        } => {
            let node = target(project, scope)?.node_mut(node).ok_or_else(|| format!("no node '{}'", node))?;
            node.extra.insert("width".to_string(), json!(width));
            node.extra.insert("height".to_string(), json!(height));
            node.position = position.clone();
            Ok(())
        }
        Op::Select {
            nodes,
            edges,
        } => {
            let graph = target(project, scope)?;
            select(graph, &nodes.iter().map(String::as_str).collect(), &edges.iter().map(String::as_str).collect());
            Ok(())
        }
        Op::Duplicate {
            nodes,
            offset,
        } => duplicate(target(project, scope)?, nodes, offset, added),
        Op::Group {
            nodes,
            widths,
        } => make_group(project, scope, &specs, nodes, widths, added),
        Op::Ungroup {
            nodes,
        } => ungroup(project, scope, &groups, nodes, added),
        Op::MakeLocal
        | Op::RevertGroup
        | Op::RenameSocket {
            ..
        }
        | Op::RemoveSocket {
            ..
        }
        | Op::Viewport {
            ..
        } => unreachable!(),
    }
}

// MARK: - Helpers

/// the graph `scope` names: the top level, or one of the project's own groups. a built-in group is read-only until it's made local
fn target<'a>(project: &'a mut Graph, scope: Option<&str>) -> Result<&'a mut Graph, String> {
    match scope {
        None => Ok(project),
        Some(id) => match project.groups.get_mut(id) {
            Some(def) => Ok(&mut def.graph),
            None if builtin_groups().contains_key(id) => Err(format!("'{}' is a built-in group, make it local to edit it", id)),
            None => Err(format!("no group '{}'", id)),
        },
    }
}

/// the project's own copy of the group `scope`
fn local_group<'a>(project: &'a mut Graph, scope: Option<&str>) -> Result<&'a mut GroupDef, String> {
    let id = scope.ok_or("only a node group has sockets")?;
    project.groups.get_mut(id).ok_or_else(|| format!("'{}' is not one of the project's groups, make it local to edit it", id))
}

fn set_selected(extra: &mut Map<String, Value>, selected: bool) {
    if selected {
        extra.insert("selected".to_string(), Value::Bool(true));
    } else {
        extra.remove("selected");
    }
}

/// selects exactly these nodes and edges
fn select(graph: &mut Graph, nodes: &HashSet<&str>, edges: &HashSet<&str>) {
    for node in &mut graph.nodes {
        set_selected(&mut node.extra, nodes.contains(node.id.as_str()));
    }
    for edge in &mut graph.edges {
        set_selected(&mut edge.extra, edges.contains(edge.id.as_str()));
    }
}

/// what new ids for a node start with: group nodes are named after their group (`evaluate_instrument-2`)
fn id_prefix(node: &RfNode) -> &str {
    match node.data.get("group_id").and_then(Value::as_str) {
        Some(group_id) if node.node_type == GROUP => group_id,
        _ => node.resolved_node_type(),
    }
}

/// the next `{prefix}-{N}` not in `taken`, like Graph::next_node_id
fn next_free_id<'a>(taken: impl Iterator<Item = &'a str>, prefix: &str) -> String {
    let start = format!("{}-", prefix);
    let max = taken.filter_map(|id| id.strip_prefix(&start)).filter_map(|rest| rest.parse::<u64>().ok()).max().unwrap_or(0);
    format!("{}{}", start, max + 1)
}

/// the node's zone partner, the other end of a for each zone
fn zone_of(node: &RfNode) -> Option<&str> {
    node.data.get("zone").and_then(Value::as_str)
}

/// the ids plus the other end of every zone among them, half a zone can't run
fn with_zone_partners(graph: &Graph, ids: &[String]) -> HashSet<String> {
    let mut all: HashSet<String> = ids.iter().cloned().collect();
    for node in &graph.nodes {
        if all.contains(&node.id) {
            if let Some(partner) = zone_of(node) {
                all.insert(partner.to_string());
            }
        }
    }
    all
}

/// an edge like `base` (keeping its extra fields) between new ends
fn edge_like(base: &RfEdge, from_node: &str, from_output: &str, to_node: &str, to_input: &str) -> RfEdge {
    let mut edge = RfEdge::new(from_node, from_output, to_node, to_input);
    edge.extra = base.extra.clone();
    edge.extra.remove("selected");
    edge
}

// MARK: - Nodes

fn add_nodes(graph: &mut Graph, specs: &Specs, scope: Option<&str>, nodes: &[NewNode], added: &mut Vec<Added>) -> Result<(), String> {
    let mut ids: Vec<String> = Vec::new();
    for new in nodes {
        // a group id as the type is a group node running it
        let (node_type, mut data) = if specs.groups.contains_key(&new.node_type) {
            (GROUP.to_string(), Map::from_iter([("group_id".to_string(), json!(new.node_type))]))
        } else {
            (new.node_type.clone(), new.data.clone())
        };
        let mut node = RfNode {
            id: String::new(),
            node_type,
            position: new.position.clone(),
            data: Map::new(),
            extra: Map::new(),
        };
        node.data.append(&mut data);

        // check what it is and that it can go here
        match node.node_type.as_str() {
            GROUP => {
                let group_id = node.data.get("group_id").and_then(Value::as_str).unwrap_or("");
                if !specs.groups.contains_key(group_id) {
                    return Err(format!("no group '{}'", group_id));
                }
                if Some(group_id) == scope {
                    return Err("a group can't contain itself".to_string());
                }
            }
            GROUP_INPUT | GROUP_OUTPUT if scope.is_none() => return Err("the group input and output only go inside a group".to_string()),
            node_type if specs.for_type(node_type).is_none() => return Err(format!("unknown node type '{}'", node_type)),
            _ => {}
        }

        node.id = graph.next_node_id(id_prefix(&node));
        node.inputs_mut();
        ids.push(node.id.clone());
        let pair = node.node_type == FOR_EACH_INPUT && zone_of(&node).is_none();
        let input_id = node.id.clone();
        graph.nodes.push(node);

        // a zone is always added as a pair, each end points at the other
        if pair {
            let output_id = graph.next_node_id(FOR_EACH_OUTPUT);
            graph.nodes.push(RfNode {
                id: output_id.clone(),
                node_type: FOR_EACH_OUTPUT.to_string(),
                position: Position {
                    x: new.position.x + ZONE_WIDTH,
                    y: new.position.y,
                },
                data: Map::from_iter([("zone".to_string(), json!(input_id)), ("inputs".to_string(), json!({}))]),
                extra: Map::new(),
            });
            graph.node_mut(&input_id).unwrap().data.insert("zone".to_string(), json!(output_id));
            ids.push(output_id);
        }
    }

    // the new nodes become the selection, like blender
    select(graph, &ids.iter().map(String::as_str).collect(), &HashSet::new());
    added.extend(ids.iter().map(|id| Added {
        id: id.clone(),
        position: graph.node(id).unwrap().position.clone(),
    }));
    Ok(())
}

fn delete(graph: &mut Graph, nodes: &[String], edges: &[String]) {
    let nodes = with_zone_partners(graph, nodes);
    let edges: HashSet<&String> = edges.iter().collect();
    graph.nodes.retain(|n| !nodes.contains(&n.id));
    graph.edges.retain(|e| !edges.contains(&e.id) && !nodes.contains(e.from_node()) && !nodes.contains(e.to_node()));
}

fn duplicate(graph: &mut Graph, nodes: &[String], offset: &Position, added: &mut Vec<Added>) -> Result<(), String> {
    let selected = with_zone_partners(graph, nodes);
    let originals: Vec<RfNode> = graph.nodes.iter().filter(|n| selected.contains(&n.id)).cloned().collect();
    if originals.is_empty() {
        return Err("nothing to duplicate".to_string());
    }

    // copies get the next free id, each added before the next one is named
    let mut ids: HashMap<String, String> = HashMap::new();
    for node in &originals {
        let mut copy = node.clone();
        copy.id = graph.next_node_id(id_prefix(node));
        copy.position = Position {
            x: node.position.x + offset.x,
            y: node.position.y + offset.y,
        };
        ids.insert(node.id.clone(), copy.id.clone());
        graph.nodes.push(copy);
    }
    // copied zone ends point at each other's copies, an end whose partner is missing isn't paired
    for new_id in ids.values() {
        let node = graph.node_mut(new_id).unwrap();
        if let Some(partner) = zone_of(node).map(str::to_string) {
            match ids.get(&partner) {
                Some(copy) => node.data.insert("zone".to_string(), json!(copy)),
                None => node.data.remove("zone"),
            };
        }
    }
    // the connections between copied nodes
    let edges: Vec<RfEdge> = graph.edges.iter().filter(|e| ids.contains_key(e.from_node()) && ids.contains_key(e.to_node())).map(|e| edge_like(e, &ids[e.from_node()], e.from_output(), &ids[e.to_node()], e.to_input())).collect();
    graph.edges.extend(edges);

    let copies: Vec<&str> = originals.iter().map(|n| ids[&n.id].as_str()).collect();
    select(graph, &copies.iter().copied().collect(), &HashSet::new());
    added.extend(copies.iter().map(|id| Added {
        id: id.to_string(),
        position: graph.node(id).unwrap().position.clone(),
    }));
    Ok(())
}

// MARK: - Connect

fn connect(project: &mut Graph, scope: Option<&str>, specs: &Specs, from_node: &str, from_output: &str, to_node: &str, to_input: &str) -> Result<(), String> {
    let (mut from_output, mut to_input) = (from_output.to_string(), to_input.to_string());

    // the group input's empty socket adds a group input, the group output's a group output, named and typed after the other end
    if from_output == NEW_SOCKET || to_input == NEW_SOCKET {
        let graph = target(project, scope)?;
        let side = if from_output == NEW_SOCKET {
            Side::Inputs
        } else {
            Side::Outputs
        };
        let other = graph
            .node(if side == Side::Inputs {
                to_node
            } else {
                from_node
            })
            .ok_or("no node at the other end")?;
        let handle = match side {
            Side::Inputs => input_handle(specs, other, &to_input),
            Side::Outputs => output_handle(specs, other, &from_output),
        };
        let def = local_group(project, scope)?;
        let sockets = sockets_mut(def, side);
        let id = socket_id(&handle.name, sockets);
        sockets.push(HandleSpec {
            id: id.clone(),
            ..handle
        });
        match side {
            Side::Inputs => from_output = id,
            Side::Outputs => to_input = id,
        }
    }

    link(target(project, scope)?, from_node, &from_output, to_node, &to_input)
}

/// connects an output to an input, replacing what fed the input. only checks what would break the graph (missing
/// nodes, cycles), a connection between types that don't match is allowed and shown as a bad connection
fn link(graph: &mut Graph, from_node: &str, from_output: &str, to_node: &str, to_input: &str) -> Result<(), String> {
    if graph.node(from_node).is_none() || graph.node(to_node).is_none() {
        return Err("one end of the connection is missing".to_string());
    }
    if from_node == to_node {
        return Err("cannot connect a node to itself".to_string());
    }
    if graph.reaches(to_node, from_node) {
        return Err(format!("connecting {} -> {} would create a cycle", from_node, to_node));
    }
    graph.edges.retain(|e| !(e.to_node() == to_node && e.to_input() == to_input));
    graph.edges.push(RfEdge::new(from_node, from_output, to_node, to_input));
    Ok(())
}

/// the spec of one socket of a node: `Dyn<T>` inputs (object_maps_0, ...) and outputs (location_z, ...) fall back to their
/// base, typed `T`. unknown ones are `Any`. keep in sync with inputHandle/outputHandle in groups.ts (edge colors)
fn find_handle(handles: &[HandleSpec], id: &str) -> HandleSpec {
    let is_dyn = |h: &&HandleSpec| h.data_type.starts_with("Dyn<");
    let found = handles.iter().find(|h| h.id == id).or_else(|| handles.iter().filter(is_dyn).find(|h| id.starts_with(&format!("{}_", h.id)))).or_else(|| handles.iter().find(is_dyn));
    match found {
        Some(h) => HandleSpec {
            data_type: h.data_type.strip_prefix("Dyn<").and_then(|t| t.strip_suffix('>')).unwrap_or(&h.data_type).to_string(),
            hidden: false,
            ..h.clone()
        },
        None => HandleSpec {
            id: id.to_string(),
            name: id.to_string(),
            data_type: "Any".to_string(),
            description: String::new(),
            hidden: false,
        },
    }
}

fn input_handle(specs: &Specs, node: &RfNode, id: &str) -> HandleSpec {
    find_handle(specs.for_node(node).map(|s| s.handles.inputs.clone()).unwrap_or_default().as_slice(), id)
}

fn output_handle(specs: &Specs, node: &RfNode, id: &str) -> HandleSpec {
    find_handle(specs.for_node(node).map(|s| s.handles.outputs.clone()).unwrap_or_default().as_slice(), id)
}

// MARK: - Group Sockets

fn sockets_mut(def: &mut GroupDef, side: Side) -> &mut Vec<HandleSpec> {
    match side {
        Side::Inputs => &mut def.interface.inputs,
        Side::Outputs => &mut def.interface.outputs,
    }
}

/// a socket id from a name that isn't taken yet, `Object Map` -> `object_map`, `object_map_2`, ...
pub fn socket_id(name: &str, taken: &[HandleSpec]) -> String {
    let lower = name.to_lowercase();
    let base: String = lower.split(|c: char| !c.is_ascii_alphanumeric()).filter(|part| !part.is_empty()).collect::<Vec<_>>().join("_");
    let base = if base.is_empty() {
        "socket".to_string()
    } else {
        base
    };
    let taken: HashSet<&str> = taken.iter().map(|h| h.id.as_str()).collect();
    if !taken.contains(base.as_str()) && base != NEW_SOCKET {
        return base;
    }
    (2..).map(|n| format!("{}_{}", base, n)).find(|id| !taken.contains(id.as_str())).unwrap()
}

fn rename_socket(project: &mut Graph, scope: Option<&str>, side: Side, id: &str, name: &str) -> Result<(), String> {
    let socket = sockets_mut(local_group(project, scope)?, side).iter_mut().find(|h| h.id == id).ok_or_else(|| format!("no socket '{}'", id))?;
    socket.name = name.to_string();
    Ok(())
}

/// removes a socket, its connections go with it: inside on the group input or output, outside on every group node running the group
fn remove_socket(project: &mut Graph, scope: Option<&str>, side: Side, id: &str) -> Result<(), String> {
    let sockets = sockets_mut(local_group(project, scope)?, side);
    sockets.retain(|h| h.id != id);
    let group_id = scope.unwrap();

    // inside: the group's inputs come out of the group input, its outputs go into the group output
    let inside = |graph: &mut Graph| {
        let ends: HashSet<String> = graph
            .nodes
            .iter()
            .filter(|n| {
                n.node_type
                    == if side == Side::Inputs {
                        GROUP_INPUT
                    } else {
                        GROUP_OUTPUT
                    }
            })
            .map(|n| n.id.clone())
            .collect();
        graph.edges.retain(|e| match side {
            Side::Inputs => !(ends.contains(e.from_node()) && e.from_output() == id),
            Side::Outputs => !(ends.contains(e.to_node()) && e.to_input() == id),
        });
    };
    // outside: on group nodes running this group
    let outside = |graph: &mut Graph| {
        let ends: HashSet<String> = graph.nodes.iter().filter(|n| n.node_type == GROUP && n.data.get("group_id").and_then(Value::as_str) == Some(group_id)).map(|n| n.id.clone()).collect();
        graph.edges.retain(|e| match side {
            Side::Inputs => !(ends.contains(e.to_node()) && e.to_input() == id),
            Side::Outputs => !(ends.contains(e.from_node()) && e.from_output() == id),
        });
    };

    outside(project);
    for (gid, def) in project.groups.iter_mut() {
        if gid == group_id {
            inside(&mut def.graph);
        } else {
            outside(&mut def.graph);
        }
    }
    Ok(())
}

fn make_local(project: &mut Graph, scope: Option<&str>) -> Result<(), String> {
    let id = scope.ok_or("only a node group can be made local")?;
    if !project.groups.contains_key(id) {
        let def = builtin_groups().get(id).ok_or_else(|| format!("'{}' is not a built-in group", id))?;
        project.groups.insert(id.to_string(), def.clone());
    }
    Ok(())
}

fn revert_group(project: &mut Graph, scope: Option<&str>) -> Result<(), String> {
    let id = scope.ok_or("only a node group can be reverted")?;
    if !builtin_groups().contains_key(id) {
        return Err(format!("'{}' is not a built-in group", id));
    }
    project.groups.remove(id);
    Ok(())
}

// MARK: - Make Group / Ungroup

/// a group id that isn't taken yet, `node_group`, `node_group_2`, ...
fn next_group_id(groups: &BTreeMap<String, GroupDef>) -> String {
    if !groups.contains_key("node_group") {
        return "node_group".to_string();
    }
    (2..).map(|n| format!("node_group_{}", n)).find(|id| !groups.contains_key(id)).unwrap()
}

/// a group name that isn't taken yet, Blender style: `NodeGroup`, `NodeGroup.001`, ...
fn next_group_name(groups: &BTreeMap<String, GroupDef>) -> String {
    let names: HashSet<&str> = groups.values().map(|g| g.name.as_str()).collect();
    if !names.contains("NodeGroup") {
        return "NodeGroup".to_string();
    }
    (1..).map(|n| format!("NodeGroup.{:03}", n)).find(|name| !names.contains(name.as_str())).unwrap()
}

/// a node inside a group, the group input or output
fn boundary_node(id: &str, node_type: &str, position: Position) -> RfNode {
    RfNode {
        id: id.to_string(),
        node_type: node_type.to_string(),
        position,
        data: Map::from_iter([("inputs".to_string(), json!({}))]),
        extra: Map::new(),
    }
}

/// moves the selected nodes into a new group (Ctrl+G). connections crossing the selection become the group's sockets,
/// each outside value used inside gets one input, each inside value used outside gets one output
fn make_group(project: &mut Graph, scope: Option<&str>, specs: &Specs, selected: &[String], widths: &HashMap<String, f64>, added: &mut Vec<Added>) -> Result<(), String> {
    let mut graph = target(project, scope)?.clone();
    let selected: HashSet<&str> = selected.iter().map(String::as_str).collect();
    let nodes: Vec<RfNode> = graph.nodes.iter().filter(|n| selected.contains(n.id.as_str())).cloned().collect();
    if nodes.is_empty() {
        return Err("nothing to group".to_string());
    }
    if nodes.iter().any(|n| n.node_type == GROUP_INPUT || n.node_type == GROUP_OUTPUT) {
        return Err("the group input and output can't be grouped".to_string());
    }
    // a zone has to stay in one piece
    if nodes.iter().any(|n| (n.node_type == FOR_EACH_INPUT || n.node_type == FOR_EACH_OUTPUT) && zone_of(n).is_some_and(|z| !selected.contains(z))) {
        return Err("half a zone can't be grouped".to_string());
    }

    let all = all_groups(project);
    let group_id = next_group_id(&all);
    let group_node_id = graph.next_node_id(&group_id);
    let (group_input, group_output) = ("group_input-1", "group_output-1");
    let (mut inputs, mut outputs): (Vec<HandleSpec>, Vec<HandleSpec>) = (Vec::new(), Vec::new());
    let (mut inner, mut outer): (Vec<RfEdge>, Vec<RfEdge>) = (Vec::new(), Vec::new());
    // one socket per outside output (inputs) or inside output (outputs), shared by every connection from it
    let (mut input_for, mut output_for): (HashMap<(String, String), String>, HashMap<(String, String), String>) = (HashMap::new(), HashMap::new());

    for edge in &graph.edges {
        let consumer_in = selected.contains(edge.to_node());
        let producer_in = selected.contains(edge.from_node());
        let key = (edge.from_node().to_string(), edge.from_output().to_string());
        if consumer_in && producer_in {
            inner.push(edge.clone());
        } else if consumer_in {
            // outside value used inside: a group input socket
            let id = match input_for.get(&key) {
                Some(id) => id.clone(),
                None => {
                    let handle = input_handle(specs, graph.node(edge.to_node()).unwrap(), edge.to_input());
                    let id = socket_id(&handle.name, &inputs);
                    inputs.push(HandleSpec {
                        id: id.clone(),
                        ..handle
                    });
                    input_for.insert(key, id.clone());
                    outer.push(edge_like(edge, edge.from_node(), edge.from_output(), &group_node_id, &id));
                    id
                }
            };
            inner.push(edge_like(edge, group_input, &id, edge.to_node(), edge.to_input()));
        } else if producer_in {
            // inside value used outside: a group output socket
            let id = match output_for.get(&key) {
                Some(id) => id.clone(),
                None => {
                    let handle = output_handle(specs, graph.node(edge.from_node()).unwrap(), edge.from_output());
                    let id = socket_id(&handle.name, &outputs);
                    outputs.push(HandleSpec {
                        id: id.clone(),
                        ..handle
                    });
                    output_for.insert(key, id.clone());
                    inner.push(edge_like(edge, edge.from_node(), edge.from_output(), group_output, &id));
                    id
                }
            };
            outer.push(edge_like(edge, &group_node_id, &id, edge.to_node(), edge.to_input()));
        } else {
            outer.push(edge.clone());
        }
    }

    // the group input and output go left and right of the grouped nodes, the group node where they were
    let min_x = nodes.iter().map(|n| n.position.x).fold(f64::INFINITY, f64::min);
    let min_y = nodes.iter().map(|n| n.position.y).fold(f64::INFINITY, f64::min);
    let max_y = nodes.iter().map(|n| n.position.y).fold(f64::NEG_INFINITY, f64::max);
    let right = nodes.iter().map(|n| n.position.x + widths.get(&n.id).copied().unwrap_or(NODE_WIDTH)).fold(f64::NEG_INFINITY, f64::max);
    let mid_y = (min_y + max_y) / 2.0;

    let mut inside_nodes = vec![boundary_node(
        group_input,
        GROUP_INPUT,
        Position {
            x: min_x - 300.0,
            y: mid_y,
        },
    )];
    inside_nodes.extend(nodes.into_iter().map(|mut n| {
        n.extra.retain(|k, _| !UI_KEYS.contains(&k.as_str()));
        n
    }));
    inside_nodes.push(boundary_node(
        group_output,
        GROUP_OUTPUT,
        Position {
            x: right + 100.0,
            y: mid_y,
        },
    ));
    let def = GroupDef {
        name: next_group_name(&all),
        description: String::new(),
        category: String::new(),
        interface: super::model::HandleSpecs {
            inputs,
            outputs,
        },
        graph: Graph {
            nodes: inside_nodes,
            edges: inner,
            ..Default::default()
        },
    };

    // the group node takes the selection's place and becomes the selection
    let group_node = RfNode {
        id: group_node_id.clone(),
        node_type: GROUP.to_string(),
        position: Position {
            x: min_x,
            y: mid_y,
        },
        data: Map::from_iter([("group_id".to_string(), json!(group_id)), ("inputs".to_string(), json!({}))]),
        extra: Map::new(),
    };
    graph.nodes.retain(|n| !selected.contains(n.id.as_str()));
    graph.nodes.push(group_node);
    graph.edges = outer;
    select(&mut graph, &HashSet::from([group_node_id.as_str()]), &HashSet::new());
    added.push(Added {
        id: group_node_id.clone(),
        position: Position {
            x: min_x,
            y: mid_y,
        },
    });

    *target(project, scope)? = graph;
    project.groups.insert(group_id, def);
    Ok(())
}

/// replaces group nodes with the nodes inside their groups (Alt+G). connections through the group input and output are
/// joined up directly, inner nodes whose ids are taken get new ones
fn ungroup(project: &mut Graph, scope: Option<&str>, groups: &BTreeMap<String, GroupDef>, group_nodes: &[String], added: &mut Vec<Added>) -> Result<(), String> {
    let mut graph = target(project, scope)?.clone();
    let mut placed: Vec<String> = Vec::new();
    for node_id in group_nodes {
        let Some(def) = graph.node(node_id).filter(|n| n.node_type == GROUP).and_then(|n| n.data.get("group_id")).and_then(Value::as_str).and_then(|id| groups.get(id)) else {
            continue;
        };
        placed.extend(ungroup_one(&mut graph, node_id, def));
    }
    if placed.is_empty() {
        return Err("no group nodes to ungroup".to_string());
    }

    select(&mut graph, &placed.iter().map(String::as_str).collect(), &HashSet::new());
    added.extend(placed.iter().map(|id| Added {
        id: id.clone(),
        position: graph.node(id).unwrap().position.clone(),
    }));
    *target(project, scope)? = graph;
    Ok(())
}

/// puts the nodes of `def` in place of the group node, returns their ids
fn ungroup_one(graph: &mut Graph, node_id: &str, def: &GroupDef) -> Vec<String> {
    let group_node = graph.node(node_id).unwrap().clone();
    let boundary: HashSet<&str> = def.graph.nodes.iter().filter(|n| n.node_type == GROUP_INPUT || n.node_type == GROUP_OUTPUT).map(|n| n.id.as_str()).collect();
    let inner: Vec<&RfNode> = def.graph.nodes.iter().filter(|n| !boundary.contains(n.id.as_str())).collect();

    // new ids for inner nodes that clash with the parent's
    let mut taken: HashSet<String> = graph.nodes.iter().filter(|n| n.id != node_id).map(|n| n.id.clone()).collect();
    let mut ids: HashMap<String, String> = HashMap::new();
    for node in &inner {
        let id = if taken.contains(&node.id) {
            next_free_id(taken.iter().map(String::as_str), id_prefix(node))
        } else {
            node.id.clone()
        };
        taken.insert(id.clone());
        ids.insert(node.id.clone(), id);
    }

    // the inner nodes go around where the group node was, keeping their layout
    let xs = inner.iter().map(|n| n.position.x);
    let ys = inner.iter().map(|n| n.position.y);
    let cx = (xs.clone().fold(f64::INFINITY, f64::min) + xs.fold(f64::NEG_INFINITY, f64::max)) / 2.0;
    let cy = (ys.clone().fold(f64::INFINITY, f64::min) + ys.fold(f64::NEG_INFINITY, f64::max)) / 2.0;
    let placed: Vec<RfNode> = inner
        .iter()
        .map(|n| {
            let mut node = (*n).clone();
            node.id = ids[&n.id].clone();
            if let Some(partner) = zone_of(n).and_then(|z| ids.get(z)) {
                node.data.insert("zone".to_string(), json!(partner));
            }
            node.position = Position {
                x: group_node.position.x + n.position.x - cx,
                y: group_node.position.y + n.position.y - cy,
            };
            node
        })
        .collect();

    // where each group input's value comes from outside, and what feeds each group output inside
    let outside_into: HashMap<&str, &RfEdge> = graph.edges.iter().filter(|e| e.to_node() == node_id).map(|e| (e.to_input(), e)).collect();
    let inside_out: HashMap<&str, &RfEdge> = def.graph.edges.iter().filter(|e| boundary.contains(e.to_node())).map(|e| (e.to_input(), e)).collect();
    // the producer of a value inside the group, following a group input back out to the parent
    let producer_of = |from_node: &str, from_output: &str| -> Option<(String, String)> {
        if !boundary.contains(from_node) {
            return Some((ids.get(from_node).cloned().unwrap_or(from_node.to_string()), from_output.to_string()));
        }
        outside_into.get(from_output).map(|e| (e.from_node().to_string(), e.from_output().to_string()))
    };

    let mut edges: Vec<RfEdge> = Vec::new();
    for e in &graph.edges {
        if e.to_node() == node_id {
            continue;
        }
        if e.from_node() == node_id {
            // a parent node reads a group output: connect it to whatever feeds that output inside
            if let Some((from, output)) = inside_out.get(e.from_output()).and_then(|inner| producer_of(inner.from_node(), inner.from_output())) {
                edges.push(edge_like(e, &from, &output, e.to_node(), e.to_input()));
            }
        } else {
            edges.push(e.clone());
        }
    }
    for e in &def.graph.edges {
        if boundary.contains(e.to_node()) {
            continue;
        }
        if let Some((from, output)) = producer_of(e.from_node(), e.from_output()) {
            let to = ids.get(e.to_node()).cloned().unwrap_or(e.to_node().to_string());
            edges.push(edge_like(e, &from, &output, &to, e.to_input()));
        }
    }

    let new_ids = placed.iter().map(|n| n.id.clone()).collect();
    graph.nodes.retain(|n| n.id != node_id);
    graph.nodes.extend(placed);
    graph.edges = edges;
    new_ids
}
