//! The accept loop: what `axum::serve` does, plus a limit on how long a client
//! may take to send its request headers (review N7). Without one, a client
//! that opens connections and trickles bytes holds them for as long as it
//! likes (slowloris).

use std::{future::Future, net::SocketAddr, time::Duration};

use axum::{extract::ConnectInfo, Extension, Router};
use hyper_util::{
    rt::{TokioExecutor, TokioIo, TokioTimer},
    server::{conn::auto::Builder, graceful::GracefulShutdown},
    service::TowerToHyperService,
};
use tokio::net::TcpListener;

/// How long in-flight requests get to finish once shutdown is requested.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(30);

/// Serves `app` on `listener` until `shutdown` resolves. A zero
/// `header_read_timeout` leaves header reads unlimited.
pub async fn serve(
    listener: TcpListener,
    app: Router,
    header_read_timeout: Duration,
    shutdown: impl Future<Output = ()>,
) -> std::io::Result<()> {
    let mut builder = Builder::new(TokioExecutor::new());
    if !header_read_timeout.is_zero() {
        builder
            .http1()
            .timer(TokioTimer::new())
            .header_read_timeout(header_read_timeout);
    }
    let graceful = GracefulShutdown::new();
    tokio::pin!(shutdown);
    loop {
        let (stream, peer): (_, SocketAddr) = tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok(connection) => connection,
                Err(error) => {
                    // Usually too many open files: back off instead of spinning.
                    tracing::warn!(error = %error, "Could not accept a connection");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
            },
            _ = &mut shutdown => break,
        };
        // The peer address keys the sign-in rate limits.
        let service = TowerToHyperService::new(app.clone().layer(Extension(ConnectInfo(peer))));
        let connection = builder
            .serve_connection_with_upgrades(TokioIo::new(stream), service)
            .into_owned();
        let watched = graceful.watch(connection);
        tokio::spawn(async move {
            if let Err(error) = watched.await {
                tracing::debug!(error = %error, "Connection ended with an error");
            }
        });
    }
    tokio::select! {
        _ = graceful.shutdown() => {}
        _ = tokio::time::sleep(SHUTDOWN_GRACE) => {
            tracing::warn!("Some connections were still open after the shutdown grace period");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn a_client_that_never_finishes_its_headers_is_disconnected() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = Router::new().route("/ping", axum::routing::get(|| async { "pong" }));
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        let server = tokio::spawn(serve(listener, app, Duration::from_millis(300), async {
            let _ = stopped.await;
        }));

        // A normal request still works.
        let mut good = tokio::net::TcpStream::connect(address).await.unwrap();
        good.write_all(b"GET /ping HTTP/1.1\r\nHost: test\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        let mut answer = String::new();
        good.read_to_string(&mut answer).await.unwrap();
        assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
        assert!(answer.ends_with("pong"));

        // Headers that stop halfway are cut off by the server.
        let mut slow = tokio::net::TcpStream::connect(address).await.unwrap();
        slow.write_all(b"GET /ping HTTP/1.1\r\nHost: te").await.unwrap();
        let mut rest = Vec::new();
        let read = tokio::time::timeout(Duration::from_secs(5), slow.read_to_end(&mut rest)).await;
        assert!(read.is_ok(), "the server kept the half-sent request open");

        let _ = stop.send(());
        let _ = tokio::time::timeout(Duration::from_secs(5), server).await;
    }
}
