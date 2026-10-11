// each tab's undo history. every change to a tab's graph (`rf_instance`) goes through `commit`, which records it,
// see `graph::history` for how changes are kept and put back

use lazy_static::lazy_static;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};
use tauri::Emitter;

use super::{lock as lock_state, push_graph, AppState, WINDOW};
use crate::graph::execute::run_instance;
use crate::graph::history::{Diff, EntryInfo, History, HistoryInfo, Step};

/// sent to every window when the history of the tab on screen changes (or another tab is shown), payload `HistoryInfo`
pub const HISTORY_EVENT: &str = "history_changed";

lazy_static! {
    // by tab id, a tab without one has nothing to undo yet
    static ref HISTORY: Mutex<HashMap<String, History>> = Mutex::new(HashMap::new());
}

/// how a change to a tab's graph is recorded
pub enum Capture {
    /// an undo step (or part of one, see `Step`)
    Record(Step),
    /// changes the graph without committing it to history (no undo step): values filled in automatically, the viewport
    Skip,
}

// a panic while holding the lock still leaves usable histories
fn lock() -> MutexGuard<'static, HashMap<String, History>> {
    HISTORY.lock().unwrap_or_else(PoisonError::into_inner)
}

/// tells every window about the history of the tab on screen (the history panel)
fn notify(info: HistoryInfo) {
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
/// no such tab). call with the state locked, and send the graph to the frontend after
pub fn commit(state: &mut AppState, tab: &str, rf_instance: HashMap<String, Value>, capture: Capture) -> bool {
    let active = state.active_instance_id == tab;
    state.graph_rev += 1;
    let graph_rev = state.graph_rev;
    let Some(instance) = state.instance_mut(tab) else {
        return false;
    };
    let before = std::mem::replace(&mut instance.rf_instance, rf_instance);
    instance.graph_rev = graph_rev;
    let diff = Diff::between(&before, &instance.rf_instance);
    let affects_output = diff.affects_output();
    let Capture::Record(step) = capture else {
        return affects_output;
    };
    let info = {
        let mut histories = lock();
        let history = histories.entry(tab.to_string()).or_default();
        history.record(diff, step).then(|| history.info())
    };
    if let Some(info) = info.filter(|_| active) {
        notify(info);
    }
    affects_output
}

/// ends a tab's transaction, see `Step::txn`
pub fn end(tab: &str, txn: &str) {
    let info = {
        let mut histories = lock();
        let Some(history) = histories.get_mut(tab) else {
            return;
        };
        history.end(txn);
        history.info()
    };
    if lock_state().active_instance_id == tab {
        notify(info);
    }
}

/// a tab's history and graph, changed together with both locked. bumps the tab's graph revision when `change` says it
/// changed the graph, then sends the graph and history to the frontend. returns what `change` gave
fn change<T>(tab: &str, change: impl FnOnce(&mut History, &mut HashMap<String, Value>) -> Option<T>) -> Option<(T, bool)> {
    let (result, info, paused) = {
        let mut state = lock_state();
        let active = state.active_instance_id == tab;
        state.graph_rev += 1;
        let graph_rev = state.graph_rev;
        let instance = state.instance_mut(tab)?;
        let mut histories = lock();
        let history = histories.entry(tab.to_string()).or_default();
        let result = change(history, &mut instance.rf_instance)?;
        instance.graph_rev = graph_rev;
        (result, active.then(|| history.info()), instance.execution_paused)
    };
    push_graph(tab);
    if let Some(info) = info {
        notify(info);
    }
    Some((result, paused))
}

/// cancels a tab's transaction, its changes are undone. sends the graph to the frontend if anything changed, returns
/// whether the realtime graph should re-run
pub fn cancel(tab: &str, txn: &str) -> bool {
    change(tab, |history, project| history.cancel(txn, project).then_some(())).is_some_and(|(_, paused)| !paused)
}

/// undoes (or redoes) one step of a tab and sends its new graph to the frontend. returns the step and whether the
/// realtime graph should re-run (it changed what the graph computes and execution isn't paused), the caller re-runs it
pub fn step(tab: &str, redo: bool) -> Option<(EntryInfo, bool)> {
    let ((entry, affects_output), paused) = change(tab, |history, project| {
        if redo {
            history.redo(project)
        } else {
            history.undo(project)
        }
    })?;
    Some((entry, affects_output && !paused))
}

/// undoes or redoes a tab until the first `current` entries are done, sending the graph once. returns whether the
/// realtime graph should re-run
pub fn goto(tab: &str, current: usize) -> bool {
    change(tab, |history, project| (history.info().current != current).then(|| history.goto(current, project))).is_some_and(|(affects_output, paused)| affects_output && !paused)
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
    lock().get(tab).map_or_else(|| History::default().info(), History::info)
}

// MARK: - Commands
// undo and redo act on the tab on screen, the node editor's transactions name their tab

/// undoes the newest step of the tab on screen and re-runs it, returns the step (none if there was nothing to undo)
#[tauri::command]
pub async fn history_undo() -> Option<EntryInfo> {
    run_step(false).await
}

/// redoes the newest undone step of the tab on screen and re-runs it
#[tauri::command]
pub async fn history_redo() -> Option<EntryInfo> {
    run_step(true).await
}

/// the history of the tab on screen, for the history panel
#[tauri::command]
pub fn get_history() -> HistoryInfo {
    let id = lock_state().active_instance_id.clone();
    info(&id)
}

/// undoes or redoes the tab on screen until the first `current` entries are done (a row in the history panel), it
/// re-runs once
#[tauri::command]
pub async fn history_goto(current: usize) {
    let id = lock_state().active_instance_id.clone();
    if goto(&id, current) {
        run_instance(id, true).await;
    }
}

/// ends a transaction from the node editor (a grab was placed)
#[tauri::command]
pub fn history_end(tab: String, txn: String) {
    end(&tab, &txn);
}

/// cancels a transaction from the node editor (a grab was cancelled), what it did is undone
#[tauri::command]
pub async fn history_cancel(tab: String, txn: String) {
    if cancel(&tab, &txn) {
        run_instance(tab, true).await;
    }
}

async fn run_step(redo: bool) -> Option<EntryInfo> {
    let id = lock_state().active_instance_id.clone();
    let (entry, run) = step(&id, redo)?;
    if run {
        run_instance(id, true).await;
    }
    Some(entry)
}
