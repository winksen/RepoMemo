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
//! The cookie is `SameSite=Strict` by default, so it needs the web app and the API on one
//! site (one host behind the proxy; ports may differ). `REPOMEMO_COOKIE_SAMESITE` (V0.4.17) lets it be `Lax` or
//! `None` for a web app on another address; then a request that presents it from an origin that is not listed in
//! `REPOMEMO_ALLOWED_ORIGIN` is refused. Its path is `/v1/auth`
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
/// The name over HTTPS: the `__Secure-` prefix makes a browser refuse the cookie unless it was set
/// with `Secure` from a secure page, so a plain-HTTP response cannot plant or overwrite it (E14).
const SECURE_COOKIE_NAME: &str = "__Secure-repomemo_refresh";
const COOKIE_PATH: &str = "/v1/auth";
/// Request bodies of the paths below are tiny JSON objects.
const MAX_REQUEST_BYTES: usize = 64 * 1024;
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

/// Requests whose body may carry the refresh token.
const PRESENTS_TOKEN: &[&str] = &["/v1/auth/login", "/v1/auth/refresh", "/v1/auth/logout"];
/// Responses that issue a session.
const ISSUES_SESSION: &[&str] = &[
    "/v1/auth/identity/complete",
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

/// The value of the cookie called `name` among the browser's cookies.
fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().strip_prefix(name)?.strip_prefix('='))
        .map(str::to_owned)
        .find(|value| !value.is_empty())
}

/// The refresh token the browser sent in its cookie: under the `__Secure-` name, or, from a browser that
/// still holds the cookie an earlier release set, under the old name (it is replaced by the next response).
fn cookie_token(headers: &HeaderMap) -> Option<String> {
    cookie_value(headers, SECURE_COOKIE_NAME).or_else(|| cookie_value(headers, COOKIE_NAME))
}

/// The name the cookie is set under: with the `__Secure-` prefix when the public address is HTTPS.
fn cookie_name(secure: bool) -> &'static str {
    if secure {
        SECURE_COOKIE_NAME
    } else {
        COOKIE_NAME
    }
}

fn set_cookie(token: &str, max_age_seconds: i64, secure: bool, same_site: &str) -> HeaderValue {
    let flag = if secure { "; Secure" } else { "" };
    let name = cookie_name(secure);
    HeaderValue::from_str(&format!(
        "{name}={token}; Path={COOKIE_PATH}; HttpOnly; SameSite={same_site}; Max-Age={max_age_seconds}{flag}"
    ))
    .unwrap_or_else(|_| HeaderValue::from_static(""))
}

/// Removes the cookie under the plain name that an earlier release set, once the server sets the `__Secure-`
/// one (HTTPS), so a browser does not keep two.
fn clear_legacy_cookie() -> HeaderValue {
    HeaderValue::from_str(&format!(
        "{COOKIE_NAME}=; Path={COOKIE_PATH}; HttpOnly; SameSite=Strict; Max-Age=0"
    ))
    .unwrap_or_else(|_| HeaderValue::from_static(""))
}

