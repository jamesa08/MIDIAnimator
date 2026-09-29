// run from /src-tauri
// cargo test --test ipc_test
use serde_json::json;
use std::sync::Mutex;
use MIDIAnimator::ipc::{apply_scene_update, take_messages, Message};
use MIDIAnimator::state::{js_update_state, AppState, STATE};

// the tests share the global state, so they run one at a time
static STATE_TEST: Mutex<()> = Mutex::new(());

fn lock_test() -> std::sync::MutexGuard<'static, ()> {
    STATE_TEST.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

// a scene like the tracker's execute() builds, one object group per name with `objects` objects each
fn scene_json(groups: &[&str], objects: usize) -> String {
    let object_groups: Vec<_> = groups
        .iter()
        .map(|group| {
            let objects: Vec<_> = (0..objects).map(|i| json!({"name": format!("{group}_obj_{i} \"}}"), "position": [0.0, 1.0, 2.0], "rotation": [0.0, 0.0, 0.0], "scale": [1.0, 1.0, 1.0], "blend_shapes": {"keys": [], "reference": null}, "anim_curves": []})).collect();
            json!({"name": group, "objects": objects})
        })
        .collect();
    json!({"Scene": {"name": "Scene", "object_groups": object_groups}}).to_string()
}

// a message framed like the add-on's send_message, json + newline
fn frame(message: &str, uuid: &str) -> Vec<u8> {
    let message = Message { sender: "client".to_string(), message: message.to_string(), uuid: uuid.to_string() };
    (serde_json::to_string(&message).unwrap() + "\n").into_bytes()
}

fn group_names() -> Vec<String> {
    STATE.lock().unwrap().scene_data["Scene"].object_groups.iter().map(|group| group.name.clone()).collect()
}

#[test]
fn take_messages_waits_for_the_full_message() {
    let framed = frame(&scene_json(&["Pianos"], 200), "a");
    assert!(framed.len() > 3 * 4096, "message should span several reads");

    // fed in 4 KiB reads, nothing comes out until the last one
    let mut data = Vec::new();
    let mut messages = Vec::new();
    for chunk in framed.chunks(4096) {
        assert!(messages.is_empty());
        data.extend_from_slice(chunk);
        messages = take_messages(&mut data);
    }
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].uuid, "a");
    assert!(data.is_empty());
}

#[test]
fn take_messages_splits_messages_in_one_read() {
    // a response, a scene update and the start of a third message all in one read
    let mut data = frame("OK", "a");
    data.extend(frame(&scene_json(&["Drums"], 3), "b"));
    let third = frame("next", "c");
    data.extend_from_slice(&third[..10]);

    let messages = take_messages(&mut data);
    assert_eq!(messages.iter().map(|m| m.uuid.as_str()).collect::<Vec<_>>(), ["a", "b"]);

    // the partial message is kept for the next read
    data.extend_from_slice(&third[10..]);
    let messages = take_messages(&mut data);
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].message, "next");
}

#[test]
fn take_messages_skips_a_bad_message() {
    // a bad line doesn't block the messages after it
    let mut data = b"not json\n".to_vec();
    data.extend(frame("OK", "a"));
    let messages = take_messages(&mut data);
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].uuid, "a");
}

#[test]
fn scene_update_replaces_scene_data() {
    let _guard = lock_test();
    *STATE.lock().unwrap() = AppState::default();

    assert_eq!(apply_scene_update(&scene_json(&["Pianos"], 2)), Ok(true));
    assert_eq!(group_names(), ["Pianos"]);

    // a new top level collection comes through
    let mut data = frame(&scene_json(&["Pianos", "Drums"], 2), "b");
    let messages = take_messages(&mut data);
    assert_eq!(apply_scene_update(&messages[0].message), Ok(true));
    assert_eq!(group_names(), ["Pianos", "Drums"]);
}

#[test]
fn scene_update_while_paused_is_pending() {
    let _guard = lock_test();
    *STATE.lock().unwrap() = AppState::default();
    apply_scene_update(&scene_json(&["Pianos"], 1)).unwrap();
    STATE.lock().unwrap().execution_paused = true;

    // waits for review instead of replacing the scene
    assert_eq!(apply_scene_update(&scene_json(&["Pianos", "Drums"], 1)), Ok(false));
    let state = STATE.lock().unwrap();
    assert_eq!(state.scene_data["Scene"].object_groups.len(), 1);
    assert_eq!(state.pending_scene_data.as_ref().unwrap()["Scene"].object_groups.len(), 2);
}

#[test]
fn scene_update_rejects_bad_scene() {
    let _guard = lock_test();
    *STATE.lock().unwrap() = AppState::default();
    assert!(apply_scene_update(r#"{"Scene": {"object_groups": 5}}"#).is_err());
    assert!(STATE.lock().unwrap().scene_data.is_empty());
}

#[test]
fn frontend_state_push_keeps_scene_data() {
    let _guard = lock_test();
    *STATE.lock().unwrap() = AppState::default();

    // the front end's copy is from before the scene update
    let stale = STATE.lock().unwrap().clone();
    apply_scene_update(&scene_json(&["Pianos", "Drums"], 1)).unwrap();

    let mut pushed = serde_json::to_value(&stale).unwrap();
    pushed["rf_instance"] = json!({"nodes": [{"id": "a"}], "edges": []});
    js_update_state(pushed.to_string());

    // the graph is taken, the stale scene data isn't
    let state = STATE.lock().unwrap();
    assert_eq!(state.rf_instance["nodes"], json!([{"id": "a"}]));
    assert_eq!(state.scene_data["Scene"].object_groups.len(), 2);
}
