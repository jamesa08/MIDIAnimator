// a log file for what happens with no window to show it in (the Blender bridge starting, connections, writes, panics).
// release builds opened from Finder have nowhere for stdout to go, so everything the app prints (println!, eprintln!,
// panics) is also written to ~/Library/Logs/com.jamesa08.midianimator/motionkeys.log, which Save Diagnostics and
// scripts/bridge_diagnostics.sh read

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};
use tauri::Manager;

static FILE: Mutex<Option<File>> = Mutex::new(None);

// stdout and stderr go through the log file, so log() only has to print
static CAPTURED: AtomicBool = AtomicBool::new(false);

// past this the log moves to motionkeys.old.log and a new one starts
const MAX_SIZE: u64 = 1024 * 1024;

/// opens the log file, sends stdout and stderr to it as well and records panics, called first thing in setup
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

    #[cfg(unix)]
    CAPTURED.store(capture_output(), Ordering::Relaxed);

    // which build is running and from where (a quarantined app runs from a random AppTranslocation folder)
    log(format!("== MotionKeys {} ({}) started", env!("CARGO_PKG_VERSION"), env!("GIT_HASH")));
    log(format!("running from {}", std::env::current_exe().map_or("unknown".to_string(), |path| path.display().to_string())));
}

/// a line on stdout, which also lands in the log file
pub fn log(message: impl AsRef<str>) {
    if CAPTURED.load(Ordering::Relaxed) {
        println!("{}", message.as_ref());
        return;
    }
    // stdout isn't captured (Windows, or before init): the line goes to the file here
    let line = format!("{} {}", timestamp(), message.as_ref());
    println!("{}", line);
    write_line(&line);
}

fn timestamp() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string()
}

// appends a line to the log file, once it's open
fn write_line(line: &str) {
    if let Some(file) = FILE.lock().unwrap_or_else(PoisonError::into_inner).as_mut() {
        writeln!(file, "{}", line).ok();
    }
}

/// points stdout and stderr at pipes, a thread per pipe passes each line on to where it went before (the terminal)
/// and writes it to the log file with a timestamp. true once stdout is captured
#[cfg(unix)]
fn capture_output() -> bool {
    use std::io::{BufRead, BufReader};
    use std::os::fd::FromRawFd;

    let mut stdout_captured = false;
    for (fd, name) in [(libc::STDOUT_FILENO, "stdout"), (libc::STDERR_FILENO, "stderr")] {
        // flush what's buffered so it goes out before the switch
        std::io::stdout().flush().ok();
        std::io::stderr().flush().ok();

        let mut pipe = [0; 2];
        // SAFETY: plain fd calls on fds this function owns, a failed call leaves the stream as it was
        let original = unsafe {
            if libc::pipe(pipe.as_mut_ptr()) != 0 {
                continue;
            }
            let original = libc::dup(fd);
            if original < 0 || libc::dup2(pipe[1], fd) < 0 {
                libc::close(pipe[0]);
                libc::close(pipe[1]);
                if original >= 0 {
                    libc::close(original);
                }
                continue;
            }
            libc::close(pipe[1]);
            original
        };
        // SAFETY: both fds were just made here and nothing else owns them
        let (reader, mut terminal) = unsafe { (File::from_raw_fd(pipe[0]), File::from_raw_fd(original)) };

        // nothing in here may print, it would come straight back through the pipe
        let spawned = std::thread::Builder::new().name(format!("log {name}")).spawn(move || {
            for line in BufReader::new(reader).split(b'\n') {
                let Ok(line) = line else {
                    break;
                };
                terminal.write_all(&line).ok();
                terminal.write_all(b"\n").ok();
                write_line(&format!("{} {}", timestamp(), String::from_utf8_lossy(&line)));
            }
        });
        if spawned.is_ok() && fd == libc::STDOUT_FILENO {
            stdout_captured = true;
        }
    }
    stdout_captured
}

// a panic in a spawned task or thread is otherwise lost. once stderr is captured the default message (thread, location
// and message) already lands in the log, so the hook only adds a line when it isn't
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if !CAPTURED.load(Ordering::Relaxed) {
            let thread = std::thread::current();
            let message = info.payload().downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| info.payload().downcast_ref::<String>().cloned()).unwrap_or_default();
            let location = info.location().map_or("unknown".to_string(), |location| location.to_string());
            log(format!("panic on thread {}: {} at {}", thread.name().unwrap_or("unnamed"), message, location));
        }
        default_hook(info);
    }));
}
