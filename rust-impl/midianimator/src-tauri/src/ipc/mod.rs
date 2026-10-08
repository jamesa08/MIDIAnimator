use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufReader, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::Duration;
use tokio::runtime::Runtime;
use uuid::Uuid;

use crate::graph::execute::run_instance;
use crate::scene_generics;
use crate::settings::get_setting;
use crate::state::{relink, update_state, STATE};
use crate::utils::log::log;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct Message {
    pub sender: String,
    pub message: String,
    pub uuid: String,
}

struct Server {
    clients: Arc<Mutex<Vec<TcpStream>>>,
    message_map: Arc<Mutex<HashMap<String, mpsc::Sender<String>>>>,
}

pub static DEFAULT_PORT: u16 = 6577;

// the port the server listens on, read from the ipc.port setting once at startup (a changed setting waits for a restart)
static BOUND_PORT: AtomicU16 = AtomicU16::new(0);

/// the port Blender connects to, 0 before the server starts
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
                log(format!("Blender bridge listening on 127.0.0.1:{port}"));
                // the connection popover shows the port
                update_state();
                return Some(listener);
            }
            Err(e) if attempt == BIND_ATTEMPTS => log(format!("Blender bridge gave up on 127.0.0.1:{port} after {BIND_ATTEMPTS} attempts: {e}")),
            Err(e) => {
                if attempt == 1 {
                    log(format!("Blender bridge could not listen on 127.0.0.1:{port}: {e}, trying again every {}s", BIND_RETRY_DELAY.as_secs()));
                }
                thread::sleep(BIND_RETRY_DELAY);
            }
        }
    }
    None
}

// create a server instance
// this is a lazy static variable, so it will only be created once
// and will be shared across all threads
// this is necessary because the server needs to be accessed across threads, and in other functions
static SERVER: Lazy<Arc<Mutex<Server>>> = Lazy::new(|| {
    // create a server instance
    let server = Server {
        clients: Arc::new(Mutex::new(Vec::new())),
        message_map: Arc::new(Mutex::new(HashMap::new())),
    };
    let server = Arc::new(Mutex::new(server));

    // clone the server instance to be used in the thread
    let server_clone = Arc::clone(&server);

    // listens on the configured port, binding (and retrying) happens here so it never holds up the app
    thread::spawn(move || {
        let Some(listener) = bind_listener(port()) else {
            return;
        };
        let rt = Runtime::new().unwrap();
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    log(format!("Blender connected from {}", stream.peer_addr().map_or("unknown".to_string(), |addr| addr.to_string())));

                    // we are connected to the client
                    let mut state = STATE.lock().unwrap();
                    state.connected = true;
                    // also need to call get_client_info here to get the connected application info
                    drop(state);
                    update_state();

                    let server = Arc::clone(&server_clone);
                    let server_unwrapped = server.lock().unwrap();

                    // lock the clients list and add the new client
                    let mut clients = server_unwrapped.clients.lock().unwrap();

                    clients.push(stream.try_clone().unwrap());
                    drop(clients); // drop lock to avoid deadlock

                    let server_clone = Arc::clone(&server);
                    // when spawnning a new thread, get the client information as well
                    // Blender links to the tab it was linked to before, or the tab on screen (it checks the scene first)
                    rt.spawn(async move {
                        request_client_info().await;
                        relink().await;
                    });

                    thread::spawn(move || {
                        handle_client(stream, server_clone);
                    });
                }
                Err(e) => {
                    log(format!("Blender bridge failed to accept a connection: {e}"));
                }
            }
        }
    });
    return server;
});

#[allow(unused_must_use)]
pub fn start_server() {
    SERVER.lock().unwrap();
}

/// closes the connection to Blender, each client's handle_client thread then sees it closed and clears the state
#[tauri::command]
pub fn disconnect() {
    let server = SERVER.lock().unwrap();
    for client in server.clients.lock().unwrap().iter() {
        client.shutdown(Shutdown::Both).ok();
    }
}

