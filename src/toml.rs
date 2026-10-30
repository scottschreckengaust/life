//! Just enough TOML for a life file, with zero dependencies.
//!
//! Supported: comments, `key = value`, dotted and quoted keys, `[table]`, `[[array.of.tables]]`,
//! strings (basic with escapes, and literal), integers, floats, booleans, arrays (multi-line),
//! and inline tables. Not supported: dates, multi-line strings. Errors carry a line number.

use std::{collections::BTreeMap, fmt};

/// A parsed value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Array(Vec<Value>),
    Table(Table),
}

/// Keys in a stable order.
pub type Table = BTreeMap<String, Value>;

/// A parse error, with the line it happened on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub line: usize,
    pub msg: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.msg)
    }
}

impl std::error::Error for Error {}

impl Value {
    pub fn kind(&self) -> &'static str {
        match self {
            Value::Str(_) => "a string",
            Value::Int(_) => "an integer",
            Value::Float(_) => "a number",
            Value::Bool(_) => "a boolean",
            Value::Array(_) => "an array",
            Value::Table(_) => "a table",
        }
    }
}

/// Parse a document into its root table.
pub fn parse(src: &str) -> Result<Table, Error> {
    Parser { s: src.as_bytes(), i: 0, line: 1, depth: 0 }.document()
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
    line: usize,
    /// How many arrays and inline tables we're inside.
    depth: usize,
}

/// Arrays and inline tables nest at most this deep. The parser recurses once per level, so
/// without a limit a file of 100 000 `[`s overflowed the stack and aborted the process.
const MAX_DEPTH: usize = 64;

