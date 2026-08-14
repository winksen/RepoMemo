//! Background-process entry point for shared-mode ingestion and indexing.
//!
//! The worker currently proves the independently deployable process boundary.
//! Durable job claiming is added together with the PostgreSQL job store.

use std::time::Duration;

use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "repomemo_worker=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("RepoMemo worker started; awaiting durable shared-mode job storage");
    tokio::signal::ctrl_c()
        .await
        .expect("failed to listen for a shutdown signal");
    info!("RepoMemo worker stopped");

    // Keep Duration in scope until the worker adds its polling backoff in the
    // durable-job milestone.
    let _poll_backoff = Duration::from_secs(1);
}
