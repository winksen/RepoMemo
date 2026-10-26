//! A private certificate authority for one server (V0.4.55). A server behind a company's own CA fails the HTTP plugin's TLS check, and
//! the plugin has no way to add a root. So when the administrator's file names a certificate bundle for the server, the shell runs a
//! small relay on the loopback: the page's requests for that server go to the relay (still through the plugin, which is allowed to
//! call that one address), and the relay forwards them to the server over TLS trusting **only the bundle**. It forwards to the one
//! origin it was started for and answers nothing without the run's secret, so it is no way to reach, or to trust, anything else.

use std::{error::Error, net::TcpListener as StdListener, sync::Arc};

use bytes::Bytes;
use futures_util::TryStreamExt;
use http_body_util::{combinators::BoxBody, BodyExt, Full, StreamBody};
use hyper::{
    body::{Frame, Incoming},
    header::{HeaderMap, HeaderName, HeaderValue},
    service::service_fn,
    Request, Response, StatusCode, Uri,
};
use hyper_util::rt::TokioIo;
use serde::Serialize;

/// The header that carries the run's secret from the page to the relay.
pub const TOKEN_HEADER: &str = "x-repomemo-route";
/// The biggest bundle file read, and the most certificates taken from it.
pub const MAX_BUNDLE_BYTES: u64 = 256 * 1024;
pub const MAX_CERTIFICATES: usize = 10;

type RelayBody = BoxBody<Bytes, Box<dyn Error + Send + Sync>>;

/// Where the page sends the requests for a server whose certificate the relay checks.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Route {
    /// The relay's address (`http://127.0.0.1:port`), which the HTTP plugin may call in place of the server.
    pub proxy: String,
    /// The secret the page sends in [`TOKEN_HEADER`].
    pub token: String,
}

/// The certificates of a PEM bundle: one to ten, each well formed.
pub fn parse_bundle(pem: &[u8]) -> Result<Vec<reqwest::Certificate>, String> {
    let certificates = reqwest::Certificate::from_pem_bundle(pem)
        .map_err(|error| format!("it is not a PEM certificate bundle ({error})"))?;
    if certificates.is_empty() {
        return Err("it holds no certificate (a PEM file has -----BEGIN CERTIFICATE----- blocks)".to_owned());
    }
    if certificates.len() > MAX_CERTIFICATES {
        return Err(format!(
            "it holds {} certificates; the most taken is {MAX_CERTIFICATES}",
            certificates.len()
        ));
    }
    Ok(certificates)
}

/// The address a request is forwarded to: the server's origin and the request's path and query, and nothing the request names itself.
pub fn upstream_url(origin: &str, uri: &Uri) -> Option<String> {
    let target = uri.path_and_query()?.as_str();
    (target.starts_with('/') && !target.starts_with("//") && uri.scheme().is_none() && uri.authority().is_none())
        .then(|| format!("{origin}{target}"))
}

/// Whether a header belongs to one connection and is not passed on, or is the relay's own.
pub fn is_connection_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
            | "host"
            | TOKEN_HEADER
    )
}

fn text(status: StatusCode, message: &str) -> Response<RelayBody> {
    let body = Full::new(Bytes::from(message.to_owned()))
        .map_err(|never| match never {})
        .boxed();
    let mut response = Response::new(body);
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert("content-type", HeaderValue::from_static("text/plain; charset=utf-8"));
    response
}

fn carried(from: &HeaderMap) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    for (name, value) in from {
        if is_connection_header(name.as_str()) {
            continue;
        }
        if let (Ok(name), Ok(value)) = (
            reqwest::header::HeaderName::from_bytes(name.as_str().as_bytes()),
            reqwest::header::HeaderValue::from_bytes(value.as_bytes()),
        ) {
            headers.append(name, value);
        }
    }
    headers
}

async fn forward(
    client: reqwest::Client,
    origin: Arc<String>,
    token: Arc<String>,
    request: Request<Incoming>,
) -> Result<Response<RelayBody>, hyper::Error> {
    let allowed = request
        .headers()
        .get(TOKEN_HEADER)
        .and_then(|value| value.to_str().ok())
        == Some(token.as_str());
    if !allowed {
        return Ok(text(
            StatusCode::FORBIDDEN,
            "This relay answers only the RepoMemo app that started it.",
        ));
    }
    let Some(url) = upstream_url(&origin, request.uri()) else {
        return Ok(text(
            StatusCode::BAD_REQUEST,
            "This relay forwards a path of the one server it was started for, and nothing else.",
        ));
    };
    let (parts, body) = request.into_parts();
    let upstream = client
        .request(parts.method, url)
        .headers(carried(&parts.headers))
        .body(reqwest::Body::wrap_stream(body.into_data_stream()));
    let response = match upstream.send().await {
        Ok(response) => response,
        Err(error) => {
            let reason = error
                .source()
                .map(|source| source.to_string())
                .unwrap_or_else(|| error.to_string());
            return Ok(text(
                StatusCode::BAD_GATEWAY,
                &format!("The server could not be reached through its own certificate authority: {reason}"),
            ));
        }
    };
    let (status, headers) = (response.status(), response.headers().clone());
    let mut answer = Response::new(
        StreamBody::new(
            response
                .bytes_stream()
                .map_ok(Frame::data)
                .map_err(|error| Box::new(error) as Box<dyn Error + Send + Sync>),
        )
        .boxed(),
    );
    *answer.status_mut() = status;
    for (name, value) in &headers {
        if is_connection_header(name.as_str()) {
            continue;
        }
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(name.as_str().as_bytes()),
            HeaderValue::from_bytes(value.as_bytes()),
        ) {
            answer.headers_mut().append(name, value);
        }
    }
    Ok(answer)
}