pub async fn request_client_info() {
    let script = r"import bpy
def execute():
    version = bpy.app.version_string
    file_name = bpy.data.filepath.split('/')[-1]
    return {'version': version, 'file_name': file_name}";

    let Some(result) = send_message(script.to_string()).await else {
        log("Blender didn't answer the client info request");
        return;
    };

    // a quote in the file name makes python's dict repr invalid JSON
    let map_object: HashMap<String, String> = match serde_json::from_str(&result.replace("'", "\"")) {
        Ok(map_object) => map_object,
        Err(e) => {
            log(format!("couldn't read Blender's client info ({e}): {result}"));
            return;
        }
    };
    let version = map_object.get("version").cloned().unwrap_or_default();
    let file_name = map_object.get("file_name").cloned().unwrap_or_default();
    log(format!("Blender {version} linked, file {file_name:?}"));

    let mut state = STATE.lock().unwrap();
    state.connected_application = "blender".to_string();
    state.connected_version = version;
    state.connected_file_name = file_name;
    drop(state);
    update_state();
}

/// takes every complete message out of `data`, leaving a trailing partial message for the next read.
/// both sides end each message with a newline and JSON escapes newlines inside strings,
/// so a newline always ends a message, even when several arrive in one read.
/// a line that isn't a valid message is logged and dropped so it can't block the ones after it
pub fn take_messages(data: &mut Vec<u8>) -> Vec<Message> {
    let mut messages = Vec::new();
    while let Some(end) = data.iter().position(|byte| *byte == b'\n') {
        let line: Vec<u8> = data.drain(..=end).collect();
        let line = String::from_utf8_lossy(&line);
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Message>(&line) {
            Ok(message) => messages.push(message),
            Err(e) => println!("Failed to parse IPC message ({} bytes): {}", line.len(), e),
        }
    }
    messages
}

/// makes a scene sent by the Blender tracker the scene data of the tab Blender is linked to, returns the tab if it should
/// run (it's on screen, a tab in the background runs once it's shown). while the tab is paused for review the scene
/// becomes its pending scene data instead, so accepting it uses the newest scene. with no tab linked it goes nowhere.
/// doesn't notify the front end, the caller calls `update_state()`
pub fn apply_scene_update(message: &str) -> Result<Option<String>, String> {
    let scene_data = serde_json::from_str::<HashMap<String, scene_generics::Scene>>(message).map_err(|e| e.to_string())?;
    let mut state = STATE.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(id) = state.connected_instance_id.clone() else {
        return Ok(None);
    };
    let shown = state.active_instance_id == id;
    let Some(instance) = state.instance_mut(&id) else {
        return Ok(None);
    };
    if instance.execution_paused {
        instance.pending_scene_data = Some(scene_data);
        return Ok(None);
    }
    instance.scene_data = scene_data;
    Ok(shown.then_some(id))
}

