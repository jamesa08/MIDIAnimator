// selected sockets: a socket is selected on its own like an edge (a click, or a box around sockets only). the editor tags
// the selected sockets together and drags links from all of them at once, deleting them removes their tags. selecting
// is an undo step like selecting nodes

use serde::Deserialize;
use serde_json::{json, Value};

use super::model::Graph;
use super::ops::Side;

/// on a node, which of its sockets are selected: `{"inputs": [socket], "outputs": [socket]}`
pub const SELECTED_SOCKETS: &str = "selectedSockets";

/// one socket of a node
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct SocketRef {
    pub node: String,
    pub side: Side,
    pub socket: String,
}

/// selects exactly these sockets, on top of whatever nodes and edges are selected
pub fn select(graph: &mut Graph, selected: &[SocketRef]) {
    for node in &mut graph.nodes {
        let sockets = |side: Side| -> Vec<Value> { selected.iter().filter(|s| s.node == node.id && s.side == side).map(|s| json!(s.socket)).collect() };
        let (inputs, outputs) = (sockets(Side::Inputs), sockets(Side::Outputs));
        if inputs.is_empty() && outputs.is_empty() {
            node.extra.remove(SELECTED_SOCKETS);
        } else {
            node.extra.insert(SELECTED_SOCKETS.to_string(), json!({ "inputs": inputs, "outputs": outputs }));
        }
    }
}

/// the selected sockets
pub fn selected(graph: &Graph) -> Vec<SocketRef> {
    let mut selected = Vec::new();
    for node in &graph.nodes {
        for (side, key) in [(Side::Inputs, "inputs"), (Side::Outputs, "outputs")] {
            for socket in node.extra.get(SELECTED_SOCKETS).and_then(|s| s.get(key)).and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
                selected.push(SocketRef {
                    node: node.id.clone(),
                    side,
                    socket: socket.to_string(),
                });
            }
        }
    }
    selected
}
