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

/// The most keys a limiter holds at once. Past it, windows that ran out are dropped, and if that is not
/// enough the oldest are, so a flood of addresses cannot grow the map without bound (V0.2.145).
const MAX_TRACKED_KEYS: usize = 100_000;

/// Fixed-window request counters per key.
pub struct RateLimiter {
    windows: Mutex<HashMap<String, Window>>,
    max_keys: usize,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::with_max_keys(MAX_TRACKED_KEYS)
    }
}

impl RateLimiter {
    pub fn with_max_keys(max_keys: usize) -> Self {
        Self {
            windows: Mutex::default(),
            max_keys: max_keys.max(1),
        }
    }

    pub(crate) fn len(&self) -> usize {
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
        if windows.len() >= self.max_keys && !windows.contains_key(key) {
            // Windows past their period count for nothing; then the oldest go, a tenth of the bound at a time.
            windows.retain(|_, window| now.duration_since(window.started) < quota.period);
            if windows.len() >= self.max_keys {
                let mut oldest: Vec<(Instant, String)> = windows
                    .iter()
                    .map(|(key, window)| (window.started, key.clone()))
                    .collect();
                oldest.sort();
                for (_, key) in oldest.into_iter().take((self.max_keys / 10).max(1)) {
                    windows.remove(&key);
                }
            }
        }
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
pub struct LoginThrottle {
    failures: Mutex<HashMap<String, Failures>>,
    max_keys: usize,
}

impl Default for LoginThrottle {
    fn default() -> Self {
        Self::with_max_keys(MAX_TRACKED_KEYS)
    }
}

impl LoginThrottle {
    pub fn with_max_keys(max_keys: usize) -> Self {
        Self {
            failures: Mutex::default(),
            max_keys: max_keys.max(1),
        }
    }

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
        if failures.len() >= self.max_keys && !failures.contains_key(key) {
            // Counts that went quiet for a lockout period are forgotten; if that is not enough, the quietest
            // keys that are not locked out go. A lockout is never evicted to make room.
            failures.retain(|_, entry| {
                entry.locked_until.is_some_and(|until| until > now) || now.duration_since(entry.last_at) <= lockout
            });
            if failures.len() >= self.max_keys {
                let mut quiet: Vec<(Instant, String)> = failures
                    .iter()
                    .filter(|(_, entry)| entry.locked_until.is_none_or(|until| until <= now))
                    .map(|(key, entry)| (entry.last_at, key.clone()))
                    .collect();
                quiet.sort();
                for (_, key) in quiet.into_iter().take((self.max_keys / 10).max(1)) {
                    failures.remove(&key);
                }
            }
        }
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

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.failures.lock().map(|failures| failures.len()).unwrap_or(0)
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

/// Which file links were used, and by whom first (review L1).
#[derive(Default)]
pub struct FileLinkUses {
    used: Mutex<HashMap<String, (Instant, String)>>,
}

impl FileLinkUses {
    /// Whether this fetch of the link may go ahead. A link nobody fetched yet
    /// is open to whoever holds it; once fetched it stays open only to that
    /// client, and only for `grace`. `record` is false for probes (HEAD).
    pub fn claim(&self, link_id: &str, client: &str, record: bool, grace: Duration, lifetime: Duration) -> bool {
        let Ok(mut used) = self.used.lock() else {
            return true;
        };
        let now = Instant::now();
        used.retain(|_, (first, _)| now.duration_since(*first) < lifetime);
        match used.get(link_id) {
            Some((first, first_client)) => first_client == client && now.duration_since(*first) <= grace,
            None => {
                if record {
                    used.insert(link_id.to_owned(), (now, client.to_owned()));
                }
                true
            }
        }
    }

    fn len(&self) -> usize {
        self.used.lock().map(|used| used.len()).unwrap_or(0)
    }
}

/// Rate limits and sign-in throttling, shared by all requests.
#[derive(Default)]
pub struct Guards {
    /// File links that were fetched.
    pub file_links: FileLinkUses,
    /// Sign-in, registration and token refresh, per client address.
    pub auth: RateLimiter,
    /// AI requests, per user.
    pub ai: RateLimiter,
    /// Searches, figures, the map and health, per user.
    pub heavy: RateLimiter,
    /// Files uploaded, per user.
    pub uploads: RateLimiter,
    /// Failed sign-ins per account and address.
    pub logins: LoginThrottle,
}

impl Guards {
    /// Rate-limit keys tracked, and sign-in keys locked out right now.
    pub fn stats(&self) -> (usize, usize) {
        (
            self.auth.len() + self.ai.len() + self.heavy.len() + self.uploads.len() + self.file_links.len(),
            self.logins.locked_count(),
        )
    }

    pub fn prune(&self) -> usize {
        // Windows are at most an hour long; lockouts are kept while active.
        let max_age = Duration::from_secs(2 * 3600);
        self.auth.prune(max_age)
            + self.ai.prune(max_age)
            + self.heavy.prune(max_age)
            + self.uploads.prune(max_age)
            + self.logins.prune(max_age)
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

impl ClientIp {
    /// What limits and lockouts are counted against: the address itself for IPv4, the /64 it belongs to for IPv6
    /// (a person or an attacker holds a whole /64, so counting each address apart would give them as many
    /// budgets as they like). Logs keep the full address in `.0`.
    pub fn limit_key(&self) -> String {
        network_key(&self.0)
    }
}

/// The key an address is counted under (see [`ClientIp::limit_key`]).
pub(crate) fn network_key(address: &str) -> String {
    match address.parse::<IpAddr>() {
        Ok(IpAddr::V6(v6)) => match v6.to_ipv4_mapped() {
            Some(v4) => v4.to_string(),
            None => {
                let [a, b, c, d, ..] = v6.segments();
                format!("{}/64", std::net::Ipv6Addr::new(a, b, c, d, 0, 0, 0, 0))
            }
        },
        _ => address.to_owned(),
    }
}

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

/// Counts one request to a heavy read endpoint (search, map, health, figures,
/// file queries) against the person's per-minute allowance (review M11).
pub(crate) fn consume_heavy_quota(state: &AppState, subject: &crate::AuthenticatedSubject) -> Result<(), ApiError> {
    let quota = state.settings().heavy_quota;
    state.guards.heavy.check(&subject.user_id, quota).map_err(|wait| {
        tracing::warn!(target: "audit", user_id = %subject.user_id, "Heavy request limit reached");
        too_many_requests(
            format!(
                "You have made {} searches and reports in a minute, which is the limit on this server. Try again in {}.",
                quota.limit,
                wait_text(wait)
            ),
            wait,
        )
    })
}

/// The path of a request as it may be logged. A file link carries its bearer
/// token in the path, which must not reach a log (review L1).
pub(crate) fn loggable_path(path: &str) -> String {
    match path.strip_prefix("/v1/shared-files/") {
        Some(rest) => {
            let name = rest.split_once('/').map_or("", |(_, name)| name);
            format!("/v1/shared-files/[link]/{name}")
        }
        None => path.to_owned(),
    }
}

/// The log span of a request: what tower-http would record, with the path
/// made safe by [`loggable_path`].
pub(crate) fn redacted_span<B>(request: &axum::http::Request<B>) -> tracing::Span {
    tracing::debug_span!(
        "request",
        method = %request.method(),
        uri = %loggable_path(request.uri().path()),
        version = ?request.version(),
    )
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
    fn file_link_secrets_stay_out_of_logged_paths() {
        assert_eq!(
            loggable_path("/v1/shared-files/eyJhbGciOi.secret.sig/plan.eml"),
            "/v1/shared-files/[link]/plan.eml"
        );
        assert_eq!(loggable_path("/v1/workspaces/abc"), "/v1/workspaces/abc");
    }

    #[test]
    fn a_file_link_belongs_to_the_client_that_first_used_it() {
        let uses = FileLinkUses::default();
        let (grace, lifetime) = (Duration::from_millis(60), Duration::from_secs(5));
        assert!(
            uses.claim("link", "10.0.0.1", false, grace, lifetime),
            "a probe opens nothing"
        );
        assert!(uses.claim("link", "10.0.0.2", true, grace, lifetime), "first fetch");
        assert!(
            uses.claim("link", "10.0.0.2", true, grace, lifetime),
            "the same client repeats within the grace"
        );
        assert!(
            !uses.claim("link", "10.0.0.1", true, grace, lifetime),
            "another client is refused"
        );
        std::thread::sleep(Duration::from_millis(80));
        assert!(
            !uses.claim("link", "10.0.0.2", true, grace, lifetime),
            "even the first client, after the grace"
        );
        assert!(
            uses.claim("other", "10.0.0.1", true, grace, lifetime),
            "links are independent"
        );
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
