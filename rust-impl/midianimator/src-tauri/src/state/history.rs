// the project's undo history. every change to the project graph (`rf_instance`) goes through `commit`, which records it,
// see `graph::history` for how changes are kept and put back

use lazy_static::lazy_static;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};
use tauri::Emitter;

use super::{update_state, AppState, STATE, WINDOW};
use crate::graph::history::{Diff, EntryInfo, History, HistoryInfo, Step};

/// sent to every window when the history changes, payload `HistoryInfo`
pub const HISTORY_EVENT: &str = "history_changed";

lazy_static! {
    static ref HISTORY: Mutex<History> = Mutex::new(History::default());
}

/// how a change to the project graph is recorded
pub enum Capture {
    /// an undo step (or part of one, see `Step`)
    Record(Step),
    /// changes the graph without an undo step: values filled in automatically, migrations
    Ambient,
}

// a panic while holding the lock still leaves a usable history
fn lock() -> MutexGuard<'static, History> {
    HISTORY.lock().unwrap_or_else(PoisonError::into_inner)
}

/// tells every window about the history (the history panel)
fn notify(info: HistoryInfo) {
    if let Some(window) = WINDOW.lock().unwrap_or_else(PoisonError::into_inner).as_ref() {
        window.emit(HISTORY_EVENT, info).ok();
    }
}

/// replaces the project graph and records the change. call with the state locked, and send the state to the frontend after
pub fn commit(state: &mut AppState, rf_instance: HashMap<String, Value>, capture: Capture) {
    let before = std::mem::replace(&mut state.rf_instance, rf_instance);
    let Capture::Record(step) = capture else {
        return;
    };
    let diff = Diff::between(&before, &state.rf_instance);
    let info = {
        let mut history = lock();
        history.record(diff, step).then(|| history.info())
    };
    if let Some(info) = info {
        notify(info);
    }
}

/// ends a transaction, see `Step::txn`
pub fn end(txn: &str) {
    let info = {
        let mut history = lock();
        history.end(txn);
        history.info()
    };
    notify(info);
}

/// cancels a transaction, its changes are undone. sends the state to the frontend if anything changed
pub fn cancel(txn: &str) -> bool {
    let cancelled = {
        let mut state = STATE.lock().unwrap_or_else(PoisonError::into_inner);
        let mut history = lock();
        history.cancel(txn, &mut state.rf_instance).then(|| history.info())
    };
    let Some(info) = cancelled else {
        return false;
    };
    update_state();
    notify(info);
    true
}

/// undoes (or redoes) one step and sends the new graph to the frontend. returns the step and whether execution is paused,
/// the caller re-runs the realtime graph
pub fn step(redo: bool) -> Option<(EntryInfo, bool)> {
    let (entry, paused, info) = {
        let mut state = STATE.lock().unwrap_or_else(PoisonError::into_inner);
        let mut history = lock();
        let entry = if redo {
            history.redo(&mut state.rf_instance)
        } else {
            history.undo(&mut state.rf_instance)
        }?;
        (entry, state.execution_paused, history.info())
    };
    update_state();
    notify(info);
    Some((entry, paused))
}

/// forgets the history (a project was loaded or a new one started)
pub fn clear() {
    let info = {
        let mut history = lock();
        history.clear();
        history.info()
    };
    notify(info);
}

pub fn info() -> HistoryInfo {
    lock().info()
}

// MARK: - Commands

/// undoes the newest step and re-runs the realtime graph, returns the step (none if there was nothing to undo)
#[tauri::command]
pub async fn history_undo() -> Option<EntryInfo> {
    run_step(false).await
}

/// redoes the newest undone step and re-runs the realtime graph
#[tauri::command]
pub async fn history_redo() -> Option<EntryInfo> {
    run_step(true).await
}

/// the whole history, for the history panel
#[tauri::command]
pub fn get_history() -> HistoryInfo {
    info()
}

async fn run_step(redo: bool) -> Option<EntryInfo> {
    let (entry, paused) = step(redo)?;
    if !paused {
        crate::graph::execute::execute_graph(true).await;
    }
    Some(entry)
}
