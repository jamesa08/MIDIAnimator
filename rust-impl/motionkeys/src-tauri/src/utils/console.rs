// the console: what the app did that the user should see (runs, node errors, writes, the 3D app coming and
// going, files that didn't open or save, panics), kept in a ring buffer the console window (src/windows/Console.tsx)
// and the canvas overlay read. new entries go to every window as CONSOLE_ENTRIES_EVENT, a few at a time.
// debug mode (settings.json console.debug) also streams stdout and stderr, every window's js console and a timing
// entry per node per run. turning it off stops the streaming, the entries above are always kept

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, OnceLock, PoisonError};
use std::time::Duration;
use tauri::Emitter;

/// how many entries the console keeps, the oldest go first
pub const CAPACITY: usize = 5000;
/// sent to every window with new entries, payload a list of Entry
pub const CONSOLE_ENTRIES_EVENT: &str = "console_entries";
/// sent to every window when the console is cleared
pub const CONSOLE_CLEARED_EVENT: &str = "console_cleared";
// new entries wait this long for the ones after them, so a burst (debug output) goes out as one event
const BATCH: Duration = Duration::from_millis(30);

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Error,
    Warn,
    Info,
    Debug,
}

/// where an entry came from: a run, a node in it, the bridge, the app, rust's stdout/stderr or a window's js console
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Run,
    Node,
    Bridge,
    App,
    Rust,
    Js,
}

/// one line in the console. `time`: milliseconds since the unix epoch. `node`: the node it's about, a path
/// (`group-1/viewer-2`) in the tab `tab`. `ms`: how long what it's about took
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: u64,
    pub time: i64,
    pub level: Level,
    pub source: Source,
    pub message: String,
    pub node: Option<String>,
    pub tab: Option<String>,
    pub ms: Option<f64>,
}

impl Entry {
    /// an entry stamped now, its id is given when it's added
    pub fn new(level: Level, source: Source, message: impl Into<String>) -> Self {
        Self {
            id: 0,
            time: chrono::Utc::now().timestamp_millis(),
            level,
            source,
            message: message.into(),
            node: None,
            tab: None,
            ms: None,
        }
    }

    /// about a tab
    pub fn tab(mut self, tab: &str) -> Self {
        self.tab = Some(tab.to_string());
        self
    }

    /// about a node in a tab, the console links to it
    pub fn node(mut self, tab: &str, node: &str) -> Self {
        self.tab = Some(tab.to_string());
        self.node = Some(node.to_string());
        self
    }

    /// how long it took
    pub fn ms(mut self, ms: f64) -> Self {
        self.ms = Some(ms);
        self
    }
}

// MARK: - Log

/// the entries, the newest last. at `capacity` the oldest one goes for each new one
pub struct Log {
    entries: VecDeque<Entry>,
    next_id: u64,
    capacity: usize,
}

