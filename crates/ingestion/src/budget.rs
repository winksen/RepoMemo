//! A time budget for document readers (review M11, roadmap P9).
//!
//! The readers are pure functions over bytes, but a crafted PDF or workbook
//! can make one of them run for a very long time. Each reading runs on its own
//! thread and the caller stops waiting after the budget. A thread cannot be
//! killed, so an abandoned reading keeps running until it ends by itself; to
//! keep that from piling up, new readings are refused while too many
//! abandoned ones are still alive.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, bail, Result};

/// Seconds a reading may take unless the operator says otherwise.
pub const DEFAULT_TIME_LIMIT_SECONDS: u64 = 60;

/// Abandoned readings that may still be running before new ones are refused.
const MAX_ABANDONED: usize = 4;

static TIME_LIMIT_SECONDS: AtomicU64 = AtomicU64::new(DEFAULT_TIME_LIMIT_SECONDS);
static ABANDONED: AtomicUsize = AtomicUsize::new(0);

/// Sets how long one reading may take; 0 removes the limit (the reading then
/// still runs on its own thread).
pub fn set_time_limit(limit: Duration) {
    TIME_LIMIT_SECONDS.store(limit.as_secs(), Ordering::Relaxed);
}

/// Seconds one reading may take; 0 is no limit.
pub(crate) fn time_limit_seconds() -> u64 {
    TIME_LIMIT_SECONDS.load(Ordering::Relaxed)
}

/// Readings that ran over their budget and are still running.
pub fn abandoned_readings() -> usize {
    ABANDONED.load(Ordering::Relaxed)
}

#[derive(PartialEq, Eq)]
enum Phase {
    Running,
    Abandoned,
    Done,
}

/// Runs `work` on its own thread and waits for it at most the time limit.
pub(crate) fn within_budget<T, F>(what: &str, work: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    let seconds = TIME_LIMIT_SECONDS.load(Ordering::Relaxed);
    if abandoned_readings() >= MAX_ABANDONED {
        bail!("Document readers are busy with files that took too long. Try again later.");
    }
    let (sender, receiver) = mpsc::channel();
    let phase = Arc::new(Mutex::new(Phase::Running));
    let worker_phase = phase.clone();
    std::thread::Builder::new()
        .name("document-reader".to_owned())
        .spawn(move || {
            let outcome = work();
            let _ = sender.send(outcome);
            if let Ok(mut phase) = worker_phase.lock() {
                if *phase == Phase::Abandoned {
                    ABANDONED.fetch_sub(1, Ordering::Relaxed);
                }
                *phase = Phase::Done;
            }
        })
        .map_err(|error| anyhow!("could not start a document reader: {error}"))?;

    let received = if seconds == 0 {
        receiver.recv().map_err(|_| RecvTimeoutError::Disconnected)
    } else {
        receiver.recv_timeout(Duration::from_secs(seconds))
    };
    match received {
        Ok(outcome) => outcome,
        Err(RecvTimeoutError::Disconnected) => bail!("The {what} stopped unexpectedly. The file may be damaged."),
        Err(RecvTimeoutError::Timeout) => {
            let mut finished = false;
            if let Ok(mut phase) = phase.lock() {
                if *phase == Phase::Done {
                    finished = true;
                } else {
                    *phase = Phase::Abandoned;
                    ABANDONED.fetch_add(1, Ordering::Relaxed);
                }
            }
            // It finished in the moment between the timeout and the check.
            if finished {
                if let Ok(outcome) = receiver.try_recv() {
                    return outcome;
                }
            }
            bail!("The {what} took longer than {seconds} seconds, so the file was skipped.")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One test touches the shared limit, so they run as one.
    #[test]
    fn readings_stop_waiting_after_the_limit_and_report_failures() {
        set_time_limit(Duration::from_secs(5));
        assert_eq!(within_budget("test", || Ok(7)).unwrap(), 7);
        let failed = within_budget::<(), _>("test", || bail!("not a document")).unwrap_err();
        assert!(failed.to_string().contains("not a document"));
        let stopped = within_budget::<(), _>("test", || panic!("reader bug")).unwrap_err();
        assert!(stopped.to_string().contains("stopped unexpectedly"));

        set_time_limit(Duration::from_secs(1));
        let started = std::time::Instant::now();
        let slow = within_budget("test", || {
            std::thread::sleep(Duration::from_millis(2_500));
            Ok(())
        })
        .unwrap_err();
        assert!(slow.to_string().contains("took longer than 1 seconds"));
        assert!(started.elapsed() < Duration::from_millis(2_300));
        assert_eq!(abandoned_readings(), 1);
        std::thread::sleep(Duration::from_millis(1_800));
        assert_eq!(abandoned_readings(), 0, "the abandoned reading ended by itself");
        set_time_limit(Duration::from_secs(DEFAULT_TIME_LIMIT_SECONDS));
    }
}
