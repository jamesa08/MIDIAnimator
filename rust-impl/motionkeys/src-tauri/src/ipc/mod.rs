// the bridge connection (docs/bridge_protocol.md). a bridge (the Blender add-on) connects to 127.0.0.1 on the ipc.port
// setting and says hello, then the two send each other one JSON message per line: {id, type, reply_to, body}. requests
// get a reply whose reply_to is the request's id, the bridge also sends the scene unasked when it changes. one bridge is
// connected at a time, a second one is turned away

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicU16, AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard, Once, PoisonError};
use std::thread;
use std::time::Duration;
use uuid::Uuid;

use crate::bridge::curves::carry_over_curves;
use crate::bridge::scene_data::compare_scene_data;
use crate::graph::execute::run_instance;
use crate::graph::executors::scene::NOT_CONNECTED;
use crate::scene_generics;
use crate::settings::get_setting;
use crate::state::{relink, update_state, STATE};
use crate::utils::console::{self, Level, Source};
use crate::utils::log::log;

/// the protocol versions this app speaks, oldest first. a bridge says which ones it speaks, the newest both do is used
pub const PROTOCOL_VERSIONS: &[u32] = &[1];

/// one message, either way
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Envelope {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    /// the id of the request this answers, none for a request or a message sent unasked
    #[serde(default)]
    pub reply_to: Option<String>,
    #[serde(default)]
    pub body: Value,
}

impl Envelope {
    pub fn new(kind: &str, body: Value, reply_to: Option<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            kind: kind.to_string(),
            reply_to,
            body,
        }
    }

    /// the message as it's sent, JSON and a newline
    pub fn line(&self) -> Vec<u8> {
        let mut line = serde_json::to_vec(self).unwrap_or_default();
        line.push(b'\n');
        line
    }
}

/// what a bridge says about itself and its app when it connects
#[derive(Debug, Deserialize, Clone)]
pub struct Hello {
    /// the protocol versions the bridge speaks
    pub protocol: Vec<u32>,
    /// the app's id, like "blender"
    pub app: String,
    /// the app's name as it's shown, like "Blender"
    pub app_name: String,
    #[serde(default)]
    pub app_version: String,
    #[serde(default)]
    pub bridge_version: String,
    /// the open file's name
    #[serde(default)]
    pub file: String,
    /// the app's icon, a PNG or SVG data URL
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub capabilities: Capabilities,
}

/// what a bridge can do besides the requests every bridge answers
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Capabilities {
    /// it sends the curves of watched objects when they change
    #[serde(default)]
    pub watch: bool,
    /// the script languages it runs (run_script)
    #[serde(default)]
    pub scripts: Vec<String>,
}

/// the newest protocol version the bridge and this app both speak
pub fn negotiate(theirs: &[u32]) -> Option<u32> {
    PROTOCOL_VERSIONS.iter().rev().find(|version| theirs.contains(version)).copied()
}

// an icon bigger than this (decoded) is left out
const MAX_ICON_BYTES: usize = 64 * 1024;

/// the bridge's icon when it's a PNG or SVG data URL that isn't too big
pub fn valid_icon(icon: Option<String>) -> Option<String> {
    let max_base64 = MAX_ICON_BYTES.div_ceil(3) * 4;
    icon.filter(|icon| ["data:image/png;base64,", "data:image/svg+xml;base64,"].iter().any(|prefix| icon.strip_prefix(prefix).is_some_and(|data| data.len() <= max_base64)))
}

/// the bridge that said hello
struct Bridge {
    connection: u64,
    app_name: String,
    capabilities: Capabilities,
    /// messages are written whole, one at a time
    writer: Arc<Mutex<TcpStream>>,
}

struct Server {
    bridge: Option<Bridge>,
    /// every open connection, ones that haven't said hello yet too, so `disconnect` closes them all
    connections: HashMap<u64, TcpStream>,
    /// requests waiting for their reply, by id
    pending: HashMap<String, mpsc::Sender<Envelope>>,
}

static SERVER: Lazy<Mutex<Server>> = Lazy::new(|| {
    Mutex::new(Server {
        bridge: None,
        connections: HashMap::new(),
        pending: HashMap::new(),
    })
});

