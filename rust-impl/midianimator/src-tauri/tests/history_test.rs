// run from /src-tauri
// cargo test --test history_test
//
// fixture: tests/fixtures/simple_scene_3_executor.mkproj, saved from the app, with short node ids

use std::collections::HashMap;

use serde_json::{json, Map, Value};
use MIDIAnimator::graph::builtin::{all_groups, builtin_groups};
use MIDIAnimator::graph::edit;
use MIDIAnimator::graph::history::{Diff, History, Source, Step};
use MIDIAnimator::graph::model::{node_specs, Graph, NodeSpec, Position, Specs};
use MIDIAnimator::state::{migrate_rf_instance, SavedProject};

type Project = HashMap<String, Value>;

/// loads the node specs from default_nodes.json
fn specs() -> Vec<NodeSpec> {
    let data = std::fs::read_to_string("src/configs/default_nodes.json").unwrap();
    let default_nodes: HashMap<String, Value> = serde_json::from_str(&data).unwrap();
    node_specs(&default_nodes)
}

/// the fixture project, migrated like the app does on load
fn fixture() -> Project {
    let data = std::fs::read_to_string("tests/fixtures/simple_scene_3_executor.mkproj").unwrap();
    let mut project: SavedProject = serde_json::from_str(&data).unwrap();
    migrate_rf_instance(&mut project.rf_instance);
    project.rf_instance
}

/// the project without the UI only fields history leaves alone, to compare projects
fn key(project: &Project) -> Value {
    let strip = |graph: &mut Map<String, Value>| {
        for (list, ui) in [("nodes", &["selected", "dragging", "measured", "resizing"][..]), ("edges", &["selected"][..])] {
            for item in graph.get_mut(list).and_then(Value::as_array_mut).into_iter().flatten() {
                if let Some(item) = item.as_object_mut() {
                    item.retain(|k, _| !ui.contains(&k.as_str()));
                }
            }
        }
    };
    let mut root: Map<String, Value> = project.clone().into_iter().collect();
    strip(&mut root);
    for def in root.get_mut("groups").and_then(Value::as_object_mut).into_iter().flat_map(|g| g.values_mut()) {
        if let Some(def) = def.as_object_mut() {
            strip(def);
        }
    }
    Value::Object(root)
}

/// makes an edit to the project like the app does (change the graph, then record the difference)
fn record(history: &mut History, project: &mut Project, step: Step, edit: impl FnOnce(&mut Project)) -> bool {
    let before = project.clone();
    edit(project);
    history.record(Diff::between(&before, project), step)
}

/// small deterministic random numbers, so a failing seed can be run again
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        (!items.is_empty()).then(|| &items[self.below(items.len())])
    }
}

/// one random edit made through `graph::edit` (or the way the UI changes groups), `None` if it didn't apply
fn random_edit(rng: &mut Rng, specs: &[NodeSpec], project: &Project) -> Option<(String, Project)> {
    let mut graph = Graph::from_rf(project).unwrap();
    let groups = all_groups(&graph);

    // most edits are at the top level, some inside one of the project's own groups
    let local: Vec<String> = graph.groups.keys().cloned().collect();
    let scope = if rng.below(3) == 0 {
        rng.pick(&local).cloned()
    } else {
        None
    };
    let specs = Specs {
        specs,
        groups: &groups,
        scope: scope.as_ref().map(|id| &groups[id]),
    };
    let mut target = match &scope {
        Some(id) => graph.groups[id].graph.clone(),
        None => graph.clone(),
    };
    let ids: Vec<String> = target.nodes.iter().map(|n| n.id.clone()).collect();

    let op = match rng.below(9) {
        0 => {
            let spec = rng.pick(specs.specs)?;
            edit::add_node(&mut target, &specs, &spec.id, None, None, None).ok()?;
            "add_node"
        }
        1 | 2 => {
            // a random output to a random input, most of these don't type check and are skipped
            let from = rng.pick(&ids)?.clone();
            let to = rng.pick(&ids)?.clone();
            let from_spec = specs.for_node(target.node(&from)?)?;
            let to_spec = specs.for_node(target.node(&to)?)?;
            let output = rng.pick(&from_spec.handles.outputs)?.id.clone();
            let input = rng.pick(&to_spec.handles.inputs)?.id.clone();
            edit::connect(&mut target, &specs, &HashMap::new(), &from, &output, &to, &input).ok()?;
            "connect"
        }
        3 => {
            let edge = rng.pick(&target.edges)?.clone();
            edit::disconnect(&mut target, edge.to_node(), edge.to_input()).ok()?;
            "disconnect"
        }
        4 => {
            let id = rng.pick(&ids)?.clone();
            let spec = specs.for_node(target.node(&id)?)?;
            let input = rng.pick(&spec.handles.inputs)?;
            let value = match input.data_type.as_str() {
                "f64" => json!(rng.below(100) as f64),
                "String" => json!(format!("value {}", rng.below(100))),
                _ => return None,
            };
            edit::set_inputs(&mut target, &specs, &id, &Map::from_iter([(input.id.clone(), value)])).ok()?;
            "set_inputs"
        }
        5 => {
            let id = rng.pick(&ids)?.clone();
            edit::remove_node(&mut target, &id).ok()?;
            "remove_node"
        }
        6 => {
            let id = rng.pick(&ids)?.clone();
            target.node_mut(&id)?.position = Position {
                x: rng.below(1000) as f64,
                y: rng.below(1000) as f64,
            };
            "move"
        }
        // make a built-in group the project's own, or go back to the built-in
        7 => {
            let builtin: Vec<&String> = builtin_groups().keys().collect();
            let id = (*rng.pick(&builtin)?).clone();
            match graph.groups.remove(&id) {
                Some(_) => "revert_group",
                None => {
                    graph.groups.insert(id.clone(), builtin_groups()[&id].clone());
                    "make_local"
                }
            }
        }
        _ => {
            let id = rng.pick(&local)?.clone();
            graph.groups.get_mut(&id)?.name = format!("Group {}", rng.below(100));
            "rename_group"
        }
    };

    // node edits were made on a copy of the graph they're in
    if !op.ends_with("group") && op != "make_local" {
        match &scope {
            Some(id) => graph.groups.get_mut(id)?.graph = target,
            None => graph = target,
        }
    }
    let op = if scope.is_some() && !op.ends_with("group") && op != "make_local" {
        format!("{} in group", op)
    } else {
        op.to_string()
    };
    Some((op, graph.to_rf()))
}

