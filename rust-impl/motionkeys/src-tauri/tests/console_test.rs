// run from /src-tauri
// cargo test --test console_test

use motionkeys::utils::console::{console_clear, console_entries, console_log, Entry, Level, Log, Source, CAPACITY};

fn entry(message: &str) -> Entry {
    Entry::new(Level::Info, Source::App, message)
}

#[test]
fn entries_get_ids_that_go_up() {
    let mut log = Log::new(10);
    let ids: Vec<u64> = ["a", "b", "c"].iter().map(|m| log.push(entry(m)).id).collect();
    assert_eq!(ids, [1, 2, 3]);
    let messages: Vec<String> = log.entries().into_iter().map(|e| e.message).collect();
    assert_eq!(messages, ["a", "b", "c"]);
}

#[test]
fn the_oldest_entries_go_past_the_capacity() {
    let mut log = Log::new(3);
    for i in 0..5 {
        log.push(entry(&i.to_string()));
    }
    let kept: Vec<(u64, String)> = log.entries().into_iter().map(|e| (e.id, e.message)).collect();
    assert_eq!(kept, [(3, "2".to_string()), (4, "3".to_string()), (5, "4".to_string())]);
}

#[test]
fn a_clear_keeps_the_ids_going() {
    let mut log = Log::new(3);
    log.push(entry("a"));
    log.push(entry("b"));
    log.clear();
    assert_eq!(log.len(), 0);
    // a window that caught up before the clear can't mistake a new entry for one it has
    assert_eq!(log.push(entry("c")).id, 3);
}

#[test]
fn an_entry_can_link_to_a_node() {
    let linked = Entry::new(Level::Error, Source::Node, "failed").node("tab-1", "group-1/viewer-2").ms(1.5);
    assert_eq!((linked.tab.as_deref(), linked.node.as_deref(), linked.ms), (Some("tab-1"), Some("group-1/viewer-2"), Some(1.5)));
    // what the windows get
    let json = serde_json::to_value(&linked).unwrap();
    assert_eq!((json["level"].as_str(), json["source"].as_str()), (Some("error"), Some("node")));
}

// the app's console, one test so nothing else clears it partway
#[test]
fn the_app_console_keeps_entries_until_cleared() {
    console_clear();
    console_log(Level::Warn, Source::App, "status bar warning".to_string());
    // js output only comes in debug mode, which is off
    console_log(Level::Info, Source::Js, "console.log".to_string());
    let messages: Vec<String> = console_entries().into_iter().map(|e| e.message).collect();
    assert_eq!(messages, ["status bar warning"]);

    for i in 0..CAPACITY + 10 {
        console_log(Level::Debug, Source::App, i.to_string());
    }
    let entries = console_entries();
    assert_eq!(entries.len(), CAPACITY);
    assert_eq!(entries.last().unwrap().message, (CAPACITY + 9).to_string());
    console_clear();
    assert!(console_entries().is_empty());
}
