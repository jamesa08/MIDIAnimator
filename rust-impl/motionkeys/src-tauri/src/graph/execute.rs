use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, PoisonError};

use serde::Serialize;

use crate::blender::curves::{curve_sources, sync_curves};
use crate::graph::builtin::all_groups;
use crate::graph::executors::io::node_error;
use crate::graph::executors::scene::{with_scene, written};
use crate::graph::model::{node_specs, Graph};
use crate::graph::run::{run, write_paths, Memo, NodeTiming, RunCtx, Status};
use crate::node_registry::get_node_registry;
use crate::state::{lock, update_state, LastWrite};
use crate::utils::console::{self, Entry, Level, Source};
use crate::utils::log::log;

pub use crate::graph::run::panic_message;

/// sent to every window after a tab ran, payload the tab's id. its timings are read with `get_run_timings`
pub const RUN_TIMINGS_EVENT: &str = "run_timings";

// what nodes gave in each tab's last run (by tab id), a node given the same values again isn't run again.
// and how each tab's last run went, node by node
lazy_static::lazy_static! {
    static ref MEMO: Mutex<HashMap<String, Memo>> = Mutex::new(HashMap::new());
    static ref TIMINGS: Mutex<HashMap<String, RunTimings>> = Mutex::new(HashMap::new());
}

/// a tab's last run: whether it was realtime, how long it took in all (in milliseconds) and how each node went
#[derive(Serialize, Clone, Debug)]
pub struct RunTimings {
    pub tab: String,
    pub realtime: bool,
    pub ms: f64,
    pub nodes: Vec<NodeTiming>,
}

/// forgets what a tab's nodes gave and how its last run went (it was closed, or a project was loaded)
pub fn forget_memo(id: &str) {
    MEMO.lock().unwrap_or_else(PoisonError::into_inner).remove(id);
    TIMINGS.lock().unwrap_or_else(PoisonError::into_inner).remove(id);
}

/// how a tab's last run went, none before it ran
#[tauri::command]
pub fn get_run_timings(tab: String) -> Option<RunTimings> {
    TIMINGS.lock().unwrap_or_else(PoisonError::into_inner).get(&tab).cloned()
}

// "1 keyframe", "2 keyframes"
fn plural(count: usize, word: &str) -> String {
    format!(
        "{} {}{}",
        count,
        word,
        if count == 1 {
            ""
        } else {
            "s"
        }
    )
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
        console::push(
            Entry::new(
                Level::Info,
                Source::Run,
                if realtime {
                    "Run started"
                } else {
                    "Write started"
                },
            )
            .tab(&id),
        );
        (instance.rf_instance.clone(), instance.scene_data.get("Scene").cloned(), instance.open_group.clone(), node_specs(&state.default_nodes))
    };

    // get current nodes & edges, a graph that can't be read doesn't run at all
    let graph = match Graph::from_rf(&rf_instance) {
        Ok(graph) => graph,
        Err(e) => {
            log(format!("not running {}, its graph can't be read: {}", id, e));
            console::push(Entry::new(Level::Error, Source::Run, format!("The graph can't be read: {}", e)).tab(&id));
            return;
        }
    };

    let registry = get_node_registry();
    let groups = all_groups(&graph);

    // one run at a time uses the memos, a second one waits. the state isn't locked while this one is
    let mut memo_lock = MEMO.lock().unwrap_or_else(PoisonError::into_inner);
    let mut memo = memo_lock.remove(&id).unwrap_or_default();
    memo.set_fps(scene.as_ref().and_then(|scene| scene.fps));
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
    let total = now.elapsed().as_secs_f64() * 1000.0;
    log(format!(
        "took {} ms to execute {} ({})",
        total,
        id,
        if realtime {
            "realtime"
        } else {
            "write"
        }
    ));

    // the tab on screen ran, or Blender was written to: the frontend gets the new state
    let mut write = None;
    let failed_writer: Option<String>;
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
        failed_writer = writers.iter().find(|path| results.get(*path).and_then(node_error).is_some()).cloned();
        let last_write = (!realtime).then(|| LastWrite {
            seq: 0,
            error: failed_writer.as_ref().and_then(|path| results.get(path).and_then(node_error)).map(str::to_string),
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
            write = Some(last_write.clone());
            state.last_write = Some(last_write);
        }
        shown || wrote
    };
    if send {
        update_state();
    }
    log_run(&id, realtime, total, record.timings, write, failed_writer);
}

