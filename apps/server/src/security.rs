//! Request-level protections: rate limits, sign-in throttling, the client
//! address they are keyed on, and the security headers added to every
//! response.
//!
//! Counters live in memory. The server is a single process, so this is exact;
//! a restart forgets them, which only ever errs on the side of letting a user
//! back in.

use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::Mutex,
    time::{Duration, Instant},
};

use axum::{
    extract::{ConnectInfo, FromRequestParts, Request, State},
    http::{header, request::Parts, HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};

use crate::{ApiError, AppState};

/// How many requests a key may make per fixed window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quota {
    pub limit: u32,
    pub period: Duration,
}

impl Quota {
    pub const fn per_minute(limit: u32) -> Self {
        Self {
            limit,
            period: Duration::from_secs(60),
        }
    }

    pub const fn per_hour(limit: u32) -> Self {
        Self {
            limit,
            period: Duration::from_secs(3600),
        }
    }

    /// A limit of zero turns the quota off.
    pub fn is_enabled(&self) -> bool {
        self.limit > 0
    }
}

struct Window {
    started: Instant,
    count: u32,
}

/// Fixed-window request counters per key.
#[derive(Default)]
pub struct RateLimiter {
    windows: Mutex<HashMap<String, Window>>,
}

impl RateLimiter {
    fn len(&self) -> usize {
        self.windows.lock().map(|windows| windows.len()).unwrap_or(0)
    }

    /// Counts one request for `key`. Returns how long to wait when the quota
    /// for the current window is used up.
    pub fn check(&self, key: &str, quota: Quota) -> Result<(), Duration> {
        if !quota.is_enabled() {
            return Ok(());
        }
        let now = Instant::now();
        let Ok(mut windows) = self.windows.lock() else {
            return Ok(());
        };
        let window = windows
            .entry(key.to_owned())
            .or_insert(Window { started: now, count: 0 });
        if now.duration_since(window.started) >= quota.period {
            window.started = now;
            window.count = 0;
        }
        if window.count >= quota.limit {
            return Err(quota.period.saturating_sub(now.duration_since(window.started)));
        }
        window.count += 1;
        Ok(())
    }

    /// Forgets windows older than `max_age`, so the map does not grow with
    /// every address ever seen.
    pub fn prune(&self, max_age: Duration) -> usize {
        let now = Instant::now();
        self.windows
            .lock()
            .map(|mut windows| {
                let before = windows.len();
                windows.retain(|_, window| now.duration_since(window.started) < max_age);
                before - windows.len()
            })
            .unwrap_or(0)
    }
}

struct Failures {
    count: u32,
    last_at: Instant,
    locked_until: Option<Instant>,
}

/// Consecutive failed sign-ins per key, with a temporary lockout once a key
/// reaches the configured number of failures.
#[derive(Default)]
pub struct LoginThrottle {
    failures: Mutex<HashMap<String, Failures>>,
}

impl LoginThrottle {
    /// How much longer `key` is locked out, if it is.
    pub fn locked_for(&self, key: &str) -> Option<Duration> {
        let now = Instant::now();
        let failures = self.failures.lock().ok()?;
        let until = failures.get(key)?.locked_until?;
        (until > now).then(|| until - now)
    }

    /// Records a failure and returns true when it locked the key out.
    pub fn record_failure(&self, key: &str, max_failures: u32, lockout: Duration) -> bool {
        if max_failures == 0 {
            return false;
        }
        let now = Instant::now();
        let Ok(mut failures) = self.failures.lock() else {
            return false;
        };
        let entry = failures.entry(key.to_owned()).or_insert(Failures {
            count: 0,
            last_at: now,
            locked_until: None,
        });
        // Failures spread over longer than a lockout period start over.
        if now.duration_since(entry.last_at) > lockout || entry.locked_until.is_some_and(|until| until <= now) {
            entry.count = 0;
            entry.locked_until = None;
        }
        entry.count += 1;
        entry.last_at = now;
        if entry.count >= max_failures {
            entry.locked_until = Some(now + lockout);
            entry.count = 0;
            return true;
        }
        false
    }

    pub fn clear(&self, key: &str) {
        if let Ok(mut failures) = self.failures.lock() {
            failures.remove(key);
        }
    }

    /// Clears every failure count and lockout of an account (keys
    /// `login:<email>` and `login:<email>|<address>`). Returns how many keys
    /// were cleared.
    pub fn clear_account(&self, email: &str) -> usize {
        let account = format!("login:{email}");
        let pair_prefix = format!("{account}|");
        self.failures
            .lock()
            .map(|mut failures| {
                let before = failures.len();
                failures.retain(|key, _| key != &account && !key.starts_with(&pair_prefix));
                before - failures.len()
            })
            .unwrap_or(0)
    }

