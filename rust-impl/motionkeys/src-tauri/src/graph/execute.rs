use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::pin::pin;
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::Serialize;
use serde_json::Value;
use tokio::sync::Notify;

use crate::bridge::curves::{curve_sources, sync_curves};
use crate::bridge::scene_data::on_screen;
use crate::graph::builtin::all_groups;
use crate::graph::executors::io::node_error;
use crate::graph::executors::scene::{with_scene, written, written_objects};
use crate::graph::model::{node_specs, Graph, NodeSpec};
use crate::graph::run::{run, write_paths, Memo, NodeTiming, Record, RunCtx, Status};
use crate::node_registry::get_node_registry;
use crate::scene_generics::Scene;
use crate::state::{lock, update_state, LastWrite};
use crate::utils::console::{self, Entry, Level, Source};
use crate::utils::log::log;

pub use crate::graph::run::panic_message;

/// sent to every window after a tab ran, payload the tab's id. its timings are read with `get_run_timings`
pub const RUN_TIMINGS_EVENT: &str = "run_timings";

// what nodes gave in each tab's last run (by tab id), a node given the same values again isn't run again.
// and how each tab's last run went, node by node. each tab's runs (`Runs`) and what its memo holds of its writes
// (`Writes`)
lazy_static::lazy_static! {
    static ref MEMO: Mutex<HashMap<String, Memo>> = Mutex::new(HashMap::new());
    static ref TIMINGS: Mutex<HashMap<String, RunTimings>> = Mutex::new(HashMap::new());
    static ref RUNS: Mutex<HashMap<String, Runs>> = Mutex::new(HashMap::new());
    static ref WRITES: Mutex<HashMap<String, Writes>> = Mutex::new(HashMap::new());
}

// woken when a run finishes, the requests waiting for a run check whether it covered them
static RAN: Notify = Notify::const_new();

/// a tab's last run: whether it was realtime, how long it took in all (in milliseconds) and how each node went
#[derive(Serialize, Clone, Debug)]
pub struct RunTimings {
    pub tab: String,
    pub realtime: bool,
    pub ms: f64,
    pub nodes: Vec<NodeTiming>,
}

/// a tab's runs, one at a time. requests are numbered, and a run covers every request made before it took its snapshot
/// of the tab: the requests made while one runs wait for the next one, which covers them all. a full run covers the
/// realtime requests too
#[derive(Default)]
struct Runs {
    /// the newest request's number
    requested: u64,
    /// the newest request for a full run (it writes to Blender), 0 before there's one
    full_requested: u64,
    /// every realtime request up to this number is covered by a run that finished
    realtime_done: u64,
    /// every full request up to this number is covered, by a full run or by its being turned away
    full_done: u64,
    /// why the newest runs didn't run (none if they ran), by the newest request each covered: what the realtime requests
    /// they covered get. a request waiting can wake after the run after its own finished, so the last few are kept
    error: VecDeque<(u64, Option<String>)>,
    /// how the newest full runs went, by the newest request each covered: what the full requests they covered get
    full: VecDeque<(u64, Result<Option<LastWrite>, String>)>,
    /// a run of the tab is going
    running: bool,
    /// the newest request the tab's results are from, a run covering only older ones never replaces them
    applied: u64,
}

fn runs() -> MutexGuard<'static, HashMap<String, Runs>> {
    RUNS.lock().unwrap_or_else(PoisonError::into_inner)
}

/// how many runs' outcomes a tab keeps for the requests still waiting on them
const OUTCOMES_KEPT: usize = 8;

/// keeps a run's outcome by the newest request it covered, dropping the oldest past `OUTCOMES_KEPT`
fn keep<T>(outcomes: &mut VecDeque<(u64, T)>, upto: u64, outcome: T) {
    outcomes.push_back((upto, outcome));
    while outcomes.len() > OUTCOMES_KEPT {
        outcomes.pop_front();
    }
}

