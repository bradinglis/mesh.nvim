//! Minimal JSON writer for [`DbValue`].
//!
//! `DbValue` is a small, closed enum, so a hand-rolled writer keeps key order
//! (the SQL column order) without a dependency on `serde_json`'s ordered maps.

use crate::db::DbValue;

/// Serialises a value as a single JSON document.
pub fn to_string(value: &DbValue, pretty: bool) -> String {
    let mut out = String::new();
    write_value(value, &mut out, pretty, 0);
    out
}

/// Serialises a list as one JSON document per line; other values as one line.
pub fn to_jsonl(value: &DbValue) -> String {
    match value {
        DbValue::List(items) => items
            .iter()
            .map(|item| to_string(item, false))
            .collect::<Vec<_>>()
            .join("\n"),
        other => to_string(other, false),
    }
}

fn write_value(value: &DbValue, out: &mut String, pretty: bool, indent: usize) {
    match value {
        DbValue::None => out.push_str("null"),
        DbValue::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
        DbValue::Int(value) => out.push_str(&value.to_string()),
        DbValue::Float(value) => write_float(*value, out),
        DbValue::String(value) => write_string(value, out),
        DbValue::List(items) => {
            // Nulls are dropped so lists stay dense, matching the Lua module.
            let items: Vec<&DbValue> = items
                .iter()
                .filter(|item| !matches!(item, DbValue::None))
                .collect();
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                newline(out, pretty, indent + 1);
                write_value(item, out, pretty, indent + 1);
            }
            newline(out, pretty, indent);
            out.push(']');
        }
        DbValue::Table(entries) => {
            if entries.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push('{');
            for (index, (key, item)) in entries.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                newline(out, pretty, indent + 1);
                write_string(key, out);
                out.push(':');
                if pretty {
                    out.push(' ');
                }
                write_value(item, out, pretty, indent + 1);
            }
            newline(out, pretty, indent);
            out.push('}');
        }
    }
}

fn newline(out: &mut String, pretty: bool, indent: usize) {
    if !pretty {
        return;
    }
    out.push('\n');
    for _ in 0..indent {
        out.push_str("  ");
    }
}

fn write_float(value: f64, out: &mut String) {
    // JSON has no NaN or infinity; Rust's Display already gives the shortest
    // representation that round-trips.
    if value.is_finite() {
        out.push_str(&value.to_string());
    } else {
        out.push_str("null");
    }
}

fn write_string(value: &str, out: &mut String) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if (ch as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> DbValue {
        DbValue::List(vec![DbValue::Table(vec![
            ("id".to_string(), DbValue::String("A".to_string())),
            ("score".to_string(), DbValue::Float(0.5)),
            (
                "tags".to_string(),
                DbValue::List(vec![DbValue::String("x".to_string())]),
            ),
            ("anchor".to_string(), DbValue::None),
        ])])
    }

    #[test]
    fn compact_keeps_key_order_and_escapes() {
        let json = to_string(&table(), false);
        assert_eq!(
            json,
            r#"[{"id":"A","score":0.5,"tags":["x"],"anchor":null}]"#
        );
    }

    #[test]
    fn pretty_indents() {
        let json = to_string(
            &DbValue::Table(vec![("a".to_string(), DbValue::Int(1))]),
            true,
        );
        assert_eq!(json, "{\n  \"a\": 1\n}");
    }

    #[test]
    fn jsonl_splits_lists() {
        let value = DbValue::List(vec![DbValue::Int(1), DbValue::Int(2)]);
        assert_eq!(to_jsonl(&value), "1\n2");
    }

    #[test]
    fn strings_escape_control_characters() {
        let value = DbValue::String("a\"b\\c\n".to_string());
        assert_eq!(to_string(&value, false), r#""a\"b\\c\n""#);
    }

    #[test]
    fn list_nulls_are_dropped() {
        let value = DbValue::List(vec![DbValue::None, DbValue::String("x".to_string())]);
        assert_eq!(to_string(&value, false), r#"["x"]"#);
    }
}
