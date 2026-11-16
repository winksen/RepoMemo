//! CSV migration (V0.7.18): the tasks of a workspace out as a CSV a spreadsheet can open safely, and a CSV in with a field
//! mapping and a report that explains every row before anything is written. A retry creates nothing twice, and exported cells that
//! a spreadsheet could read as a formula are written as text.

use crate::*;
use repomemo_domain::{CsvImportReport, CsvRowReport};
use repomemo_storage::ImportedTaskRow;
use std::collections::{BTreeMap, HashMap};

/// The most data rows one import takes.
const MAX_ROWS: usize = 1_000;
/// The most tasks one export holds.
const MAX_EXPORT_ROWS: i64 = 10_000;

/// The columns a CSV can be mapped from, in the order they are exported.
const FIELDS: [&str; 9] = [
    "external_id",
    "title",
    "description",
    "status",
    "priority",
    "assignee",
    "due_at",
    "project",
    "labels",
];

// ---------------------------------------------------------------------------------------------------------------------------
// The format

/// Reads RFC 4180 CSV: quoted cells with commas, doubled quotes and line breaks, `\r\n` or `\n` rows, an optional byte order
/// mark. Blank lines are skipped.
pub(crate) fn parse_csv(text: &str) -> Result<Vec<Vec<String>>, String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut after_quote = false;
    let mut chars = text.chars().peekable();
    while let Some(character) = chars.next() {
        if quoted {
            if character == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    cell.push('"');
                } else {
                    quoted = false;
                    after_quote = true;
                }
            } else {
                cell.push(character);
            }
            continue;
        }
        match character {
            '"' if cell.is_empty() && !after_quote => quoted = true,
            '"' => return Err(format!("A quote appears inside a cell in row {}.", rows.len() + 1)),
            ',' => {
                row.push(std::mem::take(&mut cell));
                after_quote = false;
            }
            '\r' | '\n' => {
                if character == '\r' && chars.peek() == Some(&'\n') {
                    chars.next();
                }
                row.push(std::mem::take(&mut cell));
                after_quote = false;
                if !(row.len() == 1 && row[0].is_empty()) {
                    rows.push(std::mem::take(&mut row));
                } else {
                    row.clear();
                }
            }
            _ if after_quote => return Err(format!("Text follows a closing quote in row {}.", rows.len() + 1)),
            _ => cell.push(character),
        }
    }
    if quoted {
        return Err("A quoted cell is not closed.".to_owned());
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        if !(row.len() == 1 && row[0].is_empty()) {
            rows.push(row);
        }
    }
    Ok(rows)
}

/// One cell for a spreadsheet: text that a spreadsheet would run as a formula (it starts with `=`, `+`, `-`, `@`, a tab or a
/// carriage return) gets a leading apostrophe so it stays text, and a cell with a comma, a quote or a line break is quoted.
pub(crate) fn csv_cell(value: &str) -> String {
    let mut text = value.to_owned();
    let formula = text.starts_with(['=', '+', '-', '@', '\t', '\r']);
    if formula {
        text.insert(0, '\'');
    }
    if formula || text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text
    }
}

/// The reverse of the apostrophe the export adds, so a file taken out and brought back is the same.
fn unescape_cell(value: &str) -> String {
    let mut chars = value.chars();
    if chars.next() == Some('\'') {
        if let Some(second) = chars.next() {
            if matches!(second, '=' | '+' | '-' | '@' | '\t' | '\r') {
                return value[1..].to_owned();
            }
        }
    }
    value.to_owned()
}

// ---------------------------------------------------------------------------------------------------------------------------
// Export

