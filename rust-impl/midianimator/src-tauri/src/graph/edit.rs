// validated edits on a `Graph`. each function is one logical edit, if something is wrong the graph is left untouched
// and the error message says what to do instead

use serde_json::{Map, Value};
use std::collections::HashMap;

use super::model::{compatible, dyn_inner, is_param, node_inputs, node_outputs, Graph, NodeSpec, Position, RfEdge, RfNode, Specs};
use super::ops::Side;
use super::run::GROUP;
use super::tags;
use std::borrow::Cow;

/// horizontal gap used when placing new nodes automatically
const AUTO_PLACE_DX: f64 = 350.0;

/// what an edit did, sent back to the caller
#[derive(Debug, Clone)]
pub struct EditResult {
    /// short description of what changed
    pub message: String,
    /// nodes whose outline should be shown after the edit
    pub touched: Vec<String>,
}

/// looks up the spec for a node that's already in the graph
fn spec_for<'a>(graph: &Graph, specs: &Specs<'a>, node_id: &str) -> Result<Cow<'a, NodeSpec>, String> {
    // get the node's type, then find the spec for it
    let node = graph.node(node_id).ok_or_else(|| format!("no node '{}'", node_id))?;
    specs.for_node(node).ok_or_else(|| format!("node '{}' has unknown type '{}'", node_id, node.resolved_node_type()))
}

/// `node › Handle Label`
fn handle_label(node_id: &str, name: &str) -> String {
    format!("{} › {}", node_id, name)
}

/// lists a spec's inputs as `id (type)` for error messages
fn input_ids(spec: &NodeSpec) -> String {
    spec.handles.inputs.iter().map(|h| format!("{} ({})", h.id, h.data_type)).collect::<Vec<_>>().join(", ")
}

/// checks that every key is a declared input and that simple values have the right JSON type.
/// `null` is allowed and means "unset".
pub fn validate_inputs(spec: &NodeSpec, inputs: &Map<String, Value>) -> Result<(), String> {
    // make sure the input exists on this node type
    for (key, value) in inputs {
        let Some(handle) = spec.input(key) else {
            return Err(format!("'{}' has no input '{}'; inputs are: {}", spec.id, key, input_ids(spec)));
        };
        // check simple types, anything more complex is let through
        let ok = match (handle.data_type.as_str(), value) {
            // null always passes, it means "unset"
            (_, Value::Null) => true,
            ("String", v) => v.is_string(),
            ("f64", v) => v.is_number(),
            _ => true,
        };
        // wrong type for this input
        if !ok {
            return Err(format!("input '{}' of '{}' expects {}, got {}", key, spec.id, handle.data_type, value));
        }
    }
    Ok(())
}

/// merges values into a node's inputs, a `null` value removes the input instead
fn merge_inputs(node: &mut RfNode, inputs: &Map<String, Value>) {
    let target = node.inputs_mut();
    for (key, value) in inputs {
        if value.is_null() {
            target.remove(key);
        } else {
            target.insert(key.clone(), value.clone());
        }
    }
}

/// gives a new animation generator without a name the first free "Animation N", so it works as soon as it's added
pub fn fill_default_name(graph: &Graph, node: &mut RfNode) {
    if node.node_type != "animation_generator" || node.input_value("name").and_then(Value::as_str).is_some_and(|name| !name.is_empty()) {
        return;
    }
    let taken: Vec<&str> = graph.nodes.iter().filter(|n| n.node_type == "animation_generator").filter_map(|n| n.input_value("name")?.as_str()).collect();
    let name = (1..).map(|i| format!("Animation {}", i)).find(|name| !taken.contains(&name.as_str())).unwrap();
    node.inputs_mut().insert("name".to_string(), Value::String(name));
}

