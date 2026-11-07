// run from /src-tauri
// cargo test --test expr_test
//
// the expression language (src/utils/expr.rs): the corpus of expressions and what python gives for them, the cases users
// already write, every error kind, and random text that must never make it panic

use motionkeys::utils::expr::{check, compile, Env, ErrorKind, ExprError, Function, Value, MAX_DEPTH, MAX_LEN};

// the corpus's variables, in the order they're given
const VARS: [&str; 6] = ["x", "y", "note", "vel", "i", "z"];

fn values() -> Vec<Value> {
    vec![Value::Float(12.5), Value::Float(0.8), Value::Int(46), Value::Int(100), Value::Int(-3), Value::Int(0)]
}

// runs `text` with the corpus variables
fn run(text: &str) -> Result<f64, ExprError> {
    compile(text, &Env::new(&VARS))?.eval_number(&values())
}

fn num(text: &str) -> f64 {
    run(text).unwrap_or_else(|e| panic!("{} failed: {}", text, e))
}

fn kind(text: &str) -> ErrorKind {
    match run(text) {
        Ok(v) => panic!("{} gave {} instead of an error", text, v),
        Err(e) => e.kind,
    }
}

// the kind's name as the corpus writes it
fn kind_name(kind: ErrorKind) -> String {
    serde_json::to_value(kind).unwrap().as_str().unwrap().to_string()
}

// MARK: - Corpus

#[test]
fn corpus() {
    let corpus = std::fs::read_to_string("tests/fixtures/expr_corpus.txt").unwrap();
    let mut failures = vec![];
    let mut count = 0;
    for line in corpus.lines() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let at = line.rfind("=> ").unwrap_or_else(|| panic!("no ' => ' in corpus line: {}", line));
        let (text, expected) = (line[..at].trim_end(), line[at + 3..].trim());
        count += 1;

        let program = compile(text, &Env::new(&VARS));
        let got = program.and_then(|p| {
            // ints and floats are told apart, 3 and 3.0 are different results
            let value = p.eval(&values())?;
            p.eval_number(&values())?;
            Ok(value)
        });
        let matches = match (expected.strip_prefix("error "), &got) {
            (Some(kind), Err(e)) => kind_name(e.kind) == kind,
            (Some(_), Ok(_)) => false,
            (None, Ok(Value::Int(i))) => !expected.contains(['.', 'e']) && expected.parse::<i64>().ok() == Some(*i),
            (None, Ok(Value::Float(f))) => expected.contains(['.', 'e']) && expected.parse::<f64>().is_ok_and(|want| want == *f || (want - f).abs() <= 1e-15 * want.abs()),
            (None, _) => false,
        };
        if !matches {
            let got = match got {
                Ok(v) => format!("{} ({})", v, v.type_name()),
                Err(e) => format!("error {} ({})", kind_name(e.kind), e.message),
            };
            failures.push(format!("{}  =>  expected {}, got {}", text, expected, got));
        }
    }
    assert!(count > 250, "the corpus has only {} expressions", count);
    assert!(failures.is_empty(), "{} of {} corpus expressions changed:\n{}", failures.len(), count, failures.join("\n"));
}

// MARK: - Python semantics

#[test]
fn precedence() {
    assert_eq!(num("1 + 2 * 3"), 7.0);
    assert_eq!(num("2 + 3 * 4 ** 2"), 50.0);
    assert_eq!(num("10 - 4 - 3"), 3.0);
    // ** binds tighter than unary minus and is right-associative
    assert_eq!(num("-2 ** 2"), -4.0);
    assert_eq!(num("2 ** 3 ** 2"), 512.0);
    assert_eq!(num("2 ** -1"), 0.5);
    // not binds looser than comparisons
    assert_eq!(num("not 1 == 2"), 1.0);
    // and binds tighter than or
    assert_eq!(num("1 or 0 and 0"), 1.0);
}

#[test]
fn chained_comparisons() {
    assert_eq!(num("36 <= note < 60"), 1.0);
    assert_eq!(num("36 <= note < 46"), 0.0);
    assert_eq!(num("1 < 2 > 0"), 1.0);
    // 46 == True is false, like python
    assert_eq!(num("note == 46 == True"), 0.0);
    // the middle runs once and the chain stops at the first false, so the error after it never happens
    assert_eq!(num("1 > 2 > [][0]"), 0.0);
}

#[test]
fn ternaries() {
    assert_eq!(num("1 if 0 else 2 if 0 else 3"), 3.0);
    assert_eq!(num("1 if 0 else 2 if 1 else 3"), 2.0);
    assert_eq!(num("x if x > 10 else x * 2 if x > 5 else 0"), 12.5);
    // only the branch taken runs
    assert_eq!(num("[1, 2][note] if note < 2 else 7"), 7.0);
}

