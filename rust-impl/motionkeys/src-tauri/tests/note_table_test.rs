// run from /src-tauri
// cargo test --test note_table_test
//
// the Note Table node (src/utils/note_table.rs) and the partial output that feeds it: Assign Notes to Objects' Note Map
// doesn't read the Animation Generator, so the table can feed the generator Assign uses without a cycle

use std::collections::{BTreeMap, HashMap};

use serde_json::{json, Map, Value};

use motionkeys::graph::executors::io::NodeFunction;
use motionkeys::graph::model::{node_specs, Graph, Position, RfEdge, RfNode};
use motionkeys::graph::run::{run, RunCtx};
use motionkeys::node_registry::get_node_registry;
use motionkeys::utils::expr::{compile, Env};
use motionkeys::utils::mapper::{NoteVars, DEFAULT_FPS, MAPPER_VARS};
use motionkeys::utils::note_table::{applied, expression, fallback, preview, Fallback, NoteTable, Operation, Override, Source, Target};

// MARK: - Helpers

fn node(id: &str, inputs: Value) -> RfNode {
    let mut data = Map::new();
    data.insert("inputs".to_string(), inputs);
    RfNode {
        id: id.to_string(),
        node_type: id.rsplit_once('-').unwrap().0.to_string(),
        position: Position::default(),
        data,
        extra: Map::new(),
    }
}

// (from_node, from_output, to_node, to_input) links
fn graph(nodes: Vec<RfNode>, links: &[(&str, &str, &str, &str)]) -> Graph {
    Graph {
        nodes,
        edges: links.iter().map(|(a, b, c, d)| RfEdge::new(a, b, c, d)).collect(),
        ..Default::default()
    }
}

fn cubes(count: usize) -> Value {
    let objects: Vec<_> = (1..=count).map(|i| json!({ "name": format!("Cube.{:03}", i), "position": {"x": 0.0, "y": 0.0, "z": 0.0}, "rotation": {"x": 0.0, "y": 0.0, "z": 0.0}, "scale": {"x": 1.0, "y": 1.0, "z": 1.0}, "blend_shapes": { "keys": [], "reference": null }, "anim_curves": [] })).collect();
    json!([{ "name": "Cubes", "objects": objects }])
}

fn key(time: f64, value: f64) -> Value {
    json!({ "amplitude": 0.0, "back": 0.0, "easing": "AUTO", "interpolation": "LINEAR", "period": 0.0, "handle_left": [time, value], "handle_left_type": "AUTO_CLAMPED", "handle_right": [time, value], "handle_right_type": "AUTO_CLAMPED", "co": [time, value] })
}

fn run_graph(root: &Graph) -> (HashMap<String, Value>, Option<String>) {
    let data = std::fs::read_to_string("src/configs/default_nodes.json").unwrap();
    let specs = node_specs(&serde_json::from_str(&data).unwrap());
    let registry: HashMap<String, NodeFunction> = get_node_registry();
    let groups = BTreeMap::new();
    let ctx = RunCtx::new(&specs, &registry, &groups, true);
    let (record, error) = run(&ctx, root);
    (record.results.into_iter().collect(), error)
}

// a table on notes 58-64 from a map of seven cubes, or the given notes
fn map(notes: &[(&str, &[u8])]) -> BTreeMap<String, Vec<u8>> {
    notes.iter().map(|(o, n)| (o.to_string(), n.to_vec())).collect()
}

fn cube_map() -> BTreeMap<String, Vec<u8>> {
    (0..7).map(|i| (format!("Cube.{:03}", i + 1), vec![58 + i as u8])).collect()
}

fn text(table: &NoteTable, note_map: &BTreeMap<String, Vec<u8>>) -> String {
    expression(table, &preview(table, note_map).unwrap())
}

// MARK: - The loop

