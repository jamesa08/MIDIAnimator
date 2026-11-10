// each tab's undo history. every change to a tab's graph (`rf_instance`) goes through `commit`, which records it,
// see `graph::history` for how changes are kept and put back

use lazy_static::lazy_static;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use tauri::Emitter;

use super::{lock as lock_state, push_graph, AppState, WINDOW};
use crate::graph::execute::run_instance;
use crate::graph::history::{Diff, EntryInfo, History, HistoryInfo, Step};

/// sent to every window when a tab's history changes (or another tab is shown), payload `HistoryInfo`. the history panel
/// shows the newest one of the tab on screen
pub const HISTORY_EVENT: &str = "history_changed";

lazy_static! {
    // by tab id, a tab without one has nothing to undo yet
    static ref HISTORY: Mutex<HashMap<String, History>> = Mutex::new(HashMap::new());
}

/// the revision the last history was stamped with, only goes up
static REV: AtomicU64 = AtomicU64::new(0);

/// how a change to a tab's graph is recorded
pub enum Capture {
    /// an undo step (or part of one, see `Step`)
    Record(Step),
    /// changes the graph without committing it to history (no undo step): values filled in automatically, the viewport
    Skip,
}

/// what `commit` did
pub struct Committed {
    /// the change can change what the graph computes
    pub affects_output: bool,
    /// the tab's history if the change was recorded, send it with `notify` once the state is unlocked
    pub history: Option<HistoryInfo>,
}

// a panic while holding the lock still leaves usable histories
fn lock() -> MutexGuard<'static, HashMap<String, History>> {
    HISTORY.lock().unwrap_or_else(PoisonError::into_inner)
}

/// a tab's history with its tab and a new revision. call with the histories locked, so the revisions follow the order
/// the histories changed in (the windows can get them in another order, they keep the newest)
fn stamped(tab: &str, history: &History) -> HistoryInfo {
    HistoryInfo {
        tab: tab.to_string(),
        rev: REV.fetch_add(1, Ordering::Relaxed) + 1,
        ..history.info()
    }
}

/// tells every window about a tab's history (the history panel). call without holding the state lock: the window lock
/// is taken here, and a thread holding it may be waiting on the state
pub fn notify(info: HistoryInfo) {
    if let Some(window) = WINDOW.lock().unwrap_or_else(PoisonError::into_inner).as_ref() {
        window.emit(HISTORY_EVENT, info).ok();
    }
}

/// sends the history of the tab on screen, after another tab is shown. call without holding the state lock
pub fn notify_active() {
    let id = lock_state().active_instance_id.clone();
    notify(info(&id));
}

/// replaces a tab's graph and records the change, returns whether it changes what the graph computes (false if there's
/// no such tab) and the history to send. call with the state locked, then send the graph and the history once it's
/// unlocked
pub fn commit(state: &mut AppState, tab: &str, rf_instance: HashMap<String, Value>, capture: Capture) -> Committed {
    state.graph_rev += 1;
    let graph_rev = state.graph_rev;
    let Some(instance) = state.instance_mut(tab) else {
        return Committed {
            affects_output: false,
            history: None,
        };
    };
    let before = std::mem::replace(&mut instance.rf_instance, rf_instance);
    instance.graph_rev = graph_rev;
    let diff = Diff::between(&before, &instance.rf_instance);
    let affects_output = diff.affects_output();
    let Capture::Record(step) = capture else {
        return Committed {
            affects_output,
            history: None,
        };
    };
    let history = {
        let mut histories = lock();
        let history = histories.entry(tab.to_string()).or_default();
        history.record(diff, step).then(|| stamped(tab, history))
    };
    Committed {
        affects_output,
        history,
    }
}

/// ends a tab's transaction, see `Step::txn`
pub fn end(tab: &str, txn: &str) {
    let info = {
        let mut histories = lock();
        let Some(history) = histories.get_mut(tab) else {
            return;
        };
        history.end(txn);
        stamped(tab, history)
    };
    notify(info);
}

/// a tab's history and graph, changed together with both locked. bumps the tab's graph revision when `change` says it
/// changed the graph, then sends the graph and history to the frontend. returns what `change` gave
fn change<T>(tab: &str, change: impl FnOnce(&mut History, &mut HashMap<String, Value>) -> Option<T>) -> Option<(T, bool)> {
    let (result, info, paused) = {
        let mut state = lock_state();
        state.graph_rev += 1;
        let graph_rev = state.graph_rev;
        let instance = state.instance_mut(tab)?;
        let mut histories = lock();
        let history = histories.entry(tab.to_string()).or_default();
        let result = change(history, &mut instance.rf_instance)?;
        instance.graph_rev = graph_rev;
        (result, stamped(tab, history), instance.execution_paused)
    };
    push_graph(tab);
    notify(info);
    Some((result, paused))
}

/// cancels a tab's transaction, what its open entry did is undone. sends the graph to the frontend if anything changed.
/// returns whether everything the transaction did was undone (false if another step ended it first, what it did before
/// that step stays) and whether the realtime graph should re-run
pub fn cancel(tab: &str, txn: &str) -> (bool, bool) {
    let mut whole = false;
    let changed = change(tab, |history, project| {
        let current = history.info().current;
        whole = history.cancel(txn, project);
        (history.info().current != current).then_some(())
    });
    (whole, changed.is_some_and(|(_, paused)| !paused))
}

/// undoes (or redoes) one step of a tab and sends its new graph to the frontend. returns the step and whether the
/// realtime graph should re-run (it changed what the graph computes and execution isn't paused), the caller re-runs it
pub fn step(tab: &str, redo: bool) -> Option<(EntryInfo, bool)> {
    step_if(tab, redo, |_| Ok(())).ok().flatten()
}

