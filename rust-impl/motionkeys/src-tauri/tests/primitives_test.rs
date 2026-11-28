// run from /src-tauri
// cargo test --test primitives_test

use motionkeys::graph::executors::io::Inputs;
use motionkeys::graph::executors::primitives::{boolean, boolean_math, compare, map_range, math, string, switch, value, BOOLEAN_OPERATIONS, COMPARE_OPERATIONS, MATH_OPERATIONS};
use serde_json::{json, Value};

// MARK: - Input

#[test]
fn inputs_give_what_was_set() {
    assert_eq!(value(&Inputs::from([("value", json!(2.5))])).unwrap().to_json()["result"], json!(2.5));
    assert_eq!(boolean(&Inputs::from([("boolean", json!(true))])).unwrap().to_json()["result"], json!(true));
    assert_eq!(string(&Inputs::from([("string", json!("Drum"))])).unwrap().to_json()["result"], json!("Drum"));
    // an unset string is empty
    assert_eq!(string(&Inputs::default()).unwrap().to_json()["result"], json!(""));
}

// MARK: - Math

fn math_of(operation: &str, a: f64, b: f64, clamp: bool) -> Result<f64, String> {
    let outputs = math(&Inputs::from([("operation", json!(operation)), ("clamp", json!(clamp)), ("a", json!(a)), ("b", json!(b))]))?;
    Ok(outputs.to_json()["result"].as_f64().unwrap())
}

#[test]
fn math_operations() {
    let cases = [
        ("add", 2.0, 3.0, 5.0),
        ("subtract", 2.0, 3.0, -1.0),
        ("multiply", 2.0, 3.0, 6.0),
        ("divide", 3.0, 2.0, 1.5),
        ("power", 2.0, 3.0, 8.0),
        ("minimum", 2.0, 3.0, 2.0),
        ("maximum", 2.0, 3.0, 3.0),
        // truncated keeps a's sign, floored keeps b's (python's %)
        ("modulo", -7.0, 3.0, -1.0),
        ("floored_modulo", -7.0, 3.0, 2.0),
        ("absolute", -2.0, 0.0, 2.0),
        // halves up like Blender, not python's round half to even
        ("round", 2.5, 0.0, 3.0),
        ("round", -2.5, 0.0, -2.0),
        ("floor", 2.7, 0.0, 2.0),
        ("ceil", 2.2, 0.0, 3.0),
        ("sine", std::f64::consts::FRAC_PI_2, 0.0, 1.0),
        ("cosine", 0.0, 0.0, 1.0),
    ];
    for (operation, a, b, expected) in cases {
        let result = math_of(operation, a, b, false).unwrap();
        assert!((result - expected).abs() < 1e-12, "{}({}, {}) = {}, not {}", operation, a, b, result, expected);
    }
    // every operation has a case
    for (id, _) in MATH_OPERATIONS {
        assert!(cases.iter().any(|c| c.0 == *id), "no case for {}", id);
    }
}

#[test]
fn math_clamps_between_0_and_1() {
    assert_eq!(math_of("add", 2.0, 3.0, true).unwrap(), 1.0);
    assert_eq!(math_of("subtract", 2.0, 3.0, true).unwrap(), 0.0);
    assert_eq!(math_of("multiply", 0.5, 0.5, true).unwrap(), 0.25);
}

#[test]
fn math_errors() {
    assert_eq!(math_of("divide", 1.0, 0.0, false).unwrap_err(), "division by zero");
    assert!(math_of("power", -8.0, 0.5, false).is_err());
    assert_eq!(math_of("cube", 1.0, 1.0, false).unwrap_err(), "unknown operation 'cube'");
    // a number left out is an error, the spec's default fills it in a run
    assert!(math(&Inputs::from([("operation", json!("add")), ("clamp", json!(false)), ("a", json!(1.0))])).is_err());
}

// MARK: - Map Range