// generator -> assign -> (note map) -> table -> generator's amplitude mapper
fn looped() -> Graph {
    let generator = node("animation_generator-1", json!({ "name": "hit", "note_on_keyframes": [{ "array_index": 2, "auto_smoothing": "NONE", "data_path": "location", "extrapolation": "CONSTANT", "keyframe_points": [key(0.0, 0.0), key(0.1, 1.0)], "range": [0.0, 0.1] }] }));
    let assign = node("assign_notes_to_objects-1", json!({ "object_groups": cubes(7), "object_group_name": "Cubes", "note_numbers": "58 59 60 61 62 63 64" }));
    let table = node("note_table-1", json!({ "table": { "shape": "linear", "curve": "t", "min": 0, "max": 1 } }));
    graph(vec![generator, assign, table], &[("animation_generator-1", "generator", "assign_notes_to_objects-1", "generator"), ("assign_notes_to_objects-1", "object_notes", "note_table-1", "note_map")])
}

#[test]
fn a_note_map_can_feed_the_generator_it_comes_from() {
    let g = looped();
    // the table into the generator's mapper isn't a cycle: the note map doesn't read the generator
    assert!(!g.would_cycle("note_table-1", "expression", "animation_generator-1", "amplitude_mapper"));
    // the object map does read it
    assert!(g.would_cycle("assign_notes_to_objects-1", "object_map", "animation_generator-1", "name"));
    // the note map into an input it does read is a cycle
    let mut g2 = g.clone();
    g2.edges.push(RfEdge::new("note_table-1", "expression", "assign_notes_to_objects-1", "object_group_name"));
    assert!(g2.would_cycle("note_table-1", "expression", "assign_notes_to_objects-1", "note_numbers"));
}

#[test]
fn the_loop_runs_and_the_generator_gets_the_table() {
    let mut g = looped();
    g.edges.push(RfEdge::new("note_table-1", "expression", "animation_generator-1", "amplitude_mapper"));
    let (results, error) = run_graph(&g);
    assert_eq!(error, None);

    // the table has a row per mapped note, with its object, spaced evenly from 0 to 1
    let preview = &results["note_table-1"]["preview"];
    assert_eq!(preview["rows"].as_array().unwrap().len(), 7);
    assert_eq!(preview["rows"][0]["objects"], json!(["Cube.001"]));
    assert_eq!(preview["rows"][6]["value"], json!(1.0));
    let expression = results["note_table-1"]["expression"].as_str().unwrap();
    assert_eq!(expression, "y * ((note - 58) / 6) if 58 <= note <= 64 else y");

    // the generator Assign gives the objects has the table's expression as its amplitude mapper
    assert_eq!(results["animation_generator-1"]["generator"]["amplitude_mapper"], json!(expression));
    assert_eq!(results["assign_notes_to_objects-1"]["object_map"]["animations"]["hit"]["amplitude_mapper"], json!(expression));
    assert_eq!(results["assign_notes_to_objects-1"]["object_notes"]["Cube.007"], json!([64]));
}

#[test]
fn a_typed_mapper_still_works() {
    let mut g = looped();
    g.nodes[0].data.get_mut("inputs").unwrap()["time_mapper"] = json!("x * 2");
    let (results, error) = run_graph(&g);
    assert_eq!(error, None);
    assert_eq!(results["animation_generator-1"]["generator"]["time_mapper"], json!("x * 2"));
}

// MARK: - Expressions

#[test]
fn even_spacing_over_scattered_notes() {
    // a drum kit's notes are far apart, the curve still steps evenly across them
    let drums = map(&[("Kick", &[36]), ("Snare", &[38]), ("Hi-Hat", &[42]), ("Open Hat", &[46]), ("Crash", &[49]), ("Ride", &[51])]);
    let table = NoteTable::default();
    let p = preview(&table, &drums).unwrap();
    let values: Vec<f64> = p.rows.iter().map(|r| r.value).collect();
    assert_eq!(values, vec![0.0, 0.2, 0.4, 0.6, 0.8, 1.0]);
    // the ride's 1 leaves y alone, like every note off the table
    assert_eq!(text(&table, &drums), "y * [0, 1, 0.2, 1, 1, 1, 0.4, 1, 1, 1, 0.6, 1, 1, 0.8][note - 36] if 36 <= note <= 49 else y");
}