/// `step` once `allow` has said yes to the step it would take (none if there's nothing to step), checked with the
/// history locked so no other step can come in between. an error from `allow` is given back, nothing changes
pub fn step_if(tab: &str, redo: bool, allow: impl FnOnce(Option<(EntryInfo, bool)>) -> Result<(), String>) -> Result<Option<(EntryInfo, bool)>, String> {
    let mut refused = None;
    let stepped = change(tab, |history, project| {
        if let Err(e) = allow(history.top(redo)) {
            refused = Some(e);
            return None;
        }
        if redo {
            history.redo(project)
        } else {
            history.undo(project)
        }
    });
    match refused {
        Some(e) => Err(e),
        None => Ok(stepped.map(|((entry, affects_output), paused)| (entry, affects_output && !paused))),
    }
}

/// undoes or redoes a tab until the first `current` entries are done, or until `entry` (an id) is the last one done,
/// sending the graph once. returns whether the realtime graph should re-run, an error if `entry` isn't in the history
/// any more
pub fn goto(tab: &str, current: usize, entry: Option<u64>) -> Result<bool, String> {
    let mut missing = false;
    let ran = change(tab, |history, project| {
        let info = history.info();
        let target = match entry {
            None => current,
            Some(id) => match info.entries.iter().position(|e| e.id == id) {
                Some(at) => at + 1,
                None => {
                    missing = true;
                    return None;
                }
            },
        };
        (info.current != target).then(|| history.goto(target, project))
    });
    if missing {
        return Err("that step is no longer in the history".to_string());
    }
    Ok(ran.is_some_and(|(affects_output, paused)| affects_output && !paused))
}

/// forgets a closed tab's history
pub fn forget(tab: &str) {
    lock().remove(tab);
}

/// forgets every tab's history (a project was loaded or a new one started)
pub fn clear_all() {
    lock().clear();
}

/// a tab's history
pub fn info(tab: &str) -> HistoryInfo {
    let histories = lock();
    match histories.get(tab) {
        Some(history) => stamped(tab, history),
        None => stamped(tab, &History::default()),
    }
}

// MARK: - Commands
// undo, redo and going to a row name the tab the user sees and are refused once another one is on screen, the node
// editor's transactions name their tab. the realtime graph re-runs after the reply, so the next edit isn't held up

/// the tab a command from a window acts on: the tab it names if that's still on screen (the tab on screen if it names
/// none). another tab is refused, the user was looking at that one
fn on_screen(tab: Option<String>) -> Result<String, String> {
    let active = lock_state().active_instance_id.clone();
    match tab {
        Some(tab) if tab != active => Err(format!("{} isn't on screen any more", tab)),
        _ => Ok(active),
    }
}

/// undoes the newest step of the tab on screen and re-runs it, returns the step (none if there was nothing to undo).
/// the menu's when no window is focused, windows use `history_step`
#[tauri::command]
pub async fn history_undo() -> Option<EntryInfo> {
    run_step(None, false).ok().flatten()
}

/// redoes the newest undone step of the tab on screen and re-runs it
#[tauri::command]
pub async fn history_redo() -> Option<EntryInfo> {
    run_step(None, true).ok().flatten()
}

/// undoes (or with `redo`, redoes) one step of `tab` and re-runs it, returns the step (none if there was nothing to
/// undo). refused if `tab` isn't on screen any more
#[tauri::command]
pub async fn history_step(tab: Option<String>, redo: bool) -> Result<Option<EntryInfo>, String> {
    run_step(tab, redo)
}

/// the history of `tab` (the tab on screen if none), for the history panel
#[tauri::command]
pub fn get_history(tab: Option<String>) -> HistoryInfo {
    let id = tab.unwrap_or_else(|| lock_state().active_instance_id.clone());
    info(&id)
}

/// undoes or redoes `tab` until the first `current` entries are done, or `entry` (an id) is the last one done (a row in
/// the history panel), it re-runs once. refused if `tab` isn't on screen any more, or `entry` isn't in its history
#[tauri::command]
pub async fn history_goto(tab: Option<String>, current: usize, entry: Option<u64>) -> Result<(), String> {
    let id = on_screen(tab)?;
    if goto(&id, current, entry)? {
        tauri::async_runtime::spawn(run_instance(id, true));
    }
    Ok(())
}

/// ends a transaction from the node editor (a grab was placed)
#[tauri::command]
pub fn history_end(tab: String, txn: String) {
    end(&tab, &txn);
    // a drag's steps didn't run the graph, it runs once now
    let held = lock_state().instance_mut(&tab).is_some_and(|instance| std::mem::take(&mut instance.run_held));
    if held {
        tauri::async_runtime::spawn(run_instance(tab, true));
    }
}

/// cancels a transaction from the node editor (a grab was cancelled), what it did is undone. returns false if it
/// couldn't all be: another step ended the transaction first, the editor removes what it added itself
#[tauri::command]
pub async fn history_cancel(tab: String, txn: String) -> bool {
    let (whole, run) = cancel(&tab, &txn);
    if run {
        tauri::async_runtime::spawn(run_instance(tab, true));
    }
    whole
}

/// steps the tab a window names, the realtime graph re-runs after
fn run_step(tab: Option<String>, redo: bool) -> Result<Option<EntryInfo>, String> {
    let id = on_screen(tab)?;
    let Some((entry, run)) = step(&id, redo) else {
        return Ok(None);
    };
    if run {
        tauri::async_runtime::spawn(run_instance(id, true));
    }
    Ok(Some(entry))
}