/// adds a new node of `node_type` and returns its new id in `touched`
///
/// the position comes from `position` if given, otherwise it's placed to the right of `after`,
/// otherwise to the right of the rightmost node in the graph
pub fn add_node(graph: &mut Graph, specs: &Specs, node_type: &str, inputs: Option<&Map<String, Value>>, position: Option<Position>, after: Option<&str>) -> Result<EditResult, String> {
    // make sure the node type exists and the inputs are valid before changing anything
    let Some(spec) = specs.for_type(node_type) else {
        return Err(format!("unknown node type '{}'; call node_types_list", node_type));
    };
    if let Some(inputs) = inputs {
        validate_inputs(&spec, inputs)?;
    }
    // a group id adds a group node running that group, named after it
    let group_id = specs.groups.contains_key(node_type).then(|| node_type.to_string());

    // figure out where to put the node
    let position = match (position, after) {
        // an explicit position always wins
        (Some(position), _) => position,
        // place it to the right of the `after` node
        (None, Some(after)) => {
            let after_id = graph.resolve(after)?;
            let anchor = &graph.node(&after_id).unwrap().position;
            Position {
                x: anchor.x + AUTO_PLACE_DX,
                y: anchor.y,
            }
        }
        // place it to the right of the rightmost node, or at the origin if the graph is empty
        (None, None) => match graph.nodes.iter().max_by(|a, b| a.position.x.total_cmp(&b.position.x)) {
            Some(rightmost) => Position {
                x: rightmost.position.x + AUTO_PLACE_DX,
                y: rightmost.position.y,
            },
            None => Position::default(),
        },
    };

    // create the node with the next short id for this type
    let id = graph.next_node_id(node_type);
    let mut node = RfNode {
        id: id.clone(),
        node_type: if group_id.is_some() {
            GROUP.to_string()
        } else {
            node_type.to_string()
        },
        position,
        data: Map::new(),
        extra: Map::new(),
    };
    if let Some(group_id) = group_id {
        node.data.insert("group_id".to_string(), Value::String(group_id));
    }
    // make sure `inputs` exists even if nothing was set, then add any given values
    node.inputs_mut();
    if let Some(inputs) = inputs {
        merge_inputs(&mut node, inputs);
    }
    fill_default_name(graph, &mut node);
    graph.nodes.push(node);

    Ok(EditResult {
        message: format!("added {} \"{}\"", id, spec.name),
        touched: vec![id],
    })
}