#[test]
fn notes_on_several_objects_are_one_row() {
    let shared = map(&[("Light.L", &[60, 64]), ("Light.R", &[60, 67]), ("Strobe", &[72])]);
    let p = preview(&NoteTable::default(), &shared).unwrap();
    assert_eq!(p.rows.iter().map(|r| r.note).collect::<Vec<_>>(), vec![60, 64, 67, 72]);
    assert_eq!(p.rows[0].objects, vec!["Light.L", "Light.R"]);
}

#[test]
fn curves_over_neighbouring_notes_stay_formulas() {
    // ease in over the cubes, written as the curve itself
    let table = NoteTable {
        shape: "ease_in".into(),
        curve: "t ** 2".into(),
        ..Default::default()
    };
    assert_eq!(text(&table, &cube_map()), "y * (((note - 58) / 6) ** 2) if 58 <= note <= 64 else y");
    // a scaled log curve from 0.5 to 2
    let table = NoteTable {
        shape: "logarithmic".into(),
        curve: "log(1 + 9 * t, 10)".into(),
        min: 0.5,
        max: 2.0,
        ..Default::default()
    };
    assert_eq!(text(&table, &cube_map()), "y * (0.5 + 1.5 * (log(1 + 9 * ((note - 58) / 6), 10))) if 58 <= note <= 64 else y");
}

#[test]
fn operations_and_fallbacks() {
    let one = map(&[("Crash", &[46])]);
    let constant = |operation, target, fallback, min| NoteTable {
        shape: "constant".into(),
        operation,
        target,
        fallback,
        min,
        ..Default::default()
    };
    // the cymbal choke
    assert_eq!(text(&constant(Operation::Multiply, Target::Amplitude, Fallback::Zero, 1.0), &one), "y if note == 46 else 0");
    // time: stretch, shift, or set
    assert_eq!(text(&constant(Operation::Multiply, Target::Time, Fallback::Keep, 2.0), &one), "x * 2 if note == 46 else x");
    assert_eq!(text(&constant(Operation::Add, Target::Time, Fallback::Keep, -6.0), &one), "x - 6 if note == 46 else x");
    assert_eq!(text(&constant(Operation::Set, Target::Amplitude, Fallback::Keep, 0.5), &one), "0.5 if note == 46 else y");
    // a set lookup leaves the notes between alone
    let scattered = map(&[("A", &[40]), ("B", &[42]), ("C", &[43]), ("D", &[45])]);
    let set = NoteTable {
        operation: Operation::Set,
        overrides: vec![Override {
            note: 42,
            value: 9.0,
        }],
        ..Default::default()
    };
    assert_eq!(text(&set, &scattered), "[0, y, 9, 0.666666667, y, 1][note - 40] if 40 <= note <= 45 else y");
}

#[test]
fn a_typed_value_wins_over_the_curve() {
    let table = NoteTable {
        overrides: vec![Override {
            note: 61,
            value: 0.9,
        }],
        ..Default::default()
    };
    let p = preview(&table, &cube_map()).unwrap();
    let row = p.rows.iter().find(|r| r.note == 61).unwrap();
    assert!(row.overridden);
    assert_eq!(row.value, 0.9);
    assert!(text(&table, &cube_map()).contains("0.9"));
}

#[test]
fn range_mode_has_every_note() {
    let table = NoteTable {
        source: Source::Range,
        from: 40,
        to: 50,
        ..Default::default()
    };
    assert_eq!(text(&table, &BTreeMap::new()), "y * ((note - 40) * 0.1) if 40 <= note <= 50 else y");
}

