use crate::state::WINDOW;
use std::sync::{Arc, Mutex};
use tauri::Listener;

pub async fn evaluate_js(code: String) -> String {
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);

    let listener_id: Arc<Mutex<Option<tauri::EventId>>> = Arc::new(Mutex::new(None));
    let listener_id_clone = listener_id.clone();

    tokio::spawn(async move {
        let window = WINDOW.lock().unwrap();
        let random = uuid::Uuid::new_v4().to_string();

        let wrapper_code = format!(
            r#"{0}
        (function() {{
                try {{
                    execute().then((result) => {{
                        window.__TAURI__.event.emit("__js_result_{1}", {{ result: result }})
                    }});
                }} catch (error) {{
                    console.log("JS ERROR:", error);
                    window.__TAURI__.event.emit("__js_result_{1}", {{ result: JSON.stringify({{ error: error.toString() }}) }})
                }}
            }})();
            "#,
            code, random
        );

        let win_ref = window.as_ref().unwrap();

        win_ref.eval(&wrapper_code).unwrap();

        let listener_handle = win_ref.once(format!("__js_result_{0}", random), move |event| {
            let payload = event.payload().to_string();
            let _ = tx.try_send(payload);
        });

        *listener_id.lock().unwrap() = Some(listener_handle);
    });

    let result = rx.recv().await.unwrap();

    if !result.is_empty() {
        let window = WINDOW.lock().unwrap();
        let win_ref = window.as_ref().unwrap();
        win_ref.unlisten(*listener_id_clone.lock().unwrap().as_ref().unwrap());
        drop(window);
        result
    } else {
        "".to_string()
    }
}

pub fn evaluate_js_oneshot(code: String) {
    if let Ok(window) = WINDOW.lock() {
        if let Some(window) = window.as_ref() {
            let _ = window.eval(&code);
        }
    }
}
