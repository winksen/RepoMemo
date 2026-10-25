//! The server serving the built web app itself (`REPOMEMO_WEB_DIR`, V0.4.20), for a single-process install: one
//! program, one port, no web server in front. Off by default. It answers what the API does not: a file of the folder, the
//! page for a client route, `/config.json` for this host, with the same headers the web image sends.

use std::path::{Component, Path, PathBuf};

use axum::{
    body::Body,
    http::{header, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
};

use crate::ServerConfig;

/// Whether the API answers a path: everything under `/v1/` and the health routes.
pub(crate) fn is_api_path(path: &str) -> bool {
    path == "/v1" || path.starts_with("/v1/") || path == "/health" || path.starts_with("/health/")
}

/// What `/config.json` says when the server is also the web host: the API is the page's own host.
pub(crate) fn config_json(product_name: &str) -> String {
    serde_json::json!({ "apiUrl": "", "productName": product_name }).to_string()
}

/// The file a request path names inside `root`, if it is a plain file there. A path with `..`, a drive, a
/// backslash or a NUL, or one that leaves the folder through a link, names nothing.
pub(crate) fn file_for(root: &Path, request_path: &str) -> Option<PathBuf> {
    let relative = request_path.trim_start_matches('/');
    if relative.is_empty() || relative.contains(['\\', '\0']) {
        return None;
    }
    let mut path = root.to_path_buf();
    for component in Path::new(relative).components() {
        match component {
            Component::Normal(part) => path.push(part),
            _ => return None,
        }
    }
    let resolved = path.canonicalize().ok()?;
    let root = root.canonicalize().ok()?;
    (resolved.starts_with(&root) && resolved.is_file()).then_some(resolved)
}

/// The media type of a file from its extension.
pub(crate) fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json" | "map") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("txt") => "text/plain; charset=utf-8",
        Some("wasm") => "application/wasm",
        _ => "application/octet-stream",
    }
}

/// The page, a file or a refusal for a request the API does not answer.
pub(crate) async fn respond(config: &ServerConfig, root: &Path, method: Method, path: String) -> Response {
    let head = method == Method::HEAD;
    if method != Method::GET && !head {
        return (StatusCode::METHOD_NOT_ALLOWED, [(header::ALLOW, "GET, HEAD")]).into_response();
    }
    let path = path.as_str();
    let (body, kind, cache) = if path == "/config.json" {
        (
            config_json(&config.product_name).into_bytes(),
            "application/json",
            "no-store",
        )
    } else if let Some(file) = file_for(root, path) {
        let Ok(bytes) = tokio::fs::read(&file).await else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let cache = if path.starts_with("/assets/") {
            "public, max-age=31536000, immutable"
        } else {
            "no-cache"
        };
        (bytes, content_type(&file), cache)
    } else if Path::new(path).extension().is_some() {
        // A missing file with an extension is missing; only client routes get the page.
        return StatusCode::NOT_FOUND.into_response();
    } else {
        match tokio::fs::read(root.join("index.html")).await {
            Ok(bytes) => (bytes, "text/html; charset=utf-8", "no-cache"),
            Err(_) => return StatusCode::NOT_FOUND.into_response(),
        }
    };
    let mut response = Response::new(if head { Body::empty() } else { Body::from(body) });
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(kind));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    if kind.starts_with("text/html") {
        // The policy the build wrote beside the page.
        if let Ok(policy) = tokio::fs::read_to_string(root.join("content-security-policy.txt")).await {
            if let Ok(value) = HeaderValue::from_str(policy.trim()) {
                headers.insert(header::CONTENT_SECURITY_POLICY, value);
            }
        }
    }
    if config
        .public_url
        .as_deref()
        .is_some_and(|url| url.starts_with("https://"))
    {
        headers.insert(
            header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=31536000"),
        );
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder() -> PathBuf {
        let root = std::env::temp_dir().join(format!("repomemo-web-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("assets")).unwrap();
        std::fs::write(root.join("index.html"), "<div id=\"root\"></div>").unwrap();
        std::fs::write(root.join("assets").join("app-abc123.js"), "console.log(1)").unwrap();
        std::fs::write(root.join("content-security-policy.txt"), "default-src 'self'\n").unwrap();
        root
    }

    #[test]
    fn the_api_paths_are_the_versioned_routes_and_the_health_ones() {
        for path in ["/v1", "/v1/workspaces", "/health", "/health/ready"] {
            assert!(is_api_path(path), "{path}");
        }
        for path in ["/", "/workspaces/1", "/v10", "/healthy", "/assets/app.js"] {
            assert!(!is_api_path(path), "{path}");
        }
    }

    #[test]
    fn only_plain_files_inside_the_folder_are_found() {
        let root = folder();
        assert!(file_for(&root, "/assets/app-abc123.js").is_some());
        assert!(file_for(&root, "/index.html").is_some());
        for path in [
            "/",
            "/assets",
            "/../secret",
            "/assets/../../secret",
            "/a\\b",
            "/missing.js",
            "//etc/passwd",
        ] {
            assert!(file_for(&root, path).is_none(), "{path}");
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn the_config_for_this_host_has_no_api_address() {
        let value: serde_json::Value = serde_json::from_str(&config_json("Acme Memo")).unwrap();
        assert_eq!(value["apiUrl"], "");
        assert_eq!(value["productName"], "Acme Memo");
    }

    #[test]
    fn media_types_follow_the_extension() {
        assert_eq!(content_type(Path::new("a.js")), "text/javascript; charset=utf-8");
        assert_eq!(content_type(Path::new("a.CSS")), "text/css; charset=utf-8");
        assert_eq!(content_type(Path::new("a.woff2")), "font/woff2");
        assert_eq!(content_type(Path::new("a.unknown")), "application/octet-stream");
    }
}
