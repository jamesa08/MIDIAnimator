// signal tags, like Q-SYS signal names: a name on a socket that connects it without a wire. an output's tag is the
// source of that name in its graph (one output per name), every input with the same tag takes its value. the names
// live on the nodes (`output_tags`, `input_tags` in their data), each connection they make is an ordinary edge marked
// `tagged` so running, checking and undo see it like any other. `sync` keeps those edges in line with the names after
// every edit. an input whose tag names no output (or would make a cycle) is a broken tag and isn't connected

use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};

use super::model::{Graph, RfEdge, RfNode};
use super::ops::Side;
use super::sockets::SocketRef;

/// where a node keeps its tags, socket id to name
pub const INPUT_TAGS: &str = "input_tags";
pub const OUTPUT_TAGS: &str = "output_tags";
/// on an edge a tag made, the UI draws its ends as tags instead of a wire
pub const TAGGED: &str = "tagged";

fn key(side: Side) -> &'static str {
    match side {
        Side::Inputs => INPUT_TAGS,
        Side::Outputs => OUTPUT_TAGS,
    }
}

/// the tag on one of a node's sockets
pub fn tag(node: &RfNode, side: Side, socket: &str) -> Option<String> {
    node.data.get(key(side)).and_then(|tags| tags.get(socket)).and_then(Value::as_str).map(str::to_string)
}

/// every tag on one side of a node, socket id to name
pub fn tags(node: &RfNode, side: Side) -> Vec<(String, String)> {
    node.data.get(key(side)).and_then(Value::as_object).map_or(Vec::new(), |tags| tags.iter().filter_map(|(socket, name)| Some((socket.clone(), name.as_str()?.to_string()))).collect())
}

/// sets or (with `None`) removes the tag on a socket, a node without tags keeps no empty map
fn put(node: &mut RfNode, side: Side, socket: &str, name: Option<&str>) {
    let key = key(side);
    let mut tags = node.data.get(key).and_then(Value::as_object).cloned().unwrap_or_default();
    match name {
        Some(name) => tags.insert(socket.to_string(), json!(name)),
        None => tags.remove(socket),
    };
    if tags.is_empty() {
        node.data.remove(key);
    } else {
        node.data.insert(key.to_string(), Value::Object(tags));
    }
}

pub fn is_tagged(edge: &RfEdge) -> bool {
    edge.extra.get(TAGGED) == Some(&Value::Bool(true))
}

fn set_tagged(edge: &mut RfEdge, tagged: bool) {
    if tagged {
        edge.extra.insert(TAGGED.to_string(), Value::Bool(true));
    } else {
        edge.extra.remove(TAGGED);
    }
}

/// the output each name is the tag of, the first one in graph order if a name is on more than one
pub fn sources(graph: &Graph) -> HashMap<String, (String, String)> {
    let mut sources = HashMap::new();
    for node in &graph.nodes {
        for (socket, name) in tags(node, Side::Outputs) {
            sources.entry(name).or_insert((node.id.clone(), socket));
        }
    }
    sources
}

/// makes the graph's edges match its tags: every tagged input whose name has an output is connected to it by a tagged
/// edge (in place of a wire), every other tagged edge goes. an edge still marked tagged whose input lost its tag stays
/// as a wire
pub fn sync(graph: &mut Graph) {
    let sources = sources(graph);
    let edges = std::mem::take(&mut graph.edges);
    let mut kept = Vec::with_capacity(edges.len());
    for mut edge in edges {
        match graph.node(edge.to_node()).and_then(|n| tag(n, Side::Inputs, edge.to_input())) {
            None => {
                set_tagged(&mut edge, false);
                kept.push(edge);
            }
            // a wire into a tagged input, or a tagged edge from an output that's no longer its tag's
            Some(name) => {
                if is_tagged(&edge) && sources.get(&name).is_some_and(|(node, socket)| node == edge.from_node() && socket == edge.from_output()) {
                    kept.push(edge);
                }
            }
        }
    }
    graph.edges = kept;

    // tagged inputs that aren't connected yet, a node can't take its own value and a cycle never finishes
    let wanted: Vec<(String, String, String, String)> = graph.nodes.iter().flat_map(|node| tags(node, Side::Inputs).into_iter().map(move |(socket, name)| (node.id.clone(), socket, name))).filter_map(|(to, input, name)| sources.get(&name).map(|(from, output)| (from.clone(), output.clone(), to, input))).collect();
    for (from, output, to, input) in wanted {
        if from == to || graph.edge_into(&to, &input).is_some() || graph.reaches(&to, &from) {
            continue;
        }
        let mut edge = RfEdge::new(&from, &output, &to, &input);
        set_tagged(&mut edge, true);
        graph.edges.push(edge);
    }
}

