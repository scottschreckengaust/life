//! Just enough JSON writing for `--format json`, without a dependency.
//!
//! Output is compact and deterministic: the same journal always gives the same bytes.

use std::fmt::Write;

/// Write `s` as a quoted JSON string. Control characters are escaped; everything else,
/// emoji included, is written as UTF-8.
pub fn string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Write `x` rounded to 3 decimal places: `8.0` is `8`, `7.25` is `7.25`, `1/3` is `0.333`.
pub fn number(out: &mut String, x: f64) {
    debug_assert!(x.is_finite(), "{x} isn't a JSON number");
    let x = if x.is_finite() { (x * 1000.0).round() / 1000.0 } else { 0.0 };
    // `+ 0.0` turns -0.0 into 0.0.
    let _ = write!(out, "{}", x + 0.0);
}

/// Writes `"key":` (with a leading comma when it isn't the first member) and hands back
/// `out` for the value.
#[derive(Debug)]
pub struct Obj<'a> {
    out: &'a mut String,
    first: bool,
}

impl<'a> Obj<'a> {
    /// Members appended to an object whose `{` (and maybe some members) are already in `out`.
    pub fn continuing(out: &'a mut String, first: bool) -> Self {
        Obj { out, first }
    }

    /// The writer for the value of `key`.
    pub fn key(&mut self, key: &str) -> &mut String {
        if !std::mem::take(&mut self.first) {
            self.out.push(',');
        }
        string(self.out, key);
        self.out.push(':');
        self.out
    }

    /// `"key":"v"`.
    pub fn str(&mut self, key: &str, v: &str) {
        let out = self.key(key);
        string(out, v);
    }

    /// `"key":v`, by the [`number`] rule.
    pub fn num(&mut self, key: &str, v: f64) {
        let out = self.key(key);
        number(out, v);
    }

    /// `"key":v`, for integers.
    pub fn int(&mut self, key: &str, v: impl std::fmt::Display) {
        let out = self.key(key);
        out.push_str(&v.to_string());
    }

    /// `"key":true` or `false`.
    pub fn bool(&mut self, key: &str, v: bool) {
        self.key(key).push_str(if v { "true" } else { "false" });
    }

    /// `"key":null`.
    pub fn null(&mut self, key: &str) {
        self.key(key).push_str("null");
    }
}

/// Write `{` … `}` around whatever `f` adds.
pub fn object(out: &mut String, f: impl FnOnce(&mut Obj)) {
    out.push('{');
    f(&mut Obj::continuing(out, true));
    out.push('}');
}

/// Write `[` … `]`, one element per item.
pub fn array<T>(out: &mut String, items: impl IntoIterator<Item = T>, mut f: impl FnMut(&mut String, T)) {
    out.push('[');
    for (i, item) in items.into_iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        f(out, item);
    }
    out.push(']');
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> String {
        let mut out = String::new();
        string(&mut out, v);
        out
    }

    fn n(v: f64) -> String {
        let mut out = String::new();
        number(&mut out, v);
        out
    }

    #[test]
    fn strings_escape_what_json_requires() {
        assert_eq!(s(r#"say "hi""#), r#""say \"hi\"""#);
        assert_eq!(s(r"C:\life"), r#""C:\\life""#);
        assert_eq!(s("a\nb\tc\r"), r#""a\nb\tc\r""#);
        assert_eq!(s("\u{1}"), r#""\u0001""#);
        assert_eq!(s("\u{1f}"), r#""\u001f""#);
    }

    #[test]
    fn emoji_pass_through() {
        assert_eq!(s("⏰ 🌍 “quotes”"), "\"⏰ 🌍 “quotes”\"");
    }

    #[test]
    fn numbers_round_to_three_places() {
        assert_eq!(n(8.0), "8");
        assert_eq!(n(7.25), "7.25");
        assert_eq!(n(1.0 / 3.0), "0.333");
        assert_eq!(n(2.0 / 3.0), "0.667");
        assert_eq!(n(-0.0), "0");
        assert_eq!(n(-0.0001), "0");
        assert_eq!(n(86_400_000.0), "86400000");
        assert_eq!(n(0.001), "0.001");
    }

    #[test]
    fn objects_and_arrays() {
        let mut out = String::new();
        object(&mut out, |o| {
            o.str("a", "x");
            o.int("b", 2);
            o.bool("c", true);
            o.null("d");
            let out = o.key("e");
            array(out, [1.5, 2.0], number);
        });
        assert_eq!(out, r#"{"a":"x","b":2,"c":true,"d":null,"e":[1.5,2]}"#);
    }
}