fn server() -> MutexGuard<'static, Server> {
    SERVER.lock().unwrap_or_else(PoisonError::into_inner)
}

static NEXT_CONNECTION: AtomicU64 = AtomicU64::new(1);

pub static DEFAULT_PORT: u16 = 6577;

// the port the server listens on, read from the ipc.port setting once at startup (a changed setting waits for a restart)
static BOUND_PORT: AtomicU16 = AtomicU16::new(0);

/// the port bridges connect to, 0 before the server starts
pub fn bound_port() -> u16 {
    BOUND_PORT.load(Ordering::Relaxed)
}

/// a port setting that's a whole number from 1024 to 65535, anything else (a quoted "6577" too) is None
pub fn valid_port(value: &serde_json::Value) -> Option<u16> {
    value.as_u64().and_then(|port| u16::try_from(port).ok()).filter(|port| *port >= 1024)
}

// port from the ipc.port setting, falls back to the default when missing or invalid
fn port() -> u16 {
    valid_port(&get_setting("ipc.port")).unwrap_or(DEFAULT_PORT)
}

// a port something else holds (like a copy of MotionKeys that's still quitting) is tried again every few seconds, up to a minute
const BIND_RETRY_DELAY: Duration = Duration::from_secs(3);
const BIND_ATTEMPTS: u32 = 20;

/// binds 127.0.0.1:`port`, trying again while it's taken. None once it gives up, the port then stays 0
fn bind_listener(port: u16) -> Option<TcpListener> {
    for attempt in 1..=BIND_ATTEMPTS {
        match TcpListener::bind(("127.0.0.1", port)) {
            Ok(listener) => {
                BOUND_PORT.store(port, Ordering::Relaxed);
                log(format!("bridge server listening on 127.0.0.1:{port}"));
                // the connection popover shows the port
                update_state();
                return Some(listener);
            }
            Err(e) if attempt == BIND_ATTEMPTS => log(format!("bridge server gave up on 127.0.0.1:{port} after {BIND_ATTEMPTS} attempts: {e}")),
            Err(e) => {
                if attempt == 1 {
                    log(format!("bridge server could not listen on 127.0.0.1:{port}: {e}, trying again every {}s", BIND_RETRY_DELAY.as_secs()));
                }
                thread::sleep(BIND_RETRY_DELAY);
            }
        }
    }
    None
}

/// starts listening for bridges, binding (and retrying) happens on its own thread so it never holds up the app
pub fn start_server() {
    static STARTED: Once = Once::new();
    STARTED.call_once(|| {
        thread::spawn(|| {
            let Some(listener) = bind_listener(port()) else {
                return;
            };
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        let connection = NEXT_CONNECTION.fetch_add(1, Ordering::Relaxed);
                        if let Ok(clone) = stream.try_clone() {
                            server().connections.insert(connection, clone);
                        }
                        thread::spawn(move || handle_client(stream, connection));
                    }
                    Err(e) => log(format!("bridge server failed to accept a connection: {e}")),
                }
            }
        });
    });
}

/// closes the connection to the bridge, each connection's handle_client thread then sees it closed and clears the state
#[tauri::command]
pub fn disconnect() {
    for stream in server().connections.values() {
        stream.shutdown(Shutdown::Both).ok();
    }
}

// how long each request waits for its reply when its setting is missing or not a whole number above 0, in seconds
fn default_timeout(kind: &str) -> u64 {
    match kind {
        "get_scene" => 10,
        "write" => 60,
        _ => 30,
    }
}

/// how long a request waits for its reply, from the ipc.timeouts settings
pub fn timeout(kind: &str) -> Duration {
    let seconds = get_setting(&format!("ipc.timeouts.{kind}")).as_u64().filter(|seconds| *seconds > 0);
    Duration::from_secs(seconds.unwrap_or_else(|| default_timeout(kind)))
}

/// why a request got no reply body. `?` turns it into its message
#[derive(Debug)]
pub enum RequestError {
    /// no reply within the request's timeout, the bridge may still be working on it
    Timeout(String),
    /// an error reply, no bridge, or a disconnect
    Failed(String),
}

impl From<RequestError> for String {
    fn from(error: RequestError) -> Self {
        match error {
            RequestError::Timeout(message) | RequestError::Failed(message) => message,
        }
    }
}

