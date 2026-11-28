// the primitive nodes, like Blender's: values typed in once (Value, Boolean, String) and the utilities that work on them
// (Math, Map Range, Compare, Boolean Math, Switch). every operation is an expression run on the mappers' engine, so
// the math is the same math everywhere

use super::io::{Inputs, NodeResult, Outputs};
use crate::utils::expr::{self, Env, Program, Value};

// compiles one of the operations below for the variables `vars`
fn program(text: &str, vars: &[&str]) -> Result<Program, String> {
    expr::compile(text, &Env::new(vars)).map_err(|e| e.to_string())
}

// the expression for the operation picked on a node, from its list of (id, expression)
fn operation<'a>(operations: &'a [(&str, &str)], picked: &str) -> Result<&'a str, String> {
    operations.iter().find(|(id, _)| *id == picked).map(|(_, text)| *text).ok_or_else(|| format!("unknown operation '{}'", picked))
}

// MARK: - Input

/// Node: value
///
/// inputs:
/// "value": `f64`, typed in on the node
///
/// outputs:
/// "result": `f64`
#[node_registry::node]
pub fn value(value: f64) -> NodeResult {
    let mut outputs = Outputs::new();
    outputs.set("result", value);
    Ok(outputs)
}

/// Node: boolean
///
/// inputs:
/// "boolean": `bool`, set on the node
///
/// outputs:
/// "result": `bool`
#[node_registry::node]
pub fn boolean(boolean: bool) -> NodeResult {
    let mut outputs = Outputs::new();
    outputs.set("result", boolean);
    Ok(outputs)
}

/// Node: string
///
/// inputs:
/// "string": `String`, typed in on the node
///
/// outputs:
/// "result": `String`
#[node_registry::node]
pub fn string(string: Option<&String>) -> NodeResult {
    let mut outputs = Outputs::new();
    outputs.set("result", string.cloned().unwrap_or_default());
    Ok(outputs)
}

// MARK: - Utilities

/// the Math node's operations in a and b, same order and ids as mathOperations in src/nodes/math.tsx. round is
/// Blender's (halves up), modulo is Blender's truncated one (A's sign), floored_modulo is python's %
pub const MATH_OPERATIONS: &[(&str, &str)] = &[("add", "a + b"), ("subtract", "a - b"), ("multiply", "a * b"), ("divide", "a / b"), ("power", "a ** b"), ("minimum", "min(a, b)"), ("maximum", "max(a, b)"), ("modulo", "a - b * int(a / b)"), ("floored_modulo", "a % b"), ("absolute", "abs(a)"), ("round", "floor(a + 0.5)"), ("floor", "floor(a)"), ("ceil", "ceil(a)"), ("sine", "sin(a)"), ("cosine", "cos(a)")];

/// Node: math
///
/// inputs:
/// "operation": `String`, an id from MATH_OPERATIONS
/// "clamp": `bool`, clamps the result between 0 and 1
/// "a": `f64`
/// "b": `f64`, unused by the operations on a alone
///
/// outputs:
/// "result": `f64`
#[node_registry::node]
pub fn math(operation: &String, clamp: bool, a: f64, b: f64) -> NodeResult {
    let text = self::operation(MATH_OPERATIONS, operation)?;
    let text = if clamp {
        format!("clamp({}, 0, 1)", text)
    } else {
        text.to_string()
    };
    let result = program(&text, &["a", "b"])?.eval_number(&[a.into(), b.into()]).map_err(|e| e.to_string())?;

    let mut outputs = Outputs::new();
    outputs.set("result", result);
    Ok(outputs)
}

/// Node: map_range
///
/// inputs:
/// "clamp": `bool`, keeps the result inside the to range
/// "value": `f64`
/// "from_min", "from_max": `f64`, the range value is in. equal gives to_min, like Blender
/// "to_min", "to_max": `f64`, the range it's mapped onto
///
/// outputs:
/// "result": `f64`
#[node_registry::node]
pub fn map_range(clamp: bool, value: f64, from_min: f64, from_max: f64, to_min: f64, to_max: f64) -> NodeResult {
    let mapped = "(to_min if from_max == from_min else lerp(to_min, to_max, (value - from_min) / (from_max - from_min)))";
    let text = if clamp {
        format!("clamp({}, min(to_min, to_max), max(to_min, to_max))", mapped)
    } else {
        mapped.to_string()
    };
    let vars = ["value", "from_min", "from_max", "to_min", "to_max"];
    let result = program(&text, &vars)?.eval_number(&[value.into(), from_min.into(), from_max.into(), to_min.into(), to_max.into()]).map_err(|e| e.to_string())?;

    let mut outputs = Outputs::new();
    outputs.set("result", result);
    Ok(outputs)
}

/// the Compare node's comparisons of a and b, same order and ids as compareOperations in src/nodes/compare.tsx. equal
/// is within epsilon, like Blender
pub const COMPARE_OPERATIONS: &[(&str, &str)] = &[("less_than", "a < b"), ("less_equal", "a <= b"), ("greater_than", "a > b"), ("greater_equal", "a >= b"), ("equal", "abs(a - b) <= epsilon"), ("not_equal", "abs(a - b) > epsilon")];

/// Node: compare
///
/// inputs:
/// "operation": `String`, an id from COMPARE_OPERATIONS
/// "a", "b": `f64`
/// "epsilon": `f64`, how far apart a and b can be and still be equal
///
/// outputs:
/// "result": `bool`
#[node_registry::node]
pub fn compare(operation: &String, a: f64, b: f64, epsilon: f64) -> NodeResult {
    let text = self::operation(COMPARE_OPERATIONS, operation)?;
    let result = program(text, &["a", "b", "epsilon"])?.eval(&[a.into(), b.into(), epsilon.into()]).map_err(|e| e.to_string())?;

    let mut outputs = Outputs::new();
    outputs.set("result", result.truthy());
    Ok(outputs)
}

/// the Boolean Math node's operations on a and b, same order and ids as booleanOperations in src/nodes/boolean_math.tsx
pub const BOOLEAN_OPERATIONS: &[(&str, &str)] = &[("and", "a and b"), ("or", "a or b"), ("not", "not a"), ("nand", "not (a and b)"), ("nor", "not (a or b)"), ("equal", "a == b"), ("not_equal", "a != b")];

/// Node: boolean_math
///
/// inputs:
/// "operation": `String`, an id from BOOLEAN_OPERATIONS
/// "a": `bool`
/// "b": `bool`, unused by not
///
/// outputs:
/// "result": `bool`
#[node_registry::node]
pub fn boolean_math(operation: &String, a: bool, b: bool) -> NodeResult {
    let text = self::operation(BOOLEAN_OPERATIONS, operation)?;
    let result = program(text, &["a", "b"])?.eval(&[Value::from(a), Value::from(b)]).map_err(|e| e.to_string())?;

    let mut outputs = Outputs::new();
    outputs.set("result", result.truthy());
    Ok(outputs)
}

/// Node: switch
///
/// inputs:
/// "switch": `bool`, picks "true" when on, "false" when off
/// "false", "true": `Any`
///
/// outputs:
/// "output": `Any`, the picked input as it came in, unset when it isn't connected
#[node_registry::node]
pub fn switch(switch: bool, inputs: &Inputs) -> NodeResult {
    let mut outputs = Outputs::new();
    if let Some(picked) = inputs.val(if switch {
        "true"
    } else {
        "false"
    }) {
        outputs.set_val("output", picked.clone());
    }
    Ok(outputs)
}