/// Starts the relay for one server, trusting only `certificates`, and answers where the page reaches it. It runs until the app ends.
pub fn start(origin: &str, certificates: Vec<reqwest::Certificate>) -> Result<Route, String> {
    let mut builder = reqwest::Client::builder()
        .use_rustls_tls()
        .tls_built_in_root_certs(false)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(15));
    for certificate in certificates {
        builder = builder.add_root_certificate(certificate);
    }
    let client = builder
        .build()
        .map_err(|error| format!("the certificate bundle could not be used ({error})"))?;
    let listener = StdListener::bind(("127.0.0.1", 0)).map_err(|error| error.to_string())?;
    listener.set_nonblocking(true).map_err(|error| error.to_string())?;
    let port = listener.local_addr().map_err(|error| error.to_string())?.port();
    let token = uuid::Uuid::new_v4().simple().to_string();
    let (origin, secret) = (Arc::new(origin.to_owned()), Arc::new(token.clone()));
    tauri::async_runtime::spawn(async move {
        let Ok(listener) = tokio::net::TcpListener::from_std(listener) else {
            return;
        };
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                continue;
            };
            let (client, origin, secret) = (client.clone(), origin.clone(), secret.clone());
            tokio::spawn(async move {
                let service =
                    service_fn(move |request| forward(client.clone(), origin.clone(), secret.clone(), request));
                let _ = hyper::server::conn::http1::Builder::new()
                    .serve_connection(TokioIo::new(stream), service)
                    .await;
            });
        }
    });
    Ok(Route {
        proxy: format!("http://127.0.0.1:{port}"),
        token,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CA: &str = include_str!("../testdata/test-ca.pem");

    #[test]
    fn a_bundle_is_one_to_ten_certificates() {
        assert_eq!(parse_bundle(CA.as_bytes()).unwrap().len(), 1);
        assert_eq!(parse_bundle(format!("{CA}\n{CA}").as_bytes()).unwrap().len(), 2);
        assert!(parse_bundle(CA.repeat(11).as_bytes())
            .unwrap_err()
            .contains("11 certificates"));
        assert!(parse_bundle(b"").unwrap_err().contains("no certificate"));
        assert!(parse_bundle(b"just some text").unwrap_err().contains("no certificate"));
        assert!(parse_bundle(b"-----BEGIN CERTIFICATE-----\n!!!notbase64!!!\n-----END CERTIFICATE-----\n").is_err());
    }

    #[test]
    fn only_a_path_of_the_one_server_is_forwarded() {
        let origin = "https://memo.example.com";
        let uri = |text: &str| text.parse::<Uri>().unwrap();
        assert_eq!(
            upstream_url(origin, &uri("/v1/auth/login")).as_deref(),
            Some("https://memo.example.com/v1/auth/login")
        );
        assert_eq!(
            upstream_url(origin, &uri("/health?x=1")).as_deref(),
            Some("https://memo.example.com/health?x=1")
        );
        assert_eq!(
            upstream_url(origin, &uri("http://evil.example.com/steal")),
            None,
            "a request naming another host"
        );
        assert_eq!(upstream_url(origin, &uri("//evil.example.com/steal")), None);
        assert_eq!(upstream_url(origin, &uri("*")), None);
    }

    #[test]
    fn the_headers_of_one_connection_and_the_relays_own_are_not_passed_on() {
        for name in [
            "Connection",
            "Keep-Alive",
            "Transfer-Encoding",
            "Upgrade",
            "Host",
            "Proxy-Authorization",
            "TE",
            "X-RepoMemo-Route",
        ] {
            assert!(is_connection_header(name), "{name}");
        }
        for name in [
            "Authorization",
            "Content-Type",
            "Content-Length",
            "X-RepoMemo-Client",
            "Accept",
        ] {
            assert!(!is_connection_header(name), "{name}");
        }
    }
}
