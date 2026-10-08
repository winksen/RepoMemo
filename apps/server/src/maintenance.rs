//! Periodic housekeeping that keeps a long-running server healthy.
//!
//! Each pass, every `maintenance_interval`:
//! - purges expired refresh tokens and finished jobs past their retention;
//! - deletes blobs no artifact references any more, and their cached
//!   previews (blob garbage collection);
//! - retries indexing that failed and embeddings that are missing, so a
//!   provider that comes back is picked up without a restart;
//! - forgets idle rate-limit counters, event channels and client sightings,
//!   and system audit events past their retention;
//! - lets SQLite refresh its statistics and fold its write-ahead log back.
//!
//! A step that fails is logged and the others still run. The interval and the
//! other settings are read again before every pass, so changes a system
//! administrator makes apply without a restart. A system administrator can
//! also start a pass by hand ([`run_tracked`]).

use std::time::{Duration, Instant};

use chrono::Utc;
use serde::Serialize;

use crate::AppState;

/// Blobs stored again (or newly stored) within this window are never
/// collected, whatever references them yet.
const BLOB_GRACE: Duration = Duration::from_secs(3600);
/// Blobs examined per pass, so a large backlog is spread over passes.
const BLOB_BATCH: i64 = 2_000;
/// Files in the blob folder with no record are swept this often.
const UNTRACKED_SWEEP_EVERY: Duration = Duration::from_secs(24 * 3600);
/// First pass shortly after startup, once startup work has settled.
const FIRST_PASS_DELAY: Duration = Duration::from_secs(60);
/// While maintenance is turned off, the setting is checked again this often.
const DISABLED_RECHECK: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
pub struct MaintenanceSettings {
    pub interval: Duration,
    /// Delete unreferenced blobs and their previews.
    pub blob_gc: bool,
    /// Finished jobs are kept this many days; 0 keeps them forever.
    pub job_retention_days: i64,
    /// Indexing that failed for good is retried this often.
    pub index_retry_interval: Duration,
}

impl Default for MaintenanceSettings {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(3600),
            blob_gc: true,
            job_retention_days: 90,
            index_retry_interval: Duration::from_secs(6 * 3600),
        }
    }
}

/// What one pass did, for the log, the System page and tests.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct MaintenanceReport {
    pub refresh_tokens_purged: u64,
    pub jobs_pruned: u64,
    pub blobs_removed: usize,
    pub blob_bytes_reclaimed: i64,
    pub previews_removed: usize,
    pub untracked_files_removed: u64,
    pub counters_pruned: usize,
    pub channels_pruned: usize,
    pub system_events_pruned: u64,
    pub failures: usize,
}

impl MaintenanceReport {
    fn is_noteworthy(&self) -> bool {
        self.refresh_tokens_purged > 0
            || self.jobs_pruned > 0
            || self.blobs_removed > 0
            || self.previews_removed > 0
            || self.untracked_files_removed > 0
            || self.failures > 0
    }
}

/// Starts the maintenance loop on the current runtime.
pub fn start(state: AppState) {
    if state.settings().maintenance.interval.is_zero() {
        tracing::info!("Background maintenance is turned off");
    }
    tokio::spawn(async move {
        let mut wait = FIRST_PASS_DELAY.min(state.settings().maintenance.interval.max(DISABLED_RECHECK));
        let mut last_index_retry = Instant::now();
        let mut last_untracked_sweep: Option<Instant> = None;
        loop {
            set_next_run(&state, Some(wait));
            tokio::time::sleep(wait).await;
            let settings = state.settings().maintenance.clone();
            if settings.interval.is_zero() {
                set_next_run(&state, None);
                wait = DISABLED_RECHECK;
                continue;
            }
            let retry_indexing = last_index_retry.elapsed() >= settings.index_retry_interval;
            if retry_indexing {
                last_index_retry = Instant::now();
            }
            let sweep_untracked = match last_untracked_sweep {
                Some(last) => last.elapsed() >= UNTRACKED_SWEEP_EVERY,
                None => true,
            };
            if sweep_untracked {
                last_untracked_sweep = Some(Instant::now());
            }
            if let Some(report) = run_tracked(&state, "scheduled", retry_indexing, sweep_untracked).await {
                if report.is_noteworthy() {
                    tracing::info!(?report, "Background maintenance pass finished");
                } else {
                    tracing::debug!(?report, "Background maintenance pass finished");
                }
            }
            wait = state.settings().maintenance.interval.max(DISABLED_RECHECK);
        }
    });
}