    fn locked_count(&self) -> usize {
        let now = Instant::now();
        self.failures
            .lock()
            .map(|failures| {
                failures
                    .values()
                    .filter(|entry| entry.locked_until.is_some_and(|until| until > now))
                    .count()
            })
            .unwrap_or(0)
    }

    pub fn prune(&self, max_age: Duration) -> usize {
        let now = Instant::now();
        self.failures
            .lock()
            .map(|mut failures| {
                let before = failures.len();
                failures.retain(|_, entry| {
                    entry.locked_until.is_some_and(|until| until > now) || now.duration_since(entry.last_at) < max_age
                });
                before - failures.len()
            })
            .unwrap_or(0)
    }
}

/// Rate limits and sign-in throttling, shared by all requests.
#[derive(Default)]
pub struct Guards {
    /// Sign-in, registration and token refresh, per client address.
    pub auth: RateLimiter,
    /// AI requests, per user.
    pub ai: RateLimiter,
    /// Failed sign-ins per account and address.
    pub logins: LoginThrottle,
}

impl Guards {
    /// Rate-limit keys tracked, and sign-in keys locked out right now.
    pub fn stats(&self) -> (usize, usize) {
        (self.auth.len() + self.ai.len(), self.logins.locked_count())
    }

    pub fn prune(&self) -> usize {
        // Windows are at most an hour long; lockouts are kept while active.
        let max_age = Duration::from_secs(2 * 3600);
        self.auth.prune(max_age) + self.ai.prune(max_age) + self.logins.prune(max_age)
    }
}

/// Which reverse proxies may tell the server who the client is.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProxyTrust {
    /// `REPOMEMO_TRUST_PROXY`: read `X-Forwarded-For` / `X-Real-IP` at all.
    pub enabled: bool,
    /// `REPOMEMO_TRUSTED_PROXIES`: proxy addresses besides loopback.
    pub proxies: Vec<IpAddr>,
}

impl ProxyTrust {
    fn trusts(&self, address: IpAddr) -> bool {
        address.is_loopback() || self.proxies.contains(&address)
    }
}

/// The address a request came from: the TCP peer, or, when that peer is a
/// trusted proxy, the address it reports (see [`client_ip`]).
#[derive(Debug, Clone)]
pub struct ClientIp(pub String);

impl FromRequestParts<AppState> for ClientIp {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let peer = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|info| info.0.ip());
        Ok(Self(client_ip(&parts.headers, peer, &state.settings().proxy)))
    }
}

/// The client's address. Proxy headers count only when proxy trust is on and
/// the TCP peer is a trusted proxy (loopback or a listed address); anyone
/// else could write them. `X-Forwarded-For` is read from the right, skipping
/// trusted proxies: proxies append to it, so the leftmost entries are
/// whatever the client sent (review M3).
pub(crate) fn client_ip(headers: &axum::http::HeaderMap, peer: Option<IpAddr>, proxy: &ProxyTrust) -> String {
    if proxy.enabled && peer.is_some_and(|address| proxy.trusts(address)) {
        let forwarded = headers
            .get_all("x-forwarded-for")
            .iter()
            .filter_map(|value| value.to_str().ok())
            .flat_map(|value| value.split(','))
            .map(str::trim)
            .collect::<Vec<_>>();
        let from_chain = forwarded
            .iter()
            .rev()
            .filter_map(|entry| entry.parse::<IpAddr>().ok())
            .find(|address| !proxy.trusts(*address));
        let real_ip = || {
            headers
                .get("x-real-ip")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.trim().parse::<IpAddr>().ok())
        };
        if let Some(address) = from_chain.or_else(real_ip) {
            return address.to_string();
        }
    }
    peer.map(|address| address.to_string())
        .unwrap_or_else(|| "unknown".to_owned())
}

/// The 429 answer for a used-up quota, with `Retry-After`.
pub fn too_many_requests(message: impl Into<String>, retry_after: Duration) -> ApiError {
    let seconds = retry_after.as_secs().max(1);
    let mut error = ApiError::new(axum::http::StatusCode::TOO_MANY_REQUESTS, "rate_limited", message);
    error.retry_after = Some(seconds);
    error
}

/// Readable "in N minutes" for lockout and quota messages.
pub fn wait_text(duration: Duration) -> String {
    let seconds = duration.as_secs().max(1);
    if seconds < 90 {
        format!("{seconds} seconds")
    } else {
        format!("{} minutes", seconds.div_ceil(60))
    }
}

/// What the security headers add beyond the fixed set.
#[derive(Debug, Clone, Copy, Default)]
pub struct HeaderPolicy {
    /// The server is reached over HTTPS (`REPOMEMO_PUBLIC_URL`), so browsers
    /// are told to keep using it (HSTS, review M9).
    pub hsts: bool,
}