/// connects `from_output` on `from_node` to `to_input` on `to_node` (in data-flow terms)
///
/// note: an input can only have one edge, connecting to an input that is already connected replaces the old edge. a multi
/// input keeps every edge
pub fn connect(graph: &mut Graph, specs: &Specs, results: &HashMap<String, Value>, from_node: &str, from_output: &str, to_node: &str, to_input: &str) -> Result<EditResult, String> {
    // resolve the node ids (prefixes are allowed) and get their specs
    let from_id = graph.resolve(from_node)?;
    let to_id = graph.resolve(to_node)?;
    let from_spec = spec_for(graph, specs, &from_id)?;
    let to_spec = spec_for(graph, specs, &to_id)?;

    // find the output, this includes dynamic outputs if the node has executed
    let outputs = node_outputs(&from_spec, results.get(&from_id));
    let Some(output) = outputs.iter().find(|h| h.id == from_output) else {
        // output doesn't exist, list the ones that do
        let available = outputs.iter().map(|h| format!("{} ({})", h.id, h.data_type)).collect::<Vec<_>>().join(", ");
        let mut msg = format!(
            "'{}' has no output '{}'; outputs are: {}",
            from_id,
            from_output,
            if available.is_empty() {
                "none"
            } else {
                &available
            }
        );
        // dynamic outputs only show up after execution, so give a hint
        if from_spec.handles.outputs.iter().any(|h| h.data_type.starts_with("Dyn<")) {
            msg.push_str(". Dynamic outputs appear after the node executes with its parameters set");
        }
        return Err(msg);
    };
    // hidden handles have no position on the canvas, an edge to one draws from nowhere and breaks the UI
    if output.hidden {
        let mut msg = format!("'{}' on '{}' is hidden in the UI and must not be connected", from_output, from_id);
        // for a `Dyn<T>` output, point to the dynamic outputs that can be connected instead
        if output.data_type.starts_with("Dyn<") {
            let dynamic: Vec<&str> = outputs.iter().filter(|h| !h.hidden && h.description.starts_with("Dynamic output")).map(|h| h.id.as_str()).collect();
            if dynamic.is_empty() {
                msg.push_str("; connect one of its dynamic outputs instead, they appear after the node executes with its parameters set");
            } else {
                msg.push_str(&format!("; connect one of its dynamic outputs instead: {}", dynamic.join(", ")));
            }
        } else {
            msg.push_str("; its value is shown inside the node and in graph_outline");
        }
        return Err(msg);
    }
    // make sure the input exists and isn't a parameter (hidden input), this includes dynamic inputs
    let inputs = node_inputs(&to_spec, graph, &to_id);
    let Some(input) = inputs.iter().find(|h| h.id == to_input) else {
        let available = inputs.iter().filter(|h| dyn_inner(h).is_none()).map(|h| format!("{} ({})", h.id, h.data_type)).collect::<Vec<_>>().join(", ");
        return Err(format!("'{}' has no input '{}'; inputs are: {}", to_id, to_input, available));
    };
    // a `Dyn<T>` input grows numbered inputs, point to the free one
    if dyn_inner(input).is_some() {
        let free = inputs.iter().rev().find(|h| h.description == format!("Dynamic input of {}.", input.id)).map_or(String::new(), |h| h.id.clone());
        return Err(format!("'{}' on '{}' is dynamic and can't be connected itself; connect to its next free input '{}' instead", to_input, to_id, free));
    }
    if is_param(&to_spec, to_input) {
        return Err(format!("'{}' on '{}' is hidden in the UI and must not be connected; it is a parameter, set it with graph_set_inputs", to_input, to_id));
    }
    // no self connections or cycles
    if from_id == to_id {
        return Err("cannot connect a node to itself".to_string());
    }
    if graph.reaches(&to_id, &from_id) {
        return Err(format!("connecting {} -> {} would create a cycle", from_id, to_id));
    }
    // make sure the types line up
    if !compatible(&output.data_type, &input.data_type) {
        return Err(format!("type mismatch: {} is {} but {} expects {}", handle_label(&from_id, &output.name), output.data_type, handle_label(&to_id, &input.name), input.data_type));
    }

    // only one edge per input, replace whatever feeds it now. a multi input only has to check for the same edge
    let mut message = String::new();
    if let Some(index) = graph.edges.iter().position(|e| e.to_node() == to_id && e.to_input() == to_input && (!input.multi || (e.from_node() == from_id && e.from_output() == from_output))) {
        let old = graph.edges.remove(index);
        // it's the same edge, put it back and return early as there's nothing to do
        if old.from_node() == from_id && old.from_output() == from_output {
            graph.edges.insert(index, old);
            return Ok(EditResult {
                message: format!("already connected: {} -> {}", handle_label(&from_id, &output.name), handle_label(&to_id, &input.name)),
                touched: vec![from_id, to_id],
            });
        }
        message = format!(" (replaced {} -> {})", old.from_node(), old.from_output());
    }
    // add the new edge, it takes the place of a tag on the input
    graph.edges.push(RfEdge::new(&from_id, from_output, &to_id, to_input));
    tags::clear_input(graph, &to_id, to_input);

    // let the caller know if a value set on the input is now being ignored
    if graph.node(&to_id).and_then(|n| n.input_value(to_input)).is_some() {
        message.push_str("; the value set on this input is ignored while connected");
    }

    Ok(EditResult {
        message: format!("connected {} -> {}{}", handle_label(&from_id, &output.name), handle_label(&to_id, &input.name), message),
        touched: vec![from_id, to_id],
    })
}