// random edits undo back to the start one step at a time, then redo to the end, matching the project at every step
#[test]
fn undo_redo_random_edits() {
    let specs = specs();
    let mut ops: HashMap<String, usize> = HashMap::new();
    for seed in 1..=30u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let mut project = fixture();
        let mut history = History::new(1000);
        let mut states = vec![key(&project)];

        // only edits that changed something are steps
        for _ in 0..80 {
            let Some((op, edited)) = random_edit(&mut rng, &specs, &project) else {
                continue;
            };
            if record(&mut history, &mut project, Step::new(&op, Source::Mcp), |p| *p = edited) {
                states.push(key(&project));
                *ops.entry(op).or_default() += 1;
            }
        }
        assert!(states.len() > 10, "seed {} made only {} steps", seed, states.len() - 1);

        for (i, state) in states.iter().enumerate().rev().skip(1) {
            assert!(history.undo(&mut project).is_some());
            assert_eq!(&key(&project), state, "seed {}: undo to step {}", seed, i);
        }
        assert!(history.undo(&mut project).is_none());

        for (i, state) in states.iter().enumerate().skip(1) {
            assert!(history.redo(&mut project).is_some());
            assert_eq!(&key(&project), state, "seed {}: redo to step {}", seed, i);
        }
        assert!(history.redo(&mut project).is_none());
    }

    // every kind of edit came up, at the top level and inside groups
    for op in ["add_node", "connect", "disconnect", "set_inputs", "remove_node", "move", "make_local", "revert_group", "rename_group", "add_node in group", "remove_node in group", "move in group"] {
        assert!(ops.get(op).is_some_and(|n| *n > 0), "no {} steps: {:?}", op, ops);
    }
}

// a new edit after undoing drops what could be redone
#[test]
fn new_edit_drops_redo() {
    let mut project = fixture();
    let mut history = History::default();
    let move_viewer = |x: f64| move |p: &mut Project| p.get_mut("nodes").unwrap()[0]["position"]["x"] = json!(x);

    record(&mut history, &mut project, Step::new("move", Source::Ui), move_viewer(1.0));
    record(&mut history, &mut project, Step::new("move", Source::Ui), move_viewer(2.0));
    history.undo(&mut project);
    assert_eq!(history.info().entries.len(), 2);
    assert_eq!(history.info().current, 1);

    record(&mut history, &mut project, Step::new("move", Source::Ui), move_viewer(3.0));
    assert_eq!(history.info().entries.len(), 2);
    assert!(history.redo(&mut project).is_none());
}

// the viewport and UI only fields aren't steps, and putting a record back keeps the UI fields it has now
#[test]
fn ui_fields_are_not_recorded() {
    let mut project = fixture();
    let mut history = History::default();

    // panning and selecting change nothing that's recorded
    let recorded = record(&mut history, &mut project, Step::new("edit", Source::Ui), |p| {
        p.insert("viewport".to_string(), json!({ "x": 50, "y": 20, "zoom": 2 }));
        p.get_mut("nodes").unwrap()[0]["selected"] = json!(true);
    });
    assert!(!recorded);

    // move a node, then the UI measures it again, undo keeps the new size
    record(&mut history, &mut project, Step::new("move", Source::Ui), |p| p.get_mut("nodes").unwrap()[0]["position"]["x"] = json!(-500.0));
    project.get_mut("nodes").unwrap()[0]["measured"] = json!({ "width": 1, "height": 2 });
    history.undo(&mut project);
    let viewer = &project["nodes"][0];
    assert_ne!(viewer["position"]["x"], json!(-500.0));
    assert_eq!(viewer["measured"], json!({ "width": 1, "height": 2 }));
    assert_eq!(project["viewport"], json!({ "x": 50, "y": 20, "zoom": 2 }));
}