pub(crate) async fn export_tasks_csv(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Query(params): Query<crate::task_list::TaskListParams>,
) -> Result<Response, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    let query = crate::task_list::resolve_query(&state, &subject, &workspace_id, &params).await?;
    let mut csv = String::from("\u{feff}key,title,description,status,priority,state,assignee_email,due_at,project,milestone,labels,created_at,completed_at\r\n");
    let mut offset = 0;
    loop {
        let page = state
            .storage
            .query_collaboration_tasks(&workspace_id, &query, &subject.user_id, 200, offset)
            .await
            .map_err(map_storage_error)?;
        for task in &page.tasks {
            let cells = [
                task.key.clone(),
                task.title.clone(),
                task.description.clone(),
                task.status.clone(),
                task.priority.clone(),
                task.state_name.clone(),
                task.assignee
                    .as_ref()
                    .and_then(|member| member.email.clone())
                    .unwrap_or_default(),
                task.due_at.clone().unwrap_or_default(),
                task.project_name.clone().unwrap_or_default(),
                task.milestone_name.clone().unwrap_or_default(),
                task.labels
                    .iter()
                    .map(|label| label.name.as_str())
                    .collect::<Vec<_>>()
                    .join("; "),
                task.created_at.clone(),
                task.completed_at.clone().unwrap_or_default(),
            ];
            csv.push_str(&cells.iter().map(|cell| csv_cell(cell)).collect::<Vec<_>>().join(","));
            csv.push_str("\r\n");
        }
        match page.next_offset {
            Some(next) if next < MAX_EXPORT_ROWS => offset = next,
            _ => break,
        }
    }
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"tasks.csv\"".to_owned(),
            ),
        ],
        csv,
    )
        .into_response())
}

// ---------------------------------------------------------------------------------------------------------------------------
// Import

/// The names a column may have for each field when no mapping is given, besides the field's own name.
fn aliases(field: &str) -> &'static [&'static str] {
    match field {
        "external_id" => &["id", "key", "issue key", "issue id", "external id"],
        "title" => &["summary", "name", "subject", "task"],
        "description" => &["details", "body", "notes"],
        "status" => &["state"],
        "assignee" => &["assignee_email", "assignee email", "owner", "assigned to"],
        "due_at" => &["due", "due date", "deadline"],
        "labels" => &["label", "tags"],
        _ => &[],
    }
}

fn normalize_status(value: &str) -> Option<&'static str> {
    match value.trim().to_lowercase().replace(['-', ' '], "_").as_str() {
        "" | "open" | "todo" | "to_do" | "new" | "backlog" => Some("open"),
        "in_progress" | "inprogress" | "doing" | "started" | "active" => Some("in_progress"),
        "blocked" | "on_hold" | "waiting" => Some("blocked"),
        "done" | "closed" | "complete" | "completed" | "resolved" => Some("done"),
        _ => None,
    }
}

fn normalize_priority(value: &str) -> Option<&'static str> {
    match value.trim().to_lowercase().as_str() {
        "" | "medium" | "normal" | "med" => Some("medium"),
        "low" | "minor" => Some("low"),
        "high" | "major" => Some("high"),
        "urgent" | "critical" | "blocker" => Some("urgent"),
        _ => None,
    }
}

/// A due date as the task stores it: a date alone stays a date, a time becomes an instant in UTC.
fn normalize_due(value: &str) -> Option<String> {
    let value = value.trim();
    if chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok() {
        return Some(value.to_owned());
    }
    if let Ok(instant) = chrono::DateTime::parse_from_rfc3339(value) {
        return Some(instant.with_timezone(&chrono::Utc).to_rfc3339());
    }
    chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M")
        .ok()
        .map(|moment| moment.and_utc().to_rfc3339())
}

