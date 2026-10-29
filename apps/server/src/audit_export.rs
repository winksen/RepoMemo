//! Export of the system audit trail (roadmap O8).
//!
//! The trail is what system administrators did, kept in the database. An
//! export hands a date range of it to someone who keeps such records
//! elsewhere (an auditor, a log store), as JSON lines or CSV:
//!
//! * It is **streamed in pages**, oldest first, so a long trail never sits in
//!   memory and the download starts at once.
//! * It stops at the moment the export began, so the file is a consistent cut.
//! * **The download is itself audited** (`audit_exported`, with the range and
//!   format) before the first byte is sent, so even a download that is cut short
//!   is on record. The cut is the moment the export began, so a file does not
//!   contain its own record; the next export does.
//! * CSV cells that a spreadsheet would run as a formula (they start with
//!   `=`, `+`, `-`, `@`, a tab or a return) are prefixed with an apostrophe,
//!   because the detail of an event can carry text people typed.
//!
//! Only full system administrators export, with a session or a token that has
//! the `system` scope.

use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Duration, NaiveDate, SecondsFormat, Utc};
use repomemo_domain::SystemAuditEvent;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::mpsc::Sender;
use tokio_stream::wrappers::ReceiverStream;

use crate::system::{audit, require_full_system_admin, require_system_change};
use crate::{map_storage_error, ApiError, AppState, AuthenticatedSubject};

/// Events read from the database at a time.
const PAGE: i64 = 1000;
const PIPELINE_DEPTH: usize = 4;
const CSV_HEADER: &str = "id,time,actor_id,actor_email,actor_name,action,detail\r\n";

type Piece = Result<Vec<u8>, std::io::Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    JsonLines,
    Csv,
}

impl Format {
    fn parse(value: Option<&str>) -> Result<Self, ApiError> {
        match value.map(str::trim) {
            None | Some("") | Some("jsonl") | Some("json") | Some("ndjson") => Ok(Self::JsonLines),
            Some("csv") => Ok(Self::Csv),
            Some(other) => Err(ApiError::bad_request(format!(
                "The format must be jsonl or csv, not {other}."
            ))),
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Self::JsonLines => "jsonl",
            Self::Csv => "csv",
        }
    }

    fn content_type(self) -> &'static str {
        match self {
            Self::JsonLines => "application/x-ndjson",
            Self::Csv => "text/csv; charset=utf-8",
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ExportQuery {
    /// `jsonl` (the default) or `csv`.
    format: Option<String>,
    /// The first day (`2026-10-01`) or moment (RFC 3339) to include.
    from: Option<String>,
    /// The last day to include (`2026-10-15`), or the moment (RFC 3339) to stop before.
    to: Option<String>,
    /// `identity` for the provider and sign-in events only; the default is every event.
    scope: Option<String>,
}

/// A day, or a moment, as the RFC 3339 time stored events are compared with.
/// `end_of_day` turns a day into the start of the next one, so `to=2026-10-15`
/// includes all of the 15th.
fn parse_bound(value: &str, end_of_day: bool, name: &str) -> Result<String, ApiError> {
    let value = value.trim();
    if let Ok(day) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        let day = if end_of_day { day + Duration::days(1) } else { day };
        let start = day.and_hms_opt(0, 0, 0).expect("midnight exists").and_utc();
        return Ok(start.to_rfc3339_opts(SecondsFormat::AutoSi, false));
    }
    DateTime::parse_from_rfc3339(value)
        .map(|moment| moment.with_timezone(&Utc).to_rfc3339_opts(SecondsFormat::AutoSi, false))
        .map_err(|_| {
            ApiError::bad_request(format!(
                "\"{name}\" must be a day such as 2026-10-15 or a time such as 2026-10-15T09:30:00Z."
            ))
        })
}

/// `GET /v1/system/audit/export?format=&from=&to=`: the audit trail of a
/// date range as a download.
pub(crate) async fn export_audit(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Query(query): Query<ExportQuery>,
) -> Result<Response, ApiError> {
    require_full_system_admin(&subject)?;
    // The whole trail is sensitive: a token needs the system scope, as for any System change.
    require_system_change(&subject)?;
    let format = Format::parse(query.format.as_deref())?;
    let identity_only = match query.scope.as_deref().map(str::trim) {
        None | Some("") | Some("all") => false,
        Some("identity") => true,
        Some(other) => {
            return Err(ApiError::bad_request(format!(
                "The scope must be all or identity, not {other}."
            )))
        }
    };
    let now = Utc::now().to_rfc3339_opts(SecondsFormat::AutoSi, false);
    let from = query
        .from
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| parse_bound(value, false, "from"))
        .transpose()?;
    // A cut at the moment the export began: later events are not in this file.
    let until = match query.to.as_deref().filter(|value| !value.trim().is_empty()) {
        Some(value) => {
            let bound = parse_bound(value, true, "to")?;
            if bound < now {
                bound
            } else {
                now.clone()
            }
        }
        None => now.clone(),
    };
    if from.as_deref().is_some_and(|from| from >= until.as_str()) {
        return Err(ApiError::bad_request("\"from\" must be earlier than \"to\"."));
    }

