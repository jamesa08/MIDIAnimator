// run from /src-tauri
// cargo test --test ipc_test
use serde_json::{json, Value};
use std::sync::Mutex;
use motionkeys::bridge::scene_data::{compare_scene_data, AppChange};
use motionkeys::graph::executors::scene::NOT_CONNECTED;
use motionkeys::ipc::{accept_hello, apply_info, apply_scene_update, negotiate, take_messages, timeout, valid_icon, valid_port, Envelope, PROTOCOL_VERSIONS};
use motionkeys::scene_generics::{find_group, Scene};
use std::time::Duration;
use motionkeys::state::{AppState, STATE};

// the tests share the global state, so they run one at a time
static STATE_TEST: Mutex<()> = Mutex::new(());

fn lock_test() -> std::sync::MutexGuard<'static, ()> {
    STATE_TEST.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

// a scene named Scene like the tracker's build_scene() builds, one group per name with `objects` objects each
fn scene_json(groups: &[&str], objects: usize) -> Value {
    named_scene_json("Scene", groups, objects)
}

fn named_scene_json(name: &str, groups: &[&str], objects: usize) -> Value {
    let groups: Vec<_> = groups
        .iter()
        .map(|group| {
            let objects: Vec<_> = (0..objects).map(|i| json!({"id": format!("{group}_obj_{i} \"}}"), "name": format!("{group}_obj_{i} \"}}"), "transform": {"position": [0.0, 1.0, 2.0], "rotation": [0.0, 0.0, 0.0], "scale": [1.0, 1.0, 1.0]}, "channels": [{"id": "location[0]", "name": "Location", "group": "Object", "axis": "X", "semantic": "position.x", "animated": false}]})).collect();
            json!({"id": format!("collection:{group}"), "name": group, "objects": objects})
        })
        .collect();
    json!({"id": name, "name": name, "object_groups": groups, "fps": 24.0, "axes": {"up": "z", "handedness": "right", "angle_unit": "radians"}})
}

// a message framed like the add-on sends it, json + newline
fn frame(kind: &str, body: Value, id: &str) -> Vec<u8> {
    Envelope {
        id: id.to_string(),
        kind: kind.to_string(),
        reply_to: None,
        body,
    }
    .line()
}

// a hello like the Blender bridge's
fn hello(protocol: Value) -> Envelope {
    let body = json!({"protocol": protocol, "app": "blender", "app_name": "Blender", "app_version": "5.2.2", "bridge_version": "0.5.0", "file": "it's.blend", "capabilities": {"watch": true, "scripts": ["python"]}});
    Envelope {
        id: "h".to_string(),
        kind: "hello".to_string(),
        reply_to: None,
        body,
    }
}

// a fresh state with Blender connected and linked to the first tab
fn linked_state() -> AppState {
    let mut state = AppState::default();
    state.connected = true;
    state.connected_instance_id = Some("tab-1".to_string());
    state
}

fn group_names(tab: &str) -> Vec<String> {
    STATE.lock().unwrap().instance(tab).unwrap().scene.as_ref().unwrap().object_groups.iter().map(|group| group.name.clone()).collect()
}

#[test]
fn take_messages_waits_for_the_full_message() {
    let framed = frame("scene", scene_json(&["Pianos"], 200), "a");
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
    assert_eq!(messages[0].id, "a");
    assert!(data.is_empty());
}

#[test]
fn take_messages_splits_messages_in_one_read() {
    // a reply, a scene update and the start of a third message all in one read
    let mut data = frame("curves", json!({}), "a");
    data.extend(frame("scene", scene_json(&["Drums"], 3), "b"));
    let third = frame("write_result", json!({"keyframes": 3}), "c");
    data.extend_from_slice(&third[..10]);

    let messages = take_messages(&mut data);
    assert_eq!(messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["a", "b"]);

    // the partial message is kept for the next read
    data.extend_from_slice(&third[10..]);
    let messages = take_messages(&mut data);
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].kind, "write_result");
    assert_eq!(messages[0].body["keyframes"], 3);
}

#[test]
fn take_messages_skips_a_bad_message() {
    // a bad line doesn't block the messages after it
    let mut data = b"not json\n".to_vec();
    data.extend(frame("scene", json!({}), "a"));
    let messages = take_messages(&mut data);
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].id, "a");
}

