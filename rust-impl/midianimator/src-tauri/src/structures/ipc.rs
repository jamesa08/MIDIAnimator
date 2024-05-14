use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::mpsc;
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
    let listener = TcpListener::bind(format!("127.0.0.1:{port}", port=PORT).to_string()).unwrap();
    println!("MIDIAnimator IPC server started. Listening on port {:?}", PORT);

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
                    
                    let mut clients = server_unwrapped.clients.lock().unwrap();
                    
                    // when a new client connects, add it to the clients list
                    clients.push(stream.try_clone().unwrap());
                    
                    // no longer need clients variable, drop it to avoid deadlock
                    drop(clients);

                    let server_clone = Arc::clone(&server);
                    thread::spawn(move || handle_client(stream, server_clone));
                }
                Err(e) => {
                    println!("Error: {}", e);
                }
            }
        }
    });
    return server
});

#[allow(unused_must_use)]
pub fn start_server() {
    // we don't need the result, since it will run in the background
    SERVER.lock().unwrap();
}

// handle a client connection
fn handle_client(stream: TcpStream, server: Arc<Mutex<Server>>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let writer = stream;

    // keep reading messages from the client until the connection is closed
    loop {
        let mut data = String::new();

        // messages should be formatted in JSON
        match reader.read_line(&mut data) {
            Ok(0) => break,
            Ok(_) => {
                let message: Message = match serde_json::from_str(&data) {
                    Ok(msg) => msg,
                    Err(e) => {
                        println!("Error parsing JSON from client: {}", e);
                        continue;
                    }
                };

                // at this point, message is in valid JSON format

                let tx = {
                    let server_lock = server.lock().unwrap();
                    let mut message_map = server_lock.message_map.lock().unwrap();
                    message_map.remove(&message.uuid)  // this gets the tx sender from send_message
                };
                
                // if the sender is found, send the message to the sender
                if let Some(tx) = tx {
                    // send the message to the sender
                    tx.send(message.message).unwrap();
                }
            }
            Err(e) => {
                println!("Error in handle_client(): {}", e);
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
        message: message,
        uuid: Uuid::new_v4().to_string(),
    };

    let json_msg = serde_json::to_string(&msg_struct).unwrap() + "\n";

    let server = SERVER.lock().unwrap();
    // send the message to all clients
    let mut clients = server.clients.lock().unwrap();
    for client in clients.iter_mut() {
        client.write_all(json_msg.as_bytes()).unwrap();
    }

    // no longer need clients, drop it to avoid deadlock
    drop(clients);

    // create a channel to receive the response, and insert it into the message_map
    let (tx, rx) = mpsc::channel();
    let mut message_map = server.message_map.lock().unwrap();
    message_map.insert(msg_struct.uuid.clone(), tx);
    
    // unlock the message_map and server to avoid deadlock
    drop(message_map);
    drop(server);

    // loop until a response is received or the timeout is reached
    let mut durations: i8 = 0;
    loop {
        // attempt to receive a message
        let response = rx.try_recv();
        match response {
            Ok(recv_msg) => {
                // return the response
                return Some(recv_msg);
            }
            Err(_) => {
                if durations >= 50 {
                    // no response found, return None
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
        client.write_all(json_msg.as_bytes()).unwrap();
    }

    // FIXME not sure if this is necessary
    drop(clients);
    drop(server);
}