// run from /src-tauri
// cargo test --test recent_test
//
// File > Open Recent: the newest first, each once, at most ten, named by file name with the folder for files named alike

use motionkeys::settings::{clear_recent, forget_recent, note_recent, push_recent, recent_labels, recent_projects, RECENT_LIMIT};
use motionkeys::state::{load_project, lock, open_file, save_instance_to, AppState, STATE};

// the newest goes first, one opened again moves up instead of showing twice, the oldest fall off past the limit
#[test]
fn push_keeps_newest_first() {
    let mut list = Vec::new();
    for i in 0..12 {
        push_recent(&mut list, &format!("/p/{}.mkproj", i));
    }
    assert_eq!(list.len(), RECENT_LIMIT);
    assert_eq!(list[0], "/p/11.mkproj");
    assert_eq!(list[9], "/p/2.mkproj");
    push_recent(&mut list, "/p/5.mkproj");
    assert_eq!(list.len(), RECENT_LIMIT);
    assert_eq!(list[0], "/p/5.mkproj");
    assert_eq!(list.iter().filter(|p| *p == "/p/5.mkproj").count(), 1);
}

// file names, with the folder only where two have the same name
#[test]
fn labels_add_folders_for_same_names() {
    let list = ["/a/Demos/song.mkproj", "/b/Live/song.mkproj", "/c/drums.mkproj"].map(String::from);
    assert_eq!(recent_labels(&list), ["song.mkproj — Demos", "song.mkproj — Live", "drums.mkproj"]);
}

// opening and saving a project puts it first (by its real path), a missing one can be forgotten and the menu cleared.
// the only test touching the app's list, so the others can't change it underneath
#[test]
fn opening_and_saving_note_projects() {
    let dir = std::env::temp_dir().join(format!("motionkeys_recent_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let fixture = std::fs::canonicalize("tests/fixtures/simple_scene_3_executor.mkproj").unwrap();
    let opened = dir.join("opened.mkproj");
    std::fs::copy(&fixture, &opened).unwrap();
    let opened = std::fs::canonicalize(&opened).unwrap().to_string_lossy().to_string();
    let saved = format!("{}/saved.mkproj", std::fs::canonicalize(&dir).unwrap().to_string_lossy());

    let mut state = AppState::default();
    state.ready = true;
    *STATE.lock().unwrap() = state;
    clear_recent();

    open_file(&opened).unwrap();
    assert_eq!(recent_projects(), [opened.clone()]);
    let tab = lock().active_instance_id.clone();
    save_instance_to(&tab, &saved).unwrap();
    assert_eq!(recent_projects(), [saved.clone(), opened.clone()]);
    // opened again (it's already in a tab) it moves back up
    open_file(&opened).unwrap();
    assert_eq!(recent_projects(), [opened.clone(), saved.clone()]);
    note_recent(&saved);
    assert_eq!(recent_projects(), [saved.clone(), opened.clone()]);

    // picking one whose file is gone (and no tab has open) fails and takes it off the list
    let gone = dir.join("gone.mkproj");
    std::fs::copy(&fixture, &gone).unwrap();
    let gone = std::fs::canonicalize(&gone).unwrap().to_string_lossy().to_string();
    note_recent(&gone);
    std::fs::remove_file(&gone).unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    assert!(runtime.block_on(load_project(Some(0))).unwrap_err().contains("File read error"));
    assert_eq!(recent_projects(), [saved.clone(), opened.clone()]);

    forget_recent(&opened);
    assert_eq!(recent_projects(), [saved.clone()]);
    clear_recent();
    assert!(recent_projects().is_empty());
    std::fs::remove_dir_all(&dir).ok();
}
