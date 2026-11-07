// a small expression language that's a subset of python's expression syntax: anything it takes is valid python and gives
// what python 3.12+ gives (`clamp` and `lerp` are the only extras). nothing is run with an interpreter, text goes
// through a tokenizer, a recursive descent parser and an AST, then `compile` checks every name against the variables and
// functions the caller gives and the compiled `Program` is evaluated as many times as needed.
//
// nothing here panics, every problem is an `ExprError`: a kind, a short message and the characters it's about. python
// syntax that isn't supported (strings, lambda, comprehensions, ...) says "not supported: ...".
//
// grammar, lowest precedence first (python's):
//   expr        := or_test ['if' or_test 'else' expr]
//   or_test     := and_test ('or' and_test)*
//   and_test    := not_test ('and' not_test)*
//   not_test    := 'not' not_test | comparison
//   comparison  := arith (('==' | '!=' | '<' | '<=' | '>' | '>=' | 'in' | 'not' 'in') arith)*
//   arith       := term (('+' | '-') term)*
//   term        := factor (('*' | '/' | '//' | '%') factor)*
//   factor      := ('+' | '-') factor | power
//   power       := primary ['**' factor]
//   primary     := atom (call | index)*
//   call        := '(' [expr (',' expr)* [',']] ')'
//   index       := '[' expr ']'
//   atom        := number | name | 'math' '.' name | 'True' | 'False' | 'None'
//                | '(' expr ')' | '(' [expr ','] ... ')' (tuple) | '[' [expr (',' expr)* [',']] ']'
//   number      := 3 | 0.5 | .5 | 1e-3 | 1_000 | 0x1f | 0o17 | 0b101
//
// the language is meant to stay stable, tests/fixtures/expr_corpus.txt lists expressions and what they give. a change
// that breaks one of them breaks what users wrote

use std::fmt;
use std::sync::Arc;

use serde::Serialize;

/// longest text that's compiled, in characters
pub const MAX_LEN: usize = 10_000;

/// deepest nesting of parentheses, lists and operators
pub const MAX_DEPTH: usize = 50;

// MARK: - Errors

/// what went wrong, the frontend and tests go by it
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    /// the text is longer than `MAX_LEN`
    TooLong,
    /// nested deeper than `MAX_DEPTH`
    TooDeep,
    /// not valid python
    Syntax,
    /// valid python this language doesn't have
    Unsupported,
    /// a variable or function that isn't there
    UnknownName,
    /// a function given the wrong number of arguments
    Arity,
    /// a value of the wrong type, like indexing a number
    Type,
    /// dividing by zero
    ZeroDivision,
    /// a list index past its end
    Index,
    /// a value a function can't take, like sqrt(-1)
    Domain,
    /// a number too large
    Overflow,
    /// the result is NaN or infinite
    NotFinite,
    /// the result isn't a number (a list, or None)
    NotANumber,
}

/// an error with the characters it's about, `start..end` counted in characters
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExprError {
    pub kind: ErrorKind,
    pub message: String,
    pub start: usize,
    pub end: usize,
}

impl fmt::Display for ExprError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ExprError {}

/// the characters a token or a node covers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    fn new(start: usize, end: usize) -> Self {
        Self {
            start,
            end,
        }
    }

    fn to(self, other: Span) -> Span {
        Span::new(self.start.min(other.start), self.end.max(other.end))
    }
}

fn error(kind: ErrorKind, span: Span, message: impl Into<String>) -> ExprError {
    ExprError {
        kind,
        message: message.into(),
        start: span.start,
        end: span.end,
    }
}

/// an error from a function or an operator before the span of where it happened is known
#[derive(Debug, Clone, PartialEq)]
pub struct Fail {
    pub kind: ErrorKind,
    pub message: String,
}

impl Fail {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    fn at(self, span: Span) -> ExprError {
        error(self.kind, span, self.message)
    }
}

fn type_fail(message: impl Into<String>) -> Fail {
    Fail::new(ErrorKind::Type, message)
}

// MARK: - Values

/// a value while an expression runs. booleans are ints like in python (True is 1)
#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Float(f64),
    /// a list, or a tuple when `tuple` is set. they hold the same things but aren't equal to each other, like in python
    Seq(Arc<[Value]>, bool),
    /// range(start, stop, step), never materialized
    Range(i64, i64, i64),
    None,
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Value::Float(value)
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Value::Int(value)
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Value::Int(value as i64)
    }
}

impl Value {
    /// python's name for the type, for error messages
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Seq(_, false) => "list",
            Value::Seq(_, true) => "tuple",
            Value::Range(..) => "range",
            Value::None => "NoneType",
        }
    }

    /// the number, `None` for anything that isn't one
    pub fn number(&self) -> Option<f64> {
        match self {
            Value::Int(i) => Some(*i as f64),
            Value::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// python's truthiness
    pub fn truthy(&self) -> bool {
        match self {
            Value::Int(i) => *i != 0,
            Value::Float(f) => *f != 0.0,
            Value::Seq(items, _) => !items.is_empty(),
            Value::Range(start, stop, step) => range_len(*start, *stop, *step) > 0,
            Value::None => false,
        }
    }

    fn num(&self, what: &str) -> Result<f64, Fail> {
        self.number().ok_or_else(|| type_fail(format!("{} must be a number, not {}", what, self.type_name())))
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(i) => write!(f, "{}", i),
            Value::Float(x) => write!(f, "{}", format_float(*x)),
            Value::Seq(items, tuple) => {
                let inner: Vec<String> = items.iter().map(|v| v.to_string()).collect();
                match (tuple, inner.len()) {
                    (true, 1) => write!(f, "({},)", inner[0]),
                    (true, _) => write!(f, "({})", inner.join(", ")),
                    (false, _) => write!(f, "[{}]", inner.join(", ")),
                }
            }
            Value::Range(start, stop, 1) => write!(f, "range({}, {})", start, stop),
            Value::Range(start, stop, step) => write!(f, "range({}, {}, {})", start, stop, step),
            Value::None => write!(f, "None"),
        }
    }
}

/// a float the way python's repr writes it, close enough for messages: always with a point or an exponent
pub fn format_float(x: f64) -> String {
    if x.is_nan() {
        return "nan".into();
    }
    if x.is_infinite() {
        return if x > 0.0 {
            "inf".into()
        } else {
            "-inf".into()
        };
    }
    format!("{:?}", x)
}

// how many items range(start, stop, step) has
fn range_len(start: i64, stop: i64, step: i64) -> i64 {
    let (start, stop, step) = (start as i128, stop as i128, step as i128);
    let n = if step > 0 && start < stop {
        (stop - start + step - 1) / step
    } else if step < 0 && start > stop {
        (start - stop - step - 1) / -step
    } else {
        0
    };
    n.min(i64::MAX as i128) as i64
}

// MARK: - Functions

/// a function expressions can call, by name. `math` ones can also be called as `math.name`
#[derive(Clone, Copy)]
pub struct Function {
    pub name: &'static str,
    /// the fewest and the most arguments it takes, `None` for no limit
    pub min_args: usize,
    pub max_args: Option<usize>,
    /// in python's math module
    pub math: bool,
    pub call: fn(&[Value]) -> Result<Value, Fail>,
}

impl fmt::Debug for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}()", self.name)
    }
}

impl Function {
    /// a function of numbers that gives a number, for callers adding their own
    pub const fn new(name: &'static str, min_args: usize, max_args: Option<usize>, call: fn(&[Value]) -> Result<Value, Fail>) -> Self {
        Self {
            name,
            min_args,
            max_args,
            math: false,
            call,
        }
    }

    const fn math(name: &'static str, min_args: usize, max_args: usize, call: fn(&[Value]) -> Result<Value, Fail>) -> Self {
        Self {
            name,
            min_args,
            max_args: Some(max_args),
            math: true,
            call,
        }
    }
}

/// python's built-in functions and math functions, every expression has them
pub const BUILTINS: &[Function] = &[
    // python built-ins
    Function::new("abs", 1, Some(1), builtin_abs),
    Function::new("min", 1, None, builtin_min),
    Function::new("max", 1, None, builtin_max),
    Function::new("round", 1, Some(2), builtin_round),
    Function::new("int", 0, Some(1), builtin_int),
    Function::new("float", 0, Some(1), builtin_float),
    Function::new("bool", 0, Some(1), builtin_bool),
    Function::new("sum", 1, Some(2), builtin_sum),
    Function::new("len", 1, Some(1), builtin_len),
    Function::new("range", 1, Some(3), builtin_range),
    // the math module, bare or as math.name
    Function::math("sqrt", 1, 1, math_sqrt),
    Function::math("sin", 1, 1, math_sin),
    Function::math("cos", 1, 1, math_cos),
    Function::math("tan", 1, 1, math_tan),
    Function::math("asin", 1, 1, math_asin),
    Function::math("acos", 1, 1, math_acos),
    Function::math("atan", 1, 1, math_atan),
    Function::math("atan2", 2, 2, math_atan2),
    Function::math("exp", 1, 1, math_exp),
    Function::math("log", 1, 2, math_log),
    Function::math("log10", 1, 1, math_log10),
    Function::math("floor", 1, 1, math_floor),
    Function::math("ceil", 1, 1, math_ceil),
    Function::math("radians", 1, 1, math_radians),
    Function::math("degrees", 1, 1, math_degrees),
    // not python: extras that are handy for animation
    Function::new("clamp", 3, Some(3), extra_clamp),
    Function::new("lerp", 3, Some(3), extra_lerp),
];

