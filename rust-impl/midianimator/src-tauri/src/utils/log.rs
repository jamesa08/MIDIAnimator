// a log file for what happens with no window to show it in (the Blender bridge starting, connections, panics).
// release builds opened from Finder have nowhere for println! to go, so these lines also go to
// ~/Library/Logs/com.jamesa08.midianimator/motionkeys.log, which scripts/bridge_diagnostics.sh reads

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::{Mutex, PoisonError};
use tauri::Manager;

static FILE: Mutex<Option<File>> = Mutex::new(None);

// past this the log moves to motionkeys.old.log and a new one starts
const MAX_SIZE: u64 = 1024 * 1024;

/// opens the log file and records panics in it, called first thing in setup
pub fn init(app: &tauri::AppHandle) {
    install_panic_hook();

    let Ok(dir) = app.path().app_log_dir() else {
        return;
    };
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join("motionkeys.log");
    if std::fs::metadata(&path).map_or(false, |meta| meta.len() > MAX_SIZE) {
        std::fs::rename(&path, dir.join("motionkeys.old.log")).ok();
    }
    *FILE.lock().unwrap_or_else(PoisonError::into_inner) = OpenOptions::new().create(true).append(true).open(&path).ok();

    // which build is running and from where (a quarantined app runs from a random AppTranslocation folder)
    log(format!("== MotionKeys {} ({}) started", env!("CARGO_PKG_VERSION"), env!("GIT_HASH")));
    log(format!("running from {}", std::env::current_exe().map_or("unknown".to_string(), |path| path.display().to_string())));
}

/// a timestamped line in the log file and on stdout
pub fn log(message: impl AsRef<str>) {
    let line = format!("{} {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"), message.as_ref());
    println!("{}", line);
    if let Some(file) = FILE.lock().unwrap_or_else(PoisonError::into_inner).as_mut() {
        writeln!(file, "{}", line).ok();
    }
}

// a panic in a spawned task or thread is otherwise lost, keep the default message on stderr too
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let message = info.payload().downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| info.payload().downcast_ref::<String>().cloned()).unwrap_or_default();
        let location = info.location().map_or("unknown".to_string(), |location| location.to_string());
        log(format!("panic on thread {}: {} at {}", thread.name().unwrap_or("unnamed"), message, location));
        default_hook(info);
    }));
}