// handle a client connection
fn handle_client(stream: TcpStream, server: Arc<Mutex<Server>>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    // taken now, a socket that's been shut down has no peer address
    let peer = stream.peer_addr().ok();

    // keep reading messages from the client until the connection is closed
    let mut data = Vec::new();

    loop {
        let mut buf = [0; 4096]; // 4 KiB buffer

        match reader.read(&mut buf) {
            Ok(0) => break, // connection closed
            Ok(n) => {
                data.extend_from_slice(&buf[..n]); // add the read bytes to data

                for message in take_messages(&mut data) {
                    // check if this message is a response to a message we sent
                    let tx = {
                        let server_lock = server.lock().unwrap();
                        let mut message_map = server_lock.message_map.lock().unwrap();
                        message_map.remove(&message.uuid) // this gets the tx sender from send_message
                    };

                    if let Some(tx) = tx {
                        // a response, the sender may have timed out already
                        tx.send(message.message).ok();
                    } else if message.message.contains("\"object_groups\"") {
                        // an unsolicited scene update from the tracker
                        println!("Received scene update from Blender with UUID: {} ({} bytes)", message.uuid, message.message.len());
                        match apply_scene_update(&message.message) {
                            Ok(run) => {
                                update_state();
                                // run the realtime graph so nodes like Scene Link pick up the new scene
                                if let Some(id) = run {
                                    tauri::async_runtime::spawn(run_instance(id, true));
                                }
                            }
                            Err(e) => println!("Failed to parse scene data JSON: {}", e),
                        }
                    } else {
                        // other unsolicited messages
                        println!("Received unsolicited message: UUID={}", message.uuid);
                    }
                }
            }
            Err(e) => {
                log(format!("Blender connection error: {e}"));
                data.clear();
                break;
            }
        }

        // allow a small delay to let other threads allocate the lock
        thread::sleep(Duration::from_millis(10));
    }

    // if disconnected, remove the client from the server
    // clear all previous info, the linked tab stays linked (offline) until Blender is back
    log(format!("Blender disconnected from {}", peer.map_or("unknown".to_string(), |addr| addr.to_string())));
    let mut state = STATE.lock().unwrap();
    state.connected = false;
    state.connected_application = "".to_string();
    state.connected_version = "".to_string();
    state.connected_file_name = "".to_string();
    drop(state);
    update_state();

    let server = server.lock().unwrap();
    let mut clients = server.clients.lock().unwrap();
    // shut down sockets (disconnect) go too
    clients.retain(|c| c.peer_addr().map_or(false, |addr| Some(addr) != peer));
}

pub async fn send_message(message: String) -> Option<String> {
    send_message_with_timeout(message, Duration::from_secs(5)).await
}

/// like `send_message`, but waits up to `timeout` for Blender to respond
pub async fn send_message_with_timeout(message: String, timeout: Duration) -> Option<String> {
    // create a message struct
    let msg_struct = Message {
        sender: "server".to_string(),
        message,
        uuid: Uuid::new_v4().to_string(),
    };

    let json_msg = serde_json::to_string(&msg_struct).unwrap() + "\n";

    let server = SERVER.lock().unwrap();
    // send the message to all clients
    let mut clients = server.clients.lock().unwrap();
    for client in clients.iter_mut() {
        // a closed client is removed by its handle_client thread
        write_in_chunks(client, json_msg.as_bytes()).ok();
    }
    drop(clients);

    // create a channel to receive the response, and insert it into the message_map
    let (tx, rx) = mpsc::channel();
    server.message_map.lock().unwrap().insert(msg_struct.uuid.clone(), tx);
    drop(server);

    // loop until a response is received or the timeout is reached
    let mut waited = Duration::ZERO;
    loop {
        match rx.try_recv() {
            Ok(recv_msg) => return Some(recv_msg),
            Err(_) => {
                if waited >= timeout {
                    // no response found
                    return None;
                }
                // wait for a response
                thread::sleep(Duration::from_millis(100));
                waited += Duration::from_millis(100);
            }
        }
    }
}

#[allow(dead_code)]
pub fn send_message_without_response(message: Message) {
    let json_msg = serde_json::to_string(&message).unwrap() + "\n";

    let server = SERVER.lock().unwrap();
    let mut clients = server.clients.lock().unwrap();
    for client in clients.iter_mut() {
        // a closed client is removed by its handle_client thread
        write_in_chunks(client, json_msg.as_bytes()).ok();
    }

    drop(clients); // drop lock to avoid deadlock
    drop(server);
}

// function to write data in 4 KiB chunks
fn write_in_chunks(stream: &mut TcpStream, data: &[u8]) -> std::io::Result<()> {
    let chunk_size = 4096;
    for chunk in data.chunks(chunk_size) {
        stream.write_all(chunk)?;
    }
    Ok(())
}
