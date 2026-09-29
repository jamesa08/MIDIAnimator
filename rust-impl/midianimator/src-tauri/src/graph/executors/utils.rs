use super::io::{NodeResult, Outputs};

/// Node: viewer
///
/// inputs:
/// "data": `Any`
///
/// outputs:
/// None
#[node_registry::node]
pub fn viewer() -> NodeResult {
    // :)
    Ok(Outputs::new())
}
