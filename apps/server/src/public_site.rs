//! Community reading (V0.8.18): what an anonymous reader can open, under the rules of the V0.8.15 review.
//!
//! The routes live under `/public`, answer `GET` only, never read a cookie or an authorization header, and read only the
//! `publications` table: no file, comment, account or workspace is joined, so a bug here cannot reach private data. Pages are made
//! of the snapshot text **escaped** (never inserted as HTML). Everything that is not served (off, unknown, unpublished,
//! withdrawn, stale, malformed) is the same plain `404` with no cache, so existence is not learned. Pages carry a short cache time
//! and an `ETag` of the publication's version, so a withdrawal is seen within a minute. Requests are limited per address.

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;

use crate::{
    security::{ClientIp, Quota},
    AppState,
};

const PER_MINUTE: u32 = 120;
const MAX_QUERY_CHARS: usize = 100;

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn secure(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'self'"),
    );
    response
}

/// The one answer for everything that is not served.
fn not_found() -> Response {
    let mut response = (StatusCode::NOT_FOUND, "Not found").into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    secure(response)
}

fn page(title: &str, body: &str, version_tag: Option<String>) -> Response {
    let html = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{}</title><style>body{{font-family:system-ui,sans-serif;max-width:46rem;margin:0 auto;padding:1rem;line-height:1.5;color:#222;background:#fff}}a{{color:#111}}pre{{white-space:pre-wrap;word-wrap:break-word;font:inherit}}input{{font:inherit;padding:.4rem;max-width:100%}}li{{margin:.3rem 0}}@media (prefers-color-scheme:dark){{body{{color:#ddd;background:#111}}a{{color:#eee}}}}</style></head><body>{}</body></html>",
        escape(title),
        body
    );
    let mut response = (StatusCode::OK, html).into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=60, must-revalidate"),
    );
    if let Some(tag) = version_tag.and_then(|tag| HeaderValue::from_str(&tag).ok()) {
        headers.insert(header::ETAG, tag);
    }
    secure(response)
}

/// `Some(response)` when the request must not be served: publishing is off, or the address has used its allowance.
fn refuse(state: &AppState, client: &ClientIp) -> Option<Response> {
    if !state.public_publishing {
        return Some(not_found());
    }
    if state
        .public_limiter
        .check(&format!("public:{}", client.limit_key()), Quota::per_minute(PER_MINUTE))
        .is_err()
    {
        let mut response = (StatusCode::TOO_MANY_REQUESTS, "Too many requests").into_response();
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        return Some(secure(response));
    }
    None
}

fn list_html(rows: &[(String, String)], heading: &str, query: &str) -> String {
    let items = rows
        .iter()
        .map(|(slug, title)| format!("<li><a href=\"/public/p/{}\">{}</a></li>", escape(slug), escape(title)))
        .collect::<String>();
    format!(
        "<h1>{}</h1><form action=\"/public/search\" method=\"get\"><input name=\"q\" value=\"{}\" maxlength=\"{MAX_QUERY_CHARS}\" aria-label=\"Search\"> <button>Search</button></form><ul>{}</ul>",
        escape(heading),
        escape(query),
        if items.is_empty() { "<li>Nothing is published.</li>".to_owned() } else { items }
    )
}

/// The index of everything that is published.
pub(crate) async fn index(State(state): State<AppState>, client: ClientIp) -> Response {
    if let Some(response) = refuse(&state, &client) {
        return response;
    }
    match state.storage.public_index(None, 200).await {
        Ok(rows) => page("Published pages", &list_html(&rows, "Published pages", ""), None),
        Err(_) => not_found(),
    }
}

/// One published page. A page that is not live is a plain `404`.
pub(crate) async fn read(
    State(state): State<AppState>,
    client: ClientIp,
    headers: HeaderMap,
    Path(slug): Path<String>,
) -> Response {
    if let Some(response) = refuse(&state, &client) {
        return response;
    }
    if slug.len() != 32 || !slug.chars().all(|c| c.is_ascii_hexdigit()) {
        return not_found();
    }
    let Ok(Some((title, body, version))) = state.storage.public_page(&slug).await else {
        return not_found();
    };
    let tag = format!("\"{slug}-{version}\"");
    if headers.get(header::IF_NONE_MATCH).and_then(|value| value.to_str().ok()) == Some(tag.as_str()) {
        let mut response = StatusCode::NOT_MODIFIED.into_response();
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=60, must-revalidate"),
        );
        return secure(response);
    }
    let content = format!(
        "<p><a href=\"/public\">All pages</a></p><h1>{}</h1><pre>{}</pre>",
        escape(&title),
        escape(&body)
    );
    page(&title, &content, Some(tag))
}

#[derive(Debug, Deserialize)]
pub(crate) struct SearchQuery {
    #[serde(default)]
    q: String,
}

/// Search over published snapshots only.
pub(crate) async fn search(
    State(state): State<AppState>,
    client: ClientIp,
    Query(query): Query<SearchQuery>,
) -> Response {
    if let Some(response) = refuse(&state, &client) {
        return response;
    }
    let text: String = query.q.chars().take(MAX_QUERY_CHARS).collect();
    if text.trim().is_empty() {
        return index(State(state), client).await;
    }
    match state.storage.public_index(Some(text.trim()), 50).await {
        Ok(rows) => {
            let mut response = page("Search", &list_html(&rows, "Search", text.trim()), None);
            // Results change as pages are published and withdrawn: they are not cached.
            response
                .headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            response
        }
        Err(_) => not_found(),
    }
}