/// deletes the tag on a socket like an edge: an input's tag takes its connection along, an output's leaves the inputs
/// using its name broken
pub fn remove(graph: &mut Graph, tag: &SocketRef) {
    if tag.side == Side::Inputs {
        graph.edges.retain(|e| !(is_tagged(e) && e.to_node() == tag.node && e.to_input() == tag.socket));
    }
    if let Some(node) = graph.node_mut(&tag.node) {
        put(node, tag.side, &tag.socket, None);
    }
}

/// tags a socket with `name`, an empty name removes its tag. sync the graph afterwards.
/// on an output: a new tag turns its wires into tags, a changed one renames it on every input using it, removing it turns
/// its tagged connections back into wires. on an input: it takes the value of the output with that name, or is broken
/// until there is one. a wired input tagged with a name no output has yet gives the wire's output that name too, the wire
/// becomes the tag. removing it turns its connection back into a wire
pub fn set_tag(graph: &mut Graph, node: &str, side: Side, socket: &str, name: &str) -> Result<(), String> {
    let name = name.trim();
    let old = tag(graph.node(node).ok_or_else(|| format!("no node '{}'", node))?, side, socket);
    if old.as_deref() == Some(name) || (old.is_none() && name.is_empty()) {
        return Ok(());
    }
    match side {
        Side::Outputs => {
            if name.is_empty() {
                // the connections it made become wires
                let consumers: Vec<(String, String)> = graph.edges.iter().filter(|e| e.from_node() == node && e.from_output() == socket && is_tagged(e)).map(|e| (e.to_node().to_string(), e.to_input().to_string())).collect();
                for (to, input) in consumers {
                    put(graph.node_mut(&to).unwrap(), Side::Inputs, &input, None);
                }
                put(graph.node_mut(node).unwrap(), Side::Outputs, socket, None);
                return Ok(());
            }
            if let Some((other, other_socket)) = sources(graph).get(name) {
                return Err(format!("'{}' is already the tag of {} › {}", name, other, other_socket));
            }
            match &old {
                // every input using the old name follows
                Some(old) => {
                    for other in &mut graph.nodes {
                        for (input, _) in tags(other, Side::Inputs).into_iter().filter(|(_, n)| n == old) {
                            put(other, Side::Inputs, &input, Some(name));
                        }
                    }
                }
                // its wires become the tag
                None => {
                    let wired: Vec<(String, String)> = graph.edges.iter().filter(|e| e.from_node() == node && e.from_output() == socket).map(|e| (e.to_node().to_string(), e.to_input().to_string())).collect();
                    for (to, input) in wired {
                        put(graph.node_mut(&to).unwrap(), Side::Inputs, &input, Some(name));
                    }
                }
            }
            put(graph.node_mut(node).unwrap(), Side::Outputs, socket, Some(name));
        }
        Side::Inputs => {
            if name.is_empty() {
                if let Some(edge) = graph.edges.iter_mut().find(|e| e.to_node() == node && e.to_input() == socket) {
                    set_tagged(edge, false);
                }
                put(graph.node_mut(node).unwrap(), Side::Inputs, socket, None);
                return Ok(());
            }
            // a wire whose output has no tag gets this one, when it's a new name
            let wire = graph.edges.iter().find(|e| e.to_node() == node && e.to_input() == socket && !is_tagged(e)).map(|e| (e.from_node().to_string(), e.from_output().to_string()));
            if let Some((from, output)) = wire {
                if !sources(graph).contains_key(name) && tag(graph.node(&from).unwrap(), Side::Outputs, &output).is_none() {
                    put(graph.node_mut(&from).unwrap(), Side::Outputs, &output, Some(name));
                }
            }
            put(graph.node_mut(node).unwrap(), Side::Inputs, socket, Some(name));
        }
    }
    Ok(())
}