fn map_of(value: f64, from: (f64, f64), to: (f64, f64), clamp: bool) -> f64 {
    let inputs = Inputs::from([("clamp", json!(clamp)), ("value", json!(value)), ("from_min", json!(from.0)), ("from_max", json!(from.1)), ("to_min", json!(to.0)), ("to_max", json!(to.1))]);
    map_range(&inputs).unwrap().to_json()["result"].as_f64().unwrap()
}

#[test]
fn map_range_remaps() {
    // velocity onto 0 to 1
    assert_eq!(map_of(63.5, (0.0, 127.0), (0.0, 1.0), true), 0.5);
    // a reversed to range
    assert_eq!(map_of(0.25, (0.0, 1.0), (10.0, 0.0), true), 7.5);
    // outside the range: clamped, or carried on
    assert_eq!(map_of(2.0, (0.0, 1.0), (0.0, 10.0), true), 10.0);
    assert_eq!(map_of(2.0, (0.0, 1.0), (0.0, 10.0), false), 20.0);
    assert_eq!(map_of(2.0, (0.0, 1.0), (10.0, 0.0), true), 0.0);
    // an empty from range gives to min, like Blender
    assert_eq!(map_of(5.0, (1.0, 1.0), (3.0, 4.0), false), 3.0);
}

// MARK: - Compare

fn compare_of(operation: &str, a: f64, b: f64) -> bool {
    let inputs = Inputs::from([("operation", json!(operation)), ("a", json!(a)), ("b", json!(b)), ("epsilon", json!(0.001))]);
    compare(&inputs).unwrap().to_json()["result"].as_bool().unwrap()
}

#[test]
fn compare_operations() {
    let cases = [("less_than", [true, false, false]), ("less_equal", [true, true, false]), ("greater_than", [false, false, true]), ("greater_equal", [false, true, true]), ("equal", [false, true, false]), ("not_equal", [true, false, true])];
    for (operation, expected) in cases {
        // a below, at and above b
        let got = [compare_of(operation, 1.0, 2.0), compare_of(operation, 2.0, 2.0), compare_of(operation, 3.0, 2.0)];
        assert_eq!(got, expected, "{}", operation);
    }
    for (id, _) in COMPARE_OPERATIONS {
        assert!(cases.iter().any(|c| c.0 == *id), "no case for {}", id);
    }
    // equal is within epsilon
    assert!(compare_of("equal", 1.0, 1.0005));
    assert!(!compare_of("equal", 1.0, 1.002));
}

// MARK: - Boolean Math

#[test]
fn boolean_math_operations() {
    // results for (a, b) = (off, off), (off, on), (on, off), (on, on)
    let cases = [("and", [false, false, false, true]), ("or", [false, true, true, true]), ("not", [true, true, false, false]), ("nand", [true, true, true, false]), ("nor", [true, false, false, false]), ("equal", [true, false, false, true]), ("not_equal", [false, true, true, false])];
    for (operation, expected) in cases {
        let got = [(false, false), (false, true), (true, false), (true, true)].map(|(a, b)| boolean_math(&Inputs::from([("operation", json!(operation)), ("a", json!(a)), ("b", json!(b))])).unwrap().to_json()["result"].as_bool().unwrap());
        assert_eq!(got, expected, "{}", operation);
    }
    for (id, _) in BOOLEAN_OPERATIONS {
        assert!(cases.iter().any(|c| c.0 == *id), "no case for {}", id);
    }
}

// MARK: - Switch

#[test]
fn switch_passes_on_the_picked_value() {
    let picked = |on: bool| -> Value { switch(&Inputs::from([("switch", json!(on)), ("false", json!({ "name": "off" })), ("true", json!([1, 2]))])).unwrap().to_json()["output"].clone() };
    assert_eq!(picked(false), json!({ "name": "off" }));
    assert_eq!(picked(true), json!([1, 2]));
    // nothing connected to the picked side, no output
    let outputs = switch(&Inputs::from([("switch", json!(true)), ("false", json!(1))])).unwrap().to_json();
    assert_eq!(outputs.get("output"), None);
}