/// constants from python's math module, bare or as math.name
pub const CONSTANTS: &[(&str, f64)] = &[("pi", std::f64::consts::PI), ("e", std::f64::consts::E), ("tau", std::f64::consts::TAU), ("inf", f64::INFINITY)];

fn builtin_abs(args: &[Value]) -> Result<Value, Fail> {
    match &args[0] {
        Value::Int(i) => i.checked_abs().map(Value::Int).ok_or_else(too_large),
        Value::Float(f) => Ok(Value::Float(f.abs())),
        other => Err(type_fail(format!("bad operand type for abs(): '{}'", other.type_name()))),
    }
}

// the items min, max and sum go over: one list or range, or the arguments themselves
fn items_of(args: &[Value], name: &str) -> Result<Vec<Value>, Fail> {
    if args.len() > 1 {
        return Ok(args.to_vec());
    }
    match &args[0] {
        Value::Seq(items, _) => Ok(items.to_vec()),
        Value::Range(start, stop, step) => {
            // only the ends matter for min and max, never build a long range
            let n = range_len(*start, *stop, *step);
            match n {
                0 => Ok(vec![]),
                1 => Ok(vec![Value::Int(*start)]),
                _ => Ok(vec![Value::Int(*start), Value::Int(start + (n - 1) * step)]),
            }
        }
        other => Err(type_fail(format!("'{}' object is not iterable, {}() takes a list or several numbers", other.type_name(), name))),
    }
}

fn extreme(args: &[Value], name: &str, better: fn(&Value, &Value) -> Result<bool, Fail>) -> Result<Value, Fail> {
    let items = items_of(args, name)?;
    let mut best: Option<Value> = None;
    for item in items {
        if item.number().is_none() {
            return Err(type_fail(format!("{}() takes numbers, not {}", name, item.type_name())));
        }
        best = match best {
            Some(current) if !better(&item, &current)? => Some(current),
            _ => Some(item),
        };
    }
    best.ok_or_else(|| Fail::new(ErrorKind::Domain, format!("{}() of an empty list", name)))
}

fn builtin_min(args: &[Value]) -> Result<Value, Fail> {
    extreme(args, "min", |a, b| compare(CmpOp::Lt, a, b))
}

fn builtin_max(args: &[Value]) -> Result<Value, Fail> {
    extreme(args, "max", |a, b| compare(CmpOp::Gt, a, b))
}

fn builtin_round(args: &[Value]) -> Result<Value, Fail> {
    let ndigits = match args.get(1) {
        None | Some(Value::None) => None,
        Some(Value::Int(n)) => Some(*n),
        Some(other) => return Err(type_fail(format!("round() digits must be an int, not {}", other.type_name()))),
    };
    match (&args[0], ndigits) {
        (Value::Int(i), None) => Ok(Value::Int(*i)),
        (Value::Int(i), Some(n)) if n >= 0 => Ok(Value::Int(*i)),
        (Value::Int(i), Some(n)) => round_int(*i, n).map(Value::Int),
        (Value::Float(f), None) => float_to_int(f.round_ties_even()).map(Value::Int),
        (Value::Float(f), Some(n)) => Ok(Value::Float(round_float(*f, n))),
        (other, _) => Err(type_fail(format!("type {} can't be rounded", other.type_name()))),
    }
}

// an int rounded to a negative number of digits, ties to even like python
fn round_int(i: i64, ndigits: i64) -> Result<i64, Fail> {
    if ndigits < -18 {
        return Ok(0);
    }
    let pow = 10i128.pow((-ndigits) as u32);
    let value = i as i128;
    let (mut q, r) = (value.div_euclid(pow), value.rem_euclid(pow));
    if 2 * r > pow || (2 * r == pow && q % 2 != 0) {
        q += 1;
    }
    i64::try_from(q * pow).map_err(|_| too_large())
}

// a float rounded to `ndigits` digits after the point. python rounds the exact value the float holds, ties to even, so
// does rust's formatting
fn round_float(f: f64, ndigits: i64) -> f64 {
    if !f.is_finite() || ndigits > 323 {
        return f;
    }
    if ndigits >= 0 {
        return format!("{:.*}", ndigits as usize, f).parse().unwrap_or(f);
    }
    if ndigits < -308 {
        return 0.0 * f;
    }
    // a negative number of digits rounds to tens, hundreds, ...
    let pow = 10f64.powi((-ndigits) as i32);
    let rounded = (f / pow).round_ties_even() * pow;
    if rounded.is_finite() {
        rounded
    } else {
        f
    }
}

// a whole float as an int, python's int() of inf or nan fails
fn float_to_int(f: f64) -> Result<i64, Fail> {
    if f.is_nan() {
        return Err(Fail::new(ErrorKind::Domain, "cannot convert float NaN to integer"));
    }
    if f.is_infinite() {
        return Err(Fail::new(ErrorKind::Overflow, "cannot convert float infinity to integer"));
    }
    if f < -9.223372036854776e18 || f >= 9.223372036854776e18 {
        return Err(too_large());
    }
    Ok(f as i64)
}

fn too_large() -> Fail {
    Fail::new(ErrorKind::Overflow, "number too large")
}

fn builtin_int(args: &[Value]) -> Result<Value, Fail> {
    match args.first() {
        None => Ok(Value::Int(0)),
        Some(Value::Int(i)) => Ok(Value::Int(*i)),
        Some(Value::Float(f)) => float_to_int(f.trunc()).map(Value::Int),
        Some(other) => Err(type_fail(format!("int() argument must be a number, not '{}'", other.type_name()))),
    }
}

fn builtin_float(args: &[Value]) -> Result<Value, Fail> {
    match args.first() {
        None => Ok(Value::Float(0.0)),
        Some(value) => Ok(Value::Float(value.number().ok_or_else(|| type_fail(format!("float() argument must be a number, not '{}'", value.type_name())))?)),
    }
}

fn builtin_bool(args: &[Value]) -> Result<Value, Fail> {
    Ok(Value::from(args.first().is_some_and(Value::truthy)))
}

fn builtin_sum(args: &[Value]) -> Result<Value, Fail> {
    let start = args.get(1).cloned().unwrap_or(Value::Int(0));
    if start.number().is_none() {
        return Err(type_fail(format!("sum() can't start from {}", start.type_name())));
    }
    // a range adds up without being built
    if let Value::Range(first, stop, step) = &args[0] {
        let n = range_len(*first, *stop, *step) as i128;
        let total = n * *first as i128 + *step as i128 * (n * (n - 1) / 2);
        let total = i64::try_from(total).map_err(|_| too_large())?;
        return add(&start, &Value::Int(total));
    }
    let Value::Seq(items, _) = &args[0] else {
        return Err(type_fail(format!("'{}' object is not iterable", args[0].type_name())));
    };
    // ints add up exactly, once a float comes up the rest is added with compensation like python 3.12+ (neumaier)
    let mut int_total: Option<i64> = match start {
        Value::Int(i) => Some(i),
        _ => None,
    };
    let mut float_total = start.number().unwrap_or(0.0);
    let mut compensation = 0.0;
    for item in items.iter() {
        if let (Some(total), Value::Int(i)) = (int_total, item) {
            match total.checked_add(*i) {
                Some(next) => {
                    int_total = Some(next);
                    continue;
                }
                None => return Err(too_large()),
            }
        }
        let x = item.number().ok_or_else(|| type_fail(format!("unsupported operand type(s) for +: 'float' and '{}'", item.type_name())))?;
        if let Some(total) = int_total.take() {
            float_total = total as f64;
        }
        let t = float_total + x;
        if float_total.abs() >= x.abs() {
            compensation += (float_total - t) + x;
        } else {
            compensation += (x - t) + float_total;
        }
        float_total = t;
    }
    match int_total {
        Some(total) => Ok(Value::Int(total)),
        None if compensation != 0.0 && compensation.is_finite() => Ok(Value::Float(float_total + compensation)),
        None => Ok(Value::Float(float_total)),
    }
}

