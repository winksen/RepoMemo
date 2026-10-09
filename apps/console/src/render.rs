//! Turns console answers into terminal text: the same summary, facts and
//! tables System › Console shows in the web app.

use repomemo_domain::console::{CONSOLE_BANNER, CONSOLE_TAGLINE};
use serde_json::Value;

use crate::client::Welcome;

/// Cells longer than this are cut, so a table stays readable.
const MAX_CELL: usize = 48;

/// Colours, when the output is a terminal that shows them and `NO_COLOR` is unset.
pub struct Paint {
    on: bool,
}

impl Paint {
    pub fn detect() -> Self {
        use std::io::IsTerminal;
        let wanted = std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
        Self { on: wanted && ansi_supported() }
    }

    pub fn enabled(&self) -> bool {
        self.on
    }

    fn wrap(&self, code: &str, text: &str) -> String {
        if self.on { format!("\x1b[{code}m{text}\x1b[0m") } else { text.to_owned() }
    }

    /// The RepoMemo green, for the logo and the prompt.
    pub fn accent(&self, text: &str) -> String {
        self.wrap("1;38;5;36", text)
    }

    pub fn muted(&self, text: &str) -> String {
        self.wrap("90", text)
    }

    pub fn bold(&self, text: &str) -> String {
        self.wrap("1", text)
    }

    pub fn warning(&self, text: &str) -> String {
        self.wrap("33", text)
    }

    pub fn error(&self, text: &str) -> String {
        self.wrap("31", text)
    }
}

#[cfg(windows)]
fn ansi_supported() -> bool {
    nu_ansi_term::enable_ansi_support().is_ok()
}

#[cfg(not(windows))]
fn ansi_supported() -> bool {
    true
}

/// The logo and tagline, shown as the console opens.
pub fn banner(paint: &Paint) -> String {
    let logo = CONSOLE_BANNER.lines().map(|line| paint.accent(line)).collect::<Vec<_>>().join("\n");
    format!("{logo}\n{}\n", paint.muted(CONSOLE_TAGLINE))
}

/// Who is signed in where, and how to start.
pub fn greeting(paint: &Paint, welcome: &Welcome, server: &str) -> String {
    format!(
        "Signed in as {} ({}) · RepoMemo {} at {}\n{}\n{}\n",
        paint.bold(&welcome.user),
        welcome.role,
        welcome.version,
        server,
        paint.muted(&welcome.tips),
        paint.muted("Up and Down recall earlier lines, Tab completes, Ctrl+L clears, exit or Ctrl+D leaves."),
    )
}

/// A command's answer: its summary, then its facts or table.
pub fn answer(paint: &Paint, response: &Value) -> String {
    let summary = response["summary"].as_str().unwrap_or_default();
    let mut out = if response["dry_run"] == true {
        format!("{} {}\n", paint.warning(&paint.bold("DRY RUN")), paint.warning(summary))
    } else {
        format!("{summary}\n")
    };
    let data = &response["data"];
    match response["view"]["kind"].as_str() {
        Some("facts") => out.push_str(&facts(paint, data)),
        Some("table") => {
            let columns = response["view"]["columns"]
                .as_array()
                .map(|columns| columns.iter().filter_map(|column| column.as_str().map(str::to_owned)).collect::<Vec<_>>())
                .unwrap_or_default();
            out.push_str(&table(paint, &columns, data));
        }
        _ => {}
    }
    out
}

fn cell(value: &Value) -> String {
    let text = match value {
        Value::Null => "—".to_owned(),
        Value::String(text) if text.is_empty() => "—".to_owned(),
        Value::String(text) => text.clone(),
        Value::Bool(yes) => (if *yes { "yes" } else { "no" }).to_owned(),
        other => other.to_string(),
    };
    text.replace(['\r', '\n'], " ")
}

fn width(text: &str) -> usize {
    text.chars().count()
}

fn clip(text: String, max: usize) -> String {
    if width(&text) <= max {
        text
    } else {
        format!("{}…", text.chars().take(max - 1).collect::<String>())
    }
}

fn pad(text: &str, to: usize) -> String {
    format!("{text}{}", " ".repeat(to.saturating_sub(width(text))))
}

fn facts(paint: &Paint, data: &Value) -> String {
    let pairs = data
        .as_array()
        .map(|pairs| pairs.iter().map(|pair| (cell(&pair[0]), cell(&pair[1]))).collect::<Vec<_>>())
        .unwrap_or_default();
    let label_width = pairs.iter().map(|(label, _)| width(label)).max().unwrap_or(0);
    pairs
        .iter()
        .map(|(label, value)| format!("  {}  {value}\n", paint.muted(&pad(label, label_width))))
        .collect()
}

fn table(paint: &Paint, columns: &[String], data: &Value) -> String {
    let rows = data
        .as_array()
        .map(|rows| {
            rows.iter()
                .map(|row| columns.iter().map(|column| clip(cell(&row[column.as_str()]), MAX_CELL)).collect::<Vec<_>>())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if rows.is_empty() {
        return String::new();
    }
    let headers = columns.iter().map(|column| column.replace('_', " ")).collect::<Vec<_>>();
    let widths = (0..columns.len())
        .map(|index| rows.iter().map(|row| width(&row[index])).chain([width(&headers[index])]).max().unwrap_or(0))
        .collect::<Vec<_>>();
    let line = |cells: &[String]| {
        cells.iter().zip(&widths).map(|(text, to)| pad(text, *to)).collect::<Vec<_>>().join("  ").trim_end().to_owned()
    };
    let mut out = format!("  {}\n", paint.muted(&line(&headers)));
    for row in &rows {
        out.push_str(&format!("  {}\n", line(row)));
    }
    out
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const PLAIN: Paint = Paint { on: false };

    #[test]
    fn tables_line_up_and_cut_long_cells() {
        let response = json!({
            "summary": "2 job(s), newest first.",
            "dry_run": false,
            "view": {"kind": "table", "columns": ["kind", "status", "error_message"]},
            "data": [
                {"kind": "indexing", "status": "failed", "error_message": "x".repeat(60)},
                {"kind": "repo_sync", "status": "running", "error_message": null},
            ],
        });
        let text = answer(&PLAIN, &response);
        let lines = text.lines().collect::<Vec<_>>();
        assert_eq!(lines[0], "2 job(s), newest first.");
        assert_eq!(lines[1], "  kind       status   error message");
        assert!(lines[2].starts_with("  indexing   failed   xxx") && lines[2].ends_with('…'));
        assert_eq!(width(lines[2]), 2 + 9 + 2 + 7 + 2 + MAX_CELL);
        assert_eq!(lines[3], "  repo_sync  running  —");
    }

    #[test]
    fn facts_and_dry_runs_read_well() {
        let facts = json!({"summary": "Up.", "dry_run": false, "view": {"kind": "facts"}, "data": [["Version", "0.1.0"], ["Running", true]]});
        assert_eq!(answer(&PLAIN, &facts), "Up.\n  Version  0.1.0\n  Running  yes\n");
        let dry = json!({"summary": "Would stop it.", "dry_run": true, "view": {"kind": "text"}, "data": null});
        assert_eq!(answer(&PLAIN, &dry), "DRY RUN Would stop it.\n");
    }

    #[test]
    fn the_banner_is_the_shared_one() {
        let banner = banner(&PLAIN);
        assert!(banner.starts_with(CONSOLE_BANNER.lines().next().unwrap()));
        assert!(banner.contains(CONSOLE_TAGLINE));
    }
}
