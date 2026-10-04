// edits from the node editor: `graph_apply` checks and applies ops (see `graph::ops`) to a tab's graph, records them
// as one undo step, sends the graph back and re-runs the tab when what it computes changed. copy, cut and paste go
// through the system clipboard here (`graph::clipboard` makes and reads the payload). every edit names its tab, an edit
// still on its way when another tab is shown lands on its own

use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use clipboard_rs::{Clipboard, ClipboardContext};

use super::history::{self, Capture};
use super::{lock, push_graph};
use crate::graph::builtin::all_groups;
use crate::graph::history::{Source, Step};
use crate::graph::clipboard;
use crate::graph::model::{node_specs, Graph, Position};
use crate::graph::ops::{self, Added, Ctx, Op};
use crate::graph::run::{instance_path, scoped_values};

/// a tab's graph after an edit and the nodes it added
#[derive(Serialize)]
pub struct Applied {
    pub tab: String,
    pub graph_rev: u64,
    pub rf_instance: HashMap<String, Value>,
    pub added: Vec<Added>,
}

/// applies `ops` to the graph `scope` (a group id, none for the top level) of a tab as one undo step named after the first op.
/// with `txn` it joins the other steps of that transaction. `commit_to_history` false changes the graph without an undo
/// step (values filled in automatically). if one op fails nothing changes and the error says why
#[tauri::command]
pub async fn graph_apply(tab: String, scope: Option<String>, ops: Vec<Op>, txn: Option<String>, commit_to_history: Option<bool>) -> Result<Applied, String> {
    apply(&tab, scope, ops, txn, commit_to_history != Some(false))
}

/// the system clipboard type copied nodes go under, in place of text. only MotionKeys reads it, pasting them anywhere
/// else gives nothing
const CLIPBOARD_TYPE: &str = "com.jamesa08.motionkeys.nodes";

/// copies nodes of a tab's graph `scope` (a zone as a pair, with the connections between them) to the system clipboard
#[tauri::command]
pub fn graph_copy(tab: String, scope: Option<String>, nodes: Vec<String>) -> Result<(), String> {
    let payload = {
        let state = lock();
        let instance = state.instance(&tab).ok_or_else(|| format!("no tab '{}'", tab))?;
        clipboard::copy(&Graph::from_rf(&instance.rf_instance)?, scope.as_deref(), &nodes)?
    };
    let system = ClipboardContext::new().map_err(|e| format!("could not open the clipboard: {}", e))?;
    system.set_buffer(CLIPBOARD_TYPE, payload.into_bytes()).map_err(|e| format!("could not write the clipboard: {}", e))
}

/// copies the nodes, then removes them and the edges, one undo step
#[tauri::command]
pub async fn graph_cut(tab: String, scope: Option<String>, nodes: Vec<String>, edges: Vec<String>) -> Result<Applied, String> {
    // only edges is a plain delete, there's nothing to copy
    if !nodes.is_empty() {
        graph_copy(tab.clone(), scope.clone(), nodes.clone())?;
    }
    apply(
        &tab,
        scope,
        vec![Op::Cut {
            nodes,
            edges,
        }],
        None,
        true,
    )
}

/// pastes the nodes on the system clipboard into a tab's graph `scope`, centered on `position`, one undo step
#[tauri::command]
pub async fn graph_paste(tab: String, scope: Option<String>, position: Position) -> Result<Applied, String> {
    let no_nodes = |_| "the clipboard has no nodes".to_string();
    let system = ClipboardContext::new().map_err(|e| format!("could not open the clipboard: {}", e))?;
    let text = String::from_utf8(system.get_buffer(CLIPBOARD_TYPE).map_err(no_nodes)?).map_err(|_| "the clipboard has no nodes".to_string())?;
    apply(
        &tab,
        scope,
        vec![Op::Paste {
            text,
            position,
        }],
        None,
        true,
    )
}

fn apply(tab: &str, scope: Option<String>, ops: Vec<Op>, txn: Option<String>, commit_to_history: bool) -> Result<Applied, String> {
    let (applied, run) = {
        let mut state = lock();
        let instance = state.instance(tab).ok_or_else(|| format!("no tab '{}'", tab))?;
        let mut graph = Graph::from_rf(&instance.rf_instance)?;
        let specs = node_specs(&state.default_nodes);
        // the last run's results inside the edited group, for its dynamic outputs
        let results = match &scope {
            None => instance.executed_results.clone(),
            Some(id) => instance_path(&graph, &all_groups(&graph), id).map(|path| scoped_values(&instance.executed_results, &path)).unwrap_or_default(),
        };
        let ctx = Ctx {
            specs: &specs,
            results: &results,
        };

        // the project before, for naming what the ops removed
        let before = graph.clone();

        // every op works on the same copy, it's only kept if they all worked
        let mut added = Vec::new();
        for op in &ops {
            ops::apply(&mut graph, scope.as_deref(), op, &ctx, &mut added)?;
        }

        let capture = if !commit_to_history {
            Capture::Skip
        } else {
            // what it acted on, the history panel shows it
            let detail = ops.first().map_or(String::new(), |op| ops::describe(op, &before, &graph, scope.as_deref(), &specs, &added));
            let step = Step::new(ops.first().map_or("edit", Op::name), Source::Ui).detail(detail);
            Capture::Record(match &txn {
                Some(txn) => step.txn(txn.clone()),
                None => step,
            })
        };
        let affects_output = history::commit(&mut state, tab, graph.to_rf(), capture);
        let Some(instance) = state.instance(tab) else {
            return Err(format!("no tab '{}'", tab));
        };
        let applied = Applied {
            tab: tab.to_string(),
            graph_rev: instance.graph_rev,
            rf_instance: instance.rf_instance.clone(),
            added,
        };
        (applied, affects_output && !instance.execution_paused)
    };

    push_graph(tab);
    // the editor gets its answer right away (it may be waiting to grab what it added), the run happens after
    if run {
        tauri::async_runtime::spawn(crate::graph::execute::run_instance(tab.to_string(), true));
    }
    Ok(applied)
}