fn builtin_len(args: &[Value]) -> Result<Value, Fail> {
    match &args[0] {
        Value::Seq(items, _) => Ok(Value::Int(items.len() as i64)),
        Value::Range(start, stop, step) => Ok(Value::Int(range_len(*start, *stop, *step))),
        other => Err(type_fail(format!("object of type '{}' has no len()", other.type_name()))),
    }
}

fn builtin_range(args: &[Value]) -> Result<Value, Fail> {
    let mut ints = vec![];
    for arg in args {
        match arg {
            Value::Int(i) => ints.push(*i),
            other => return Err(type_fail(format!("'{}' object cannot be interpreted as an integer", other.type_name()))),
        }
    }
    let (start, stop, step) = match ints.as_slice() {
        [stop] => (0, *stop, 1),
        [start, stop] => (*start, *stop, 1),
        [start, stop, step] => (*start, *stop, *step),
        _ => unreachable!("range() takes 1 to 3 arguments, compile checks it"),
    };
    if step == 0 {
        return Err(Fail::new(ErrorKind::Domain, "range() step must not be zero"));
    }
    Ok(Value::Range(start, stop, step))
}

// a math function of one number. like python, a NaN from a number that wasn't NaN is a domain error and an infinity from
// a finite number is a range error
fn math_1(args: &[Value], name: &str, f: fn(f64) -> f64) -> Result<Value, Fail> {
    let x = args[0].num(&format!("{}()'s argument", name))?;
    let result = f(x);
    if result.is_nan() && !x.is_nan() {
        return Err(Fail::new(ErrorKind::Domain, format!("math domain error ({}({}))", name, format_float(x))));
    }
    if result.is_infinite() && x.is_finite() {
        return Err(Fail::new(ErrorKind::Overflow, format!("math range error ({}({}))", name, format_float(x))));
    }
    Ok(Value::Float(result))
}

fn math_sqrt(args: &[Value]) -> Result<Value, Fail> {
    math_1(args, "sqrt", f64::sqrt)
}

fn math_sin(args: &[Value]) -> Result<Value, Fail> {
    math_1(args, "sin", f64::sin)
}

fn math_cos(args: &[Value]) -> Result<Value, Fail> {
    math_1(args, "cos", f64::cos)
}

fn math_tan(args: &[Value]) -> Result<Value, Fail> {
    math_1(args, "tan", f64::tan)
}

fn math_asin(args: &[Value]) -> Result<Value, Fail> {
    math_1(args, "asin", f64::asin)
}

fn math_acos(args: &[Value]) -> Result<Value, Fail> {
    math_1(args, "acos", f64::acos)
}

fn math_atan(args: &[Value]) -> Result<Value, Fail> {
    math_1(args, "atan", f64::atan)
}

fn math_atan2(args: &[Value]) -> Result<Value, Fail> {
    let y = args[0].num("atan2()'s first argument")?;
    let x = args[1].num("atan2()'s second argument")?;
    Ok(Value::Float(y.atan2(x)))
}

fn math_exp(args: &[Value]) -> Result<Value, Fail> {
    math_1(args, "exp", f64::exp)
}

fn math_log(args: &[Value]) -> Result<Value, Fail> {
    // python's log of 0 or a negative number is a domain error, not -inf or NaN
    let x = args[0].num("log()'s argument")?;
    if x <= 0.0 {
        return Err(Fail::new(ErrorKind::Domain, format!("math domain error (log({}))", format_float(x))));
    }
    let ln = x.ln();
    let Some(base) = args.get(1) else {
        return Ok(Value::Float(ln));
    };
    let base = base.num("log()'s base")?;
    if base <= 0.0 {
        return Err(Fail::new(ErrorKind::Domain, format!("math domain error (log base {})", format_float(base))));
    }
    let ln_base = base.ln();
    if ln_base == 0.0 {
        return Err(Fail::new(ErrorKind::ZeroDivision, "division by zero (log base 1)"));
    }
    Ok(Value::Float(ln / ln_base))
}

fn math_log10(args: &[Value]) -> Result<Value, Fail> {
    let x = args[0].num("log10()'s argument")?;
    if x <= 0.0 {
        return Err(Fail::new(ErrorKind::Domain, format!("math domain error (log10({}))", format_float(x))));
    }
    Ok(Value::Float(x.log10()))
}

fn math_floor(args: &[Value]) -> Result<Value, Fail> {
    match &args[0] {
        Value::Int(i) => Ok(Value::Int(*i)),
        other => float_to_int(other.num("floor()'s argument")?.floor()).map(Value::Int),
    }
}

fn math_ceil(args: &[Value]) -> Result<Value, Fail> {
    match &args[0] {
        Value::Int(i) => Ok(Value::Int(*i)),
        other => float_to_int(other.num("ceil()'s argument")?.ceil()).map(Value::Int),
    }
}

fn math_radians(args: &[Value]) -> Result<Value, Fail> {
    math_1(args, "radians", f64::to_radians)
}

fn math_degrees(args: &[Value]) -> Result<Value, Fail> {
    math_1(args, "degrees", f64::to_degrees)
}

fn extra_clamp(args: &[Value]) -> Result<Value, Fail> {
    for arg in args {
        arg.num("clamp()'s arguments")?;
    }
    // min(max(v, lo), hi)
    let low = builtin_max(&[args[0].clone(), args[1].clone()])?;
    builtin_min(&[low, args[2].clone()])
}

fn extra_lerp(args: &[Value]) -> Result<Value, Fail> {
    // a + (b - a) * t
    let span = sub(&args[1], &args[0])?;
    add(&args[0], &mul(&span, &args[2])?)
}

// MARK: - Arithmetic

#[derive(Debug, Clone, Copy, PartialEq)]
enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    FloorDiv,
    Mod,
    Pow,
}

impl BinOp {
    fn symbol(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::FloorDiv => "//",
            BinOp::Mod => "%",
            BinOp::Pow => "**",
        }
    }
}

// both sides as numbers, with python's message for anything else
fn operands(op: BinOp, a: &Value, b: &Value) -> Result<(f64, f64), Fail> {
    match (a.number(), b.number()) {
        (Some(x), Some(y)) => Ok((x, y)),
        _ => Err(type_fail(format!("unsupported operand type(s) for {}: '{}' and '{}'", op.symbol(), a.type_name(), b.type_name()))),
    }
}

fn add(a: &Value, b: &Value) -> Result<Value, Fail> {
    binary(BinOp::Add, a, b)
}

fn sub(a: &Value, b: &Value) -> Result<Value, Fail> {
    binary(BinOp::Sub, a, b)
}

fn mul(a: &Value, b: &Value) -> Result<Value, Fail> {
    binary(BinOp::Mul, a, b)
}

fn zero_division(what: &str) -> Fail {
    Fail::new(ErrorKind::ZeroDivision, what)
}

fn binary(op: BinOp, a: &Value, b: &Value) -> Result<Value, Fail> {
    // ints stay ints, except true division and negative powers
    if let (Value::Int(x), Value::Int(y)) = (a, b) {
        let (x, y) = (*x, *y);
        return match op {
            BinOp::Add => x.checked_add(y).map(Value::Int).ok_or_else(too_large),
            BinOp::Sub => x.checked_sub(y).map(Value::Int).ok_or_else(too_large),
            BinOp::Mul => x.checked_mul(y).map(Value::Int).ok_or_else(too_large),
            BinOp::Div if y == 0 => Err(zero_division("division by zero")),
            BinOp::Div => Ok(Value::Float(x as f64 / y as f64)),
            BinOp::FloorDiv | BinOp::Mod if y == 0 => Err(zero_division(if op == BinOp::Mod {
                "modulo by zero"
            } else {
                "division by zero"
            })),
            BinOp::FloorDiv => {
                // python floors, rust truncates
                let q = x.checked_div(y).ok_or_else(too_large)?;
                Ok(Value::Int(if x % y != 0 && ((x < 0) != (y < 0)) {
                    q - 1
                } else {
                    q
                }))
            }
            BinOp::Mod => {
                // the remainder takes the sign of the divisor
                let r = x.checked_rem(y).unwrap_or(0);
                Ok(Value::Int(if r != 0 && ((r < 0) != (y < 0)) {
                    r + y
                } else {
                    r
                }))
            }
            BinOp::Pow if y >= 0 => u32::try_from(y).ok().and_then(|y| x.checked_pow(y)).map(Value::Int).ok_or_else(too_large),
            BinOp::Pow => float_pow(x as f64, y as f64).map(Value::Float),
        };
    }

    let (x, y) = operands(op, a, b)?;
    let result = match op {
        BinOp::Add => x + y,
        BinOp::Sub => x - y,
        BinOp::Mul => x * y,
        BinOp::Div if y == 0.0 => return Err(zero_division("division by zero")),
        BinOp::Div => x / y,
        BinOp::FloorDiv if y == 0.0 => return Err(zero_division("division by zero")),
        BinOp::Mod if y == 0.0 => return Err(zero_division("modulo by zero")),
        BinOp::FloorDiv => float_divmod(x, y).0,
        BinOp::Mod => float_divmod(x, y).1,
        BinOp::Pow => float_pow(x, y)?,
    };
    Ok(Value::Float(result))
}