/// the outcome of the run that covered `request`: the first one that covered it or a newer request. the newest one when
/// it's no longer kept
fn covering<T>(outcomes: &VecDeque<(u64, T)>, request: u64) -> Option<&T> {
    outcomes.iter().find(|(upto, _)| *upto >= request).or(outcomes.back()).map(|(_, outcome)| outcome)
}

/// what a tab's memo holds of its writes to Blender: the objects they went to, and whether the memo forgets them before
/// the tab's next run (Blender reconnected or opened another file, or another tab wrote to one of those objects)
#[derive(Default)]
struct Writes {
    objects: BTreeSet<String>,
    forget: bool,
}

fn writes() -> MutexGuard<'static, HashMap<String, Writes>> {
    WRITES.lock().unwrap_or_else(PoisonError::into_inner)
}

/// every tab's nodes writing to Blender are stale until they write again: Blender reconnected or opened another file,
/// it may not have what they wrote. each tab's memo forgets its writes before its next run
pub fn forget_writes() {
    for writes in writes().values_mut() {
        *writes = Writes {
            forget: true,
            ..Writes::default()
        };
    }
}

// a full run of `id` sent keyframes to `objects`: another tab whose writes went to one of them forgets its writes,
// Blender has this tab's keyframes there now
fn note_writes(id: &str, objects: BTreeSet<String>) {
    let mut writes = writes();
    for (_, other) in writes.iter_mut().filter(|(tab, other)| tab.as_str() != id && !other.objects.is_disjoint(&objects)) {
        *other = Writes {
            forget: true,
            ..Writes::default()
        };
    }
    writes.entry(id.to_string()).or_default().objects.extend(objects);
}

/// forgets what a tab's nodes gave and how its last run went (it was closed, or a project was loaded)
pub fn forget_memo(id: &str) {
    MEMO.lock().unwrap_or_else(PoisonError::into_inner).remove(id);
    TIMINGS.lock().unwrap_or_else(PoisonError::into_inner).remove(id);
    writes().remove(id);
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

/// runs the tab on screen. `tab` is the tab the frontend's button is in, another one on screen by now isn't run. a full
/// run gives what it wrote, an error when it didn't run (`run_tab`)
#[tauri::command]
pub async fn execute_graph(realtime: bool, tab: Option<String>) -> Result<Option<LastWrite>, String> {
    let id = {
        let state = lock();
        on_screen(&state, tab.as_deref())?;
        state.active_instance_id.clone()
    };
    run_tab(id, realtime).await
}

/// runs a tab's graph on its scene data, the results go to that tab even if another one is shown by then.
/// a full run (`realtime` false) writes to Blender, so only the live tab does one. returns once a run that started
/// after the call has finished (`run_tab`)
pub async fn run_instance(id: String, realtime: bool) {
    run_tab(id, realtime).await.ok();
}

/// `run_instance`, and how it went: a full run gives what it wrote (its error when a node writing to Blender failed), a
/// realtime one nothing. an error when nothing ran: the tab was closed, its graph can't be read, or it isn't live and a
/// full run was asked for. a tab runs once at a time, a request made while it runs waits for the run after it, which
/// covers every request made before it started
pub async fn run_tab(id: String, realtime: bool) -> Result<Option<LastWrite>, String> {
    // this request's number
    let request = {
        let mut all = runs();
        let runs = all.entry(id.clone()).or_default();
        runs.requested += 1;
        if !realtime {
            runs.full_requested = runs.requested;
        }
        runs.requested
    };
    loop {
        // listening before the check, so a run finishing right after it still wakes this request
        let mut ran = pin!(RAN.notified());
        ran.as_mut().enable();
        let run = {
            let mut all = runs();
            let runs = all.entry(id.clone()).or_default();
            // a run that started after the request has finished
            if realtime && runs.realtime_done >= request {
                return covering(&runs.error, request).cloned().flatten().map_or(Ok(None), Err);
            }
            if !realtime && runs.full_done >= request {
                return covering(&runs.full, request).cloned().unwrap_or(Ok(None));
            }
            // with none going this request runs the tab, otherwise it waits for the one going to finish
            if runs.running {
                false
            } else {
                runs.running = true;
                true
            }
        };
        if run {
            run_next(&id).await;
        } else {
            ran.await;
        }
    }
}

/// how a run went
enum Outcome {
    /// a full run of a tab that isn't live, nothing ran: why
    NotLive(String),
    /// it ran (a full run gives what it wrote), or it can't run: the tab was closed, its graph can't be read
    Done(Result<Option<LastWrite>, String>),
}

// one run of a tab, by the request that found none going. it covers every request made before it takes its snapshot,
// as a full run when one of them asked for one
async fn run_next(id: &str) {
    // the next request runs the tab however this run ends (its request dropped meanwhile, or a panic)
    struct Running<'a>(&'a str);
    impl Drop for Running<'_> {
        fn drop(&mut self) {
            if let Some(runs) = runs().get_mut(self.0) {
                runs.running = false;
            }
            RAN.notify_waiters();
        }
    }
    let _running = Running(id);

    // the live tab's graph may read keyframes from objects Blender doesn't send yet
    sync_curves(id).await;

    // the requests made so far, the run takes its snapshot after this
    let (upto, full) = {
        let mut all = runs();
        let runs = all.entry(id.to_string()).or_default();
        (runs.requested, runs.full_requested > runs.full_done)
    };
    let outcome = run_once(id, !full, upto).await;

    // what the requests it covered get
    let mut all = runs();
    let runs = all.entry(id.to_string()).or_default();
    match outcome {
        // a full run turned away didn't run, the realtime requests it would have covered get a run of their own
        Outcome::NotLive(why) => {
            runs.full_done = upto;
            keep(&mut runs.full, upto, Err(why));
        }
        Outcome::Done(outcome) => {
            runs.realtime_done = upto;
            keep(&mut runs.error, upto, outcome.as_ref().err().cloned());
            if full {
                runs.full_done = upto;
                keep(&mut runs.full, upto, outcome);
            }
        }
    }
    drop(all);
}