fn set_next_run(state: &AppState, wait: Option<Duration>) {
    if let Ok(mut status) = state.maintenance_status.lock() {
        status.next_run_at = wait.and_then(|wait| chrono::Duration::from_std(wait).ok()).map(|wait| (Utc::now() + wait).to_rfc3339());
    }
}

/// Runs a pass and records it for the System page. Returns `None` without
/// running when another pass is still in progress.
pub async fn run_tracked(
    state: &AppState,
    trigger: &'static str,
    retry_indexing: bool,
    sweep_untracked: bool,
) -> Option<MaintenanceReport> {
    {
        let mut status = state.maintenance_status.lock().ok()?;
        if status.running {
            return None;
        }
        status.running = true;
        status.last_trigger = Some(trigger);
        status.last_started_at = Some(Utc::now().to_rfc3339());
    }
    let report = run_pass(state, retry_indexing, sweep_untracked).await;
    if let Ok(mut status) = state.maintenance_status.lock() {
        status.running = false;
        status.last_finished_at = Some(Utc::now().to_rfc3339());
        status.last_report = Some(report.clone());
    }
    Some(report)
}

/// Runs one maintenance pass.
pub async fn run_pass(state: &AppState, retry_indexing: bool, sweep_untracked: bool) -> MaintenanceReport {
    let runtime = state.settings();
    let settings = &runtime.maintenance;
    let storage = &state.storage;
    let mut report = MaintenanceReport::default();
    fn failed(report: &mut MaintenanceReport, step: &str, error: anyhow::Error) {
        tracing::warn!(step, error = %format!("{error:#}"), "A maintenance step failed");
        report.failures += 1;
    }

    match storage
        .purge_expired_refresh_tokens(&Utc::now().to_rfc3339())
        .await
    {
        Ok(count) => report.refresh_tokens_purged = count,
        Err(error) => failed(&mut report, "refresh tokens", error),
    }

    if settings.job_retention_days > 0 {
        let cutoff = (Utc::now() - chrono::Duration::days(settings.job_retention_days)).to_rfc3339();
        match storage.prune_finished_jobs(&cutoff).await {
            Ok(count) => report.jobs_pruned = count,
            Err(error) => failed(&mut report, "jobs", error),
        }
    }

    if settings.blob_gc {
        let grace = chrono::Duration::from_std(BLOB_GRACE).unwrap_or_else(|_| chrono::Duration::hours(1));
        match storage.collect_orphan_blobs(grace, BLOB_BATCH).await {
            Ok(collection) => {
                report.blobs_removed = collection.removed.len();
                report.blob_bytes_reclaimed = collection.bytes;
                for hash in &collection.removed {
                    if state.converter.forget(hash) {
                        report.previews_removed += 1;
                    }
                }
            }
            Err(error) => failed(&mut report, "blobs", error),
        }
        match state.converter.prune_orphans(storage).await {
            Ok(count) => report.previews_removed += count,
            Err(error) => failed(&mut report, "previews", error),
        }
        if sweep_untracked {
            match storage.sweep_untracked_blob_files(BLOB_GRACE).await {
                Ok(count) => report.untracked_files_removed = count,
                Err(error) => failed(&mut report, "untracked blob files", error),
            }
        }
    }

    if runtime.system_audit_retention_days > 0 {
        let cutoff = (Utc::now() - chrono::Duration::days(runtime.system_audit_retention_days)).to_rfc3339();
        match storage.prune_system_events(&cutoff).await {
            Ok(count) => report.system_events_pruned = count,
            Err(error) => failed(&mut report, "system audit events", error),
        }
    }

    report.counters_pruned = state.guards.prune() + state.telemetry.prune();
    report.channels_pruned = state.event_bus.prune();

    if retry_indexing {
        state.index_queue.resume_pending().await;
    }
    // Cheap when nothing is missing: no job is created for an up-to-date
    // workspace or one without an embedding provider.
    state.embedding_queue.resume_pending().await;

    if let Err(error) = storage.optimize().await {
        failed(&mut report, "database optimize", error);
    }
    report
}