#[test]
fn scene_update_replaces_the_scene() {
    let _guard = lock_test();
    *STATE.lock().unwrap() = linked_state();

    // the linked tab is on screen, it runs
    assert_eq!(apply_scene_update(scene_json(&["Pianos"], 2)), Ok(Some("tab-1".to_string())));
    assert_eq!(group_names("tab-1"), ["Pianos"]);

    // a new top level collection comes through
    let mut data = frame("scene", scene_json(&["Pianos", "Drums"], 2), "b");
    let messages = take_messages(&mut data);
    assert_eq!(apply_scene_update(messages[0].body.clone()), Ok(Some("tab-1".to_string())));
    assert_eq!(group_names("tab-1"), ["Pianos", "Drums"]);
}

#[test]
fn scene_update_goes_to_the_linked_tab() {
    let _guard = lock_test();
    *STATE.lock().unwrap() = linked_state();
    {
        let mut state = STATE.lock().unwrap();
        let id = state.add_instance();
        state.active_instance_id = id;
    }

    // the linked tab is in the background: it takes the scene, the tab on screen doesn't, and nothing runs yet
    assert_eq!(apply_scene_update(scene_json(&["Pianos"], 1)), Ok(None));
    assert_eq!(group_names("tab-1"), ["Pianos"]);
    assert!(STATE.lock().unwrap().instance("tab-2").unwrap().scene.is_none());
}

#[test]
fn scene_update_without_a_linked_tab_goes_nowhere() {
    let _guard = lock_test();
    *STATE.lock().unwrap() = AppState::default();
    assert_eq!(apply_scene_update(scene_json(&["Pianos"], 1)), Ok(None));
    assert!(STATE.lock().unwrap().active().scene.is_none());
}

#[test]
fn scene_update_while_paused_is_pending() {
    let _guard = lock_test();
    *STATE.lock().unwrap() = linked_state();
    apply_scene_update(scene_json(&["Pianos"], 1)).unwrap();
    STATE.lock().unwrap().active_mut().execution_paused = true;

    // waits for review instead of replacing the scene
    assert_eq!(apply_scene_update(scene_json(&["Pianos", "Drums"], 1)), Ok(None));
    let state = STATE.lock().unwrap();
    assert_eq!(state.active().scene.as_ref().unwrap().object_groups.len(), 1);
    assert_eq!(state.active().pending_scene.as_ref().unwrap().object_groups.len(), 2);
}

#[test]
fn switching_to_a_different_scene_pauses_for_review() {
    let _guard = lock_test();
    *STATE.lock().unwrap() = linked_state();
    apply_scene_update(scene_json(&["Pianos"], 1)).unwrap();

    // another scene with other collections waits for review
    assert_eq!(apply_scene_update(named_scene_json("Scene.001", &["Drums"], 1)), Ok(None));
    let state = STATE.lock().unwrap();
    assert!(state.active().execution_paused);
    assert_eq!(state.active().scene.as_ref().unwrap().name, "Scene");
    assert_eq!(state.active().pending_scene.as_ref().unwrap().name, "Scene.001");
}

#[test]
fn switching_to_a_scene_with_the_same_objects_runs() {
    let _guard = lock_test();
    *STATE.lock().unwrap() = linked_state();
    apply_scene_update(scene_json(&["Pianos"], 1)).unwrap();

    // nothing to review, a renamed scene or one with the same collections linked is taken as is
    assert_eq!(apply_scene_update(named_scene_json("Renamed", &["Pianos"], 1)), Ok(Some("tab-1".to_string())));
    let state = STATE.lock().unwrap();
    assert!(!state.active().execution_paused);
    assert_eq!(state.active().scene.as_ref().unwrap().name, "Renamed");
}

#[test]
fn scenes_are_told_apart_by_id() {
    let _guard = lock_test();
    *STATE.lock().unwrap() = linked_state();
    apply_scene_update(scene_json(&["Pianos"], 1)).unwrap();

    // the same scene renamed, its collections changing, is no switch
    let mut renamed = scene_json(&["Drums"], 1);
    renamed["name"] = json!("Renamed");
    assert_eq!(apply_scene_update(renamed), Ok(Some("tab-1".to_string())));
    assert!(!STATE.lock().unwrap().active().execution_paused);

    // another scene with the same name is
    let mut other = scene_json(&["Pianos"], 1);
    other["id"] = json!("other");
    assert_eq!(apply_scene_update(other), Ok(None));
    assert!(STATE.lock().unwrap().active().execution_paused);
}