/// sends a request to the connected bridge and waits for its reply's body, as long as its timeout setting says. an
/// error reply, no bridge, a timeout or a disconnect are errors
pub async fn request(kind: &str, body: Value) -> Result<Value, RequestError> {
    let timeout = timeout(kind);
    let envelope = Envelope::new(kind, body, None);
    let (tx, rx) = mpsc::channel();
    let (writer, app_name) = {
        let mut server = server();
        let Some(bridge) = &server.bridge else {
            return Err(RequestError::Failed(NOT_CONNECTED.to_string()));
        };
        let found = (Arc::clone(&bridge.writer), bridge.app_name.clone());
        // in before the request goes out, a quick reply finds it
        server.pending.insert(envelope.id.clone(), tx);
        found
    };

    // the request's kind goes to the log, the messages are for the user
    if let Err(e) = write_message(&writer, &envelope) {
        server().pending.remove(&envelope.id);
        log(format!("couldn't send the {kind} request to the bridge: {e}"));
        return Err(RequestError::Failed(format!("couldn't send to {app_name}: {e}")));
    }
    let reply = rx.recv_timeout(timeout);
    server().pending.remove(&envelope.id);
    match reply {
        Ok(reply) if reply.kind == "error" => Err(RequestError::Failed(error_text(&reply.body))),
        Ok(reply) => Ok(reply.body),
        Err(RecvTimeoutError::Timeout) => {
            log(format!("the bridge didn't answer the {kind} request within {}s", timeout.as_secs()));
            Err(RequestError::Timeout(format!("{app_name} didn't answer within {}s", timeout.as_secs())))
        }
        Err(RecvTimeoutError::Disconnected) => {
            log(format!("the bridge disconnected before answering the {kind} request"));
            Err(RequestError::Failed(format!("{app_name} disconnected before answering")))
        }
    }
}

/// what an error message says: its message, and its detail (a traceback) under it
fn error_text(body: &Value) -> String {
    let message = body.get("message").and_then(Value::as_str).unwrap_or("unknown error");
    match body.get("detail").and_then(Value::as_str).filter(|detail| !detail.trim().is_empty()) {
        Some(detail) => format!("{message}\n{detail}"),
        None => message.to_string(),
    }
}

/// the connected bridge's name for its app, `Blender`, for messages
pub fn connected_name() -> Option<String> {
    server().bridge.as_ref().map(|bridge| bridge.app_name.clone())
}

/// the connected app's name in a message, `the 3D app` when nothing is connected anymore
pub fn app_name() -> String {
    connected_name().unwrap_or_else(|| "the 3D app".to_string())
}

/// whether the connected bridge watches objects and sends their curves (its `watch` capability)
pub fn bridge_watches() -> bool {
    server().bridge.as_ref().is_some_and(|bridge| bridge.capabilities.watch)
}

/// runs a script in the connected app, its result. the bridge has to say in its hello that it runs `language`
pub async fn run_script(language: &str, source: &str, args: Value) -> Result<Value, String> {
    let runs = server().bridge.as_ref().map(|bridge| bridge.capabilities.scripts.iter().any(|known| known == language));
    match runs {
        None => Err(NOT_CONNECTED.to_string()),
        Some(false) => Err(format!("the connected app doesn't run {language} scripts")),
        Some(true) => {
            let body = request("run_script", json!({ "language": language, "source": source, "args": args })).await?;
            Ok(body.get("result").cloned().unwrap_or(Value::Null))
        }
    }
}

fn write_message(writer: &Mutex<TcpStream>, envelope: &Envelope) -> std::io::Result<()> {
    let mut stream = writer.lock().unwrap_or_else(PoisonError::into_inner);
    stream.write_all(&envelope.line())
}

/// takes every complete message out of `data`, leaving a trailing partial message for the next read.
/// both sides end each message with a newline and JSON escapes newlines inside strings,
/// so a newline always ends a message, even when several arrive in one read.
/// a line that isn't a valid message is logged and dropped so it can't block the ones after it
pub fn take_messages(data: &mut Vec<u8>) -> Vec<Envelope> {
    let mut messages = Vec::new();
    while let Some(end) = data.iter().position(|byte| *byte == b'\n') {
        let line: Vec<u8> = data.drain(..=end).collect();
        let line = String::from_utf8_lossy(&line);
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Envelope>(&line) {
            Ok(message) => messages.push(message),
            Err(e) => println!("Failed to parse IPC message ({} bytes): {}", line.len(), e),
        }
    }
    messages
}