/// the console's entries for a run: when it finished and how many nodes ran, the nodes that failed, what a full run
/// wrote. keeps the run's timings for the console window (`get_run_timings`)
fn log_run(id: &str, realtime: bool, total: f64, timings: Vec<NodeTiming>, write: Option<LastWrite>, failed_writer: Option<String>) {
    let ran = timings.iter().filter(|t| !t.cached && t.status != Status::Skipped).count();
    let cached = timings.iter().filter(|t| t.cached).count();
    let mut entries = vec![Entry::new(
        Level::Info,
        Source::Run,
        format!(
            "{} finished in {:.2} ms, {} ran, {} cached",
            if realtime {
                "Run"
            } else {
                "Write"
            },
            total,
            ran,
            cached
        ),
    )
    .tab(id)
    .ms(total)];

    // a realtime run (every edit) only says what failed since the last one, a full run says all of it. the error of a
    // group or a zone is the one inside it, the scene writer that failed is said below
    let mut timings_lock = TIMINGS.lock().unwrap_or_else(PoisonError::into_inner);
    let failed = |t: &NodeTiming| t.status == Status::Failed && t.own_error;
    let before: HashSet<(&str, Option<&str>)> = match timings_lock.get(id).filter(|_| realtime) {
        Some(last) => last.nodes.iter().filter(|t| failed(t)).map(|t| (t.node.as_str(), t.error.as_deref())).collect(),
        None => HashSet::new(),
    };
    for timing in timings.iter().filter(|t| failed(t) && !before.contains(&(t.node.as_str(), t.error.as_deref())) && Some(&t.node) != failed_writer.as_ref().filter(|_| !realtime)) {
        entries.push(Entry::new(Level::Error, Source::Node, format!("{}: {}", timing.label, timing.error.as_deref().unwrap_or_default())).node(id, &timing.node).ms(timing.ms));
    }
    drop(before);

    // the status bar's message for a write (lastWriteMessage in src/components/StatusBar.tsx)
    if let Some(write) = write {
        entries.push(match (&write.error, failed_writer) {
            (Some(error), Some(writer)) => Entry::new(Level::Error, Source::Blender, format!("Scene Writer failed: {}", error)).node(id, &writer),
            (Some(error), None) => Entry::new(Level::Error, Source::Blender, format!("Scene Writer failed: {}", error)).tab(id),
            (None, _) if write.written == 0 => Entry::new(Level::Info, Source::Blender, "Nothing to write").tab(id),
            (None, _) => Entry::new(Level::Info, Source::Blender, format!("Wrote {} to {} in {} ms", plural(write.keyframes, "keyframe"), plural(write.objects, "object"), write.ms)).tab(id).ms(write.ms as f64),
        });
    }

    // debug mode: every node, ran or cached, how long and how it went
    if console::debug() {
        for timing in &timings {
            let how = if timing.cached {
                "cached"
            } else {
                "ran"
            };
            let level = if timing.status == Status::Failed {
                Level::Warn
            } else {
                Level::Debug
            };
            let status = match timing.status {
                Status::Ok => "ok",
                Status::Failed => "failed",
                Status::Waiting => "waiting",
                Status::Skipped => "skipped",
            };
            entries.push(Entry::new(level, Source::Node, format!("{} {} in {:.3} ms, {}", timing.label, how, timing.ms, status)).node(id, &timing.node).ms(timing.ms));
        }
    }
    timings_lock.insert(
        id.to_string(),
        RunTimings {
            tab: id.to_string(),
            realtime,
            ms: total,
            nodes: timings,
        },
    );
    drop(timings_lock);
    console::add(entries);
    console::emit(RUN_TIMINGS_EVENT, id);
}