// python's float floor division and modulo (floatobject.c's divmod), `y` isn't 0
fn float_divmod(x: f64, y: f64) -> (f64, f64) {
    let mut m = x % y;
    let mut div = (x - m) / y;
    if m != 0.0 {
        if (y < 0.0) != (m < 0.0) {
            m += y;
            div -= 1.0;
        }
    } else {
        m = 0f64.copysign(y);
    }
    let floordiv = if div != 0.0 {
        let floored = div.floor();
        if div - floored > 0.5 {
            floored + 1.0
        } else {
            floored
        }
    } else {
        0f64.copysign(x / y)
    };
    (floordiv, m)
}

// python's float power: 0 to a negative power divides by zero, a negative number to a fractional power is complex in
// python (not supported), overflowing is an error
fn float_pow(x: f64, y: f64) -> Result<f64, Fail> {
    if y == 0.0 {
        return Ok(1.0);
    }
    if x == 0.0 && y < 0.0 {
        return Err(zero_division("zero to a negative power"));
    }
    if x < 0.0 && x.is_finite() && y.is_finite() && y.fract() != 0.0 {
        return Err(Fail::new(ErrorKind::Domain, "a negative number to a fractional power is a complex number"));
    }
    let result = x.powf(y);
    if result.is_infinite() && x.is_finite() && y.is_finite() {
        return Err(too_large());
    }
    Ok(result)
}

// MARK: - Comparisons

#[derive(Debug, Clone, Copy, PartialEq)]
enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    In,
    NotIn,
}

impl CmpOp {
    fn symbol(self) -> &'static str {
        match self {
            CmpOp::Eq => "==",
            CmpOp::Ne => "!=",
            CmpOp::Lt => "<",
            CmpOp::Le => "<=",
            CmpOp::Gt => ">",
            CmpOp::Ge => ">=",
            CmpOp::In => "in",
            CmpOp::NotIn => "not in",
        }
    }
}

// python's ==: numbers by value, lists and tuples item by item (a list never equals a tuple), ranges by their items
fn equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Seq(x, xt), Value::Seq(y, yt)) => xt == yt && x.len() == y.len() && x.iter().zip(y.iter()).all(|(a, b)| equal(a, b)),
        (Value::Range(a1, a2, a3), Value::Range(b1, b2, b3)) => {
            let (na, nb) = (range_len(*a1, *a2, *a3), range_len(*b1, *b2, *b3));
            na == nb && (na == 0 || (a1 == b1 && (na == 1 || a3 == b3)))
        }
        (Value::None, Value::None) => true,
        _ => match (a.number(), b.number()) {
            (Some(x), Some(y)) => x == y,
            _ => false,
        },
    }
}

// whether `item` is in a list, tuple or range
fn contains(container: &Value, item: &Value) -> Result<bool, Fail> {
    match container {
        Value::Seq(items, _) => Ok(items.iter().any(|v| equal(v, item))),
        Value::Range(start, stop, step) => {
            // a float is in a range when it's a whole number that is
            let i = match item {
                Value::Int(i) => *i,
                Value::Float(f) if f.fract() == 0.0 && f.abs() < 9.2e18 => *f as i64,
                _ => return Ok(false),
            };
            let n = range_len(*start, *stop, *step);
            let offset = i as i128 - *start as i128;
            Ok(offset % *step as i128 == 0 && (0..n as i128).contains(&(offset / *step as i128)))
        }
        other => Err(type_fail(format!("argument of type '{}' is not iterable, 'in' takes a list or a range", other.type_name()))),
    }
}

fn compare(op: CmpOp, a: &Value, b: &Value) -> Result<bool, Fail> {
    match op {
        CmpOp::Eq => return Ok(equal(a, b)),
        CmpOp::Ne => return Ok(!equal(a, b)),
        CmpOp::In => return contains(b, a),
        CmpOp::NotIn => return contains(b, a).map(|c| !c),
        _ => {}
    }
    // ints exactly, anything else as floats
    if let (Value::Int(x), Value::Int(y)) = (a, b) {
        return Ok(match op {
            CmpOp::Lt => x < y,
            CmpOp::Le => x <= y,
            CmpOp::Gt => x > y,
            _ => x >= y,
        });
    }
    let (Some(x), Some(y)) = (a.number(), b.number()) else {
        return Err(type_fail(format!("'{}' not supported between '{}' and '{}'", op.symbol(), a.type_name(), b.type_name())));
    };
    Ok(match op {
        CmpOp::Lt => x < y,
        CmpOp::Le => x <= y,
        CmpOp::Gt => x > y,
        _ => x >= y,
    })
}

// MARK: - Tokens

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Int(i64),
    Float(f64),
    Name(String),
    /// an operator or punctuation, `(`, `**`, `,`, ...
    Op(&'static str),
    End,
}

#[derive(Debug, Clone)]
struct Token {
    tok: Tok,
    span: Span,
}

// every operator and punctuation python has, longest first so `**=` wins over `**` and `*`
const OPERATORS: &[&str] = &["**=", "//=", ">>=", "<<=", "...", "**", "//", "==", "!=", "<=", ">=", "<<", ">>", "->", ":=", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "@=", "+", "-", "*", "/", "%", "<", ">", "=", "(", ")", "[", "]", "{", "}", ",", ".", ":", ";", "@", "&", "|", "^", "~"];

// python's keywords, the ones expressions here use are handled by the parser and the rest aren't supported
const KEYWORDS: &[&str] = &["False", "None", "True", "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global", "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while", "with", "yield"];

fn tokenize(chars: &[char]) -> Result<Vec<Token>, ExprError> {
    let mut tokens = vec![];
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let start = i;

        // spaces between tokens
        if c == ' ' || c == '\t' {
            i += 1;
            continue;
        }
        // a comment runs to the end
        if c == '#' {
            break;
        }

        // a number: digits, or a point followed by a digit
        if c.is_ascii_digit() || (c == '.' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let (tok, end) = number(chars, i)?;
            tokens.push(Token {
                tok,
                span: Span::new(start, end),
            });
            i = end;
            continue;
        }

        // a name or keyword
        if c.is_ascii_alphabetic() || c == '_' {
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            // a quote right after a name is a prefixed string, f"..." or r'...'
            if chars.get(i).is_some_and(|q| *q == '"' || *q == '\'') {
                return Err(error(ErrorKind::Unsupported, Span::new(start, i + 1), "not supported: strings"));
            }
            tokens.push(Token {
                tok: Tok::Name(chars[start..i].iter().collect()),
                span: Span::new(start, i),
            });
            continue;
        }

        // things python has that don't belong in a single line expression here
        match c {
            '"' | '\'' => return Err(error(ErrorKind::Unsupported, Span::new(i, i + 1), "not supported: strings")),
            '\n' | '\r' => return Err(error(ErrorKind::Unsupported, Span::new(i, i + 1), "not supported: line breaks")),
            '\\' => return Err(error(ErrorKind::Unsupported, Span::new(i, i + 1), "not supported: backslashes")),
            _ => {}
        }

        // an operator, the longest one that fits
        let rest: String = chars[i..chars.len().min(i + 3)].iter().collect();
        let Some(op) = OPERATORS.iter().find(|op| rest.starts_with(**op)) else {
            return Err(error(ErrorKind::Syntax, Span::new(i, i + 1), format!("unexpected character '{}'", c)));
        };
        i += op.chars().count();
        tokens.push(Token {
            tok: Tok::Op(op),
            span: Span::new(start, i),
        });
    }
    tokens.push(Token {
        tok: Tok::End,
        span: Span::new(chars.len(), chars.len()),
    });
    Ok(tokens)
}

