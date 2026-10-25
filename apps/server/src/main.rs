use repomemo_server::{init_logging, router, serve, ServerConfig};
use tokio::net::TcpListener;
use tracing::info;

// Counts memory only in a document reader process, which asks it to; the
// server itself pays for one flag check per allocation.
#[global_allocator]
static ALLOCATOR: repomemo_api::LimitedAllocator = repomemo_api::LimitedAllocator;

fn main() {
    // A document reading runs in a child process: this program, started with
    // an argument. It reads one file, answers and exits.
    if repomemo_api::run_as_document_reader_if_requested() {
        return;
    }
    // git runs this program as its credential helper when it fetches a remote
    // repository with a token: answer it and leave, without starting a server.
    if repomemo_git::remote::run_as_askpass_if_requested() {
        return;
    }
    if std::env::args().any(|argument| argument == "--version" || argument == "-V") {
        println!("{}", repomemo_server::BUILD.line());
        return;
    }
    // `--check-config` reads the environment as a start would, reports it and exits.
    if std::env::args().any(|argument| argument == "--check-config") {
        std::process::exit(repomemo_server::check_config());
    }
    run();
}

#[tokio::main]
async fn run() {
    // Console, in-memory buffer for System › Logs and optional daily files;
    // levels and format follow the environment until a system administrator
    // changes them in System › Settings.
    init_logging().expect("Invalid RepoMemo logging configuration");

    let config = match ServerConfig::from_env() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("Invalid RepoMemo server configuration: {error:#}");
            std::process::exit(1);
        }
    };
    let address = config.bind_address;
    let header_read_timeout = config.header_read_timeout;
    let listener = TcpListener::bind(address)
        .await
        .expect("failed to bind RepoMemo server address");

    info!(%address, "RepoMemo shared-backend foundation is listening");
    let app = match router(config).await {
        Ok(app) => app,
        Err(error) => {
            eprintln!("RepoMemo could not start: {error:#}");
            std::process::exit(1);
        }
    };
    serve(listener, app, header_read_timeout, shutdown_signal())
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
