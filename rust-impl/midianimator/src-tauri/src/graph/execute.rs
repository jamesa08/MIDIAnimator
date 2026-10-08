use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use crate::graph::builtin::all_groups;
use crate::graph::executors::scene::with_scene;
use crate::graph::model::{node_specs, Graph};
use crate::graph::run::{run, Memo, RunCtx};
use crate::node_registry::get_node_registry;
use crate::state::{lock, update_state};

pub use crate::graph::run::panic_message;

// what nodes gave in each tab's last run (by tab id), a node given the same values again isn't run again
lazy_static::lazy_static! {
    static ref MEMO: Mutex<HashMap<String, Memo>> = Mutex::new(HashMap::new());
}

/// forgets what a tab's nodes gave (it was closed, or a project was loaded)
pub fn forget_memo(id: &str) {
    MEMO.lock().unwrap_or_else(PoisonError::into_inner).remove(id);
}

/// runs the tab on screen
#[tauri::command]
pub async fn execute_graph(realtime: bool) {
    let id = lock().active_instance_id.clone();
    run_instance(id, realtime).await;
}

/// runs a tab's graph on its scene data, the results go to that tab even if another one is shown by then.
/// a full run (`realtime` false) writes to Blender, so only the live tab does one
pub async fn run_instance(id: String, realtime: bool) {
    let now = std::time::Instant::now();

    // copy what the run needs out of the state
    let (rf_instance, scene, open_group, specs) = {
        let state = lock();
        let Some(instance) = state.instance(&id) else {
            return;
        };
        // only the live tab writes. with Blender not connected it still runs, so its scene writers show that
        if !realtime && state.connected && !state.is_live(&id) {
            println!("not writing to Blender, {} isn't live", id);
            return;
        }
        if state.connected {
            println!("CONNECTED TO 3D SOFTWARE {}", state.connected_application);
        }
        (instance.rf_instance.clone(), instance.scene_data.get("Scene").cloned(), instance.open_group.clone(), node_specs(&state.default_nodes))
    };

    // get current nodes & edges, a graph that can't be read doesn't run at all
    let graph = match Graph::from_rf(&rf_instance) {
        Ok(graph) => graph,
        Err(e) => {
            eprintln!("ERROR: not executing, {}", e);
            return;
        }
    };

    let registry = get_node_registry();
    let groups = all_groups(&graph);

    // one run at a time uses the memos, a second one waits. the state isn't locked while this one is
    let mut memo_lock = MEMO.lock().unwrap_or_else(PoisonError::into_inner);
    let memo = memo_lock.remove(&id).unwrap_or_default();
    let mut ctx = RunCtx::new(&specs, &registry, &groups, realtime).with_memo(memo);
    ctx.inspect = open_group;

    // failed nodes keep their error in the record, the rest of the graph still ran
    let (record, _) = with_scene(scene, || run(&ctx, &graph));
    memo_lock.insert(id.clone(), ctx.into_memo());
    drop(memo_lock);

    println!("took {} ms to execute {}", now.elapsed().as_nanos() as f32 / 1_000_000.0, id);

    let shown = {
        let mut state = lock();
        let shown = state.active_instance_id == id;
        // the tab was closed while it ran
        let Some(instance) = state.instance_mut(&id) else {
            drop(state);
            forget_memo(&id);
            return;
        };
        // a realtime run skips the nodes that write to Blender, they keep what their last write gave (its error or its success)
        let mut results = record.results;
        for path in record.skipped {
            if let Some(last) = instance.executed_results.get(&path) {
                results.insert(path, last.clone());
            }
        }
        instance.executed_results = results;
        instance.executed_inputs = record.inputs;
        shown
    };
    if shown {
        update_state();
    }
}