impl Parser<'_> {
    fn err<T>(&self, msg: impl Into<String>) -> Result<T, Error> {
        Err(Error { line: self.line, msg: msg.into() })
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let c = self.peek()?;
        self.i += 1;
        if c == b'\n' {
            self.line += 1;
        }
        Some(c)
    }

    /// Spaces and tabs only.
    fn blank(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t')) {
            self.i += 1;
        }
    }

    /// Spaces, newlines and comments (inside arrays).
    fn gap(&mut self) {
        loop {
            match self.peek() {
                Some(b' ' | b'\t' | b'\r' | b'\n') => {
                    self.bump();
                }
                Some(b'#') => self.comment(),
                _ => return,
            }
        }
    }

    fn comment(&mut self) {
        while !matches!(self.peek(), None | Some(b'\n')) {
            self.i += 1;
        }
    }

    /// Whatever is left on the line must be blank or a comment.
    fn end_of_line(&mut self) -> Result<(), Error> {
        self.blank();
        if self.peek() == Some(b'#') {
            self.comment();
        }
        match self.peek() {
            None => Ok(()),
            Some(b'\n') => {
                self.bump();
                Ok(())
            }
            Some(b'\r') if self.s.get(self.i + 1) == Some(&b'\n') => {
                self.bump();
                self.bump();
                Ok(())
            }
            Some(c) => self.err(format!("unexpected `{}` after a value", c as char)),
        }
    }

    fn document(mut self) -> Result<Table, Error> {
        let mut root = Table::new();
        let mut path: Vec<String> = Vec::new();
        loop {
            self.gap();
            match self.peek() {
                None => return Ok(root),
                Some(b'[') => {
                    self.bump();
                    let array = self.peek() == Some(b'[');
                    if array {
                        self.bump();
                    }
                    self.blank();
                    path = self.key()?;
                    self.blank();
                    let close = if array { "]]" } else { "]" };
                    for _ in 0..close.len() {
                        // Look before taking: a newline here would move the error to the next line.
                        if self.peek() != Some(b']') {
                            return self.err(format!("expected `{close}` to close the header"));
                        }
                        self.bump();
                    }
                    // Errors about this header belong to its line, not the one end_of_line moves to.
                    let line = self.line;
                    self.end_of_line()?;
                    let fail = |msg: String| Error { line, msg };
                    if array {
                        let (last, parents) = path.split_last().expect("key is never empty");
                        let parent = open(&mut root, parents).map_err(fail)?;
                        match parent.entry(last.clone()).or_insert_with(|| Value::Array(Vec::new())) {
                            Value::Array(items) => items.push(Value::Table(Table::new())),
                            other => {
                                return Err(fail(format!(
                                    "`{last}` is {}, not an array of tables",
                                    other.kind()
                                )))
                            }
                        }
                    } else {
                        open(&mut root, &path).map_err(fail)?;
                    }
                }
                Some(_) => {
                    let key = self.key()?;
                    self.blank();
                    if self.peek() != Some(b'=') {
                        return self.err(format!("expected `=` after `{}`", key.join(".")));
                    }
                    self.bump();
                    self.blank();
                    let value = self.value()?;
                    let line = self.line;
                    self.end_of_line()?;
                    let table = open(&mut root, &path).map_err(|msg| Error { line, msg })?;
                    insert(table, &key, value).map_err(|msg| Error { line, msg })?;
                }
            }
        }
    }

    fn key(&mut self) -> Result<Vec<String>, Error> {
        let mut parts = Vec::new();
        loop {
            self.blank();
            let part = match self.peek() {
                Some(b'"') => self.basic_string()?,
                Some(b'\'') => self.literal_string()?,
                _ => {
                    let start = self.i;
                    while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
                    {
                        self.i += 1;
                    }
                    if start == self.i {
                        return self.err("expected a key");
                    }
                    String::from_utf8_lossy(&self.s[start..self.i]).into_owned()
                }
            };
            parts.push(part);
            self.blank();
            if self.peek() != Some(b'.') {
                return Ok(parts);
            }
            self.bump();
        }
    }

    fn value(&mut self) -> Result<Value, Error> {
        match self.peek() {
            Some(b'"') => self.basic_string().map(Value::Str),
            Some(b'\'') => self.literal_string().map(Value::Str),
            Some(b'[' | b'{') if self.depth == MAX_DEPTH => {
                self.err(format!("arrays and inline tables nest at most {MAX_DEPTH} deep"))
            }
            Some(b'[') => self.nested(Self::array),
            Some(b'{') => self.nested(Self::inline_table),
            Some(b't' | b'f') => self.boolean(),
            Some(c) if c.is_ascii_digit() || c == b'-' || c == b'+' => self.number(),
            Some(c) => self.err(format!("unexpected `{}` where a value should be", c as char)),
            None => self.err("missing value"),
        }
    }

    fn nested(&mut self, parse: fn(&mut Self) -> Result<Value, Error>) -> Result<Value, Error> {
        self.depth += 1;
        let v = parse(self);
        self.depth -= 1;
        v
    }

    fn basic_string(&mut self) -> Result<String, Error> {
        self.bump();
        let mut out = Vec::new();
        loop {
            // Look before taking, so the error is on the string's own line.
            let c = match self.peek() {
                None | Some(b'\n') => return self.err("unterminated string"),
                Some(c) => c,
            };
            self.i += 1; // not a newline, so still the same line
            match c {
                b'"' => break,
                b'\\' => match self.bump() {
                    Some(b'n') => out.push(b'\n'),
                    Some(b't') => out.push(b'\t'),
                    Some(b'"') => out.push(b'"'),
                    Some(b'\\') => out.push(b'\\'),
                    Some(b'u') => {
                        let hex = self.s.get(self.i..self.i + 4).unwrap_or_default();
                        let ch = std::str::from_utf8(hex)
                            .ok()
                            .and_then(|h| u32::from_str_radix(h, 16).ok())
                            .and_then(char::from_u32);
                        match ch {
                            Some(ch) => {
                                self.i += 4;
                                out.extend_from_slice(ch.encode_utf8(&mut [0; 4]).as_bytes());
                            }
                            None => return self.err("bad \\u escape"),
                        }
                    }
                    _ => return self.err("unknown escape in string"),
                },
                c => out.push(c),
            }
        }
        String::from_utf8(out).or_else(|_| self.err("string is not valid UTF-8"))
    }

    fn literal_string(&mut self) -> Result<String, Error> {
        self.bump();
        let start = self.i;
        loop {
            if matches!(self.peek(), None | Some(b'\n')) {
                return self.err("unterminated string");
            }
            if self.bump() == Some(b'\'') {
                break;
            }
        }
        String::from_utf8(self.s[start..self.i - 1].to_vec())
            .or_else(|_| self.err("string is not valid UTF-8"))
    }

    fn boolean(&mut self) -> Result<Value, Error> {
        for (word, v) in [("true", true), ("false", false)] {
            if self.s[self.i..].starts_with(word.as_bytes()) {
                self.i += word.len();
                return Ok(Value::Bool(v));
            }
        }
        self.err("expected `true` or `false` (strings need quotes)")
    }

    fn number(&mut self) -> Result<Value, Error> {
        let start = self.i;
        while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b'+' | b'_' | b':'))
        {
            self.i += 1;
        }
        let text: String = String::from_utf8_lossy(&self.s[start..self.i]).replace('_', "");
        if text.contains(':') {
            return self.err(format!("times need quotes: \"{text}\""));
        }
        if let Ok(n) = text.parse::<i64>() {
            return Ok(Value::Int(n));
        }
        match text.parse::<f64>() {
            Ok(n) => Ok(Value::Float(n)),
            Err(_) => self.err(format!("`{text}` is not a number")),
        }
    }

    fn array(&mut self) -> Result<Value, Error> {
        self.bump();
        let mut items = Vec::new();
        loop {
            self.gap();
            if self.peek() == Some(b']') {
                self.bump();
                return Ok(Value::Array(items));
            }
            items.push(self.value()?);
            self.gap();
            match self.bump() {
                Some(b',') => {}
                Some(b']') => return Ok(Value::Array(items)),
                _ => return self.err("expected `,` or `]` in array"),
            }
        }
    }

    fn inline_table(&mut self) -> Result<Value, Error> {
        self.bump();
        let mut table = Table::new();
        self.blank();
        if self.peek() == Some(b'}') {
            self.bump();
            return Ok(Value::Table(table));
        }
        loop {
            let key = self.key()?;
            self.blank();
            if self.peek() != Some(b'=') {
                return self.err(format!("expected `=` after `{}`", key.join(".")));
            }
            self.bump();
            self.blank();
            let value = self.value()?;
            insert(&mut table, &key, value).or_else(|m| self.err(m))?;
            self.blank();
            match self.peek() {
                Some(b',') => {
                    self.bump();
                    self.blank();
                }
                Some(b'}') => {
                    self.bump();
                    return Ok(Value::Table(table));
                }
                _ => return self.err("expected `,` or `}` in inline table (inline tables stay on one line)"),
            }
        }
    }
}

/// Walk to (creating as needed) the table at `path`. For arrays of tables, the last element.
fn open<'t>(root: &'t mut Table, path: &[String]) -> Result<&'t mut Table, String> {
    let mut table = root;
    for part in path {
        let next = table.entry(part.clone()).or_insert_with(|| Value::Table(Table::new()));
        table = match next {
            Value::Table(t) => t,
            Value::Array(items) => match items.last_mut() {
                Some(Value::Table(t)) => t,
                _ => return Err(format!("`{part}` is an array, not a table")),
            },
            other => return Err(format!("`{part}` is {}, not a table", other.kind())),
        };
    }
    Ok(table)
}

fn insert(table: &mut Table, key: &[String], value: Value) -> Result<(), String> {
    let (last, parents) = key.split_last().expect("key is never empty");
    let table = open(table, parents)?;
    if table.contains_key(last) {
        return Err(format!("`{}` is defined twice", key.join(".")));
    }
    table.insert(last.clone(), value);
    Ok(())
}
