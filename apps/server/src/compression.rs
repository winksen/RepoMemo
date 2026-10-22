//! Compressed answers (V0.2.160, review P20). Without a proxy in front, the server compresses its
//! own JSON; behind Caddy the proxy already does, and a response that carries `Content-Encoding`
//! is left as it is.

use axum::body::HttpBody;
use axum::http::{header, Response};
use tower_http::compression::{
    predicate::{Predicate, SizeAbove},
    CompressionLayer,
};

/// Answers shorter than this are sent as they are: the saving is smaller than the work.
const MIN_COMPRESSED_BYTES: u16 = 1024;

/// What is worth compressing: JSON and text. Event streams (every event must arrive at once),
/// downloads (`Content-Disposition: attachment`), answers to range requests and the formats that are
/// already compressed (PDF, images, archives) are not.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Compressible;

impl Predicate for Compressible {
    fn should_compress<B>(&self, response: &Response<B>) -> bool
    where
        B: HttpBody,
    {
        let headers = response.headers();
        if headers.contains_key(header::CONTENT_RANGE) || headers.contains_key(header::ACCEPT_RANGES) {
            return false;
        }
        let is_download = headers
            .get(header::CONTENT_DISPOSITION)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.trim_start().to_ascii_lowercase().starts_with("attachment"));
        if is_download {
            return false;
        }
        let kind = headers
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        matches!(
            kind.as_str(),
            "application/json" | "application/problem+json" | "application/x-ndjson" | "application/openmetrics-text"
        ) || (kind.starts_with("text/") && kind != "text/event-stream")
    }
}

/// gzip and Brotli, when the client asks for them.
pub(crate) fn layer() -> CompressionLayer<impl Predicate> {
    CompressionLayer::new()
        .gzip(true)
        .br(true)
        .compress_when(SizeAbove::new(MIN_COMPRESSED_BYTES).and(Compressible))
}