// reads a number starting at `start`, python's rules: underscores only between digits, no leading zeros on whole
// numbers, 0x / 0o / 0b prefixes
fn number(chars: &[char], start: usize) -> Result<(Tok, usize), ExprError> {
    let at = |i: usize| chars.get(i).copied().unwrap_or('\0');
    let mut i = start;

    // digits of a base with single underscores between them, returns them without the underscores
    let digits = |i: &mut usize, valid: fn(char) -> bool| -> Result<String, ExprError> {
        let mut text = String::new();
        while valid(at(*i)) || (at(*i) == '_' && valid(at(*i + 1)) && !text.is_empty()) {
            if at(*i) != '_' {
                text.push(at(*i));
            }
            *i += 1;
        }
        Ok(text)
    };

    // 0x1f, 0o17, 0b101
    if at(i) == '0' && matches!(at(i + 1), 'x' | 'X' | 'o' | 'O' | 'b' | 'B') {
        let (radix, valid): (u32, fn(char) -> bool) = match at(i + 1) {
            'x' | 'X' => (16, |c| c.is_ascii_hexdigit()),
            'o' | 'O' => (8, |c| ('0'..='7').contains(&c)),
            _ => (2, |c| c == '0' || c == '1'),
        };
        i += 2;
        // an underscore may follow the prefix, 0x_ff
        if at(i) == '_' {
            i += 1;
        }
        let text = digits(&mut i, valid)?;
        if text.is_empty() || at(i).is_ascii_alphanumeric() || at(i) == '_' {
            return Err(error(ErrorKind::Syntax, Span::new(start, i + 1), "invalid number"));
        }
        let value = i64::from_str_radix(&text, radix).map_err(|_| error(ErrorKind::Overflow, Span::new(start, i), "number too large"))?;
        return Ok((Tok::Int(value), i));
    }

    // whole part, fraction, exponent
    let whole = digits(&mut i, |c| c.is_ascii_digit())?;
    let mut is_float = false;
    let mut text = whole.clone();
    if at(i) == '.' {
        is_float = true;
        i += 1;
        text.push('.');
        text.push_str(&digits(&mut i, |c| c.is_ascii_digit())?);
    }
    if matches!(at(i), 'e' | 'E') {
        let mut j = i + 1;
        let mut exponent = String::from("e");
        if matches!(at(j), '+' | '-') {
            exponent.push(at(j));
            j += 1;
        }
        let exp_digits = digits(&mut j, |c| c.is_ascii_digit())?;
        if exp_digits.is_empty() {
            return Err(error(ErrorKind::Syntax, Span::new(start, j), "invalid number"));
        }
        exponent.push_str(&exp_digits);
        text.push_str(&exponent);
        is_float = true;
        i = j;
    }
    // 1j is complex, 1abc and 1_ are typos
    if matches!(at(i), 'j' | 'J') {
        return Err(error(ErrorKind::Unsupported, Span::new(start, i + 1), "not supported: complex numbers"));
    }
    if at(i).is_ascii_alphanumeric() || at(i) == '_' {
        return Err(error(ErrorKind::Syntax, Span::new(start, i + 1), "invalid number"));
    }

    if is_float {
        let value: f64 = text.parse().map_err(|_| error(ErrorKind::Syntax, Span::new(start, i), "invalid number"))?;
        return Ok((Tok::Float(value), i));
    }
    // python doesn't allow leading zeros on a whole number, other than 0 itself (00 is fine)
    if whole.len() > 1 && whole.starts_with('0') && whole.chars().any(|c| c != '0') {
        return Err(error(ErrorKind::Syntax, Span::new(start, i), "leading zeros aren't allowed in numbers, use 0o for octal"));
    }
    let value: i64 = whole.parse().map_err(|_| error(ErrorKind::Overflow, Span::new(start, i), "number too large"))?;
    Ok((Tok::Int(value), i))
}

// MARK: - Syntax tree

#[derive(Debug, Clone)]
enum Kind {
    Const(Value),
    Name(String),
    /// math.name
    MathName(String),
    List(Vec<Ast>, bool),
    Neg(Box<Ast>),
    Pos(Box<Ast>),
    Not(Box<Ast>),
    Binary(BinOp, Box<Ast>, Box<Ast>),
    /// a chain of comparisons, `a < b <= c`
    Compare(Box<Ast>, Vec<(CmpOp, Ast)>),
    And(Box<Ast>, Box<Ast>),
    Or(Box<Ast>, Box<Ast>),
    /// body if cond else orelse
    IfElse(Box<Ast>, Box<Ast>, Box<Ast>),
    /// a function by name (or math.name) and its arguments
    Call(Box<Ast>, Vec<Ast>),
    Index(Box<Ast>, Box<Ast>),
}

#[derive(Debug, Clone)]
struct Ast {
    kind: Kind,
    span: Span,
    /// how deeply nested it is, so a chain like 1+1+1+... can't go deeper than the parser's recursion allows
    depth: usize,
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    /// how many rules deep the parser is
    depth: usize,
}

type Parsed = Result<Ast, ExprError>;

impl<'a> Parser<'a> {
    fn peek(&self) -> &Tok {
        &self.tokens[self.pos].tok
    }

    fn span(&self) -> Span {
        self.tokens[self.pos].span
    }

    fn advance(&mut self) -> Token {
        let token = self.tokens[self.pos].clone();
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        token
    }

    fn is_op(&self, op: &str) -> bool {
        matches!(self.peek(), Tok::Op(o) if *o == op)
    }

    fn is_keyword(&self, word: &str) -> bool {
        matches!(self.peek(), Tok::Name(n) if n == word)
    }

    fn expect_op(&mut self, op: &str, what: &str) -> Result<Span, ExprError> {
        if self.is_op(op) {
            return Ok(self.advance().span);
        }
        Err(self.unexpected(what))
    }

    // an error for the token here, with a clear message for python syntax that isn't supported
    fn unexpected(&self, expected: &str) -> ExprError {
        let span = self.span();
        if let Some(message) = unsupported(self.peek()) {
            return error(ErrorKind::Unsupported, span, message);
        }
        let found = match self.peek() {
            Tok::End => "the end".to_string(),
            Tok::Int(i) => format!("'{}'", i),
            Tok::Float(f) => format!("'{}'", format_float(*f)),
            Tok::Name(n) => format!("'{}'", n),
            Tok::Op(op) => format!("'{}'", op),
        };
        error(ErrorKind::Syntax, span, format!("expected {}, found {}", expected, found))
    }

    // makes a node, deeper than `MAX_DEPTH` is an error
    fn node(&self, kind: Kind, span: Span) -> Parsed {
        let child = |a: &Ast| a.depth;
        let deepest = match &kind {
            Kind::Const(_) | Kind::Name(_) | Kind::MathName(_) => 0,
            Kind::List(items, _) => items.iter().map(child).max().unwrap_or(0),
            Kind::Neg(a) | Kind::Pos(a) | Kind::Not(a) => a.depth,
            Kind::Binary(_, a, b) | Kind::And(a, b) | Kind::Or(a, b) | Kind::Index(a, b) => a.depth.max(b.depth),
            Kind::Compare(first, rest) => rest.iter().map(|(_, a)| a.depth).max().unwrap_or(0).max(first.depth),
            Kind::IfElse(a, b, c) => a.depth.max(b.depth).max(c.depth),
            Kind::Call(callee, args) => args.iter().map(child).max().unwrap_or(0).max(callee.depth),
        };
        if deepest + 1 > MAX_DEPTH {
            return Err(error(ErrorKind::TooDeep, span, format!("nested too deeply (more than {} levels)", MAX_DEPTH)));
        }
        Ok(Ast {
            kind,
            span,
            depth: deepest + 1,
        })
    }