pub(crate) async fn import_tasks_csv(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<ImportTasksRequest>,
) -> Result<(StatusCode, Json<CsvImportReport>), ApiError> {
    require_workspace_write(&state, &subject, &workspace_id).await?;
    let records = parse_csv(&request.csv)
        .map_err(|message| ApiError::bad_request(format!("The CSV could not be read: {message}")))?;
    let Some((header_row, data)) = records.split_first() else {
        return Err(ApiError::bad_request("The CSV is empty: it needs a header row."));
    };
    if data.len() > MAX_ROWS {
        return Err(ApiError::bad_request(format!(
            "An import takes at most {MAX_ROWS} rows; this file has {}.",
            data.len()
        )));
    }
    let headers: Vec<String> = header_row.iter().map(|name| name.trim().to_lowercase()).collect();

    // Which column feeds which field: what the caller mapped, then the field's own name or a common alias.
    let mut mapping_used: BTreeMap<String, String> = BTreeMap::new();
    let mut column_of: HashMap<&str, usize> = HashMap::new();
    for field in FIELDS {
        let wanted = request.mapping.get(field).map(|name| name.trim().to_lowercase());
        let found = match &wanted {
            Some(name) => headers.iter().position(|header| header == name),
            None => std::iter::once(field)
                .chain(aliases(field).iter().copied())
                .find_map(|name| headers.iter().position(|header| header == name)),
        };
        if let (Some(name), None) = (&wanted, found) {
            return Err(ApiError::bad_request(format!(
                "The mapping sends {field} to a column named \"{name}\", which the file does not have. Its columns are: {}.",
                header_row.join(", ")
            )));
        }
        if let Some(index) = found {
            column_of.insert(field, index);
            mapping_used.insert(field.to_owned(), header_row[index].trim().to_owned());
        }
    }
    for name in request.mapping.keys() {
        if !FIELDS.contains(&name.as_str()) {
            return Err(ApiError::bad_request(format!(
                "\"{name}\" is not a field of a task. The fields are: {}.",
                FIELDS.join(", ")
            )));
        }
    }
    if !column_of.contains_key("title") {
        return Err(ApiError::bad_request(format!(
            "No column holds the title. Map one (for example \"mapping\": {{\"title\": \"Summary\"}}); the file's columns are: {}.",
            header_row.join(", ")
        )));
    }

    // What the workspace knows, looked at once.
    let members = state
        .storage
        .list_workspace_members(&workspace_id)
        .await
        .map_err(map_storage_error)?;
    let member_by_email: HashMap<String, String> = members
        .iter()
        .filter_map(|member| {
            member
                .user
                .email
                .as_ref()
                .map(|email| (email.to_lowercase(), member.user.id.clone()))
        })
        .collect();
    let projects: HashMap<String, String> = state
        .storage
        .list_projects(&workspace_id, None)
        .await
        .map_err(map_storage_error)?
        .into_iter()
        .map(|project| (project.name.to_lowercase(), project.id))
        .collect();
    let labels: HashMap<String, String> = state
        .storage
        .list_task_labels(&workspace_id)
        .await
        .map_err(map_storage_error)?
        .into_iter()
        .map(|label| (label.name.to_lowercase(), label.id))
        .collect();
    let named_states: HashMap<String, (String, String)> = state
        .storage
        .list_workflow_states(&workspace_id)
        .await
        .map_err(map_storage_error)?
        .into_iter()
        .filter(|workflow_state| !workflow_state.builtin)
        .map(|workflow_state| {
            (
                workflow_state.name.to_lowercase(),
                (workflow_state.id, workflow_state.category),
            )
        })
        .collect();

    // One batch identity: the one asked for, or the file itself, so the same file sent again is the same import.
    let import_key = request
        .import_key
        .as_deref()
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map_or_else(|| repomemo_storage::row_fingerprint(&[&request.csv]), str::to_owned);

    let cell = |record: &Vec<String>, field: &str| -> String {
        column_of
            .get(field)
            .and_then(|index| record.get(*index))
            .map(|value| unescape_cell(value.trim()))
            .unwrap_or_default()
    };
    struct Prepared {
        report: CsvRowReport,
        source_key: String,
        row: Option<ImportedTaskRow>,
    }
    let mut prepared: Vec<Prepared> = Vec::with_capacity(data.len());
    let mut unknown_assignees: Vec<String> = Vec::new();
    let mut seen_content: HashMap<String, usize> = HashMap::new();
    for (index, record) in data.iter().enumerate() {
        let number = index as i64 + 2;
        let mut problems = Vec::new();
        let title = cell(record, "title");
        if title.is_empty() {
            problems.push("The title is empty.".to_owned());
        } else if title.chars().count() > 180 {
            problems.push("The title is longer than 180 characters.".to_owned());
        }
        let description = cell(record, "description");
        if description.chars().count() > 5_000 {
            problems.push("The description is longer than 5,000 characters.".to_owned());
        }
        let status_text = cell(record, "status");
        let (status, state_id) = match normalize_status(&status_text) {
            Some(status) => (status.to_owned(), None),
            None => match named_states.get(&status_text.to_lowercase()) {
                Some((id, category)) => (category.clone(), Some(id.clone())),
                None => {
                    problems.push(format!(
                        "The status \"{status_text}\" is not open, in progress, blocked, done or a state of this workspace."
                    ));
                    (String::new(), None)
                }
            },
        };
        let priority_text = cell(record, "priority");
        let priority = normalize_priority(&priority_text).map_or_else(
            || {
                problems.push(format!(
                    "The priority \"{priority_text}\" is not low, medium, high or urgent."
                ));
                String::new()
            },
            str::to_owned,
        );
        let due_text = cell(record, "due_at");
        let due_at = if due_text.is_empty() {
            None
        } else {
            let normalized = normalize_due(&due_text);
            if normalized.is_none() {
                problems.push(format!(
                    "The due date \"{due_text}\" is not a date (YYYY-MM-DD) or a time."
                ));
            }
            normalized
        };
        let assignee_text = cell(record, "assignee");
        let assignee_user_id = if assignee_text.is_empty() {
            None
        } else {
            let found = member_by_email.get(&assignee_text.to_lowercase()).cloned();
            if found.is_none() {
                problems.push(format!(
                    "Unknown assignee \"{assignee_text}\": not a member of this workspace."
                ));
                if !unknown_assignees.contains(&assignee_text) {
                    unknown_assignees.push(assignee_text.clone());
                }
            }
            found
        };
        let project_text = cell(record, "project");
        let project_id = if project_text.is_empty() {
            None
        } else {
            let found = projects.get(&project_text.to_lowercase()).cloned();
            if found.is_none() {
                problems.push(format!("Unknown project \"{project_text}\"."));
            }
            found
        };
        let mut label_ids = Vec::new();
        for name in cell(record, "labels")
            .split([';', ','])
            .map(str::trim)
            .filter(|name| !name.is_empty())
        {
            match labels.get(&name.to_lowercase()) {
                Some(id) => {
                    if !label_ids.contains(id) {
                        label_ids.push(id.clone());
                    }
                }
                None => problems.push(format!("Unknown label \"{name}\".")),
            }
        }
        if label_ids.len() > repomemo_storage::MAX_LABELS_PER_TASK {
            problems.push(format!("More than {} labels.", repomemo_storage::MAX_LABELS_PER_TASK));
        }
        // The row's identity: its id in the source when it has one, otherwise what it says, counted when it repeats.
        let external = cell(record, "external_id");
        let source_key = if external.is_empty() {
            let fingerprint = repomemo_storage::row_fingerprint(&[
                &import_key,
                &title,
                &description,
                &status_text,
                &priority_text,
                &due_text,
                &assignee_text,
                &project_text,
            ]);
            let seen = seen_content.entry(fingerprint.clone()).or_insert(0);
            *seen += 1;
            format!("row:{fingerprint}#{seen}")
        } else {
            format!("ext:{external}")
        };
        let row = problems.is_empty().then(|| ImportedTaskRow {
            title: title.clone(),
            description,
            status,
            priority,
            assignee_user_id,
            due_at,
            project_id,
            state_id,
            label_ids,
        });
        prepared.push(Prepared {
            report: CsvRowReport {
                row: number,
                outcome: if problems.is_empty() { "valid" } else { "invalid" }.to_owned(),
                title,
                problems,
                task_key: None,
            },
            source_key,
            row,
        });
    }
    // A row already imported is not made again, however the file was sent.
    let keys: Vec<String> = prepared.iter().map(|entry| entry.source_key.clone()).collect();
    let known = state
        .storage
        .known_task_import_keys(&workspace_id, &keys)
        .await
        .map_err(map_storage_error)?;
    // A row whose source key was imported before, or whose id came earlier in this file, is not made again.
    let mut ids_in_file: std::collections::HashSet<String> = std::collections::HashSet::new();
    for entry in &mut prepared {
        if entry.report.outcome != "valid" {
            continue;
        }
        // A row whose id is the key of a task of this workspace (a file exported from here) is that task already.
        let existing = match entry
            .source_key
            .strip_prefix("ext:T-")
            .and_then(|number| number.parse::<i64>().ok())
        {
            Some(number) => state
                .storage
                .find_collaboration_task_by_number(&workspace_id, number)
                .await
                .map_err(map_storage_error)?
                .map(|task| task.key),
            None => None,
        };
        if let Some(key) = existing {
            entry.report.outcome = "duplicate".to_owned();
            entry
                .report
                .problems
                .push(format!("{key} already exists in this workspace: this row is skipped."));
        } else if known.contains(&entry.source_key) {
            entry.report.outcome = "duplicate".to_owned();
            entry
                .report
                .problems
                .push("Already imported: this row is skipped.".to_owned());
        } else if entry.source_key.starts_with("ext:") && !ids_in_file.insert(entry.source_key.clone()) {
            entry.report.outcome = "duplicate".to_owned();
            entry
                .report
                .problems
                .push("The same id appears earlier in this file: this row is skipped.".to_owned());
        }
    }
    let count = |outcome: &str| prepared.iter().filter(|entry| entry.report.outcome == outcome).count() as i64;
    let (valid, invalid, duplicates) = (count("valid"), count("invalid"), count("duplicate"));
    let refused = !request.dry_run && invalid > 0 && !request.skip_invalid;
    let mut created = 0;
    if !request.dry_run && !refused {
        for entry in &mut prepared {
            let Some(row) = entry.row.take().filter(|_| entry.report.outcome == "valid") else {
                continue;
            };
            match state
                .storage
                .import_task_row(&workspace_id, &subject.user_id, &entry.source_key, row)
                .await
                .map_err(map_storage_error)?
            {
                Some(task) => {
                    entry.report.outcome = "created".to_owned();
                    entry.report.task_key = Some(task.key);
                    created += 1;
                }
                None => {
                    // Imported by another attempt in the meantime.
                    entry.report.outcome = "duplicate".to_owned();
                    entry
                        .report
                        .problems
                        .push("Already imported: this row is skipped.".to_owned());
                }
            }
        }
        if created > 0 {
            record_workspace_activity(
                &state,
                &workspace_id,
                &subject.user_id,
                "tasks_imported",
                "task",
                None,
                format!("Imported {created} task(s) from a CSV."),
            )
            .await;
        }
    }
    let note = if request.dry_run {
        "This is a preview: nothing was written. Send it again with dry_run false to import the valid rows.".to_owned()
    } else if refused {
        format!("Nothing was imported: {invalid} row(s) have problems. Fix them, or send skip_invalid true to import the rest.")
    } else {
        format!("Imported {created} task(s); {invalid} invalid and {duplicates} already-imported row(s) were skipped.")
    };
    let report = CsvImportReport {
        dry_run: request.dry_run,
        committed: !request.dry_run && !refused,
        rows: data.len() as i64,
        valid,
        invalid,
        duplicates,
        created,
        mapping: mapping_used,
        unknown_assignees,
        items: prepared.into_iter().map(|entry| entry.report).collect(),
        note,
    };
    Ok((
        if refused { StatusCode::CONFLICT } else { StatusCode::OK },
        Json(report),
    ))
}

