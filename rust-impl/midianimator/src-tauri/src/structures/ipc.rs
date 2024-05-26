use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use uuid::Uuid;

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

static PORT: &str = "6577";

// create a server instance
// this is a lazy static variable, so it will only be created once
// and will be shared across all threads
// this is necessary because the server needs to be accessed across threads, and in other functions
static SERVER: Lazy<Arc<Mutex<Server>>> = Lazy::new(|| {
    // create a TCP listener on the specified port
    let listener = TcpListener::bind(format!("127.0.0.1:{port}", port = PORT)).unwrap();
    println!(
        "MIDIAnimator IPC server started. Listening on port {:?}",
        PORT
    );

    // create a server instance
    let server = Server {
        clients: Arc::new(Mutex::new(Vec::new())),
        message_map: Arc::new(Mutex::new(HashMap::new())),
    };
    let server = Arc::new(Mutex::new(server));

    // clone the server instance to be used in the thread
    let server_clone = Arc::clone(&server);

    thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    println!("New client connected {:?}", stream.peer_addr().unwrap());
                    let server = Arc::clone(&server_clone);
                    let server_unwrapped = server.lock().unwrap();

                    // lock the clients list and add the new client
                    let mut clients = server_unwrapped.clients.lock().unwrap();

                    clients.push(stream.try_clone().unwrap());
                    drop(clients); // drop lock to avoid deadlock

                    let server_clone = Arc::clone(&server);
                    thread::spawn(move || handle_client(stream, server_clone));
                }
                Err(e) => {
                    println!("Error: {}", e);
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

// handle a client connection
fn handle_client(stream: TcpStream, server: Arc<Mutex<Server>>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let writer = stream;

    // keep reading messages from the client until the connection is closed
    let mut data = Vec::new();

    loop {
        let mut buf = [0; 4096]; // 4 KiB buffer

        // need to loop over until we find the full `uuid` string with ending brace `}`.
        // This is to ensure we have read the full message.
        // if the message itself contains a uuid message, this is not a valid message.
        match reader.read(&mut buf) {
            Ok(0) => break, // connection closed
            Ok(n) => {
                data.extend_from_slice(&buf[..n]); // add the read bytes to data

                // convert the accumulated data to a string and parse it as JSON
                if let Ok(data_str) = String::from_utf8(data.clone()) {
                    // similar code is in python add-on
                    let check: Vec<&str> =
                        data_str.split("\"}").filter(|s| !s.is_empty()).collect();
                    if check.len() >= 2
                        && check.last() == Some(&"\n")
                        && check[check.len() - 2].contains("\"uuid\":")
                    {
                        // valid msg, continue

                        if let Ok(message) = serde_json::from_str::<Message>(&data_str) {
                            // find the tx sender from send_message using the UUID
                            let tx = {
                                let server_lock = server.lock().unwrap();
                                let mut message_map = server_lock.message_map.lock().unwrap();
                                message_map.remove(&message.uuid) // this gets the tx sender from send_message
                            };

                            if let Some(tx) = tx {
                                tx.send(message.message).unwrap();
                            }
                            // remove remaining data
                            // FIXME: (might want to remove just the message, but idk just yet)
                            data.clear();
                        }
                    }
                }
            }
            Err(e) => {
                println!("Error in handle_client(): {}", e);
                data.clear();
                break;
            }
        }

        // allow a small delay to let other threads allocate the lock
        thread::sleep(Duration::from_millis(10));
    }

    // if disconnected, remove the client from the server
    let server = server.lock().unwrap();
    let mut clients = server.clients.lock().unwrap();
    clients.retain(|c| !c.peer_addr().unwrap().eq(&writer.peer_addr().unwrap()));
}

pub async fn send_message(message: String) -> Option<String> {
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
        write_in_chunks(client, json_msg.as_bytes()).unwrap();
    }
    drop(clients);

    // create a channel to receive the response, and insert it into the message_map
    let (tx, rx) = mpsc::channel();
    server
        .message_map
        .lock()
        .unwrap()
        .insert(msg_struct.uuid.clone(), tx);
    drop(server);

    // loop until a response is received or the timeout is reached
    let mut durations: i8 = 0;
    loop {
        match rx.try_recv() {
            Ok(recv_msg) => return Some(recv_msg),
            Err(_) => {
                if durations >= 50 {
                    // no response found
                    return None;
                }
                // wait for a response
                thread::sleep(Duration::from_millis(100));
                durations += 1;
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
        write_in_chunks(client, json_msg.as_bytes()).unwrap();
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