/// what a run gives back from its thread
struct Ran {
    record: Record,
    /// paths of the nodes writing to Blender
    writers: Vec<String>,
    /// how many objects and keyframes the scene writers wrote, and the objects they sent keyframes to
    objects: usize,
    keyframes: usize,
    written_to: BTreeSet<String>,
}

// runs a tab once on a snapshot of it taken now. `seq` is the newest request the run covers, its results replace those
// of a run covering older ones only
async fn run_once(id: &str, realtime: bool, seq: u64) -> Outcome {
    let now = std::time::Instant::now();

    // copy what the run needs out of the state
    let (rf_instance, scene, open_group, specs) = {
        let state = lock();
        let Some(instance) = state.instance(id) else {
            return Outcome::Done(Err(format!("no tab '{}'", id)));
        };
        // only the live tab writes. with Blender not connected it still runs, so its scene writers show that
        if !realtime && state.connected && !state.is_live(id) {
            log(format!("not writing keyframes, {} isn't the live tab", id));
            return Outcome::NotLive(format!("{} isn't the live tab, only the live tab writes keyframes", instance.name()));
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
            .tab(id),
        );
        (instance.rf_instance.clone(), instance.scene.clone(), instance.open_group.clone(), node_specs(&state.default_nodes))
    };

    // the graph runs on a thread of its own, a write to Blender can take a minute
    let tab = id.to_string();
    let ran = match tokio::task::spawn_blocking(move || run_graph(&tab, realtime, &rf_instance, scene, open_group, &specs)).await {
        Ok(ran) => ran,
        // a panic carries on in the request that ran it, as it did before the run had a thread of its own
        Err(e) => match e.try_into_panic() {
            Ok(panic) => std::panic::resume_unwind(panic),
            Err(e) => return Outcome::Done(Err(e.to_string())),
        },
    };
    // a graph that can't be read doesn't run at all
    let Ran {
        record,
        writers,
        objects,
        keyframes,
        written_to,
    } = match ran {
        Ok(ran) => ran,
        Err(e) => {
            log(format!("not running {}, its graph can't be read: {}", id, e));
            console::push(Entry::new(Level::Error, Source::Run, format!("The graph can't be read: {}", e)).tab(id));
            return Outcome::Done(Err(format!("the graph can't be read: {}", e)));
        }
    };

    // the nodes writing to Blender whose last write isn't what the graph gives now
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
        let Some(instance) = state.instance_mut(id) else {
            drop(state);
            forget_memo(id);
            return Outcome::Done(Err(format!("{} was closed while it ran", id)));
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
        // an older run finishing late (its request was dropped while it ran) doesn't replace a newer one's results
        let newest = {
            let mut all = runs();
            let runs = all.entry(id.to_string()).or_default();
            let newest = runs.applied <= seq;
            runs.applied = runs.applied.max(seq);
            newest
        };
        if newest {
            instance.executed_results = results;
            instance.executed_inputs = record.inputs;
            instance.stale_writes = stale;
        }
        let wrote = last_write.is_some();
        if let Some(mut last_write) = last_write {
            last_write.seq = state.last_write.as_ref().map_or(1, |last| last.seq + 1);
            write = Some(last_write.clone());
            state.last_write = Some(last_write);
        }
        shown || wrote
    };
    // Blender has this tab's keyframes on the objects it wrote to, other tabs' writes there are gone
    if !realtime {
        note_writes(id, written_to);
    }
    if send {
        update_state();
    }
    log_run(id, realtime, total, record.timings, write.clone(), failed_writer);
    Outcome::Done(Ok(write))
}