/// removes the edges feeding `to_input` on `to_node`. for a multi input only the ones from `from_node` (and its
/// `from_output`) if they're given
pub fn disconnect(graph: &mut Graph, to_node: &str, to_input: &str, from_node: Option<&str>, from_output: Option<&str>) -> Result<EditResult, String> {
    let to_id = graph.resolve(to_node)?;
    let from_id = from_node.map(|n| graph.resolve(n)).transpose()?;
    let feeds = |e: &RfEdge| e.to_node() == to_id && e.to_input() == to_input && from_id.as_ref().is_none_or(|f| e.from_node() == f) && from_output.is_none_or(|o| e.from_output() == o);
    // find the edges, if there aren't any list the inputs that are connected
    if !graph.edges.iter().any(feeds) {
        if (from_id.is_some() || from_output.is_some()) && graph.edge_into(&to_id, to_input).is_some() {
            let from: Vec<String> = graph.edges.iter().filter(|e| e.to_node() == to_id && e.to_input() == to_input).map(|e| format!("{} › {}", e.from_node(), e.from_output())).collect();
            return Err(format!("input '{}' of '{}' has no such connection; it's connected from: {}", to_input, to_id, from.join(", ")));
        }
        let connected: Vec<&str> = graph.edges.iter().filter(|e| e.to_node() == to_id).map(|e| e.to_input()).collect();
        return Err(format!(
            "input '{}' of '{}' is not connected; connected inputs: {}",
            to_input,
            to_id,
            if connected.is_empty() {
                "none".to_string()
            } else {
                connected.join(", ")
            }
        ));
    }
    // remove the edges, a tag on the input goes too or it would connect it again
    let (old, kept): (Vec<RfEdge>, Vec<RfEdge>) = std::mem::take(&mut graph.edges).into_iter().partition(|e| feeds(e));
    graph.edges = kept;
    tags::clear_input(graph, &to_id, to_input);
    let from: Vec<String> = old.iter().map(|e| handle_label(e.from_node(), e.from_output())).collect();
    let mut touched: Vec<String> = old.iter().map(|e| e.from_node().to_string()).collect();
    touched.push(to_id.clone());
    Ok(EditResult {
        message: format!("disconnected {} -> {} › {}", from.join(", "), to_id, to_input),
        touched,
    })
}

/// sets (or unsets with `null`) values on a node's inputs
pub fn set_inputs(graph: &mut Graph, specs: &Specs, node: &str, inputs: &Map<String, Value>) -> Result<EditResult, String> {
    // resolve the node and validate the inputs against its spec
    let id = graph.resolve(node)?;
    let spec = spec_for(graph, specs, &id)?;
    validate_inputs(&spec, inputs)?;

    // remember which of these inputs are connected, their values will be ignored while connected
    let connected: Vec<String> = inputs.keys().filter(|key| graph.edge_into(&id, key).is_some()).cloned().collect();
    // write the values to the node
    merge_inputs(graph.node_mut(&id).unwrap(), inputs);

    // build the message, mention any inputs that were ignored
    let mut message = format!("set {} on {}", inputs.keys().cloned().collect::<Vec<_>>().join(", "), id);
    if !connected.is_empty() {
        message.push_str(&format!("; ignored while connected: {}", connected.join(", ")));
    }
    Ok(EditResult {
        message,
        touched: vec![id],
    })
}

/// removes a node and every edge connected to it
pub fn remove_node(graph: &mut Graph, node: &str) -> Result<EditResult, String> {
    // resolve the node id
    let id = graph.resolve(node)?;
    let mut neighbours: Vec<String> = Vec::new();
    let before = graph.edges.len();
    // remove every edge touching this node, keeping track of the nodes on the other end
    graph.edges.retain(|e| {
        let touches = e.from_node() == id || e.to_node() == id;
        if touches {
            // get the node on the other end of the edge
            let other = if e.from_node() == id {
                e.to_node()
            } else {
                e.from_node()
            };
            // only add each neighbour once
            if other != id && !neighbours.iter().any(|n| n == other) {
                neighbours.push(other.to_string());
            }
        }
        !touches
    });
    // count how many edges we removed, then remove the node itself
    let removed_edges = before - graph.edges.len();
    graph.nodes.retain(|n| n.id != id);

    // the neighbours are returned so their outlines can be shown after the edit
    Ok(EditResult {
        message: format!(
            "removed {} and {} edge{}",
            id,
            removed_edges,
            if removed_edges == 1 {
                ""
            } else {
                "s"
            }
        ),
        touched: neighbours,
    })
}

