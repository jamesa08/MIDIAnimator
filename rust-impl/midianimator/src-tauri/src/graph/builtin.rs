// node groups that ship with the app. they're shared by reference: a project only stores a built-in group once it's
// made local (edited), and the local copy (same id) then replaces the built-in for every group node using it

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde_json::Value;

use super::model::{GroupDef, Graph};
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

/// updates graphs saved before node groups: a node whose type is now a built-in group becomes a group node running it.
/// the node keeps its id, connections and values
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
    changed
}
