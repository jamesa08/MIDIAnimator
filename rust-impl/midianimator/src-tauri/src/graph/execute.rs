use std::sync::{Mutex, PoisonError};

use crate::graph::builtin::all_groups;
use crate::graph::model::{node_specs, Graph};
use crate::graph::run::{run, Memo, RunCtx};
use crate::node_registry::get_node_registry;
use crate::state::{update_state, STATE};

pub use crate::graph::run::panic_message;

// what nodes gave in the last run, a node given the same values again isn't run again
lazy_static::lazy_static! {
    static ref MEMO: Mutex<Memo> = Mutex::new(Memo::default());
}

#[tauri::command]
pub async fn execute_graph(realtime: bool) {
    let now = std::time::Instant::now();

    // copy the state, a poisoned lock (a panic somewhere else) still has a usable graph
    let state = STATE.lock().unwrap_or_else(PoisonError::into_inner).clone();

    if state.connected {
        println!("CONNECTED TO 3D SOFTWARE {}", state.connected_application);
    }

    // get current nodes & edges, a graph that can't be read doesn't run at all
    // note: an empty rf_instance (nothing pushed from the frontend yet) parses as an empty graph
    let graph = match Graph::from_rf(&state.rf_instance) {
        Ok(graph) => graph,
        Err(e) => {
            eprintln!("ERROR: not executing, {}", e);
            return;
        }
    };

    let specs = node_specs(&state.default_nodes);
    let registry = get_node_registry();
    let groups = all_groups(&graph);

    // one run at a time uses the memo, a second one waits
    let mut memo_lock = MEMO.lock().unwrap_or_else(PoisonError::into_inner);
    let mut ctx = RunCtx::new(&specs, &registry, &groups, realtime).with_memo(std::mem::take(&mut *memo_lock));
    ctx.inspect = state.open_group.clone();

    // failed nodes keep their error in the record, the rest of the graph still ran
    let (record, _) = run(&ctx, &graph);
    *memo_lock = ctx.into_memo();
    drop(memo_lock);

    println!("took {} ms to execute", now.elapsed().as_nanos() as f32 / 1_000_000.0);

    let mut state = STATE.lock().unwrap_or_else(PoisonError::into_inner);
    state.executed_results = record.results;
    state.executed_inputs = record.inputs;
    drop(state);
    update_state();
}