impl Log {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(capacity.min(CAPACITY)),
            next_id: 1,
            capacity,
        }
    }

    /// adds an entry with the next id (ids go up and are never reused, a clear doesn't start them over), returns it
    pub fn push(&mut self, mut entry: Entry) -> &Entry {
        entry.id = self.next_id;
        self.next_id += 1;
        while self.entries.len() >= self.capacity.max(1) {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
        self.entries.back().unwrap()
    }

    /// every entry kept, oldest first
    pub fn entries(&self) -> Vec<Entry> {
        self.entries.iter().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// the app's console, and what hasn't gone out to the windows yet
struct Console {
    log: Log,
    pending: Vec<Entry>,
    cleared: bool,
    // the thread sending entries to the windows is running, nothing waits to be sent before that (tests)
    sending: bool,
}

static CONSOLE: Mutex<Option<Console>> = Mutex::new(None);
static WAKE: Condvar = Condvar::new();
static APP: OnceLock<tauri::AppHandle> = OnceLock::new();
static DEBUG: AtomicBool = AtomicBool::new(false);

// runs `f` on the console, making it the first time
fn with_console<T>(f: impl FnOnce(&mut Console) -> T) -> T {
    let mut console = CONSOLE.lock().unwrap_or_else(PoisonError::into_inner);
    f(console.get_or_insert_with(|| Console {
        log: Log::new(CAPACITY),
        pending: Vec::new(),
        cleared: false,
        sending: false,
    }))
}

/// starts sending entries to the windows, called from setup right after the log file opens
pub fn init(app: &tauri::AppHandle) {
    if APP.set(app.clone()).is_err() {
        return;
    }
    with_console(|console| console.sending = true);
    std::thread::Builder::new().name("console".to_string()).spawn(send_entries).ok();
}

// waits for new entries, gives the ones after them a moment to come, then sends them all in one event. nothing in
// here may print, in debug mode it would come back as an entry
fn send_entries() {
    loop {
        {
            let mut console = CONSOLE.lock().unwrap_or_else(PoisonError::into_inner);
            while console.as_ref().is_some_and(|c| c.pending.is_empty() && !c.cleared) {
                console = WAKE.wait(console).unwrap_or_else(PoisonError::into_inner);
            }
        }
        std::thread::sleep(BATCH);
        let (cleared, entries) = with_console(|console| (std::mem::take(&mut console.cleared), std::mem::take(&mut console.pending)));
        let Some(app) = APP.get() else {
            continue;
        };
        if cleared {
            app.emit(CONSOLE_CLEARED_EVENT, ()).ok();
        }
        if !entries.is_empty() {
            app.emit(CONSOLE_ENTRIES_EVENT, entries).ok();
        }
    }
}

/// adds entries to the console and sends them to the windows. what didn't come from stdout or stderr also goes in the
/// log file
pub fn add(entries: Vec<Entry>) {
    for entry in entries.iter().filter(|entry| entry.source != Source::Rust) {
        super::log::write_entry(&format!("[{:?}] {}", entry.level, entry.message));
    }
    with_console(|console| {
        for entry in entries {
            let entry = console.log.push(entry).clone();
            if console.sending {
                console.pending.push(entry);
            }
        }
    });
    WAKE.notify_one();
}

/// adds one line, see `add`
pub fn log(level: Level, source: Source, message: impl Into<String>) {
    add(vec![Entry::new(level, source, message)]);
}

/// adds one entry, see `add`
pub fn push(entry: Entry) {
    add(vec![entry]);
}

/// true while debug mode streams everything
pub fn debug() -> bool {
    DEBUG.load(Ordering::Relaxed)
}

/// picks up the console settings, after they load and whenever they change
pub fn sync_settings() {
    DEBUG.store(crate::settings::get_setting("console.debug").as_bool().unwrap_or(false), Ordering::Relaxed);
}

/// a line rust printed, already in the log file. only kept in debug mode. called from the threads reading stdout and
/// stderr (utils/log.rs), so nothing here may print
pub fn output_line(stderr: bool, line: &str) {
    if !debug() || line.is_empty() {
        return;
    }
    let level = if stderr {
        Level::Warn
    } else {
        Level::Debug
    };
    add(vec![Entry::new(level, Source::Rust, line)]);
}

/// a panic, from the panic hook. skipped if the console is in use on this thread (it panicked while adding), waiting
/// would never end
pub fn panicked(message: String) {
    let entry = Entry::new(Level::Error, Source::Rust, message);
    let Ok(mut console) = CONSOLE.try_lock() else {
        return;
    };
    let Some(console) = console.as_mut() else {
        return;
    };
    let entry = console.log.push(entry).clone();
    if console.sending {
        console.pending.push(entry);
    }
    WAKE.notify_one();
}

/// sends an event to every window, nothing happens before setup (tests)
pub fn emit<S: Serialize + Clone>(event: &str, payload: S) {
    if let Some(app) = APP.get() {
        app.emit(event, payload).ok();
    }
}

// MARK: - Commands

/// every entry the console has, for a window that opened after they came
#[tauri::command]
pub fn console_entries() -> Vec<Entry> {
    with_console(|console| console.log.entries())
}

/// clears the console for every window
#[tauri::command]
pub fn console_clear() {
    with_console(|console| {
        console.log.clear();
        console.pending.clear();
        console.cleared = console.sending;
    });
    WAKE.notify_one();
}

/// a line from a window: its js console in debug mode, or a warning or error it showed in the status bar
#[tauri::command]
pub fn console_log(level: Level, source: Source, message: String) {
    if source == Source::Js && !debug() {
        return;
    }
    log(level, source, message);
}