#[test]
fn curve_errors_name_the_curve() {
    let bad = NoteTable {
        curve: "t ** ".into(),
        ..Default::default()
    };
    assert!(preview(&bad, &cube_map()).unwrap_err().starts_with("Curve: "));
    let unknown = NoteTable {
        curve: "tt * 2".into(),
        ..Default::default()
    };
    assert_eq!(preview(&unknown, &cube_map()).unwrap_err(), "Curve: unknown name 'tt', did you mean 't'?");
    // nothing connected: no rows, the identity
    assert_eq!(text(&NoteTable::default(), &BTreeMap::new()), "y");
}

#[test]
fn without_a_note_map_the_table_is_a_range() {
    // the source says map, nothing is connected: the node runs its range
    let table = node("note_table-1", json!({ "table": { "source": "map", "from": 40, "to": 50 } }));
    let (results, error) = run_graph(&graph(vec![table], &[]));
    assert_eq!(error, None);
    assert_eq!(results["note_table-1"]["expression"], json!("y * ((note - 40) * 0.1) if 40 <= note <= 50 else y"));
    assert_eq!(results["note_table-1"]["preview"]["rows"].as_array().unwrap().len(), 11);
}

// a small xorshift so the random tables are the same every run
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

#[test]
fn random_tables_give_what_the_table_says() {
    // every table's expression compiles and gives each note what the table says, notes off the table the fallback
    let curves = ["t", "t ** 2", "1 - (1 - t) ** 3", "sin(t * pi / 2)", "log(1 + 9 * t, 10)", "floor(t * 4) / 4", "note / 127"];
    let mut rng = Rng(0xDEADBEEFCAFEF00D);
    for round in 0..1500 {
        let notes: BTreeMap<String, Vec<u8>> = (0..rng.next() % 12).map(|i| (format!("Obj{}", i), vec![(rng.next() % 128) as u8])).collect();
        let table = NoteTable {
            target: [Target::Amplitude, Target::Time][(rng.next() % 2) as usize],
            operation: [Operation::Multiply, Operation::Add, Operation::Set][(rng.next() % 3) as usize],
            source: if round % 7 == 0 {
                Source::Range
            } else {
                Source::Map
            },
            from: (rng.next() % 100) as u8,
            to: (rng.next() % 128) as u8,
            shape: if round % 5 == 0 {
                "constant".into()
            } else {
                "x".into()
            },
            curve: curves[(rng.next() % curves.len() as u64) as usize].into(),
            min: [0.0, 0.5, -2.0, 1.0][(rng.next() % 4) as usize],
            max: [1.0, 3.0, 0.0, 12.0][(rng.next() % 4) as usize],
            overrides: if round % 3 == 0 {
                vec![Override {
                    note: (rng.next() % 128) as u8,
                    value: 0.25,
                }]
            } else {
                vec![]
            },
            fallback: [Fallback::Keep, Fallback::Zero, Fallback::Value][(rng.next() % 3) as usize],
            fallback_value: 1.5,
        };
        let p = preview(&table, &notes).unwrap();
        let text = expression(&table, &p);
        assert!(text.matches(" if ").count() <= 3, "{}", text);
        let program = compile(&text, &Env::new(&MAPPER_VARS)).unwrap_or_else(|e| panic!("{:?}: '{}' doesn't compile: {}", table, text, e));
        let values: BTreeMap<u8, f64> = p.rows.iter().map(|r| (r.note, r.value)).collect();
        for note in 0..=127u8 {
            let (x, y) = (3.0, 0.75);
            let mut vars = NoteVars::standalone(note, 100, DEFAULT_FPS);
            vars.set_key(x / DEFAULT_FPS, y, DEFAULT_FPS);
            let expected = applied(&table, x, y, values.get(&note).copied().or(fallback(&table)));
            let got = program.eval_number(&vars.values).unwrap();
            assert!((got - expected).abs() <= 1e-6 * expected.abs().max(1.0), "{:?}\n'{}' gives {} for note {}, the table gives {}", table, text, got, note, expected);
        }
    }
}