/// tags several sockets at once (`set_tag` on each): every input gets `name`, each output a name of its own (one output
/// per name), `name` for the first and numbered after it for the rest. an empty name removes their tags
pub fn set_tags(graph: &mut Graph, sockets: &[SocketRef], name: &str) -> Result<(), String> {
    let name = name.trim();
    let tagging = |node: &str, socket: &str| sockets.iter().any(|s| s.side == Side::Outputs && s.node == node && s.socket == socket);
    // the names these outputs have now are free for them
    let mut taken: HashSet<String> = graph.nodes.iter().flat_map(|n| tags(n, Side::Outputs).into_iter().filter(|(socket, _)| !tagging(&n.id, socket)).map(|(_, name)| name)).collect();
    // outputs first, an input tagged with a name no output has yet would give it to the output it's wired from
    let mut pending: Vec<(&SocketRef, String)> = Vec::new();
    for socket in sockets.iter().filter(|s| s.side == Side::Outputs).chain(sockets.iter().filter(|s| s.side == Side::Inputs)) {
        let name = if socket.side == Side::Outputs && !name.is_empty() {
            let free = free_name(name, &taken);
            taken.insert(free.clone());
            free
        } else {
            name.to_string()
        };
        pending.push((socket, name));
    }
    // an output can only take a name another of these outputs has once that one has been renamed. outputs swapping names
    // wait on each other, one of them steps aside to a free name first
    let mut stepped_aside = false;
    while !pending.is_empty() {
        let before = pending.len();
        let mut error = None;
        pending.retain(|(socket, name)| match set_tag(graph, &socket.node, socket.side, &socket.socket, name) {
            Ok(()) => false,
            Err(e) => {
                error = Some(e);
                true
            }
        });
        if pending.len() < before {
            stepped_aside = false;
            continue;
        }
        let output = pending.iter().find(|(socket, _)| socket.side == Side::Outputs).map(|(socket, name)| (*socket, name.clone()));
        match output {
            Some((socket, name)) if !stepped_aside => {
                let taken: HashSet<String> = sources(graph).into_keys().collect();
                set_tag(graph, &socket.node, socket.side, &socket.socket, &free_name(&name, &taken))?;
                stepped_aside = true;
            }
            _ => return Err(error.unwrap_or_default()),
        }
    }
    Ok(())
}

/// removes the tag from an input, for a wire connected to it in place of its tag
pub fn clear_input(graph: &mut Graph, node: &str, input: &str) {
    if let Some(node) = graph.node_mut(node) {
        put(node, Side::Inputs, input, None);
    }
}

/// turns the tagged edges `crossing` picks into wires, their inputs lose their tags. for edges about to cross into or
/// out of a group, a tag only connects inside one graph
pub fn untag(graph: &mut Graph, crossing: impl Fn(&RfEdge) -> bool) {
    let mut inputs = Vec::new();
    for edge in graph.edges.iter_mut().filter(|e| is_tagged(e) && crossing(e)) {
        set_tagged(edge, false);
        inputs.push((edge.to_node().to_string(), edge.to_input().to_string()));
    }
    for (node, input) in inputs {
        clear_input(graph, &node, &input);
    }
}

/// `name` if it's free, otherwise numbered like Q-SYS does (`audio` to `audio 2`, `audio 2` to `audio 3`)
pub fn free_name(name: &str, taken: &HashSet<String>) -> String {
    if !taken.contains(name) {
        return name.to_string();
    }
    let base = match name.rsplit_once(' ') {
        Some((base, number)) if !base.is_empty() && number.parse::<u64>().is_ok() => base,
        _ => name,
    };
    (2..).map(|n| format!("{} {}", base, n)).find(|n| !taken.contains(n)).unwrap()
}

/// gives nodes just added (pasted, duplicated, ungrouped) output tags of their own: one whose name the graph already has
/// gets a new name, and the added inputs using it follow. added inputs whose output didn't come along keep their name
pub fn adopt(graph: &mut Graph, added: &[String]) {
    let added: HashSet<&str> = added.iter().map(String::as_str).collect();
    let mut taken: HashSet<String> = graph.nodes.iter().filter(|n| !added.contains(n.id.as_str())).flat_map(|n| tags(n, Side::Outputs).into_iter().map(|(_, name)| name)).collect();
    let mut renamed: HashMap<String, String> = HashMap::new();
    for node in graph.nodes.iter_mut().filter(|n| added.contains(n.id.as_str())) {
        for (socket, name) in tags(node, Side::Outputs) {
            let free = free_name(&name, &taken);
            if free != name {
                put(node, Side::Outputs, &socket, Some(&free));
                renamed.insert(name, free.clone());
            }
            taken.insert(free);
        }
    }
    for node in graph.nodes.iter_mut().filter(|n| added.contains(n.id.as_str())) {
        for (socket, name) in tags(node, Side::Inputs) {
            if let Some(new) = renamed.get(&name) {
                put(node, Side::Inputs, &socket, Some(new));
            }
        }
    }
}

/// the tags in a pasted node's data, only names on sockets
pub fn clean(data: &Map<String, Value>) -> Map<String, Value> {
    let mut clean = Map::new();
    for key in [INPUT_TAGS, OUTPUT_TAGS] {
        let tags: Map<String, Value> = data.get(key).and_then(Value::as_object).into_iter().flatten().filter(|(_, name)| name.as_str().is_some_and(|n| !n.trim().is_empty())).map(|(k, v)| (k.clone(), v.clone())).collect();
        if !tags.is_empty() {
            clean.insert(key.to_string(), Value::Object(tags));
        }
    }
    clean
}