#[test]
fn a_scene_reads_its_channels_transforms_and_axes() {
    let scene: Scene = serde_json::from_value(scene_json(&["Pianos"], 1)).unwrap();
    let object = &scene.object_groups[0].objects[0];
    assert_eq!(object.transform.as_ref().unwrap().position, [0.0, 1.0, 2.0]);
    assert_eq!(object.channel("location[0]").unwrap().title(), "Location X");
    assert_eq!(object.channel_label("location[0]"), "Location X");
    assert_eq!(object.channel_label("not listed"), "not listed");
    assert_eq!(scene.axes.as_ref().unwrap().up.as_deref(), Some("z"));

    // scene data saved before: objects with their transform fields and shape key reprs, no ids or channels
    let saved = json!({"name": "Scene", "fps": 24.0, "object_groups": [{"name": "Pianos", "objects": [{"id": "Cube", "name": "Cube", "position": {"x": 0.0, "y": 0.0, "z": 0.0}, "rotation": {"x": 0.0, "y": 0.0, "z": 0.0}, "scale": {"x": 1.0, "y": 1.0, "z": 1.0}, "blend_shapes": {"keys": [], "reference": null}, "anim_curves": []}]}]});
    let saved: Scene = serde_json::from_value(saved).unwrap();
    assert_eq!(saved.key(), "Scene");
    assert!(saved.object_groups[0].objects[0].channels.is_empty());
    assert!(saved.object_groups[0].objects[0].transform.is_none());
}

#[test]
fn scene_update_rejects_bad_scene() {
    let _guard = lock_test();
    *STATE.lock().unwrap() = linked_state();
    assert!(apply_scene_update(json!({"name": "Scene", "object_groups": 5})).is_err());
    assert!(STATE.lock().unwrap().active().scene.is_none());
}

#[test]
fn port_setting_must_be_a_whole_number_in_range() {
    assert_eq!(valid_port(&json!(6577)), Some(6577));
    assert_eq!(valid_port(&json!(65535)), Some(65535));
    // a quoted port, out of range, fractional or missing falls back to the default
    assert_eq!(valid_port(&json!("6537")), None);
    assert_eq!(valid_port(&json!(80)), None);
    assert_eq!(valid_port(&json!(70000)), None);
    assert_eq!(valid_port(&json!(6577.5)), None);
    assert_eq!(valid_port(&json!(null)), None);
}

#[test]
fn connecting_clears_only_the_not_connected_errors() {
    let mut state = AppState::default();
    let results = &mut state.instance_mut("tab-1").unwrap().executed_results;
    results.insert("scene_writer-1".to_string(), json!({ "motionkeys_error": NOT_CONNECTED }));
    results.insert("group-1".to_string(), json!({ "motionkeys_error": format!("failed inside the group at group-1/scene_writer-1: {NOT_CONNECTED}") }));
    results.insert("scene_writer-2".to_string(), json!({ "motionkeys_error": "couldn't read Blender's write report: Traceback ..." }));
    results.insert("scene_writer-3".to_string(), json!({}));

    state.clear_not_connected();

    let mut left: Vec<_> = state.instance("tab-1").unwrap().executed_results.keys().cloned().collect();
    left.sort();
    // a real write error and a write that went through stay
    assert_eq!(left, vec!["scene_writer-2", "scene_writer-3"]);
}

#[test]
fn hello_picks_the_newest_version_both_speak() {
    let newest = *PROTOCOL_VERSIONS.last().unwrap();
    let (hello, version) = accept_hello(&hello(json!([newest, newest + 1])), None).unwrap();
    assert_eq!(version, newest);
    assert_eq!(hello.app_name, "Blender");
    assert_eq!(hello.file, "it's.blend");
    assert_eq!(hello.capabilities.scripts, ["python"]);
    assert_eq!(negotiate(&[newest + 1]), None);
}

#[test]
fn hello_is_refused_for_an_unknown_version_a_second_bridge_or_another_first_message() {
    let newest = *PROTOCOL_VERSIONS.last().unwrap();
    assert_eq!(accept_hello(&hello(json!([newest + 1])), None).unwrap_err().0, "unsupported_version");
    // the message names the app that's connected
    assert_eq!(accept_hello(&hello(json!([newest])), Some("Maya")).unwrap_err(), ("busy", "Maya is already connected to MotionKeys, disconnect it first".to_string()));
    let scene = Envelope {
        id: "s".to_string(),
        kind: "scene".to_string(),
        reply_to: None,
        body: scene_json(&["Drums"], 1),
    };
    assert_eq!(accept_hello(&scene, None).unwrap_err().0, "invalid_message");
    let bad = Envelope {
        id: "b".to_string(),
        kind: "hello".to_string(),
        reply_to: None,
        body: json!({"app": "blender"}),
    };
    assert_eq!(accept_hello(&bad, None).unwrap_err().0, "invalid_message");
}