/// tags a socket (`graph::tags`): `side` is "input" or "output", an empty name removes the tag
pub fn set_tag(graph: &mut Graph, specs: &Specs, results: &HashMap<String, Value>, node: &str, side: &str, socket: &str, name: &str) -> Result<EditResult, String> {
    let id = graph.resolve(node)?;
    let spec = spec_for(graph, specs, &id)?;
    let side = match side {
        "input" | "inputs" => Side::Inputs,
        "output" | "outputs" => Side::Outputs,
        other => return Err(format!("unknown side '{}'; use \"input\" or \"output\"", other)),
    };
    // only sockets that can be connected can be tagged
    match side {
        Side::Inputs => {
            let inputs = node_inputs(&spec, graph, &id);
            let Some(input) = inputs.iter().find(|h| h.id == socket) else {
                let available = inputs.iter().filter(|h| dyn_inner(h).is_none() && !is_param(&spec, &h.id)).map(|h| h.id.as_str()).collect::<Vec<_>>().join(", ");
                return Err(format!("'{}' has no input '{}'; inputs are: {}", id, socket, available));
            };
            if dyn_inner(input).is_some() || is_param(&spec, socket) {
                return Err(format!("'{}' on '{}' can't be connected, so it can't be tagged", socket, id));
            }
        }
        Side::Outputs => {
            let outputs = node_outputs(&spec, results.get(&id));
            let Some(output) = outputs.iter().find(|h| h.id == socket) else {
                let available = outputs.iter().filter(|h| !h.hidden).map(|h| h.id.as_str()).collect::<Vec<_>>().join(", ");
                return Err(format!("'{}' has no output '{}'; outputs are: {}", id, socket, available));
            };
            if output.hidden {
                return Err(format!("'{}' on '{}' is hidden in the UI and can't be tagged", socket, id));
            }
        }
    }

    tags::set_tag(graph, &id, side, socket, name)?;
    tags::sync(graph);

    // say what the tag connects now
    let name = name.trim();
    let label = handle_label(&id, socket);
    let mut touched = vec![id.clone()];
    let message = match side {
        _ if name.is_empty() => format!("removed the tag from {}; its tagged connections are wires now", label),
        Side::Outputs => {
            let targets: Vec<String> = graph.edges_from(&id, Some(socket)).filter(|e| tags::is_tagged(e)).map(|e| handle_label(e.to_node(), e.to_input())).collect();
            touched.extend(graph.edges_from(&id, Some(socket)).filter(|e| tags::is_tagged(e)).map(|e| e.to_node().to_string()));
            if targets.is_empty() {
                format!("tagged {} {:?}; no input uses this tag yet", label, name)
            } else {
                format!("tagged {} {:?}; it connects to {}", label, name, targets.join(", "))
            }
        }
        Side::Inputs => match graph.edge_into(&id, socket) {
            Some(edge) => {
                touched.push(edge.from_node().to_string());
                format!("tagged {} {:?}; it takes the value of {}", label, name, handle_label(edge.from_node(), edge.from_output()))
            }
            None if tags::sources(graph).contains_key(name) => format!("tagged {} {:?}; broken: connecting it would make a cycle", label, name),
            None => format!("tagged {} {:?}; broken until an output in this graph has this tag", label, name),
        },
    };
    Ok(EditResult {
        message,
        touched,
    })
}
