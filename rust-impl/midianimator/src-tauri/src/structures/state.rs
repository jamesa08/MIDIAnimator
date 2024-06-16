use std::sync::Mutex;
use serde::Serialize;
use std::sync::Arc;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref STATE: Mutex<AppState> = Mutex::new(AppState::default());
    pub static ref WINDOW: Arc<Mutex<Option<tauri::Window>>> = Arc::new(Mutex::new(None));
}

/// state struct for the application
/// this is a global state that is shared between the front end and the backend.
/// front end also has its own state, but this is the global state that is shared between the two.
/// note: the only way to update this state is through the backend, and the front end can only read from it.
///       if you wanted to change a variable, you will have to create a command in the backend that will update the state.
#[derive(Serialize, Clone, Debug)]
pub struct AppState {
    pub ready: bool,
    pub connected: bool,
    pub connected_application: String,
    pub connected_version: String,
    pub connected_file_name: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            ready: false,
            connected: false,
            connected_application: "".to_string(),
            connected_version: "".to_string(),
            connected_file_name: "".to_string()
        }
    }
}

/// this commmand is called when the front end is loaded and ready to receive commands
#[tauri::command]
pub fn ready() -> AppState {
    let mut state = STATE.lock().unwrap();
    state.ready = true;
    return state.clone();
}

/// sends state to front end
/// emits via command "update_state"
pub fn update_state() {
    loop {
        let state = STATE.lock().unwrap();
        
        if state.ready { // if the app is ready, we can send the state
            break;
        }
        drop(state);
    }
    let state = STATE.lock().unwrap();
    let window = WINDOW.lock().unwrap();
    window.as_ref().unwrap().emit("update_state", state.clone()).unwrap();
    drop(window);
    drop(state);
}