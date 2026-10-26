//! The life file's TOML subset: what it reads, and the exact error (with its line) for what it
//! rejects. Every error path in src/toml.rs is exercised here.

use spin_life::toml::{parse, Error, Value};

#[track_caller]
fn ok(src: &str) -> spin_life::toml::Table {
    parse(src).unwrap_or_else(|e| panic!("{src:?} should parse: {e}"))
}

#[track_caller]
fn rejects(src: &str, line: usize, says: &str) {
    match parse(src) {
        Ok(t) => panic!("{src:?} should be rejected, parsed {t:?}"),
        Err(e) => {
            assert!(e.msg.contains(says), "{src:?}: `{}` doesn't mention `{says}`", e.msg);
            assert_eq!(e.line, line, "{src:?}: {e}");
        }
    }
}

fn s(v: &str) -> Value {
    Value::Str(v.into())
}

#[test]
fn values() {
    let t = ok(
        "a = \"x\"\nb = 'lit\\eral'\nc = 42\nd = -1_000\ne = 2.5\nf = true\ng = false\nh = [1, 2,]\ni = {}\n",
    );
    assert_eq!(t["a"], s("x"));
    assert_eq!(t["b"], s("lit\\eral")); // literal strings keep backslashes
    assert_eq!(t["c"], Value::Int(42));
    assert_eq!(t["d"], Value::Int(-1000));
    assert_eq!(t["e"], Value::Float(2.5));
    assert_eq!(t["f"], Value::Bool(true));
    assert_eq!(t["g"], Value::Bool(false));
    assert_eq!(t["h"], Value::Array(vec![Value::Int(1), Value::Int(2)]));
    assert_eq!(t["i"], Value::Table(Default::default()));
}

#[test]
fn escapes() {
    // Built with concat! so the \u escape reaches the parser as six ASCII characters.
    let src = concat!("a = \"tab\\there\\nquote\\\" slash\\\\ snow\\", "u2603\"");
    assert!(src.is_ascii(), "{src}");
    let t = ok(src);
    assert_eq!(t["a"], s("tab\there\nquote\" slash\\ snow\u{2603}"));
}

#[test]
fn keys_tables_and_line_endings() {
    let src = "# comment\r\n[a.\"quoted key\"]\r\nx = 1 # trailing\r\n'lit' = 2\r\n[[runs]]\r\nn = 1\r\n[[runs]]\r\nn = 2\r\n[runs.inner]\r\ny = { p = 1, q.r = 'z' }\r\nmulti = [\r\n  1, # one\r\n  2\r\n]\r\n";
    let t = ok(src);
    let Value::Table(a) = &t["a"] else { panic!() };
    let Value::Table(q) = &a["quoted key"] else { panic!() };
    assert_eq!(q["x"], Value::Int(1));
    assert_eq!(q["lit"], Value::Int(2));
    let Value::Array(runs) = &t["runs"] else { panic!() };
    assert_eq!(runs.len(), 2);
    let Value::Table(second) = &runs[1] else { panic!() };
    let Value::Table(inner) = &second["inner"] else { panic!() };
    let Value::Table(y) = &inner["y"] else { panic!() };
    assert_eq!(y["p"], Value::Int(1));
    let Value::Table(qq) = &y["q"] else { panic!() };
    assert_eq!(qq["r"], s("z"));
    assert_eq!(inner["multi"], Value::Array(vec![Value::Int(1), Value::Int(2)]));
}

#[test]
fn errors_name_the_line() {
    rejects("a = 1 2", 1, "unexpected `2` after a value");
    rejects("a = 1\rb = 2", 1, "after a value"); // a CR on its own doesn't end a line
    rejects("[a\nx = 1", 1, "expected `]` to close the header");
    rejects("[[a]\n", 1, "expected `]]` to close the header");
    rejects("a = 1\n[[a]]", 2, "`a` is an integer, not an array of tables");
    rejects("a = 1\n[a.b]", 2, "`a` is an integer, not a table");
    rejects("[a]\nx = 1\n[[a.x]]\n", 3, "`x` is an integer, not an array of tables");
    rejects("[a]\n[[a]]\n", 2, "`a` is a table, not an array of tables");
    rejects("a.b = []\n[a.b.c]", 2, "`b` is an array, not a table");
    rejects("a 1", 1, "expected `=` after `a`");
    rejects("= 1", 1, "expected a key");
    rejects("a.", 1, "expected a key");
    rejects("a =", 1, "missing value");
    rejects("a = @", 1, "unexpected `@` where a value should be");
    rejects("a = \"open", 1, "unterminated string");
    rejects("a = \"open\nb = 1", 1, "unterminated string");
    rejects("a = 'open", 1, "unterminated string");
    rejects(r#"a = "\q""#, 1, "unknown escape in string");
    rejects(r#"a = "\uZZZZ""#, 1, "bad \\u escape");
    rejects(r#"a = "\uD800""#, 1, "bad \\u escape"); // a surrogate is not a char
    rejects(r#"a = "\u12""#, 1, "bad \\u escape");
    rejects("a = tru", 1, "expected `true` or `false` (strings need quotes)");
    rejects("a = 06:30", 1, "times need quotes: \"06:30\"");
    rejects("a = 1.2.3", 1, "`1.2.3` is not a number");
    rejects("a = [1 2]", 1, "expected `,` or `]` in array");
    rejects("a = {x = 1\n}", 1, "expected `,` or `}` in inline table");
    rejects("a = {x 1}", 1, "expected `=` after `x`");
    rejects("a = {x = 1, x = 2}", 1, "`x` is defined twice");
    rejects("a = 1\na = 2", 2, "`a` is defined twice");
}

#[test]
fn errors_display_with_their_line() {
    let e = Error { line: 7, msg: "boom".into() };
    assert_eq!(e.to_string(), "line 7: boom");
    let boxed: Box<dyn std::error::Error> = Box::new(e);
    assert_eq!(boxed.to_string(), "line 7: boom");
}

#[test]
fn kinds() {
    let kinds: Vec<&str> = [
        s(""),
        Value::Int(0),
        Value::Float(0.0),
        Value::Bool(true),
        Value::Array(vec![]),
        Value::Table(Default::default()),
    ]
    .iter()
    .map(Value::kind)
    .collect();
    assert_eq!(kinds, ["a string", "an integer", "a number", "a boolean", "an array", "a table"]);
}