#[cfg(test)]
mod tests {
    use super::{csv_cell, parse_csv, unescape_cell};

    #[test]
    fn a_cell_that_a_spreadsheet_could_run_is_written_as_text() {
        assert_eq!(csv_cell("Plain"), "Plain");
        assert_eq!(csv_cell("=SUM(A1:A9)"), "\"'=SUM(A1:A9)\"");
        assert_eq!(csv_cell("+1"), "\"'+1\"");
        assert_eq!(csv_cell("-2"), "\"'-2\"");
        assert_eq!(csv_cell("@cmd"), "\"'@cmd\"");
        assert_eq!(csv_cell("a, b"), "\"a, b\"");
        assert_eq!(csv_cell("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_cell("two\nlines"), "\"two\nlines\"");
        assert_eq!(unescape_cell("'=SUM(A1)"), "=SUM(A1)");
        assert_eq!(
            unescape_cell("'quoted"),
            "'quoted",
            "only a formula's apostrophe is removed"
        );
    }

    #[test]
    fn the_reader_handles_quotes_line_breaks_a_byte_order_mark_and_blank_lines() {
        let rows = parse_csv("\u{feff}a,b\r\n\"x, y\",\"say \"\"hi\"\"\"\n\n\"one\ntwo\",z").unwrap();
        assert_eq!(
            rows,
            vec![
                vec!["a".to_owned(), "b".to_owned()],
                vec!["x, y".to_owned(), "say \"hi\"".to_owned()],
                vec!["one\ntwo".to_owned(), "z".to_owned()],
            ]
        );
        assert_eq!(
            parse_csv("a,b,\n,,").unwrap(),
            vec![vec!["a", "b", ""], vec!["", "", ""]]
        );
        assert!(parse_csv("a,\"b").unwrap_err().contains("not closed"));
        assert!(parse_csv("a,b\"c\"").unwrap_err().contains("quote"));
        assert!(parse_csv("\"a\"b").unwrap_err().contains("closing quote"));
    }
}