/// Whether a request that presents the refresh cookie may come from the page it says it does. Under `Strict` the
/// browser sends the cookie to nobody else, so nothing is asked; under `Lax` or `None` an `Origin` header
/// that is not among the allowed origins is refused. A request with no `Origin` (the console, a script) is left alone.
fn origin_allowed(headers: &HeaderMap, same_site: &str, allowed: &[String]) -> bool {
    if same_site == "Strict" {
        return true;
    }
    match headers.get(header::ORIGIN).and_then(|value| value.to_str().ok()) {
        None => true,
        Some(origin) => {
            let origin = origin.trim_end_matches('/').to_ascii_lowercase();
            allowed.contains(&origin)
        }
    }
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
        if cookie_token(request.headers()).is_some() {
            let settings = state.settings();
            if !origin_allowed(request.headers(), settings.cookie_same_site, &settings.allowed_origins) {
                return (StatusCode::FORBIDDEN, "This origin may not use the sign-in cookie.").into_response();
            }
        }
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
                let has_token = object
                    .get("refresh_token")
                    .and_then(Value::as_str)
                    .is_some_and(|token| !token.is_empty());
                if !has_token {
                    object.insert("refresh_token".to_owned(), Value::String(token));
                }
            }
            let rewritten = serde_json::to_vec(&value).unwrap_or_default();
            parts.headers.remove(header::CONTENT_LENGTH);
            parts
                .headers
                .insert(header::CONTENT_TYPE, HeaderValue::from_static("application/json"));
            request = Request::from_parts(parts, Body::from(rewritten));
        }
    }

    let mut response = next.run(request).await;
    let settings = state.settings();
    let status = response.status();

    if (ENDS_SESSION.contains(&path.as_str()) && status.is_success())
        || (path == "/v1/auth/refresh" && status == StatusCode::UNAUTHORIZED)
    {
        // Both names go over HTTPS: the one in use and the one an earlier release set.
        response.headers_mut().append(
            header::SET_COOKIE,
            set_cookie("", 0, settings.secure_cookies, settings.cookie_same_site),
        );
        if settings.secure_cookies {
            response.headers_mut().append(header::SET_COOKIE, clear_legacy_cookie());
        }
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
        parts.headers.append(
            header::SET_COOKIE,
            set_cookie(&token, max_age, settings.secure_cookies, settings.cookie_same_site),
        );
        // The name an earlier release used is read once (above) and replaced here.
        if settings.secure_cookies {
            parts.headers.append(header::SET_COOKIE, clear_legacy_cookie());
        }
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
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("theme=dark; repomemo_refresh=abc123; other=1"),
        );
        assert_eq!(cookie_token(&headers).as_deref(), Some("abc123"));
        headers.insert(header::COOKIE, HeaderValue::from_static("repomemo_refresh_extra=zzz"));
        assert_eq!(cookie_token(&headers), None, "a longer name is not ours");
        headers.insert(header::COOKIE, HeaderValue::from_static("repomemo_refresh="));
        assert_eq!(cookie_token(&headers), None);
    }

    #[test]
    fn the_cookie_is_http_only_strict_and_scoped() {
        let value = set_cookie("token", 60, true, "Strict");
        let text = value.to_str().unwrap();
        for part in ["HttpOnly", "SameSite=Strict", "Path=/v1/auth", "Max-Age=60", "Secure"] {
            assert!(text.contains(part), "{part} missing from {text}");
        }
        assert!(!set_cookie("token", 60, false, "Strict")
            .to_str()
            .unwrap()
            .contains("Secure"));
    }

    #[test]
    fn the_same_site_attribute_follows_the_setting() {
        let none = set_cookie("token", 60, true, "None");
        assert!(
            none.to_str().unwrap().contains("SameSite=None; Max-Age=60; Secure"),
            "{none:?}"
        );
        let lax = set_cookie("token", 60, true, "Lax");
        assert!(lax.to_str().unwrap().contains("SameSite=Lax"));
    }

    #[test]
    fn a_cookie_from_an_unlisted_origin_is_refused_unless_the_cookie_is_strict() {
        let allowed = vec!["https://app.example.com".to_owned()];
        let from = |origin: &str| {
            let mut headers = HeaderMap::new();
            headers.insert(header::ORIGIN, HeaderValue::from_str(origin).unwrap());
            headers
        };
        assert!(origin_allowed(&from("https://app.example.com"), "None", &allowed));
        assert!(origin_allowed(&from("https://APP.example.com/"), "None", &allowed));
        assert!(!origin_allowed(&from("https://evil.example.org"), "None", &allowed));
        assert!(!origin_allowed(&from("https://evil.example.org"), "Lax", &allowed));
        assert!(
            origin_allowed(&from("https://evil.example.org"), "Strict", &allowed),
            "Strict never leaves the site"
        );
        assert!(
            origin_allowed(&HeaderMap::new(), "None", &allowed),
            "a script sends no Origin"
        );
    }

    #[test]
    fn over_https_the_cookie_carries_the_secure_prefix_and_the_old_name_is_read_once_and_cleared() {
        let secure = set_cookie("token", 60, true, "Strict");
        assert!(
            secure.to_str().unwrap().starts_with("__Secure-repomemo_refresh=token;"),
            "{secure:?}"
        );
        assert!(secure.to_str().unwrap().contains("; Secure"));
        let plain = set_cookie("token", 60, false, "Strict");
        assert!(
            plain.to_str().unwrap().starts_with("repomemo_refresh=token;"),
            "{plain:?}"
        );

        // The plain name an earlier release set is cleared on the way.
        let clears_plain = clear_legacy_cookie();
        assert!(
            clears_plain.to_str().unwrap().starts_with("repomemo_refresh=;"),
            "{clears_plain:?}"
        );
        assert!(clears_plain.to_str().unwrap().contains("Max-Age=0"));

        // Either name is read; the new one wins when a browser holds both.
        let mut headers = HeaderMap::new();
        headers.insert(header::COOKIE, HeaderValue::from_static("repomemo_refresh=old"));
        assert_eq!(
            cookie_token(&headers).as_deref(),
            Some("old"),
            "an earlier release's cookie is read once"
        );
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("repomemo_refresh=old; __Secure-repomemo_refresh=new"),
        );
        assert_eq!(cookie_token(&headers).as_deref(), Some("new"));
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("x__Secure-repomemo_refresh=nope"),
        );
        assert_eq!(cookie_token(&headers), None, "a longer name is not ours");
    }
}