// runs a tab's graph with the tab's memo, on the run's own thread. an error if the graph can't be read
fn run_graph(id: &str, realtime: bool, rf_instance: &HashMap<String, Value>, scene: Option<Scene>, open_group: String, specs: &[NodeSpec]) -> Result<Ran, String> {
    // get current nodes & edges
    let graph = Graph::from_rf(rf_instance)?;
    let registry = get_node_registry();
    let groups = all_groups(&graph);

    // the tab's memo is out of the map while it runs, other tabs' runs don't wait for this one
    let mut memo = MEMO.lock().unwrap_or_else(PoisonError::into_inner).remove(id).unwrap_or_default();
    // Blender may not have what its nodes wrote anymore
    if let Some(writes) = writes().get_mut(id).filter(|writes| writes.forget) {
        *writes = Writes::default();
        memo.forget_writes();
    }
    memo.set_fps(scene.as_ref().and_then(|scene| scene.fps));
    let mut ctx = RunCtx::new(specs, &registry, &groups, realtime).with_memo(memo);
    ctx.inspect = open_group;

    // failed nodes keep their error in the record, the rest of the graph still ran
    let sources = curve_sources(&graph, scene.as_ref());
    let ((record, _), (objects, keyframes), written_to) = with_scene(scene, sources, || (run(&ctx, &graph), written(), written_objects()));
    MEMO.lock().unwrap_or_else(PoisonError::into_inner).insert(id.to_string(), ctx.into_memo());

    Ok(Ran {
        record,
        writers: write_paths(&graph, &groups, specs),
        objects,
        keyframes,
        written_to,
    })
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
            (Some(error), Some(writer)) => Entry::new(Level::Error, Source::Bridge, format!("Scene Writer failed: {}", error)).node(id, &writer),
            (Some(error), None) => Entry::new(Level::Error, Source::Bridge, format!("Scene Writer failed: {}", error)).tab(id),
            (None, _) if write.written == 0 => Entry::new(Level::Info, Source::Bridge, "Nothing to write").tab(id),
            (None, _) => Entry::new(Level::Info, Source::Bridge, format!("Wrote {} to {} in {} ms", plural(write.keyframes, "keyframe"), plural(write.objects, "object"), write.ms)).tab(id).ms(write.ms as f64),
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
