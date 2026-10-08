//! Recent server log events kept in memory for system administrators.
//!
//! `main.rs` adds [`log_capture_layer`] to the tracing subscriber, so every
//! event that passes the log filter is also kept in a bounded ring buffer. The
//! System › Logs page reads it through the API. Security events (the `audit`
//! target) get their own buffer so ordinary noise cannot push them out.
//! Nothing here is written to disk; the server's console or log collector
//! remains the durable record.

use std::{
    collections::VecDeque,
    fmt::Write as _,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, OnceLock,
    },
};

use chrono::Utc;
use serde::Serialize;
use tracing::{field::Field, Event, Level, Subscriber};
use tracing_subscriber::{layer::Context, Layer};

const GENERAL_CAPACITY: usize = 2_000;
const AUDIT_CAPACITY: usize = 1_000;
const MAX_MESSAGE_CHARS: usize = 2_000;

#[derive(Debug, Clone, Serialize)]
pub struct LogRecord {
    /// Increases with every captured event; use it to fetch only newer ones.
    pub sequence: u64,
    pub timestamp: String,
    pub level: &'static str,
    pub target: String,
    pub message: String,
    /// The event's other fields as `name=value` pairs.
    pub fields: String,
}

pub struct LogBuffer {
    sequence: AtomicU64,
    general: Mutex<VecDeque<LogRecord>>,
    audit: Mutex<VecDeque<LogRecord>>,
}

static BUFFER: OnceLock<LogBuffer> = OnceLock::new();

/// The process-wide buffer, when log capture was installed.
pub fn buffer() -> Option<&'static LogBuffer> {
    BUFFER.get()
}

/// A tracing layer that copies events into the in-memory buffer. Install it
/// once, next to the formatting layer.
pub fn log_capture_layer() -> LogCaptureLayer {
    BUFFER.get_or_init(|| LogBuffer {
        sequence: AtomicU64::new(0),
        general: Mutex::new(VecDeque::with_capacity(GENERAL_CAPACITY)),
        audit: Mutex::new(VecDeque::with_capacity(AUDIT_CAPACITY)),
    });
    LogCaptureLayer { _private: () }
}

pub struct LogCaptureLayer {
    _private: (),
}

impl<S: Subscriber> Layer<S> for LogCaptureLayer {
    fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
        let Some(buffer) = BUFFER.get() else { return };
        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);
        let metadata = event.metadata();
        let record = LogRecord {
            sequence: buffer.sequence.fetch_add(1, Ordering::Relaxed) + 1,
            timestamp: Utc::now().to_rfc3339(),
            level: level_name(metadata.level()),
            target: metadata.target().to_owned(),
            message: truncate(visitor.message),
            fields: truncate(visitor.fields),
        };
        let (queue, capacity) = if record.target == "audit" {
            (&buffer.audit, AUDIT_CAPACITY)
        } else {
            (&buffer.general, GENERAL_CAPACITY)
        };
        if let Ok(mut queue) = queue.lock() {
            if queue.len() == capacity {
                queue.pop_front();
            }
            queue.push_back(record);
        }
    }
}

/// Which events to return.
#[derive(Debug, Clone, Default)]
pub struct LogQuery {
    /// Only this level and more severe ones (`error`, `warn`, `info`, `debug`, `trace`).
    pub min_level: Option<String>,
    /// Only security events (the `audit` target).
    pub audit_only: bool,
    /// Text that must appear in the target, message or fields.
    pub contains: Option<String>,
    /// Only events after this sequence number.
    pub after: Option<u64>,
    pub limit: usize,
}

