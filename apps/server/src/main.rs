use std::net::SocketAddr;

use repomemo_server::{router, ServerConfig};
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "repomemo_server=info,tower_http=info,audit=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = ServerConfig::from_env().expect("Invalid RepoMemo server configuration");
    let address = config.bind_address;
    let listener = TcpListener::bind(address)
        .await
        .expect("failed to bind RepoMemo server address");

    info!(%address, "RepoMemo shared-backend foundation is listening");
    let app = router(config)
        .await
        .expect("Failed to initialize RepoMemo server storage");
    // The peer address keys the sign-in rate limits.
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("RepoMemo server stopped unexpectedly");
    info!("RepoMemo server stopped");
}

/// Resolves on Ctrl-C, or SIGTERM on Unix (what service managers and
/// containers send), so in-flight requests finish before the process exits.
async fn shutdown_signal() {
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
