use super::io::{Inputs, NodeResult, Outputs};

/// Node: viewer
///
/// inputs:
/// "data": `Any`
///
/// outputs:
/// None
#[node_registry::node]
pub fn viewer(_inputs: Inputs) -> NodeResult {
    // :)
    Ok(Outputs::new())
}