#[test]
fn and_or_give_an_operand() {
    assert_eq!(num("0 or 2"), 2.0);
    assert_eq!(num("y and note"), 46.0);
    assert_eq!(num("z or x"), 12.5);
    assert!(matches!(compile("1 and 2", &Env::new(&[])).unwrap().eval(&[]).unwrap(), Value::Int(2)));
}

#[test]
fn modulo_floor_power_round_match_python() {
    // % takes the sign of the divisor, // floors
    assert_eq!(num("-7 % 3"), 2.0);
    assert_eq!(num("7 % -3"), -2.0);
    assert_eq!(num("-7.5 % 2"), 0.5);
    assert_eq!(num("-7 // 2"), -4.0);
    assert_eq!(num("7 // -2"), -4.0);
    assert_eq!(num("1 // 0.1"), 9.0);
    assert_eq!(num("5 % 0.1"), 0.09999999999999973);
    // banker's rounding, and ndigits rounds the exact value the float holds
    assert_eq!(num("round(2.5)"), 2.0);
    assert_eq!(num("round(3.5)"), 4.0);
    assert_eq!(num("round(-2.5)"), -2.0);
    assert_eq!(num("round(2.675, 2)"), 2.67);
    assert_eq!(num("round(0.125, 2)"), 0.12);
    assert_eq!(num("round(1250, -2)"), 1200.0);
    assert_eq!(num("round(1350, -2)"), 1400.0);
    assert_eq!(num("(-2.0) ** 3"), -8.0);
}

#[test]
fn what_users_already_write() {
    let env = Env::new(&["x", "y", "note", "vel"]);
    let run = |text: &str, x: f64, y: f64, note: i64, vel: i64| compile(text, &env).unwrap().eval_number(&[Value::Float(x), Value::Float(y), Value::Int(note), Value::Int(vel)]).unwrap();
    // the cymbal choke: the choke note's keys go to 0
    assert_eq!(run("y if note == 46 else 0", 3.0, 0.7, 46, 100), 0.7);
    assert_eq!(run("y if note == 46 else 0", 3.0, 0.7, 49, 100), 0.0);
    assert_eq!(run("x * 2", 3.0, 0.7, 46, 100), 6.0);
    assert_eq!(run("y * vel / 127", 3.0, 1.0, 46, 127), 1.0);
    assert_eq!(run("y * vel / 127", 3.0, 0.5, 46, 0), 0.0);
}

// MARK: - Errors

#[test]
fn every_error_kind() {
    assert_eq!(kind(&"1+".repeat(MAX_LEN)), ErrorKind::TooLong);
    assert_eq!(kind(&format!("{}1{}", "(".repeat(MAX_DEPTH + 5), ")".repeat(MAX_DEPTH + 5))), ErrorKind::TooDeep);
    assert_eq!(kind("1 +"), ErrorKind::Syntax);
    assert_eq!(kind("'text'"), ErrorKind::Unsupported);
    assert_eq!(kind("nte"), ErrorKind::UnknownName);
    assert_eq!(kind("abs(1, 2)"), ErrorKind::Arity);
    assert_eq!(kind("note[0]"), ErrorKind::Type);
    assert_eq!(kind("1 / z"), ErrorKind::ZeroDivision);
    assert_eq!(kind("[1, 2][note]"), ErrorKind::Index);
    assert_eq!(kind("sqrt(-1)"), ErrorKind::Domain);
    assert_eq!(kind("exp(1000)"), ErrorKind::Overflow);
    assert_eq!(kind("inf - inf"), ErrorKind::NotFinite);
    assert_eq!(kind("[1, 2]"), ErrorKind::NotANumber);
}

#[test]
fn errors_name_what_and_where() {
    // did you mean, with the characters of the name
    let err = check("y if nte == 46 else 0", &["x", "y", "note", "vel"]).unwrap();
    assert_eq!(err.message, "unknown name 'nte', did you mean 'note'?");
    assert_eq!((err.start, err.end), (5, 8));
    assert_eq!(check("y * vle", &["y", "vel"]).unwrap().message, "unknown name 'vle', did you mean 'vel'?");
    assert_eq!(check("sinn(x)", &["x"]).unwrap().message, "unknown name 'sinn', did you mean 'sin'?");
    assert_eq!(check("math.sinn(x)", &["x"]).unwrap().message, "unknown name 'math.sinn', did you mean 'math.sin'?");
    // nothing close, no suggestion
    assert_eq!(check("banana", &["x"]).unwrap().message, "unknown name 'banana'");

    // python syntax that isn't supported says so
    assert_eq!(check("lambda: 1", &[]).unwrap().message, "not supported: lambda");
    assert_eq!(check("[n for n in range(3)]", &[]).unwrap().message, "not supported: comprehensions");
    assert_eq!(check("x.real", &["x"]).unwrap().message, "not supported: attribute access (only math.name)");
    assert_eq!(check("x = 1", &["x"]).unwrap().message, "not supported: assignment");
    assert_eq!(check("'a'", &[]).unwrap().message, "not supported: strings");

    // a run-time error points at the operation that failed
    let program = compile("y + 1 / (note - 46)", &Env::new(&["y", "note"])).unwrap();
    let err = program.eval(&[Value::Float(1.0), Value::Int(46)]).unwrap_err();
    assert_eq!(err.kind, ErrorKind::ZeroDivision);
    assert_eq!((err.start, err.end), (4, 19));
    assert!(program.eval(&[Value::Float(1.0), Value::Int(47)]).is_ok());

    // an index error in a folded list of constants
    let err = compile("[1, -2, 3][note]", &Env::new(&["note"])).unwrap().eval(&[Value::Int(5)]).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Index);
    assert_eq!((err.start, err.end), (0, 16));
}