/// makes a scene the bridge sent the scene of the tab the bridge is linked to, returns the tab if it should run
/// (it's on screen, a tab in the background runs once it's shown). while the tab is paused for review the scene becomes
/// its pending scene instead, so accepting it uses the newest scene. switching to another scene in Blender pauses the
/// tab for review when the new scene's groups or objects differ. with no tab linked it goes nowhere. doesn't notify the
/// front end, the caller calls `update_state()`
pub fn apply_scene_update(body: Value) -> Result<Option<String>, String> {
    let mut scene = serde_json::from_value::<scene_generics::Scene>(body).map_err(|e| e.to_string())?;
    let mut state = STATE.lock().unwrap_or_else(PoisonError::into_inner);
    scene.app = Some(state.connected_application.clone()).filter(|app| !app.is_empty());
    scene.app_name = Some(state.connected_name.clone()).filter(|name| !name.is_empty());
    let Some(id) = state.connected_instance_id.clone() else {
        return Ok(None);
    };
    let shown = state.active_instance_id == id;
    let watched = state.watched_curves.clone().unwrap_or_default();
    let Some(instance) = state.instance_mut(&id) else {
        return Ok(None);
    };
    // only the watched objects come with their curves
    carry_over_curves(&instance.scene, &mut scene, &watched);
    let switched = instance.scene.as_ref().is_some_and(|old| old.key() != scene.key());
    if switched && compare_scene_data(instance.scene.as_ref(), &scene).has_changes() {
        instance.execution_paused = true;
    }
    if instance.execution_paused {
        instance.pending_scene = Some(scene);
        return Ok(None);
    }
    instance.scene = Some(scene);
    Ok(shown.then_some(id))
}

/// what a connection's first message gets: its hello and the protocol version both speak, or why it's turned away (an
/// error code and message). `connected` is the name of the app whose bridge is connected already
pub fn accept_hello(message: &Envelope, connected: Option<&str>) -> Result<(Hello, u32), (&'static str, String)> {
    if message.kind != "hello" {
        return Err(("invalid_message", format!("the bridge has to send hello first, it sent {}", message.kind)));
    }
    let hello: Hello = serde_json::from_value(message.body.clone()).map_err(|e| ("invalid_message", format!("couldn't read the bridge's hello: {e}")))?;
    let Some(version) = negotiate(&hello.protocol) else {
        let versions = |list: &[u32]| list.iter().map(u32::to_string).collect::<Vec<_>>().join(", ");
        return Err(("unsupported_version", format!("MotionKeys and the {} bridge don't share a protocol version (MotionKeys: {}, bridge: {}). Update the older one.", hello.app_name, versions(PROTOCOL_VERSIONS), versions(&hello.protocol))));
    };
    if let Some(connected) = connected {
        return Err(("busy", format!("{connected} is already connected to MotionKeys, disconnect it first")));
    }
    Ok((hello, version))
}