    // runs a rule one level deeper, too deep is an error
    fn nested(&mut self, rule: fn(&mut Self) -> Parsed) -> Parsed {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(error(ErrorKind::TooDeep, self.span(), format!("nested too deeply (more than {} levels)", MAX_DEPTH)));
        }
        let result = rule(self);
        self.depth -= 1;
        result
    }

    fn expr(&mut self) -> Parsed {
        self.nested(Self::conditional)
    }

    // expr := or_test ['if' or_test 'else' expr]
    fn conditional(&mut self) -> Parsed {
        if self.is_keyword("lambda") {
            return Err(error(ErrorKind::Unsupported, self.span(), "not supported: lambda"));
        }
        let body = self.or_test()?;
        if !self.is_keyword("if") {
            return Ok(body);
        }
        self.advance();
        let cond = self.or_test()?;
        if !self.is_keyword("else") {
            return Err(self.unexpected("'else'"));
        }
        self.advance();
        let orelse = self.expr()?;
        let span = body.span.to(orelse.span);
        self.node(Kind::IfElse(Box::new(body), Box::new(cond), Box::new(orelse)), span)
    }

    fn or_test(&mut self) -> Parsed {
        let mut left = self.and_test()?;
        while self.is_keyword("or") {
            self.advance();
            let right = self.and_test()?;
            let span = left.span.to(right.span);
            left = self.node(Kind::Or(Box::new(left), Box::new(right)), span)?;
        }
        Ok(left)
    }

    fn and_test(&mut self) -> Parsed {
        let mut left = self.not_test()?;
        while self.is_keyword("and") {
            self.advance();
            let right = self.not_test()?;
            let span = left.span.to(right.span);
            left = self.node(Kind::And(Box::new(left), Box::new(right)), span)?;
        }
        Ok(left)
    }

    fn not_test(&mut self) -> Parsed {
        if self.is_keyword("not") {
            let start = self.advance().span;
            let operand = self.nested(Self::not_test)?;
            let span = start.to(operand.span);
            return self.node(Kind::Not(Box::new(operand)), span);
        }
        self.comparison()
    }

    fn comparison_op(&mut self) -> Result<Option<CmpOp>, ExprError> {
        let op = match self.peek() {
            Tok::Op("==") => CmpOp::Eq,
            Tok::Op("!=") => CmpOp::Ne,
            Tok::Op("<") => CmpOp::Lt,
            Tok::Op("<=") => CmpOp::Le,
            Tok::Op(">") => CmpOp::Gt,
            Tok::Op(">=") => CmpOp::Ge,
            Tok::Name(n) if n == "in" => CmpOp::In,
            Tok::Name(n) if n == "is" => return Err(error(ErrorKind::Unsupported, self.span(), "not supported: 'is', use == or !=")),
            // `not` here is only `not in`
            Tok::Name(n) if n == "not" => {
                let span = self.advance().span;
                if !self.is_keyword("in") {
                    return Err(error(ErrorKind::Syntax, span, "expected 'in' after 'not'"));
                }
                self.advance();
                return Ok(Some(CmpOp::NotIn));
            }
            _ => return Ok(None),
        };
        self.advance();
        Ok(Some(op))
    }

    fn comparison(&mut self) -> Parsed {
        let first = self.arith()?;
        let mut rest: Vec<(CmpOp, Ast)> = vec![];
        while let Some(op) = self.comparison_op()? {
            rest.push((op, self.arith()?));
        }
        if rest.is_empty() {
            return Ok(first);
        }
        let span = first.span.to(rest.last().map(|(_, a)| a.span).unwrap_or(first.span));
        self.node(Kind::Compare(Box::new(first), rest), span)
    }

    fn arith(&mut self) -> Parsed {
        let mut left = self.term()?;
        loop {
            let op = match self.peek() {
                Tok::Op("+") => BinOp::Add,
                Tok::Op("-") => BinOp::Sub,
                _ => return Ok(left),
            };
            self.advance();
            let right = self.term()?;
            let span = left.span.to(right.span);
            left = self.node(Kind::Binary(op, Box::new(left), Box::new(right)), span)?;
        }
    }

    fn term(&mut self) -> Parsed {
        let mut left = self.factor()?;
        loop {
            let op = match self.peek() {
                Tok::Op("*") => BinOp::Mul,
                Tok::Op("/") => BinOp::Div,
                Tok::Op("//") => BinOp::FloorDiv,
                Tok::Op("%") => BinOp::Mod,
                _ => return Ok(left),
            };
            self.advance();
            let right = self.factor()?;
            let span = left.span.to(right.span);
            left = self.node(Kind::Binary(op, Box::new(left), Box::new(right)), span)?;
        }
    }

    // unary + and - bind looser than ** on their right: -2**2 is -(2**2)
    fn factor(&mut self) -> Parsed {
        let negate = match self.peek() {
            Tok::Op("-") => true,
            Tok::Op("+") => false,
            Tok::Op("~") => return Err(error(ErrorKind::Unsupported, self.span(), "not supported: bitwise operators")),
            _ => return self.power(),
        };
        let start = self.advance().span;
        let operand = self.nested(Self::factor)?;
        let span = start.to(operand.span);
        let kind = if negate {
            Kind::Neg(Box::new(operand))
        } else {
            Kind::Pos(Box::new(operand))
        };
        self.node(kind, span)
    }

    // power := primary ['**' factor], right-associative through factor
    fn power(&mut self) -> Parsed {
        let base = self.primary()?;
        if !self.is_op("**") {
            return Ok(base);
        }
        self.advance();
        let exponent = self.nested(Self::factor)?;
        let span = base.span.to(exponent.span);
        self.node(Kind::Binary(BinOp::Pow, Box::new(base), Box::new(exponent)), span)
    }

    // an atom and the calls and indexes after it
    fn primary(&mut self) -> Parsed {
        let mut value = self.atom()?;
        loop {
            if self.is_op("(") {
                let open = self.advance().span;
                let (args, close) = self.items(")", "an argument or ')'", true)?;
                let span = value.span.to(open).to(close);
                value = self.node(Kind::Call(Box::new(value), args), span)?;
            } else if self.is_op("[") {
                self.advance();
                let index = self.expr()?;
                if self.is_op(":") {
                    return Err(error(ErrorKind::Unsupported, self.span(), "not supported: slices"));
                }
                if self.is_op(",") {
                    return Err(error(ErrorKind::Unsupported, self.span(), "not supported: indexing with a tuple"));
                }
                let close = self.expect_op("]", "']'")?;
                let span = value.span.to(close);
                value = self.node(Kind::Index(Box::new(value), Box::new(index)), span)?;
            } else if self.is_op(".") {
                // only math.name, a float like 1.5 is one token
                let dot = self.advance().span;
                let is_math = matches!(&value.kind, Kind::Name(n) if n == "math");
                let attr = match self.peek() {
                    Tok::Name(n) if is_math => n.clone(),
                    Tok::Name(_) => return Err(error(ErrorKind::Unsupported, value.span.to(self.span()), "not supported: attribute access (only math.name)")),
                    _ => return Err(self.unexpected("a name after '.'")),
                };
                let name_span = self.advance().span;
                value = self.node(Kind::MathName(attr), value.span.to(dot).to(name_span))?;
            } else {
                return Ok(value);
            }
        }
    }

    // comma separated expressions up to `close`, a trailing comma is fine. calls report keyword and star arguments
    fn items(&mut self, close: &str, what: &str, call: bool) -> Result<(Vec<Ast>, Span), ExprError> {
        let mut items = vec![];
        loop {
            if self.is_op(close) {
                return Ok((items, self.advance().span));
            }
            if self.is_op("*") || self.is_op("**") {
                return Err(error(ErrorKind::Unsupported, self.span(), "not supported: unpacking with * or **"));
            }
            let item = self.expr()?;
            if call && self.is_op("=") {
                return Err(error(ErrorKind::Unsupported, item.span.to(self.span()), "not supported: keyword arguments"));
            }
            if self.is_keyword("for") {
                return Err(error(ErrorKind::Unsupported, self.span(), "not supported: comprehensions"));
            }
            items.push(item);
            if self.is_op(",") {
                self.advance();
            } else if !self.is_op(close) {
                return Err(self.unexpected(what));
            }
        }
    }

    fn atom(&mut self) -> Parsed {
        let span = self.span();
        match self.peek().clone() {
            Tok::Int(i) => {
                self.advance();
                self.node(Kind::Const(Value::Int(i)), span)
            }
            Tok::Float(f) => {
                self.advance();
                self.node(Kind::Const(Value::Float(f)), span)
            }
            Tok::Name(name) => {
                match name.as_str() {
                    "True" | "False" => {
                        self.advance();
                        return self.node(Kind::Const(Value::from(name == "True")), span);
                    }
                    "None" => {
                        self.advance();
                        return self.node(Kind::Const(Value::None), span);
                    }
                    _ => {}
                }
                if KEYWORDS.contains(&name.as_str()) {
                    return Err(self.unexpected("a value"));
                }
                self.advance();
                self.node(Kind::Name(name), span)
            }
            // ( expr ) or a tuple
            Tok::Op("(") => {
                self.advance();
                if self.is_op(")") {
                    let close = self.advance().span;
                    return self.node(Kind::List(vec![], true), span.to(close));
                }
                let first = self.expr()?;
                if self.is_keyword("for") {
                    return Err(error(ErrorKind::Unsupported, self.span(), "not supported: comprehensions"));
                }
                if self.is_op(":=") {
                    return Err(error(ErrorKind::Unsupported, self.span(), "not supported: assignment"));
                }
                // the parentheses are part of what an error underlines
                if self.is_op(")") {
                    let close = self.advance().span;
                    return Ok(Ast {
                        span: span.to(close),
                        ..first
                    });
                }
                if !self.is_op(",") {
                    return Err(self.unexpected("')'"));
                }
                self.advance();
                let (rest, close) = self.items(")", "',' or ')'", false)?;
                let items: Vec<Ast> = std::iter::once(first).chain(rest).collect();
                self.list(items, true, span.to(close))
            }
            Tok::Op("[") => {
                self.advance();
                let (items, close) = self.items("]", "',' or ']'", false)?;
                self.list(items, false, span.to(close))
            }
            _ => Err(self.unexpected("a value")),
        }
    }

    fn list(&self, items: Vec<Ast>, tuple: bool, span: Span) -> Parsed {
        self.node(Kind::List(items, tuple), span)
    }
}