#[test]
fn compile_knows_the_variables_used() {
    let program = compile("y * vel / 127", &Env::new(&["x", "y", "vel"])).unwrap();
    assert!(program.uses("y") && program.uses("vel"));
    assert!(!program.uses("x"));
    assert!(!program.uses("nothing"));
}

#[test]
fn callers_add_functions() {
    fn twice(args: &[Value]) -> Result<Value, motionkeys::utils::expr::Fail> {
        Ok(Value::Float(args[0].number().unwrap_or(0.0) * 2.0))
    }
    let functions = [Function::new("twice", 1, Some(1), twice)];
    let env = Env::new(&["x"]).with_functions(&functions);
    assert_eq!(compile("twice(x) + 1", &env).unwrap().eval_number(&[Value::Float(3.0)]).unwrap(), 7.0);
    assert_eq!(compile("twise(x)", &env).unwrap_err().message, "unknown name 'twise', did you mean 'twice'?");
}

#[test]
fn long_ranges_are_never_built() {
    // in, len, sum, min and max of a huge range answer right away
    assert_eq!(num("note in range(0, 10 ** 18)"), 1.0);
    assert_eq!(num("len(range(0, 10 ** 18, 7))"), 142857142857142858.0);
    assert_eq!(num("max(range(0, 10 ** 18))"), 999999999999999999.0);
    assert_eq!(kind("sum(range(0, 10 ** 18))"), ErrorKind::Overflow);
}

#[test]
fn deep_but_flat_chains_are_limited() {
    // a left-leaning chain doesn't recurse in the parser, it's still limited so running it can't overflow the stack
    let chain = vec!["1"; MAX_DEPTH + 10].join(" + ");
    assert_eq!(kind(&chain), ErrorKind::TooDeep);
    let ok = vec!["1"; MAX_DEPTH / 2].join(" + ");
    assert_eq!(num(&ok), (MAX_DEPTH / 2) as f64);
}

// MARK: - Fuzz

// a small xorshift so the fuzz test is repeatable without a dependency
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[(self.next() % items.len() as u64) as usize]
    }
}

#[test]
fn fuzz_never_panics() {
    // pieces of expressions, so a lot of the text gets deep into the parser
    let pieces = ["x", "y", "note", "vel", "i", "z", "nte", "0", "1", "2.5", "-", "+", "*", "/", "//", "%", "**", "(", ")", "[", "]", ",", ".", "math", "math.", "sin", "round", "range", "min", "max", "sum", "len", "abs", "int", "clamp", "lerp", "if", "else", "and", "or", "not", "in", "is", "lambda", "for", "==", "!=", "<", "<=", ">", ">=", "=", ":", "'", "\"", "{", "}", "1e", "0x", "1_", "inf", "pi", "None", "True", "False", " ", "#", "\n", "~", "&", "@", ";", "é", "\u{0}", "999999999999999999999", "1e400", "-9223372036854775808"];
    let mut rng = Rng(0x9E3779B97F4A7C15);
    for round in 0..200_000 {
        let text: String = if round % 10 == 0 {
            // raw characters now and then
            (0..rng.next() % 40).map(|_| char::from_u32((rng.next() % 0x2FF) as u32).unwrap_or('?')).collect()
        } else {
            (0..rng.next() % 16).map(|_| rng.pick(&pieces)).collect::<Vec<_>>().join(if rng.next() % 2 == 0 {
                " "
            } else {
                ""
            })
        };
        if let Ok(program) = compile(&text, &Env::new(&VARS)) {
            let _ = program.eval_number(&values());
            let _ = program.eval(&[Value::Int(i64::MIN), Value::Float(f64::NAN), Value::Int(i64::MAX), Value::Float(f64::INFINITY), Value::Int(-1), Value::Float(-0.0)]);
        }
    }
}

#[test]
fn deeply_nested_junk_is_an_error() {
    // pasted junk can't hang or overflow the stack
    for text in ["(".repeat(5000), "-".repeat(5000) + "1", "not ".repeat(5000) + "1", "[".repeat(5000), "2 ** ".repeat(5000) + "2", "1 if 1 else ".repeat(3000) + "1"] {
        assert!(compile(&text, &Env::new(&VARS)).is_err(), "{}...", &text[..20]);
    }
}