#[test]
fn icons_are_small_png_or_svg_data_urls() {
    assert!(valid_icon(Some("data:image/png;base64,iVBORw0KGgo=".to_string())).is_some());
    assert!(valid_icon(Some("data:image/svg+xml;base64,PHN2Zz4=".to_string())).is_some());
    assert!(valid_icon(Some("https://example.com/icon.png".to_string())).is_none());
    // 64 KiB decoded at most
    assert!(valid_icon(Some(format!("data:image/png;base64,{}", "A".repeat(87_384)))).is_some());
    assert!(valid_icon(Some(format!("data:image/png;base64,{}", "A".repeat(87_388)))).is_none());
    assert!(valid_icon(None).is_none());
}

#[test]
fn scenes_are_marked_with_the_connected_app() {
    let _guard = lock_test();
    let mut state = linked_state();
    state.connected_application = "blender".to_string();
    state.connected_name = "Blender".to_string();
    *STATE.lock().unwrap() = state;
    apply_scene_update(scene_json(&["Pianos"], 1)).unwrap();
    let state = STATE.lock().unwrap();
    let scene = state.active().scene.as_ref().unwrap();
    assert_eq!((scene.app.as_deref(), scene.app_name.as_deref()), (Some("blender"), Some("Blender")));
}

#[test]
fn a_scene_from_another_app_is_flagged() {
    let mut saved: Scene = serde_json::from_value(scene_json(&["Pianos"], 1)).unwrap();
    let mut fresh = saved.clone();
    saved.app = Some("blender".to_string());
    fresh.app = Some("maya".to_string());
    let diff = compare_scene_data(Some(&saved), &fresh);
    assert_eq!(
        diff.app_change,
        Some(AppChange {
            saved: "blender".to_string(),
            connected: "maya".to_string()
        })
    );
    assert!(diff.has_changes());

    // shown by the apps' names once they're kept
    saved.app_name = Some("Blender".to_string());
    fresh.app_name = Some("Maya".to_string());
    assert_eq!(
        compare_scene_data(Some(&saved), &fresh).app_change,
        Some(AppChange {
            saved: "Blender".to_string(),
            connected: "Maya".to_string()
        })
    );

    // a scene saved before scenes were marked isn't flagged
    saved.app = None;
    assert!(!compare_scene_data(Some(&saved), &fresh).has_changes());
}

#[test]
fn groups_saved_without_ids_match_by_name() {
    // saved before groups had ids, Blender now sends collection ids
    let saved: Scene = serde_json::from_value(scene_json(&["Pianos"], 1)).unwrap();
    let mut fresh = saved.clone();
    fresh.object_groups[0].id = "collection:Pianos".to_string();
    assert!(!compare_scene_data(Some(&saved), &fresh).has_changes());

    // with ids on both sides, the ids are compared
    let mut renamed = fresh.clone();
    renamed.object_groups[0].id = "collection:Keys".to_string();
    let diff = compare_scene_data(Some(&fresh), &renamed);
    assert_eq!(diff.missing_collections, vec!["Pianos".to_string()]);
    assert_eq!(diff.new_collections, vec!["Pianos".to_string()]);
}

#[test]
fn a_group_pick_is_its_id_or_else_its_name() {
    let mut scene: Scene = serde_json::from_value(scene_json(&["Scene group", "Scene group"], 1)).unwrap();
    scene.object_groups[0].id = "collection:Scene group".to_string();
    scene.object_groups[1].id = "scene".to_string();
    let groups = &scene.object_groups;
    // a collection named like the scene group never takes its place
    assert_eq!(find_group(groups, "scene").map(|g| g.id.as_str()), Some("scene"));
    assert_eq!(find_group(groups, "collection:Scene group").map(|g| g.id.as_str()), Some("collection:Scene group"));
    // a pick saved before groups had ids, or typed in, is a name
    assert_eq!(find_group(groups, "Scene group").map(|g| g.id.as_str()), Some("collection:Scene group"));
    assert!(find_group(groups, "Drums").is_none());
}

#[test]
fn info_changes_the_file_name() {
    let _guard = lock_test();
    let mut state = linked_state();
    state.ready = true;
    state.connected_file_name = "drums.blend".to_string();
    *STATE.lock().unwrap() = state;
    apply_info(&json!({"file": "drums_v2.blend"}));
    assert_eq!(STATE.lock().unwrap().connected_file_name, "drums_v2.blend");
    // absent fields stay as they are
    apply_info(&json!({}));
    assert_eq!(STATE.lock().unwrap().connected_file_name, "drums_v2.blend");
}

#[test]
fn timeouts_come_from_the_settings() {
    assert_eq!(timeout("get_scene"), Duration::from_secs(10));
    assert_eq!(timeout("watch"), Duration::from_secs(30));
    assert_eq!(timeout("write"), Duration::from_secs(60));
    assert_eq!(timeout("run_script"), Duration::from_secs(30));
}