    let described = format!(
        "{} to {}",
        query
            .from
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("the start"),
        query
            .to
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("now")
    );
    // Recorded before anything is sent, so a download that is cut short is still on record.
    audit(
        &state,
        &subject,
        "audit_exported",
        format!(
            "Exported the {}audit trail from {described} as {}.",
            if identity_only { "identity " } else { "" },
            format.extension()
        ),
    )
    .await;
    let total = state
        .storage
        .count_system_events_scoped(from.as_deref(), Some(&until), identity_only)
        .await
        .map_err(map_storage_error)?;

    let filename = format!(
        "repomemo-{}audit-{}-{}.{}",
        if identity_only { "identity-" } else { "" },
        short_day(query.from.as_deref(), "start"),
        short_day(query.to.as_deref(), "now"),
        format.extension()
    );
    let (sender, receiver) = tokio::sync::mpsc::channel::<Piece>(PIPELINE_DEPTH);
    let storage = state.storage.clone();
    tokio::spawn(async move {
        if let Err(error) = stream_events(&storage, from.as_deref(), &until, format, identity_only, &sender).await {
            tracing::error!(error = %format!("{error:#}"), "An audit export stopped half-way");
            let _ = sender.send(Err(std::io::Error::other(error.to_string()))).await;
        }
    });

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(format.content_type()));
    if let Ok(value) = HeaderValue::from_str(&format!("attachment; filename=\"{filename}\"")) {
        headers.insert(header::CONTENT_DISPOSITION, value);
    }
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(HeaderName::from_static("x-total-count"), HeaderValue::from(total));
    Ok((
        StatusCode::OK,
        headers,
        Body::from_stream(ReceiverStream::new(receiver)),
    )
        .into_response())
}

/// A day for a file name: the digits and dashes of what was asked, or `fallback`.
fn short_day(value: Option<&str>, fallback: &str) -> String {
    let day: String = value
        .map(str::trim)
        .unwrap_or("")
        .chars()
        .take(10)
        .filter(|character| character.is_ascii_digit() || *character == '-')
        .collect();
    if day.len() == 10 {
        day
    } else {
        fallback.to_owned()
    }
}

async fn stream_events(
    storage: &repomemo_storage::StorageEngine,
    from: Option<&str>,
    until: &str,
    format: Format,
    identity_only: bool,
    sender: &Sender<Piece>,
) -> anyhow::Result<()> {
    if format == Format::Csv && sender.send(Ok(CSV_HEADER.as_bytes().to_vec())).await.is_err() {
        return Ok(());
    }
    let mut after: Option<(String, String)> = None;
    loop {
        let page = storage
            .list_system_events_scoped(
                from,
                Some(until),
                after.as_ref().map(|(at, id)| (at.as_str(), id.as_str())),
                PAGE,
                identity_only,
            )
            .await?;
        let Some(last) = page.last() else {
            return Ok(());
        };
        after = Some((last.created_at.clone(), last.id.clone()));
        let full = page.len() as i64 == PAGE;
        let mut text = String::new();
        for event in &page {
            match format {
                Format::JsonLines => text.push_str(&json_line(event)),
                Format::Csv => text.push_str(&csv_row(event)),
            }
        }
        if sender.send(Ok(text.into_bytes())).await.is_err() {
            return Ok(());
        }
        if !full {
            return Ok(());
        }
    }
}

/// One event as a line of JSON.
pub(crate) fn json_line(event: &SystemAuditEvent) -> String {
    let actor = event
        .actor
        .as_ref()
        .map(|actor| json!({"id": actor.id, "email": actor.email, "display_name": actor.display_name}));
    let mut line = json!({
        "id": event.id,
        "time": event.created_at,
        "actor": actor,
        "action": event.action,
        "detail": event.detail,
    })
    .to_string();
    line.push('\n');
    line
}