impl LogBuffer {
    /// Matching events, newest first.
    pub fn query(&self, query: &LogQuery) -> Vec<LogRecord> {
        let min_rank = query.min_level.as_deref().map(level_rank).unwrap_or(u8::MAX);
        let needle = query.contains.as_deref().map(str::to_lowercase).filter(|text| !text.is_empty());
        let collect = |queue: &Mutex<VecDeque<LogRecord>>| -> Vec<LogRecord> {
            queue
                .lock()
                .map(|queue| queue.iter().cloned().collect())
                .unwrap_or_default()
        };
        let mut records = collect(&self.audit);
        if !query.audit_only {
            records.extend(collect(&self.general));
        }
        records.retain(|record| {
            level_rank(record.level) <= min_rank
                && query.after.map_or(true, |after| record.sequence > after)
                && needle.as_ref().map_or(true, |needle| {
                    record.message.to_lowercase().contains(needle.as_str())
                        || record.target.to_lowercase().contains(needle.as_str())
                        || record.fields.to_lowercase().contains(needle.as_str())
                })
        });
        records.sort_by(|left, right| right.sequence.cmp(&left.sequence));
        records.truncate(query.limit.clamp(1, GENERAL_CAPACITY + AUDIT_CAPACITY));
        records
    }

    pub fn len(&self) -> usize {
        let size = |queue: &Mutex<VecDeque<LogRecord>>| queue.lock().map(|queue| queue.len()).unwrap_or(0);
        size(&self.general) + size(&self.audit)
    }
}

#[derive(Default)]
struct FieldVisitor {
    message: String,
    fields: String,
}

impl tracing::field::Visit for FieldVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message.push_str(value);
        } else {
            self.push(field, value);
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            let _ = write!(self.message, "{value:?}");
        } else {
            self.push(field, &format!("{value:?}"));
        }
    }
}

impl FieldVisitor {
    fn push(&mut self, field: &Field, value: &str) {
        if !self.fields.is_empty() {
            self.fields.push(' ');
        }
        let _ = write!(self.fields, "{}={value}", field.name());
    }
}

fn truncate(text: String) -> String {
    if text.chars().count() <= MAX_MESSAGE_CHARS {
        return text;
    }
    let mut cut = text.chars().take(MAX_MESSAGE_CHARS).collect::<String>();
    cut.push('…');
    cut
}

fn level_name(level: &Level) -> &'static str {
    match *level {
        Level::ERROR => "error",
        Level::WARN => "warn",
        Level::INFO => "info",
        Level::DEBUG => "debug",
        Level::TRACE => "trace",
    }
}

fn level_rank(level: &str) -> u8 {
    match level.to_ascii_lowercase().as_str() {
        "error" => 0,
        "warn" | "warning" => 1,
        "info" => 2,
        "debug" => 3,
        _ => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber::layer::SubscriberExt;

    #[test]
    fn events_are_captured_filtered_and_audit_events_kept_apart() {
        let subscriber = tracing_subscriber::registry().with(log_capture_layer());
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(target: "repomemo_server", count = 3, "Indexed evidence");
            tracing::warn!(target: "audit", user_id = "u1", "Sign-in failed");
            tracing::debug!(target: "repomemo_server", "Noise");
        });
        let buffer = buffer().unwrap();
        let all = buffer.query(&LogQuery { limit: 100, ..Default::default() });
        let indexed = all.iter().find(|record| record.message == "Indexed evidence").unwrap();
        assert_eq!(indexed.level, "info");
        assert_eq!(indexed.fields, "count=3");

        let audit = buffer.query(&LogQuery { audit_only: true, limit: 100, ..Default::default() });
        assert!(audit.iter().all(|record| record.target == "audit"));
        assert!(audit.iter().any(|record| record.fields.contains("user_id=u1")));

        let warnings = buffer.query(&LogQuery { min_level: Some("warn".to_owned()), limit: 100, ..Default::default() });
        assert!(warnings.iter().all(|record| record.level == "warn" || record.level == "error"));
        let found = buffer.query(&LogQuery { contains: Some("INDEXED".to_owned()), limit: 100, ..Default::default() });
        assert!(!found.is_empty());
        let newest = all.first().unwrap().sequence;
        assert!(buffer.query(&LogQuery { after: Some(newest), limit: 100, ..Default::default() }).is_empty());
    }
}
