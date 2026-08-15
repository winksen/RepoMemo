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

    let config = ServerConfig::from_env().expect("Invalid RepoMemo server configuration");
    let address = config.bind_address;
    let listener = TcpListener::bind(address)
        .await
        .expect("failed to bind RepoMemo server address");

    info!(%address, "RepoMemo shared-backend foundation is listening");
    let app = router(config)
        .await
        .expect("Failed to initialize RepoMemo server storage");
    axum::serve(listener, app)
        .await
        .expect("RepoMemo server stopped unexpectedly");
}
