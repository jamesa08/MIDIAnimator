use crate::structures::state::WINDOW;
use std::sync::{Arc, Mutex};
use tauri::Manager;

/// this function is used to evaluate javascript code on the window
/// 
/// this is useful for executing javascript code from the backend & getting the result back, nice for dynamic code execution
/// in order to use this function, you will need to pass in a string with a javascript function called `execute()` that returns some string'ed value.
/// 
/// The returned value will be in JSON string format, so you will need to parse it. The result of the function is in a key called `result`. 
/// 
/// WARNING: this may change at any time. Sorry not sorry
/// 
/// Example:
/// ```rust
/// let result = evaluate_js("function execute() { return 'hello world'; }".to_string()).await;
/// println!("Result: {}", result);
/// ```
pub async fn evaluate_js(code: String) -> String {
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);

    // need to clone the listener id so we can remove it later
    let listener_id: Arc<Mutex<Option<tauri::EventHandler>>> = Arc::new(Mutex::new(None));
    let listener_id_clone = listener_id.clone();

    tokio::spawn(async move {
        let window = WINDOW.lock().unwrap();
        let random = uuid::Uuid::new_v4().to_string();

        let wrapper_code = format!(r#"{0}
        (function() {{
                try {{
                    execute().then((result) => {{
                        console.log("JS result", result);
                        window.__TAURI__.window.appWindow.emit("__js_result_{1}", {{ result: result }})
                    }});
                }} catch (error) {{
                    console.log("ERROR:", error);
                    window.__TAURI__.window.appWindow.emit("__js_result_{1}", {{ result: JSON.stringify({{ error: error.toString() }}) }})
                }}
            }})();
            "#,
            code,
            random
        );

        // eval javascript code blindly on the window. MUST be non-blocking for it to execute and for the event to get picked up (hence async)
        let _ = window.as_ref().unwrap().eval(&wrapper_code);


        let listener_handle = window.as_ref().unwrap().once_global(format!("__js_result_{0}", random), move |event| {
            if let Some(payload) = event.payload() {
                let _ = tx.try_send(payload.to_string());
            }
        });
        
        // Set the listener id so we can remove it later
        *listener_id.lock().unwrap() = Some(listener_handle);
    });


    let result = rx.recv().await.unwrap();

    if !result.is_empty() {
        let window = WINDOW.lock().unwrap();
        
        // Remove the event listener
        window.as_ref().unwrap().unlisten(*listener_id_clone.lock().unwrap().as_ref().unwrap());
        drop(window);

        return result;
    } else {
        return "".to_string();
    }
}

