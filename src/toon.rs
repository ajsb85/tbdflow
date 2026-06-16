//! A small, self-contained TOON (Token-Oriented Object Notation) encoder.
//!
//! There is no Rust TOON crate, and the reference implementation in the `toon/`
//! folder is JavaScript/TypeScript. This module implements the subset of the
//! [TOON spec](../../toon/SPEC.md) that `tbdflow` needs to emit structured,
//! token-efficient output for AI agents:
//!
//! - objects (`key: value`, two-space indent)
//! - primitive arrays (`key[N]: a,b,c`)
//! - tabular arrays of uniform objects (`key[N]{f1,f2}:` + rows)
//! - the canonical quoting rules
//!
//! Only encoding is implemented (we never need to parse TOON).

/// An ordered, JSON-like value that can be serialised to TOON.
#[derive(Debug, Clone, PartialEq)]
pub enum Toon {
    Str(String),
    Int(i64),
    Bool(bool),
    Null,
    Arr(Vec<Toon>),
    /// Ordered key/value pairs (insertion order is preserved on output).
    Obj(Vec<(String, Toon)>),
}

impl Toon {
    pub fn str<S: Into<String>>(s: S) -> Toon {
        Toon::Str(s.into())
    }

    pub fn arr(items: Vec<Toon>) -> Toon {
        Toon::Arr(items)
    }

    pub fn obj<K: Into<String>>(pairs: Vec<(K, Toon)>) -> Toon {
        Toon::Obj(pairs.into_iter().map(|(k, v)| (k.into(), v)).collect())
    }

    fn is_scalar(&self) -> bool {
        matches!(
            self,
            Toon::Str(_) | Toon::Int(_) | Toon::Bool(_) | Toon::Null
        )
    }
}

/// Encode a value as a TOON document.
pub fn encode(root: &Toon) -> String {
    let mut out = String::new();
    match root {
        Toon::Obj(pairs) => write_obj(pairs, 0, &mut out),
        Toon::Arr(items) => write_array("", items, 0, &mut out),
        scalar => {
            out.push_str(&fmt_scalar(scalar));
            out.push('\n');
        }
    }
    out
}

fn indent(level: usize) -> String {
    "  ".repeat(level)
}

fn write_obj(pairs: &[(String, Toon)], level: usize, out: &mut String) {
    for (key, value) in pairs {
        match value {
            v if v.is_scalar() => {
                out.push_str(&format!("{}{}: {}\n", indent(level), key, fmt_scalar(v)));
            }
            Toon::Obj(inner) => {
                if inner.is_empty() {
                    out.push_str(&format!("{}{}:\n", indent(level), key));
                } else {
                    out.push_str(&format!("{}{}:\n", indent(level), key));
                    write_obj(inner, level + 1, out);
                }
            }
            Toon::Arr(items) => write_array(key, items, level, out),
            _ => unreachable!(),
        }
    }
}

/// Detect a tabular array: every element is a non-empty object with identical
/// keys (in the same order) and only scalar field values.
fn tabular_fields(items: &[Toon]) -> Option<Vec<String>> {
    let first = match items.first()? {
        Toon::Obj(pairs) if !pairs.is_empty() => pairs,
        _ => return None,
    };
    let fields: Vec<String> = first.iter().map(|(k, _)| k.clone()).collect();
    for item in items {
        match item {
            Toon::Obj(pairs) => {
                if pairs.len() != fields.len() {
                    return None;
                }
                for (i, (k, v)) in pairs.iter().enumerate() {
                    if k != &fields[i] || !v.is_scalar() {
                        return None;
                    }
                }
            }
            _ => return None,
        }
    }
    Some(fields)
}

fn write_array(key: &str, items: &[Toon], level: usize, out: &mut String) {
    let prefix = indent(level);
    let label = if key.is_empty() {
        String::new()
    } else {
        key.to_string()
    };

    if items.is_empty() {
        out.push_str(&format!("{}{}: []\n", prefix, label));
        return;
    }

    if items.iter().all(|i| i.is_scalar()) {
        let cells: Vec<String> = items.iter().map(fmt_scalar).collect();
        out.push_str(&format!(
            "{}{}[{}]: {}\n",
            prefix,
            label,
            items.len(),
            cells.join(",")
        ));
        return;
    }

    if let Some(fields) = tabular_fields(items) {
        out.push_str(&format!(
            "{}{}[{}]{{{}}}:\n",
            prefix,
            label,
            items.len(),
            fields.join(",")
        ));
        for item in items {
            if let Toon::Obj(pairs) = item {
                let cells: Vec<String> = pairs.iter().map(|(_, v)| fmt_scalar(v)).collect();
                out.push_str(&format!("{}{}\n", indent(level + 1), cells.join(",")));
            }
        }
        return;
    }

    // Fallback: non-uniform array, render as a hyphen list.
    out.push_str(&format!("{}{}[{}]:\n", prefix, label, items.len()));
    for item in items {
        match item {
            s if s.is_scalar() => {
                out.push_str(&format!("{}- {}\n", indent(level + 1), fmt_scalar(s)));
            }
            Toon::Obj(pairs) => {
                if let Some(((first_k, first_v), rest)) = pairs.split_first() {
                    if first_v.is_scalar() {
                        out.push_str(&format!(
                            "{}- {}: {}\n",
                            indent(level + 1),
                            first_k,
                            fmt_scalar(first_v)
                        ));
                    } else {
                        out.push_str(&format!("{}- {}:\n", indent(level + 1), first_k));
                        write_obj(&[(first_k.clone(), first_v.clone())], level + 3, out);
                    }
                    if !rest.is_empty() {
                        write_obj(rest, level + 2, out);
                    }
                }
            }
            Toon::Arr(inner) => write_array("", inner, level + 1, out),
            _ => {}
        }
    }
}

