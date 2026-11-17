//! Human-readable rendering for [`DbValue`].
//!
//! Tables come from lists of [`DbValue::Table`] rows (the shape every read
//! endpoint returns). Columns follow `preferred` first, then any remaining keys
//! in first-seen order. Cells are truncated to a fixed width so rows stay
//! aligned in a terminal.

use crate::cli::json;
use crate::db::DbValue;

pub struct TableOptions<'a> {
    /// Column names to show first, in this order.
    pub preferred: &'a [&'a str],
    /// Maximum rows to print; `None` prints every row.
    pub max_rows: Option<usize>,
    /// Maximum display width of a column before it is truncated with `…`.
    pub max_col: usize,
    /// Bold the header row with ANSI escapes.
    pub color: bool,
}

pub fn render(value: &DbValue, opts: &TableOptions) -> String {
    match value {
        DbValue::List(rows) => render_rows(rows, opts),
        DbValue::Table(entries) => render_object(entries, opts),
        DbValue::None => String::new(),
        other => cell(other, opts.max_col),
    }
}

fn render_rows(rows: &[DbValue], opts: &TableOptions) -> String {
    if rows.is_empty() {
        return "(no results)".to_string();
    }

    let tables: Option<Vec<&Vec<(String, DbValue)>>> = rows
        .iter()
        .map(|row| match row {
            DbValue::Table(entries) => Some(entries),
            _ => None,
        })
        .collect();

    let Some(tables) = tables else {
        return rows
            .iter()
            .map(|row| cell(row, opts.max_col))
            .collect::<Vec<_>>()
            .join("\n");
    };

    let columns = columns(&tables, opts.preferred);
    let cells: Vec<Vec<String>> = tables
        .iter()
        .map(|row| {
            columns
                .iter()
                .map(|name| {
                    let value = row
                        .iter()
                        .find(|(key, _)| key == name)
                        .map(|(_, value)| value);
                    value.map(|v| cell(v, opts.max_col)).unwrap_or_default()
                })
                .collect()
        })
        .collect();

    let widths: Vec<usize> = columns
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let header = name.chars().count();
            let body = cells.iter().map(|row| row[index].chars().count()).max();
            body.map_or(header, |body| header.max(body))
                .min(opts.max_col)
        })
        .collect();

    let mut out = String::new();
    let header = columns
        .iter()
        .enumerate()
        .map(|(index, name)| pad(&truncate(name, widths[index]), widths[index]))
        .collect::<Vec<_>>()
        .join("  ");
    if opts.color {
        out.push_str("\x1b[1m");
        out.push_str(header.trim_end());
        out.push_str("\x1b[0m");
    } else {
        out.push_str(header.trim_end());
    }
    out.push('\n');
    out.push_str(
        &widths
            .iter()
            .map(|w| "-".repeat(*w))
            .collect::<Vec<_>>()
            .join("  "),
    );

    let shown = match opts.max_rows {
        Some(max) => max.min(cells.len()),
        None => cells.len(),
    };
    for row in &cells[..shown] {
        out.push('\n');
        let line = row
            .iter()
            .enumerate()
            .map(|(index, value)| pad(value, widths[index]))
            .collect::<Vec<_>>()
            .join("  ");
        out.push_str(line.trim_end());
    }

    if shown < cells.len() {
        out.push_str(&format!(
            "\n… {} more row(s) (use --all)",
            cells.len() - shown
        ));
    }

    out
}

