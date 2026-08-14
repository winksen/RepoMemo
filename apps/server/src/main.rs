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
                .unwrap_or_else(|_| "repomemo_server=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let address = std::env::var("REPOMEMO_SERVER_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8787".to_owned())
        .parse::<SocketAddr>()
        .expect("REPOMEMO_SERVER_ADDR must be a valid socket address");
    let listener = TcpListener::bind(address)
        .await
        .expect("failed to bind RepoMemo server address");

    info!(%address, "RepoMemo shared-backend foundation is listening");
    axum::serve(listener, router(ServerConfig::from_env()))
        .await
        .expect("RepoMemo server stopped unexpectedly");
}