fn fmt_scalar(value: &Toon) -> String {
    match value {
        Toon::Str(s) => maybe_quote(s),
        Toon::Int(n) => n.to_string(),
        Toon::Bool(b) => b.to_string(),
        Toon::Null => "null".to_string(),
        // Non-scalars should never reach here in valid documents.
        other => maybe_quote(&format!("{:?}", other)),
    }
}

/// Apply the TOON quoting rules. The default delimiter is a comma, which is the
/// document delimiter for object fields and the active delimiter inside arrays,
/// so a single rule set covers both contexts.
fn maybe_quote(s: &str) -> String {
    if needs_quoting(s) {
        quote(s)
    } else {
        s.to_string()
    }
}

fn needs_quoting(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    if s.starts_with(char::is_whitespace) || s.ends_with(char::is_whitespace) {
        return true;
    }
    if s == "true" || s == "false" || s == "null" {
        return true;
    }
    if looks_numeric(s) {
        return true;
    }
    if s == "-" || (s.starts_with('-') && s.len() > 1) {
        return true;
    }
    s.chars().any(|c| {
        matches!(c, ':' | '"' | '\\' | '[' | ']' | '{' | '}' | ',') || c.is_control()
    })
}

fn looks_numeric(s: &str) -> bool {
    // Anything Rust can parse as a finite float looks like a number/literal
    // and must be quoted to round-trip as a string (covers "05", "1e-6", etc.).
    s.parse::<f64>().is_ok()
}

fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_flat_object() {
        let v = Toon::obj(vec![("id", Toon::Int(1)), ("name", Toon::str("Ada"))]);
        assert_eq!(encode(&v), "id: 1\nname: Ada\n");
    }

    #[test]
    fn encodes_nested_object() {
        let v = Toon::obj(vec![(
            "user",
            Toon::obj(vec![("id", Toon::Int(1)), ("name", Toon::str("Ada"))]),
        )]);
        assert_eq!(encode(&v), "user:\n  id: 1\n  name: Ada\n");
    }

    #[test]
    fn encodes_primitive_array() {
        let v = Toon::obj(vec![(
            "tags",
            Toon::arr(vec![Toon::str("foo"), Toon::str("bar"), Toon::str("baz")]),
        )]);
        assert_eq!(encode(&v), "tags[3]: foo,bar,baz\n");
    }

    #[test]
    fn encodes_tabular_array() {
        let row = |id: i64, qty: i64| {
            Toon::obj(vec![("id", Toon::Int(id)), ("qty", Toon::Int(qty))])
        };
        let v = Toon::obj(vec![("items", Toon::arr(vec![row(1, 5), row(2, 3)]))]);
        assert_eq!(encode(&v), "items[2]{id,qty}:\n  1,5\n  2,3\n");
    }

    #[test]
    fn encodes_empty_array() {
        let v = Toon::obj(vec![("items", Toon::Arr(vec![]))]);
        assert_eq!(encode(&v), "items: []\n");
    }

    #[test]
    fn quotes_strings_that_look_like_literals() {
        assert_eq!(maybe_quote("123"), "\"123\"");
        assert_eq!(maybe_quote("true"), "\"true\"");
        assert_eq!(maybe_quote("null"), "\"null\"");
        assert_eq!(maybe_quote("05"), "\"05\"");
        assert_eq!(maybe_quote("-3.14"), "\"-3.14\"");
    }

    #[test]
    fn quotes_strings_with_delimiters_and_specials() {
        assert_eq!(maybe_quote("hello, world"), "\"hello, world\"");
        assert_eq!(maybe_quote("a:b"), "\"a:b\"");
        assert_eq!(maybe_quote(" padded "), "\" padded \"");
        assert_eq!(maybe_quote(""), "\"\"");
        assert_eq!(maybe_quote("-flag"), "\"-flag\"");
    }

    #[test]
    fn leaves_safe_strings_unquoted() {
        assert_eq!(maybe_quote("fix"), "fix");
        assert_eq!(maybe_quote("add user profile"), "add user profile");
        assert_eq!(maybe_quote("feat/login"), "feat/login");
    }

    #[test]
    fn escapes_control_characters() {
        assert_eq!(maybe_quote("line1\nline2"), "\"line1\\nline2\"");
        assert_eq!(maybe_quote("tab\there"), "\"tab\\there\"");
    }
}
