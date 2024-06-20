use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Handle {
    id: String,
    name: String,
    shape: String,
    show_label: bool
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Node {
    id: i32,
    name: String,
    position: (f64, f64),
    is_realtime: bool,
    input_handles: Vec<Handle>,
    output_handles: Vec<Handle>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NodeGraph {
    nodes: Vec<Node>,
}

impl NodeGraph {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
        }
    }

    pub fn add_node(&mut self, node: Node) {
        self.nodes.push(node);
    }

    pub fn remove_node(&mut self, id: i32) {
        self.nodes.retain(|node| node.id != id);
    }

    pub fn get_node(&self, id: i32) -> Option<&Node> {
        self.nodes.iter().find(|node| node.id == id)
    }

    pub fn get_node_mut(&mut self, id: i32) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|node| node.id == id)
    }
}