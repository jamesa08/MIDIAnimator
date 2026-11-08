use motionkeys::settings::migrate_settings;
use serde_json::json;

#[test]
fn the_import_limit_moves_to_its_new_name() {
    let mut saved = json!({ "blender": { "keyframe_import_limit": 200 }, "ipc": { "port": 7000 } });
    assert!(migrate_settings(&mut saved));
    assert_eq!(saved, json!({ "bridge": { "keyframe_import_limit": 200 }, "ipc": { "port": 7000 } }));

    // nothing saved under the old name, nothing moves
    assert!(!migrate_settings(&mut saved));

    // a value saved under the new name wins
    let mut both = json!({ "blender": { "keyframe_import_limit": 200 }, "bridge": { "keyframe_import_limit": 300 } });
    assert!(migrate_settings(&mut both));
    assert_eq!(both, json!({ "bridge": { "keyframe_import_limit": 300 } }));
}