// what a token python has but this language doesn't is, for its error message
fn unsupported(tok: &Tok) -> Option<String> {
    let message = match tok {
        Tok::Op("=" | ":=" | "+=" | "-=" | "*=" | "/=" | "//=" | "%=" | "**=" | "&=" | "|=" | "^=" | ">>=" | "<<=" | "@=") => "assignment",
        Tok::Op("&" | "|" | "^" | "~" | "<<" | ">>") => "bitwise operators",
        Tok::Op("@") => "matrix multiplication",
        Tok::Op("{" | "}") => "dicts and sets",
        Tok::Op(":") => "slices and dicts",
        Tok::Op(";") => "more than one statement",
        Tok::Op("...") => "Ellipsis",
        Tok::Name(n) if n == "lambda" => "lambda",
        Tok::Name(n) if n == "for" => "comprehensions",
        Tok::Name(n) if n == "is" => "'is', use == or !=",
        Tok::Name(n) if n == "await" || n == "yield" => "await and yield",
        Tok::Name(n) if KEYWORDS.contains(&n.as_str()) && !matches!(n.as_str(), "and" | "or" | "not" | "if" | "else" | "in" | "True" | "False" | "None") => return Some(format!("not supported: '{}'", n)),
        _ => return None,
    };
    Some(format!("not supported: {}", message))
}

// MARK: - Compile

/// what an expression can use besides python's built-ins: the variables the caller gives values for, in order, and
/// functions of its own
#[derive(Clone, Copy)]
pub struct Env<'a> {
    pub vars: &'a [&'a str],
    pub functions: &'a [Function],
}

impl<'a> Env<'a> {
    pub fn new(vars: &'a [&'a str]) -> Self {
        Self {
            vars,
            functions: &[],
        }
    }

    pub fn with_functions(mut self, functions: &'a [Function]) -> Self {
        self.functions = functions;
        self
    }

    // a function by name, the caller's first
    fn function(&self, name: &str) -> Option<Function> {
        self.functions.iter().chain(BUILTINS.iter()).find(|f| f.name == name).copied()
    }

    // every name an expression can use, for "did you mean"
    fn names(&self) -> Vec<&str> {
        self.vars.iter().copied().chain(self.functions.iter().map(|f| f.name)).chain(BUILTINS.iter().map(|f| f.name)).chain(CONSTANTS.iter().map(|(n, _)| *n)).collect()
    }
}

#[derive(Debug, Clone)]
enum NodeKind {
    Const(Value),
    Var(usize),
    List(Vec<Node>, bool),
    Neg(Box<Node>),
    Pos(Box<Node>),
    Not(Box<Node>),
    Binary(BinOp, Box<Node>, Box<Node>),
    Compare(Box<Node>, Vec<(CmpOp, Node)>),
    And(Box<Node>, Box<Node>),
    Or(Box<Node>, Box<Node>),
    IfElse(Box<Node>, Box<Node>, Box<Node>),
    Call(Function, Vec<Node>),
    Index(Box<Node>, Box<Node>),
}

/// a compiled node and the characters it came from, errors while running point at the node that failed
#[derive(Debug, Clone)]
struct Node {
    kind: NodeKind,
    span: Span,
}

/// a compiled expression, evaluate it with `eval` as many times as needed
#[derive(Debug, Clone)]
pub struct Program {
    root: Node,
    /// the variables the expression uses, by their place in `Env::vars`
    used: Vec<bool>,
    vars: Vec<String>,
    text: String,
}

/// compiles `text`: parses it and checks every name. an empty text is a syntax error, callers treat it as no expression
pub fn compile(text: &str, env: &Env) -> Result<Program, ExprError> {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() > MAX_LEN {
        return Err(error(ErrorKind::TooLong, Span::new(MAX_LEN, chars.len()), format!("expression is too long (more than {} characters)", MAX_LEN)));
    }
    let tokens = tokenize(&chars)?;
    if tokens.len() == 1 {
        return Err(error(ErrorKind::Syntax, Span::new(0, chars.len()), "empty expression"));
    }
    let mut parser = Parser {
        tokens: &tokens,
        pos: 0,
        depth: 0,
    };
    let ast = parser.expr()?;
    if *parser.peek() != Tok::End {
        return Err(parser.unexpected("the end"));
    }

    let mut compiler = Compiler {
        env,
        used: vec![false; env.vars.len()],
    };
    let root = compiler.node(&ast)?;
    Ok(Program {
        root,
        used: compiler.used,
        vars: env.vars.iter().map(|v| v.to_string()).collect(),
        text: text.to_string(),
    })
}

struct Compiler<'a, 'b> {
    env: &'a Env<'b>,
    used: Vec<bool>,
}

// the name closest to a mistyped one, if any is close
fn suggest<'a>(name: &str, candidates: &[&'a str]) -> Option<&'a str> {
    let limit = (name.chars().count() / 3).max(1);
    candidates.iter().map(|c| (edit_distance(name, c), *c)).filter(|(d, _)| *d <= limit).min_by_key(|(d, _)| *d).map(|(_, c)| c)
}

// levenshtein distance, with a swap of two letters counting as one edit
fn edit_distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut d = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for j in 0..=b.len() {
        d[0][j] = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = (a[i - 1] != b[j - 1]) as usize;
            d[i][j] = (d[i - 1][j] + 1).min(d[i][j - 1] + 1).min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                d[i][j] = d[i][j].min(d[i - 2][j - 2] + 1);
            }
        }
    }
    d[a.len()][b.len()]
}

fn unknown_name(name: &str, span: Span, candidates: &[&str]) -> ExprError {
    match suggest(name, candidates) {
        Some(close) => error(ErrorKind::UnknownName, span, format!("unknown name '{}', did you mean '{}'?", name, close)),
        None => error(ErrorKind::UnknownName, span, format!("unknown name '{}'", name)),
    }
}

// the value of a node that's a plain number or a list of them, worked out once when compiled
fn constant(node: &Node) -> Option<&Value> {
    match &node.kind {
        NodeKind::Const(value) => Some(value),
        _ => None,
    }
}

impl Compiler<'_, '_> {
    fn node(&mut self, ast: &Ast) -> Result<Node, ExprError> {
        let span = ast.span;
        let kind = match &ast.kind {
            Kind::Const(value) => NodeKind::Const(value.clone()),
            Kind::Name(name) => self.name(name, span)?,
            Kind::MathName(name) => match CONSTANTS.iter().find(|(n, _)| n == name) {
                Some((_, value)) => NodeKind::Const(Value::Float(*value)),
                None if BUILTINS.iter().any(|f| f.math && f.name == name) => return Err(error(ErrorKind::Unsupported, span, format!("not supported: functions as values, call it like math.{}(...)", name))),
                None => return Err(self.unknown_math(name, span)),
            },
            Kind::List(items, tuple) => {
                let nodes = items.iter().map(|item| self.node(item)).collect::<Result<Vec<_>, _>>()?;
                // a list of plain numbers is made once, not every time the expression runs
                match nodes.iter().map(constant).collect::<Option<Vec<&Value>>>() {
                    Some(values) => NodeKind::Const(Value::Seq(values.into_iter().cloned().collect::<Vec<_>>().into(), *tuple)),
                    None => NodeKind::List(nodes, *tuple),
                }
            }
            Kind::Neg(operand) => {
                let operand = self.node(operand)?;
                // a negative number is a constant too, so lists of them are made once
                match constant(&operand) {
                    Some(Value::Int(i)) if *i != i64::MIN => NodeKind::Const(Value::Int(-i)),
                    Some(Value::Float(f)) => NodeKind::Const(Value::Float(-f)),
                    _ => NodeKind::Neg(Box::new(operand)),
                }
            }
            Kind::Pos(operand) => NodeKind::Pos(Box::new(self.node(operand)?)),
            Kind::Not(operand) => NodeKind::Not(Box::new(self.node(operand)?)),
            Kind::Binary(op, left, right) => NodeKind::Binary(*op, Box::new(self.node(left)?), Box::new(self.node(right)?)),
            Kind::Compare(first, rest) => {
                let first = self.node(first)?;
                let rest = rest.iter().map(|(op, a)| Ok((*op, self.node(a)?))).collect::<Result<Vec<_>, ExprError>>()?;
                NodeKind::Compare(Box::new(first), rest)
            }
            Kind::And(left, right) => NodeKind::And(Box::new(self.node(left)?), Box::new(self.node(right)?)),
            Kind::Or(left, right) => NodeKind::Or(Box::new(self.node(left)?), Box::new(self.node(right)?)),
            Kind::IfElse(body, cond, orelse) => NodeKind::IfElse(Box::new(self.node(body)?), Box::new(self.node(cond)?), Box::new(self.node(orelse)?)),
            Kind::Call(callee, args) => {
                let function = self.callee(callee)?;
                // python counts arguments when it calls, checking here finds it before anything runs
                let count = args.len();
                if count < function.min_args || function.max_args.is_some_and(|max| count > max) {
                    let plural = |n: usize| {
                        if n == 1 {
                            ""
                        } else {
                            "s"
                        }
                    };
                    let takes = match (function.min_args, function.max_args) {
                        (min, Some(max)) if min == max => format!("{} argument{}", min, plural(min)),
                        (min, Some(max)) => format!("{} to {} arguments", min, max),
                        (min, None) => format!("at least {} argument{}", min, plural(min)),
                    };
                    return Err(error(ErrorKind::Arity, span, format!("{}() takes {}, got {}", function.name, takes, count)));
                }
                let args = args.iter().map(|a| self.node(a)).collect::<Result<Vec<_>, _>>()?;
                NodeKind::Call(function, args)
            }
            Kind::Index(seq, index) => NodeKind::Index(Box::new(self.node(seq)?), Box::new(self.node(index)?)),
        };
        Ok(Node {
            kind,
            span,
        })
    }