// a connection, from its hello until it closes
fn handle_client(stream: TcpStream, connection: u64) {
    let peer = stream.peer_addr().map_or("unknown".to_string(), |addr| addr.to_string());
    let Ok(writer) = stream.try_clone() else {
        server().connections.remove(&connection);
        return;
    };
    let writer = Arc::new(Mutex::new(writer));
    let mut reader = stream;
    let mut said_hello = false;
    let mut data = Vec::new();
    // 64 KiB, scene updates with keyframes are megabytes
    let mut buf = vec![0; 65536];

    'reading: loop {
        let n = match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                log(format!("bridge connection error: {e}"));
                break;
            }
        };
        data.extend_from_slice(&buf[..n]);

        for message in take_messages(&mut data) {
            if said_hello {
                handle_message(message);
                continue;
            }
            let connected = server().bridge.as_ref().map(|bridge| bridge.app_name.clone());
            let accepted = accept_hello(&message, connected.as_deref());
            match accepted {
                Ok((hello, version)) => {
                    welcome(&hello, version, connection, &writer, &message);
                    said_hello = true;
                }
                Err((code, reason)) => {
                    log(format!("turned a bridge away ({peer}): {reason}"));
                    write_message(&writer, &Envelope::new("error", json!({ "code": code, "message": reason }), Some(message.id.clone()))).ok();
                    break 'reading;
                }
            }
        }
    }

    // the bridge's requests waiting for a reply fail now instead of waiting out their timeout
    let was_bridge = {
        let mut server = server();
        server.connections.remove(&connection);
        let was_bridge = server.bridge.as_ref().is_some_and(|bridge| bridge.connection == connection);
        if was_bridge {
            server.bridge = None;
            server.pending.clear();
        }
        was_bridge
    };
    if !was_bridge {
        return;
    }

    // the linked tab stays linked (offline) until a bridge is back
    let mut state = STATE.lock().unwrap_or_else(PoisonError::into_inner);
    log(format!("the bridge disconnected from {peer}"));
    console::log(Level::Warn, Source::Bridge, format!("{} disconnected", state.connected_name));
    state.connected = false;
    state.watched_curves = None;
    state.connected_application.clear();
    state.connected_name.clear();
    state.connected_version.clear();
    state.connected_file_name.clear();
    state.connected_icon = None;
    drop(state);
    update_state();
}

// the bridge said hello: it's the connected app now, it gets the version both speak, and the tab it was linked to before
// (or the one on screen) links to it again
fn welcome(hello: &Hello, version: u32, connection: u64, writer: &Arc<Mutex<TcpStream>>, message: &Envelope) {
    server().bridge = Some(Bridge {
        connection,
        app_name: hello.app_name.clone(),
        capabilities: hello.capabilities.clone(),
        writer: Arc::clone(writer),
    });
    let reply = Envelope::new("hello", json!({ "protocol": version, "app": "motionkeys", "app_version": env!("CARGO_PKG_VERSION") }), Some(message.id.clone()));
    write_message(writer, &reply).ok();

    log(format!("{} {} connected (bridge {}, protocol {version}), file {:?}", hello.app_name, hello.app_version, hello.bridge_version, hello.file));
    console::log(Level::Info, Source::Bridge, format!("{} connected", hello.app_name));
    let mut state = STATE.lock().unwrap_or_else(PoisonError::into_inner);
    state.connected = true;
    state.connected_application = hello.app.clone();
    state.connected_name = hello.app_name.clone();
    state.connected_version = hello.app_version.clone();
    state.connected_file_name = hello.file.clone();
    state.connected_icon = valid_icon(hello.icon.clone());
    // scene writers that failed only because nothing was connected aren't red anymore
    state.clear_not_connected();
    drop(state);
    update_state();
    tauri::async_runtime::spawn(relink());
}

// a message from the bridge after its hello: a reply goes to the request waiting for it, a scene sent unasked goes to the
// live tab. messages of a type this app doesn't know are left alone, a newer bridge may send them
/// takes the hello fields an info event changes: the open file's name
pub fn apply_info(body: &Value) {
    if let Some(file) = body.get("file").and_then(Value::as_str) {
        STATE.lock().unwrap_or_else(PoisonError::into_inner).connected_file_name = file.to_string();
        update_state();
    }
}

fn handle_message(message: Envelope) {
    if let Some(reply_to) = &message.reply_to {
        let waiting = server().pending.remove(reply_to);
        // the request may have timed out already
        if let Some(tx) = waiting {
            tx.send(message).ok();
        }
        return;
    }
    match message.kind.as_str() {
        "scene" => match apply_scene_update(message.body) {
            Ok(run) => {
                update_state();
                // run the realtime graph so nodes like Scene Link pick up the new scene
                if let Some(id) = run {
                    tauri::async_runtime::spawn(run_instance(id, true));
                }
            }
            Err(e) => log(format!("couldn't read the scene the bridge sent: {e}")),
        },
        "info" => apply_info(&message.body),
        "curves" => {
            let run = crate::bridge::curves::apply_curves_update(message.body);
            update_state();
            if let Some(id) = run {
                tauri::async_runtime::spawn(run_instance(id, true));
            }
        }
        "error" => log(format!("the bridge: {}", error_text(&message.body))),
        kind => log(format!("left alone a {kind} message from the bridge")),
    }
}
