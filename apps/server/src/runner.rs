//! Running the server until it is asked to stop: what `main` and the Windows service both do (V0.4.33).

use std::future::Future;

use tokio::net::TcpListener;
use tracing::info;

use crate::{init_logging, serve, stderr_line, ServerConfig};

/// Starts the server and serves until `shutdown` resolves, then stops it cleanly (V0.4.6). Returns the exit code:
/// 0 after a clean stop, 1 when the configuration, the data folders or the start were refused.
pub async fn run_server(shutdown: impl Future<Output = ()>) -> i32 {
    // Console, in-memory buffer for System › Logs and optional daily files;
    // levels and format follow the environment until a system administrator
    // changes them in System › Settings.
    if let Err(error) = init_logging() {
        eprintln!("Invalid RepoMemo logging configuration: {error:#}");
        return 1;
    }

    let config = match ServerConfig::from_env() {
        Ok(config) => config,
        Err(error) => {
            eprintln!(
                "{}",
                stderr_line("error", &format!("Invalid RepoMemo server configuration: {error:#}"))
            );
            return 1;
        }
    };
    // A read-only, full or networked data folder is named now, not by a failure later on.
    let folders = crate::startup_report(&config);
    for warning in &folders.warnings {
        tracing::warn!("{warning}");
    }
    if let Some(error) = folders.errors.first() {
        tracing::error!("{error}");
        eprintln!(
            "{}",
            stderr_line("error", &format!("RepoMemo cannot use its data folders: {error}"))
        );
        return 1;
    }
    let address = config.bind_address;
    let header_read_timeout = config.header_read_timeout;
    let shutdown_grace = config.shutdown_grace;
    let listener = match TcpListener::bind(address).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!(
                "{}",
                stderr_line("error", &format!("RepoMemo cannot listen on {address}: {error}"))
            );
            return 1;
        }
    };

    info!(%address, "RepoMemo shared-backend foundation is listening");
    let server = match crate::start(config).await {
        Ok(server) => server,
        Err(error) => {
            eprintln!(
                "{}",
                stderr_line("error", &format!("RepoMemo could not start: {error:#}"))
            );
            return 1;
        }
    };
    let app = server.router();
    // On a stop: the live streams end, running requests finish within the grace, then the work stops,
    // interrupted jobs are closed and the write-ahead log is folded back (V0.4.6).
    let served = serve(listener, app, header_read_timeout, shutdown_grace, async {
        shutdown.await;
        server.begin_closing().await;
    })
    .await;
    server.finish().await;
    match served {
        Ok(()) => {
            info!("RepoMemo server stopped");
            0
        }
        Err(error) => {
            eprintln!(
                "{}",
                stderr_line("error", &format!("RepoMemo server stopped unexpectedly: {error}"))
            );
            1
        }
    }
}

/// Resolves on Ctrl-C, or SIGTERM on Unix (what service managers and
/// containers send), so in-flight requests finish before the process exits.
pub async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(error = %error, "Failed to listen for Ctrl-C");
            std::future::pending::<()>().await;
        }
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => {
                tracing::error!(error = %error, "Failed to listen for SIGTERM");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
    info!("Shutdown requested; finishing in-flight requests");
}
