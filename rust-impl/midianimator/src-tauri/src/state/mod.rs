use std::{collections::HashMap, sync::Mutex};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use lazy_static::lazy_static;
use tauri::api::dialog::blocking::FileDialogBuilder;
use std::fs;

use crate::scene_generics::Scene;



lazy_static! {
    pub static ref STATE: Mutex<AppState> = Mutex::new(AppState::default());
    pub static ref WINDOW: Arc<Mutex<Option<tauri::Window>>> = Arc::new(Mutex::new(None));
}

/// state struct for the application
/// 
/// this is a global state that is shared between the front end and the backend.
/// 
/// front end also has its own state, but this is the global state that is shared between the two.
/// 
/// note: the only way to update this state is through the backend, and the front end can only read from it.
///       if you wanted to change a variable, you will have to create a command in the backend that will update the state.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppState {
    pub ready: bool,
    pub connected: bool,
    pub execution_paused: bool,
    pub connected_application: String,
    pub connected_version: String,
    pub connected_file_name: String,
    pub scene_data: HashMap<String, Scene>,
    pub pending_scene_data: Option<HashMap<String, Scene>>, 
    pub rf_instance: HashMap<String, serde_json::Value>,
    pub executed_results: HashMap<String, serde_json::Value>,
    pub executed_inputs: HashMap<String, serde_json::Value>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            ready: false,
            connected: false,
            execution_paused: false,
            connected_application: "".to_string(),
            connected_version: "".to_string(),
            connected_file_name: "".to_string(),
            scene_data: HashMap::new(),
            pending_scene_data: None,
            rf_instance: HashMap::new(),
            executed_results: HashMap::new(),
            executed_inputs: HashMap::new(),
        }
    }
}

/// this commmand is called when the front end is loaded and ready to receive commands
#[tauri::command]
pub fn ready() -> AppState {
    println!("READY");
    let mut state = STATE.lock().unwrap();
    state.ready = true;
    return state.clone();
}

/// this command can be called from the front end to update changes to the state from the front end
/// you can use this by calling `window.tauri.invoke('js_update_state', {state: JSON.stringify(your_new_state_objet)})`
/// also use setBackendState() in the front end to update React's state with the new state
/// this function will replace the entire state, so make sure to include all the necessary data in the new state
/// note you cannot add new fields to the state, only update the existing fields
#[allow(unused_must_use)]
#[tauri::command]
pub fn js_update_state(state: String) {
    println!("FRONTEND STATE UPDATE");
    // println!("{:#?}", state);
    let mut cur_state = STATE.lock().unwrap();

    // re-serealize the state
    let new_state: AppState = serde_json::from_str(&state).unwrap_or_default();

    // replace the current state with the new state
    std::mem::replace(&mut *cur_state, new_state);
    drop(cur_state);
}

#[tauri::command]
pub fn log(message: String) {
    println!("{}", message);
}


/// sends state to front end
/// emits via command "update_state"
pub fn update_state() {
    println!("BACKEND STATE UPDATE");
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SavedProject {
    pub scene_data: HashMap<String, Scene>,
    pub rf_instance: HashMap<String, serde_json::Value>,
}

// save project command
#[tauri::command]
pub async fn save_project() -> Result<String, String> {
    // Clone data BEFORE any async operations
    let saved_data = {
        let state = STATE.lock().unwrap();
        SavedProject {
            scene_data: state.scene_data.clone(),
            rf_instance: state.rf_instance.clone(),
        }
    };
    
    let file_path = FileDialogBuilder::new()
        .set_title("Save Project")
        .add_filter("MIDIAnimator Project", &["mkproj"])
        .save_file();
    
    match file_path {
        Some(path) => {
            let json = serde_json::to_string_pretty(&saved_data)
                .map_err(|e| format!("Serialization error: {}", e))?;
            
            fs::write(&path, json)
                .map_err(|e| format!("File write error: {}", e))?;
            
            Ok(path.to_string_lossy().to_string())
        }
        None => Err("Save cancelled".to_string())
    }
}

// load project command
#[tauri::command]
pub async fn load_project() -> Result<AppState, String> {
    let file_path = FileDialogBuilder::new()
        .set_title("Load Project")
        .add_filter("MIDIAnimator Project", &["mkproj"])
        .pick_file();
    
    let path = file_path.ok_or("Load cancelled")?;
    
    let json = fs::read_to_string(&path)
        .map_err(|e| format!("File read error: {}", e))?;
    
    let saved_data: SavedProject = serde_json::from_str(&json)
        .map_err(|e| format!("Deserialization error: {}", e))?;
    
    let new_state = {
        let mut state = STATE.lock().unwrap();
        
        state.scene_data = saved_data.scene_data;
        state.rf_instance = saved_data.rf_instance;
        
        if state.connected {
            state.execution_paused = true;  // pause until validation
        }
        
        state.executed_results.clear();
        state.executed_inputs.clear();
        
        state.clone()
    };
    
    update_state();
    
    Ok(new_state)
}