/// One event as a CSV row.
pub(crate) fn csv_row(event: &SystemAuditEvent) -> String {
    let (id, email, name) = match &event.actor {
        Some(actor) => (
            actor.id.as_str(),
            actor.email.as_deref().unwrap_or(""),
            actor.display_name.as_str(),
        ),
        None => ("", "", ""),
    };
    let cells = [
        event.id.as_str(),
        event.created_at.as_str(),
        id,
        email,
        name,
        event.action.as_str(),
        event.detail.as_str(),
    ];
    let mut row = cells.iter().map(|cell| csv_cell(cell)).collect::<Vec<_>>().join(",");
    row.push_str("\r\n");
    row
}

/// A CSV cell: quoted when it holds a comma, quote or line break, and made
/// harmless to a spreadsheet when it would otherwise start a formula.
pub(crate) fn csv_cell(value: &str) -> String {
    let mut text = value.to_owned();
    if text.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        text.insert(0, '\'');
    }
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use repomemo_domain::SharedUser;

    fn event(detail: &str) -> SystemAuditEvent {
        SystemAuditEvent {
            id: "e1".to_owned(),
            created_at: "2026-10-15T10:00:00.5+00:00".to_owned(),
            actor: Some(SharedUser {
                id: "u1".to_owned(),
                display_name: "Ada, the \"admin\"".to_owned(),
                email: Some("ada@example.com".to_owned()),
            }),
            action: "setting_changed".to_owned(),
            detail: detail.to_owned(),
        }
    }

    #[test]
    fn csv_cells_are_quoted_and_cannot_run_as_formulas() {
        assert_eq!(csv_cell("plain"), "plain");
        assert_eq!(csv_cell("a, b"), "\"a, b\"");
        assert_eq!(csv_cell("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_cell("line\nbreak"), "\"line\nbreak\"");
        for formula in ["=SUM(A1:A9)", "+1", "-2", "@cmd", "\tx", "\rx"] {
            assert!(
                csv_cell(formula).trim_matches('"').starts_with('\''),
                "{formula:?} was left as it is"
            );
        }
        assert_eq!(
            csv_cell("=HYPERLINK(\"http://x\",\"y\")"),
            "\"'=HYPERLINK(\"\"http://x\"\",\"\"y\"\")\""
        );
        assert_eq!(csv_cell("2026-10-15T10:00:00+00:00"), "2026-10-15T10:00:00+00:00");
    }

    #[test]
    fn a_row_has_every_column_and_a_system_event_has_no_actor() {
        let row = csv_row(&event("Changed it."));
        assert_eq!(
            row,
            "e1,2026-10-15T10:00:00.5+00:00,u1,ada@example.com,\"Ada, the \"\"admin\"\"\",setting_changed,Changed it.\r\n"
        );
        let mut system = event("x");
        system.actor = None;
        assert_eq!(
            csv_row(&system),
            "e1,2026-10-15T10:00:00.5+00:00,,,,setting_changed,x\r\n"
        );
    }

    #[test]
    fn a_json_line_is_one_object_and_one_line() {
        let line = json_line(&event("Two\nlines"));
        assert!(line.ends_with('\n') && line.matches('\n').count() == 1);
        let value: serde_json::Value = serde_json::from_str(line.trim_end()).unwrap();
        assert_eq!(value["actor"]["email"], "ada@example.com");
        assert_eq!(value["detail"], "Two\nlines");
        assert_eq!(value["time"], "2026-10-15T10:00:00.5+00:00");
    }

    #[test]
    fn days_and_moments_become_the_times_events_are_compared_with() {
        assert_eq!(
            parse_bound("2026-10-01", false, "from").unwrap(),
            "2026-10-01T00:00:00+00:00"
        );
        assert_eq!(
            parse_bound("2026-10-15", true, "to").unwrap(),
            "2026-10-16T00:00:00+00:00"
        );
        assert_eq!(
            parse_bound("2026-10-15T12:30:00+02:00", true, "to").unwrap(),
            "2026-10-15T10:30:00+00:00"
        );
        assert!(parse_bound("yesterday", false, "from").is_err());
        assert!(parse_bound("2026-13-40", false, "from").is_err());
        assert!(Format::parse(Some("xml")).is_err());
        assert_eq!(Format::parse(None).unwrap(), Format::JsonLines);
        assert_eq!(short_day(Some("2026-10-01"), "start"), "2026-10-01");
        assert_eq!(short_day(Some("../../etc"), "start"), "start");
        assert_eq!(short_day(None, "now"), "now");
    }
}