fn render_object(entries: &[(String, DbValue)], opts: &TableOptions) -> String {
    let width = entries
        .iter()
        .map(|(key, _)| key.chars().count())
        .max()
        .unwrap_or(0);
    entries
        .iter()
        .map(|(key, value)| {
            format!("{}  {}", pad(key, width), cell(value, opts.max_col))
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Ordered column names. When `preferred` is non-empty only those columns are
/// shown (tables stay readable; JSON still carries every field); otherwise every
/// key is shown in first-seen order.
fn columns(tables: &[&Vec<(String, DbValue)>], preferred: &[&str]) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for row in tables {
        for (key, _) in row.iter() {
            if !seen.contains(key) {
                seen.push(key.clone());
            }
        }
    }

    if preferred.is_empty() {
        return seen;
    }

    let mut columns: Vec<String> = Vec::new();
    for name in preferred {
        let name = name.to_string();
        if seen.contains(&name) && !columns.contains(&name) {
            columns.push(name);
        }
    }
    columns
}

/// Renders one cell: lists join with commas, tables become compact JSON.
/// Newlines are flattened and long text is truncated with an ellipsis.
fn cell(value: &DbValue, max_col: usize) -> String {
    let text = match value {
        DbValue::None => String::new(),
        DbValue::String(text) => sanitize(text),
        DbValue::Int(value) => value.to_string(),
        DbValue::Float(value) => format_float(*value),
        DbValue::Bool(value) => value.to_string(),
        DbValue::List(items) => items
            .iter()
            .map(|item| match item {
                DbValue::String(text) => sanitize(text),
                DbValue::None => String::new(),
                other => format_float_or(other),
            })
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join(", "),
        DbValue::Table(_) => json::to_string(value, false),
    };
    truncate(&text, max_col)
}

fn format_float_or(value: &DbValue) -> String {
    match value {
        DbValue::Float(value) => format_float(*value),
        other => json::to_string(other, false),
    }
}

/// Rounds a float to four decimals and trims trailing zeros, so scores and
/// timestamps print compactly.
fn format_float(value: f64) -> String {
    if !value.is_finite() {
        return value.to_string();
    }
    let mut text = format!("{value:.4}");
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    text
}

/// Strips newlines and tabs so a cell stays on one line.
fn sanitize(text: &str) -> String {
    text.replace(['\n', '\r', '\t'], " ")
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut out: String = text.chars().take(max - 1).collect();
    out.push('…');
    out
}

fn pad(text: &str, width: usize) -> String {
    let len = text.chars().count();
    if len >= width {
        return text.to_string();
    }
    format!("{text}{}", " ".repeat(width - len))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(pairs: &[(&str, DbValue)]) -> DbValue {
        DbValue::Table(
            pairs
                .iter()
                .map(|(key, value)| (key.to_string(), value.clone()))
                .collect(),
        )
    }

    fn opts<'a>(preferred: &'a [&'a str]) -> TableOptions<'a> {
        TableOptions {
            preferred,
            max_rows: None,
            max_col: 20,
            color: false,
        }
    }

    #[test]
    fn renders_aligned_table() {
        let value = DbValue::List(vec![
            row(&[
                ("id", DbValue::String("A".to_string())),
                ("score", DbValue::Float(0.5)),
            ]),
            row(&[
                ("id", DbValue::String("BB".to_string())),
                ("score", DbValue::Float(0.125)),
            ]),
        ]);
        let text = render(&value, &opts(&["id", "score"]));
        assert_eq!(text, "id  score\n--  -----\nA   0.5\nBB  0.125");
    }

    #[test]
    fn preferred_limits_columns_to_the_list() {
        let value = DbValue::List(vec![row(&[
            ("title", DbValue::String("T".to_string())),
            ("id", DbValue::String("A".to_string())),
            ("path", DbValue::String("/a".to_string())),
        ])]);
        let text = render(&value, &opts(&["id", "title"]));
        assert!(text.starts_with("id  title\n"));
        assert!(!text.contains("path"));
    }

    #[test]
    fn without_preferred_every_key_is_shown() {
        let value = DbValue::List(vec![row(&[
            ("title", DbValue::String("T".to_string())),
            ("id", DbValue::String("A".to_string())),
        ])]);
        let text = render(&value, &opts(&[]));
        assert!(text.starts_with("title  id"));
    }

    #[test]
    fn truncates_long_cells() {
        let value = DbValue::List(vec![row(&[("title", DbValue::String("a".repeat(50)))])]);
        let text = render(&value, &opts(&["title"]));
        let body = text.lines().nth(2).unwrap();
        assert!(body.ends_with('…'));
        assert_eq!(body.chars().count(), 20);
    }

    #[test]
    fn caps_rows_with_footer() {
        let value = DbValue::List((0..5).map(|i| row(&[("id", DbValue::Int(i))])).collect());
        let mut options = opts(&["id"]);
        options.max_rows = Some(2);
        let text = render(&value, &options);
        assert!(text.contains("… 3 more row(s)"));
    }

    #[test]
    fn joins_list_cells() {
        let value = DbValue::List(vec![row(&[(
            "tags",
            DbValue::List(vec![
                DbValue::String("a".to_string()),
                DbValue::String("b".to_string()),
            ]),
        )])]);
        let text = render(&value, &opts(&["tags"]));
        assert!(text.contains("a, b"));
    }

    #[test]
    fn empty_list_is_reported() {
        assert_eq!(render(&DbValue::List(vec![]), &opts(&[])), "(no results)");
    }
}