    fn name(&mut self, name: &str, span: Span) -> Result<NodeKind, ExprError> {
        if let Some(slot) = self.env.vars.iter().position(|v| *v == name) {
            self.used[slot] = true;
            return Ok(NodeKind::Var(slot));
        }
        if let Some((_, value)) = CONSTANTS.iter().find(|(n, _)| *n == name) {
            return Ok(NodeKind::Const(Value::Float(*value)));
        }
        if self.env.function(name).is_some() {
            return Err(error(ErrorKind::Unsupported, span, format!("not supported: functions as values, call it like {}(...)", name)));
        }
        if name == "math" {
            return Err(error(ErrorKind::Unsupported, span, "not supported: math on its own, use math.name"));
        }
        Err(unknown_name(name, span, &self.env.names()))
    }

    fn unknown_math(&self, name: &str, span: Span) -> ExprError {
        let math: Vec<&str> = BUILTINS.iter().filter(|f| f.math).map(|f| f.name).chain(CONSTANTS.iter().map(|(n, _)| *n)).collect();
        match suggest(name, &math) {
            Some(close) => error(ErrorKind::UnknownName, span, format!("unknown name 'math.{}', did you mean 'math.{}'?", name, close)),
            None => error(ErrorKind::UnknownName, span, format!("unknown name 'math.{}'", name)),
        }
    }

    // the function a call calls: a name, or math.name
    fn callee(&mut self, callee: &Ast) -> Result<Function, ExprError> {
        match &callee.kind {
            Kind::Name(name) => {
                if let Some(function) = self.env.function(name) {
                    return Ok(function);
                }
                if self.env.vars.contains(&name.as_str()) || CONSTANTS.iter().any(|(n, _)| n == name) {
                    return Err(error(ErrorKind::Type, callee.span, format!("'{}' is a number, not a function", name)));
                }
                let functions: Vec<&str> = self.env.functions.iter().chain(BUILTINS.iter()).map(|f| f.name).collect();
                Err(unknown_name(name, callee.span, &functions))
            }
            Kind::MathName(name) => match BUILTINS.iter().find(|f| f.math && f.name == name) {
                Some(function) => Ok(*function),
                None if CONSTANTS.iter().any(|(n, _)| n == name) => Err(error(ErrorKind::Type, callee.span, format!("'math.{}' is a number, not a function", name))),
                None => Err(self.unknown_math(name, callee.span)),
            },
            _ => Err(error(ErrorKind::Type, callee.span, "only functions can be called")),
        }
    }
}

// MARK: - Evaluate

impl Program {
    /// whether the expression uses the variable `name`
    pub fn uses(&self, name: &str) -> bool {
        self.vars.iter().position(|v| v == name).is_some_and(|slot| self.used[slot])
    }

    /// the text it was compiled from
    pub fn text(&self) -> &str {
        &self.text
    }

    /// runs the expression with a value for every variable in `Env::vars`, in the same order
    pub fn eval(&self, values: &[Value]) -> Result<Value, ExprError> {
        eval(&self.root, values)
    }

    /// runs the expression and gives a finite number, anything else (NaN, inf, a list, None) is an error
    pub fn eval_number(&self, values: &[Value]) -> Result<f64, ExprError> {
        let value = self.eval(values)?;
        let whole = Span::new(0, self.text.chars().count());
        match value.number() {
            Some(n) if n.is_finite() => Ok(n),
            Some(n) => Err(error(ErrorKind::NotFinite, whole, format!("result is {}, not a finite number", format_float(n)))),
            None => Err(error(ErrorKind::NotANumber, whole, format!("result is {}, not a number", value.type_name()))),
        }
    }
}

fn eval(node: &Node, values: &[Value]) -> Result<Value, ExprError> {
    let span = node.span;
    match &node.kind {
        NodeKind::Const(value) => Ok(value.clone()),
        NodeKind::Var(slot) => values.get(*slot).cloned().ok_or_else(|| error(ErrorKind::UnknownName, span, "variable without a value")),
        NodeKind::List(items, tuple) => {
            let items = items.iter().map(|item| eval(item, values)).collect::<Result<Vec<_>, _>>()?;
            Ok(Value::Seq(items.into(), *tuple))
        }
        NodeKind::Neg(operand) => match eval(operand, values)? {
            Value::Int(i) => i.checked_neg().map(Value::Int).ok_or_else(|| too_large().at(span)),
            Value::Float(f) => Ok(Value::Float(-f)),
            other => Err(type_fail(format!("bad operand type for unary -: '{}'", other.type_name())).at(span)),
        },
        NodeKind::Pos(operand) => match eval(operand, values)? {
            value @ (Value::Int(_) | Value::Float(_)) => Ok(value),
            other => Err(type_fail(format!("bad operand type for unary +: '{}'", other.type_name())).at(span)),
        },
        NodeKind::Not(operand) => Ok(Value::from(!eval(operand, values)?.truthy())),
        NodeKind::Binary(op, left, right) => {
            let a = eval(left, values)?;
            let b = eval(right, values)?;
            binary(*op, &a, &b).map_err(|f| f.at(span))
        }
        NodeKind::Compare(first, rest) => {
            // a < b < c is a < b and b < c, b runs once and the chain stops at the first false
            let mut left = eval(first, values)?;
            for (op, right) in rest {
                let right = eval(right, values)?;
                if !compare(*op, &left, &right).map_err(|f| f.at(span))? {
                    return Ok(Value::from(false));
                }
                left = right;
            }
            Ok(Value::from(true))
        }
        // and / or give one of their operands, like python
        NodeKind::And(left, right) => {
            let a = eval(left, values)?;
            if a.truthy() {
                eval(right, values)
            } else {
                Ok(a)
            }
        }
        NodeKind::Or(left, right) => {
            let a = eval(left, values)?;
            if a.truthy() {
                Ok(a)
            } else {
                eval(right, values)
            }
        }
        // only the branch taken runs, so the other can't fail
        NodeKind::IfElse(body, cond, orelse) => {
            if eval(cond, values)?.truthy() {
                eval(body, values)
            } else {
                eval(orelse, values)
            }
        }
        NodeKind::Call(function, args) => {
            let args = args.iter().map(|arg| eval(arg, values)).collect::<Result<Vec<_>, _>>()?;
            (function.call)(&args).map_err(|f| f.at(span))
        }
        NodeKind::Index(seq, index) => {
            let container = eval(seq, values)?;
            let index = eval(index, values)?;
            subscript(&container, &index).map_err(|f| f.at(span))
        }
    }
}

// container[index], negative indexes count from the end
fn subscript(container: &Value, index: &Value) -> Result<Value, Fail> {
    let (len, name) = match container {
        Value::Seq(items, true) => (items.len() as i64, "tuple"),
        Value::Seq(items, false) => (items.len() as i64, "list"),
        Value::Range(start, stop, step) => (range_len(*start, *stop, *step), "range"),
        other => return Err(type_fail(format!("'{}' object is not subscriptable", other.type_name()))),
    };
    let Value::Int(i) = index else {
        return Err(type_fail(format!("{} indices must be integers, not {}", name, index.type_name())));
    };
    let position = if *i < 0 {
        i + len
    } else {
        *i
    };
    if position < 0 || position >= len {
        return Err(Fail::new(ErrorKind::Index, format!("{} index out of range (index {}, length {})", name, i, len)));
    }
    match container {
        Value::Seq(items, _) => Ok(items[position as usize].clone()),
        Value::Range(start, _, step) => Ok(Value::Int(start + position * step)),
        _ => Err(type_fail("not subscriptable")),
    }
}

/// checks `text` against `vars` without running it, for the frontend while typing. `None` when it compiles
pub fn check(text: &str, vars: &[&str]) -> Option<ExprError> {
    compile(text, &Env::new(vars)).err()
}
