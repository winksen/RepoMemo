//! The web app's refresh token lives in an `HttpOnly` cookie, never in script
//! reach (review M2, roadmap decision 2).
//!
//! Only requests that say they come from the web app (`X-RepoMemo-Client:
//! web`) are touched; the console and scripts keep exchanging the token in
//! JSON bodies. For the web app:
//!
//! - responses that issue a session (sign-in, registration, refresh, password
//!   change or reset, first administrator) hand the refresh token to the
//!   browser as a cookie and blank it in the body;
//! - requests that present a refresh token (sign-in, refresh, sign-out) get it
//!   from the cookie when the body has none;
//! - signing out, or a refresh the server refuses, clears the cookie.
//!
//! The cookie is `SameSite=Strict`, so it needs the web app and the API on one
//! site (one host behind the proxy; ports may differ). Its path is `/v1/auth`
//! rather than only `/v1/auth/refresh` so that sign-in (lockout bypass) and
//! sign-out (server-side revocation) receive it too.

use axum::{
    body::{to_bytes, Body},
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde_json::{json, Value};

use crate::AppState;

const COOKIE_NAME: &str = "repomemo_refresh";
const COOKIE_PATH: &str = "/v1/auth";
/// Request bodies of the paths below are tiny JSON objects.
const MAX_REQUEST_BYTES: usize = 64 * 1024;
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

/// Requests whose body may carry the refresh token.
const PRESENTS_TOKEN: &[&str] = &["/v1/auth/login", "/v1/auth/refresh", "/v1/auth/logout"];
/// Responses that issue a session.
const ISSUES_SESSION: &[&str] = &[
    "/v1/auth/register",
    "/v1/auth/login",
    "/v1/auth/refresh",
    "/v1/auth/password-reset",
    "/v1/profile/password",
    "/v1/setup/admin",
];
/// Requests after which this browser has no session.
const ENDS_SESSION: &[&str] = &["/v1/auth/logout", "/v1/auth/logout-all"];

fn is_web_client(headers: &HeaderMap) -> bool {
    headers
        .get("x-repomemo-client")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("web"))
}

/// The refresh token the browser sent in its cookie.
fn cookie_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().strip_prefix(COOKIE_NAME)?.strip_prefix('='))
        .map(str::to_owned)
        .find(|value| !value.is_empty())
}

fn set_cookie(token: &str, max_age_seconds: i64, secure: bool) -> HeaderValue {
    let secure = if secure { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "{COOKIE_NAME}={token}; Path={COOKIE_PATH}; HttpOnly; SameSite=Strict; Max-Age={max_age_seconds}{secure}"
    ))
    .unwrap_or_else(|_| HeaderValue::from_static(""))
}

fn bad_request() -> Response {
    (StatusCode::BAD_REQUEST, "The request body could not be read.").into_response()
}

pub(crate) async fn web_session(State(state): State<AppState>, request: Request, next: Next) -> Response {
    if !is_web_client(request.headers()) {
        return next.run(request).await;
    }
    let path = request.uri().path().to_owned();
    let mut request = request;

    if request.method() == Method::POST && PRESENTS_TOKEN.contains(&path.as_str()) {
        if let Some(token) = cookie_token(request.headers()) {
            let (mut parts, body) = request.into_parts();
            let Ok(bytes) = to_bytes(body, MAX_REQUEST_BYTES).await else {
                return bad_request();
            };
            let mut value: Value = if bytes.iter().all(u8::is_ascii_whitespace) {
                json!({})
            } else {
                match serde_json::from_slice(&bytes) {
                    Ok(value) => value,
                    // Not JSON: leave it to the handler to refuse.
                    Err(_) => {
                        return next.run(Request::from_parts(parts, Body::from(bytes))).await;
                    }
                }
            };
            if let Some(object) = value.as_object_mut() {
                let has_token = object.get("refresh_token").and_then(Value::as_str).is_some_and(|token| !token.is_empty());
                if !has_token {
                    object.insert("refresh_token".to_owned(), Value::String(token));
                }
            }
            let rewritten = serde_json::to_vec(&value).unwrap_or_default();
            parts.headers.remove(header::CONTENT_LENGTH);
            parts.headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/json"));
            request = Request::from_parts(parts, Body::from(rewritten));
        }
    }

    let mut response = next.run(request).await;
    let settings = state.settings();
    let status = response.status();

    if (ENDS_SESSION.contains(&path.as_str()) && status.is_success())
        || (path == "/v1/auth/refresh" && status == StatusCode::UNAUTHORIZED)
    {
        response
            .headers_mut()
            .append(header::SET_COOKIE, set_cookie("", 0, settings.secure_cookies));
        return response;
    }

    if status.is_success() && ISSUES_SESSION.contains(&path.as_str()) {
        let (mut parts, body) = response.into_parts();
        let Ok(bytes) = to_bytes(body, MAX_RESPONSE_BYTES).await else {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        };
        let Ok(mut value) = serde_json::from_slice::<Value>(&bytes) else {
            return Response::from_parts(parts, Body::from(bytes));
        };
        let token = value
            .get("refresh_token")
            .and_then(Value::as_str)
            .filter(|token| !token.is_empty())
            .map(str::to_owned);
        let Some(token) = token else {
            return Response::from_parts(parts, Body::from(bytes));
        };
        value["refresh_token"] = Value::String(String::new());
        let max_age = settings.refresh_token_ttl_days.max(1) * 24 * 60 * 60;
        parts.headers.append(header::SET_COOKIE, set_cookie(&token, max_age, settings.secure_cookies));
        parts.headers.remove(header::CONTENT_LENGTH);
        return Response::from_parts(parts, Body::from(serde_json::to_vec(&value).unwrap_or_default()));
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cookie_is_found_among_others() {
        let mut headers = HeaderMap::new();
        headers.insert(header::COOKIE, HeaderValue::from_static("theme=dark; repomemo_refresh=abc123; other=1"));
        assert_eq!(cookie_token(&headers).as_deref(), Some("abc123"));
        headers.insert(header::COOKIE, HeaderValue::from_static("repomemo_refresh_extra=zzz"));
        assert_eq!(cookie_token(&headers), None, "a longer name is not ours");
        headers.insert(header::COOKIE, HeaderValue::from_static("repomemo_refresh="));
        assert_eq!(cookie_token(&headers), None);
    }

    #[test]
    fn the_cookie_is_http_only_strict_and_scoped() {
        let value = set_cookie("token", 60, true);
        let text = value.to_str().unwrap();
        for part in ["HttpOnly", "SameSite=Strict", "Path=/v1/auth", "Max-Age=60", "Secure"] {
            assert!(text.contains(part), "{part} missing from {text}");
        }
        assert!(!set_cookie("token", 60, false).to_str().unwrap().contains("Secure"));
    }
}