// changes in one transaction are one step until it ends, a cancelled one is undone and forgotten
#[test]
fn transactions() {
    let mut project = fixture();
    let start = key(&project);
    let mut history = History::default();
    let set_x = |i: usize, x: f64| move |p: &mut Project| p.get_mut("nodes").unwrap()[i]["position"]["x"] = json!(x);

    // a duplicate then a grab, both in one transaction
    record(&mut history, &mut project, Step::new("duplicate", Source::Ui).txn("t1"), set_x(0, 10.0));
    record(&mut history, &mut project, Step::new("move", Source::Ui).txn("t1"), set_x(1, 20.0));
    history.end("t1");
    assert_eq!(history.info().entries.len(), 1);
    assert_eq!(history.info().entries[0].op, "duplicate");

    // the next change is a step of its own
    record(&mut history, &mut project, Step::new("move", Source::Ui).txn("t1"), set_x(2, 30.0));
    assert_eq!(history.info().entries.len(), 2);
    history.end("t1");

    // a cancelled transaction leaves nothing behind
    let before_cancel = key(&project);
    record(&mut history, &mut project, Step::new("duplicate", Source::Ui).txn("t2"), set_x(3, 40.0));
    record(&mut history, &mut project, Step::new("move", Source::Ui).txn("t2"), set_x(4, 50.0));
    assert!(history.cancel("t2", &mut project));
    assert_eq!(key(&project), before_cancel);
    assert_eq!(history.info().entries.len(), 2);

    // an edit from somewhere else ends an open transaction
    record(&mut history, &mut project, Step::new("move", Source::Ui).txn("t3"), set_x(3, 60.0));
    record(&mut history, &mut project, Step::new("add_node", Source::Mcp), set_x(4, 70.0));
    record(&mut history, &mut project, Step::new("move", Source::Ui).txn("t3"), set_x(5, 80.0));
    assert_eq!(history.info().entries.len(), 5);
    assert!(!history.cancel("t3", &mut project) || history.info().entries.len() == 4);

    while history.undo(&mut project).is_some() {}
    assert_eq!(key(&project), start);
}

// changes with the same merge key join the newest step, and drop out when they end where they started
#[test]
fn merge_keys() {
    let mut project = fixture();
    let mut history = History::default();
    let name = |s: &'static str| move |p: &mut Project| p.get_mut("nodes").unwrap()[0]["data"]["inputs"]["name"] = json!(s);

    record(&mut history, &mut project, Step::new("set_inputs", Source::Ui).merge("viewer-1.name"), name("a"));
    record(&mut history, &mut project, Step::new("set_inputs", Source::Ui).merge("viewer-1.name"), name("ab"));
    record(&mut history, &mut project, Step::new("set_inputs", Source::Ui).merge("viewer-1.name"), name("abc"));
    assert_eq!(history.info().entries.len(), 1);
    history.undo(&mut project);
    assert!(project["nodes"][0]["data"]["inputs"].get("name").is_none());

    // typed and deleted again, nothing changed
    let data = project["nodes"][0]["data"].clone();
    let mut history = History::default();
    record(&mut history, &mut project, Step::new("set_inputs", Source::Ui).merge("k"), name("x"));
    record(&mut history, &mut project, Step::new("set_inputs", Source::Ui).merge("k"), |p| p.get_mut("nodes").unwrap()[0]["data"] = data);
    assert_eq!(history.info().entries.len(), 0);
}

// only the newest `limit` steps are kept
#[test]
fn limit() {
    let mut project = fixture();
    let mut history = History::new(3);
    for x in 0..5 {
        record(&mut history, &mut project, Step::new("move", Source::Ui), |p| p.get_mut("nodes").unwrap()[0]["position"]["x"] = json!(x as f64 * 100.0));
    }
    assert_eq!(history.info().entries.len(), 3);
    while history.undo(&mut project).is_some() {}
    assert_eq!(project["nodes"][0]["position"]["x"], json!(100.0));
}

// a removed group comes back with its nodes and edges in their order, and goes again on redo
#[test]
fn group_removal() {
    let mut project = fixture();
    let def = serde_json::to_value(&builtin_groups().values().next().unwrap()).unwrap();
    project.insert("groups".to_string(), json!({ "g": def }));
    let with_group = key(&project);
    let mut history = History::default();

    record(&mut history, &mut project, Step::new("revert_group", Source::Ui), |p| {
        p.remove("groups");
    });
    let without_group = key(&project);
    history.undo(&mut project);
    assert_eq!(key(&project), with_group);
    history.redo(&mut project);
    assert_eq!(key(&project).get("groups").map_or(0, |g| g.as_object().unwrap().len()), 0);
    assert_eq!(key(&project)["nodes"], without_group["nodes"]);
}
