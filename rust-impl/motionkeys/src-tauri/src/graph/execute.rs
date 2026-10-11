use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use crate::blender::curves::{curve_sources, sync_curves};
use crate::graph::builtin::all_groups;
use crate::graph::executors::io::node_error;
use crate::graph::executors::scene::{with_scene, written};
use crate::graph::model::{node_specs, Graph};
use crate::graph::run::{run, write_paths, Memo, RunCtx};
use crate::node_registry::get_node_registry;
use crate::state::{lock, update_state, LastWrite};
use crate::utils::log::log;

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

    // the live tab's graph may read keyframes from objects Blender doesn't send yet
    sync_curves(&id).await;

    // copy what the run needs out of the state
    let (rf_instance, scene, open_group, specs) = {
        let state = lock();
        let Some(instance) = state.instance(&id) else {
            return;
        };
        // only the live tab writes. with Blender not connected it still runs, so its scene writers show that
        if !realtime && state.connected && !state.is_live(&id) {
            log(format!("not writing to Blender, {} isn't the live tab", id));
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
            log(format!("not running {}, its graph can't be read: {}", id, e));
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
    let ((record, _), (objects, keyframes)) = with_scene(scene, curve_sources(&graph), || (run(&ctx, &graph), written()));
    memo_lock.insert(id.clone(), ctx.into_memo());
    drop(memo_lock);

    // the nodes writing to Blender whose last write isn't what the graph gives now
    let writers = write_paths(&graph, &groups, &specs);
    let stale: Vec<String> = writers.iter().filter(|path| !record.fresh.contains(path)).cloned().collect();

    let ms = now.elapsed().as_millis() as u64;
    log(format!("took {} ms to execute {} ({})", now.elapsed().as_nanos() as f32 / 1_000_000.0, id, if realtime { "realtime" } else { "write" }));

    // the tab on screen ran, or Blender was written to: the frontend gets the new state
    let send = {
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
        // a full run's writes, for the status bar and the run button
        let last_write = (!realtime).then(|| LastWrite {
            seq: 0,
            error: writers.iter().find_map(|path| results.get(path).and_then(node_error)).map(str::to_string),
            written: writers.iter().filter(|path| record.fresh.contains(path)).count(),
            objects,
            keyframes,
            ms,
        });
        instance.executed_results = results;
        instance.executed_inputs = record.inputs;
        instance.stale_writes = stale;
        let wrote = last_write.is_some();
        if let Some(mut last_write) = last_write {
            last_write.seq = state.last_write.as_ref().map_or(1, |last| last.seq + 1);
            state.last_write = Some(last_write);
        }
        shown || wrote
    };
    if send {
        update_state();
    }
}