/// Adds conservative security headers to every response that did not set its
/// own. The API only serves JSON, server-sent events and file downloads to
/// script, so nothing needs to be framed, sniffed, cached or sent a referrer.
pub async fn security_headers(State(policy): State<HeaderPolicy>, request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    if policy.hsts {
        headers
            .entry(header::STRICT_TRANSPORT_SECURITY)
            .or_insert_with(|| HeaderValue::from_static("max-age=31536000"));
    }
    let defaults: [(HeaderName, &'static str); 5] = [
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        (header::X_FRAME_OPTIONS, "DENY"),
        (header::REFERRER_POLICY, "no-referrer"),
        (header::CACHE_CONTROL, "no-store"),
        (
            header::CONTENT_SECURITY_POLICY,
            "default-src 'none'; frame-ancestors 'none'",
        ),
    ];
    for (name, value) in defaults {
        headers.entry(name).or_insert_with(|| HeaderValue::from_static(value));
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limiter_counts_per_key_and_window() {
        let limiter = RateLimiter::default();
        let quota = Quota {
            limit: 2,
            period: Duration::from_millis(80),
        };
        assert!(limiter.check("a", quota).is_ok());
        assert!(limiter.check("a", quota).is_ok());
        let wait = limiter.check("a", quota).unwrap_err();
        assert!(wait <= Duration::from_millis(80));
        assert!(limiter.check("b", quota).is_ok(), "keys are independent");
        std::thread::sleep(Duration::from_millis(90));
        assert!(limiter.check("a", quota).is_ok(), "a new window starts");
        assert!(limiter
            .check(
                "c",
                Quota {
                    limit: 0,
                    period: Duration::from_secs(1)
                }
            )
            .is_ok());
        // A disabled quota never opens a window.
        assert_eq!(limiter.prune(Duration::ZERO), 2);
    }

    #[test]
    fn login_throttle_locks_after_repeated_failures_and_clears_on_success() {
        let throttle = LoginThrottle::default();
        let lockout = Duration::from_secs(60);
        assert!(!throttle.record_failure("user", 3, lockout));
        assert!(!throttle.record_failure("user", 3, lockout));
        assert!(throttle.locked_for("user").is_none());
        assert!(throttle.record_failure("user", 3, lockout));
        assert!(throttle.locked_for("user").is_some());
        assert!(throttle.locked_for("other").is_none());
        throttle.clear("user");
        assert!(throttle.locked_for("user").is_none());
        throttle.record_failure("login:a@example.com|10.0.0.1", 1, lockout);
        throttle.record_failure("login:a@example.com", 1, lockout);
        throttle.record_failure("login:ab@example.com", 1, lockout);
        assert_eq!(throttle.locked_count(), 3);
        assert_eq!(throttle.clear_account("a@example.com"), 2, "only that account");
        assert_eq!(throttle.locked_count(), 1);
        assert!(
            !throttle.record_failure("user", 0, lockout),
            "zero disables the lockout"
        );
    }

    #[test]
    fn client_addresses_only_come_from_proxy_headers_when_trusted() {
        let off = ProxyTrust::default();
        let on = ProxyTrust {
            enabled: true,
            proxies: vec!["10.0.0.1".parse().unwrap()],
        };
        let proxy = Some("10.0.0.1".parse().unwrap());
        let stranger = Some("198.51.100.7".parse().unwrap());
        let mut headers = axum::http::HeaderMap::new();
        // The client wrote the first entry; the proxy appended the real address.
        headers.insert("x-forwarded-for", HeaderValue::from_static("1.2.3.4, 203.0.113.9"));
        assert_eq!(
            client_ip(&headers, proxy, &off),
            "10.0.0.1",
            "headers are ignored unless trusted"
        );
        assert_eq!(
            client_ip(&headers, proxy, &on),
            "203.0.113.9",
            "the rightmost entry the proxy added"
        );
        assert_eq!(
            client_ip(&headers, stranger, &on),
            "198.51.100.7",
            "only a trusted peer's headers count"
        );
        let local = Some("127.0.0.1".parse().unwrap());
        assert_eq!(
            client_ip(&headers, local, &on),
            "203.0.113.9",
            "a proxy on the same machine is trusted"
        );
        headers.insert("x-forwarded-for", HeaderValue::from_static("203.0.113.9, 10.0.0.1"));
        assert_eq!(
            client_ip(&headers, proxy, &on),
            "203.0.113.9",
            "trusted proxies in the chain are skipped"
        );
        headers.insert("x-forwarded-for", HeaderValue::from_static("not-an-address"));
        assert_eq!(client_ip(&headers, proxy, &on), "10.0.0.1");
        headers.insert("x-real-ip", HeaderValue::from_static("203.0.113.10"));
        assert_eq!(client_ip(&headers, proxy, &on), "203.0.113.10");
        assert_eq!(client_ip(&axum::http::HeaderMap::new(), None, &on), "unknown");
    }
}
