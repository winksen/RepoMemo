//! Load shedding (V0.2.161, review P21). Past the read pool and the writer, requests used to queue
//! until their timeout. The server now counts the requests being served, and refuses one at once
//! when too many are: `503` with `Retry-After` when the server as a whole is full, `429` when one
//! account holds more than its share. Health checks, sign-in and the event streams are outside the
//! count, so they keep working when everything else is refused.

use crate::*;
use axum::{extract::Request, middleware::Next, response::Response};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

/// The count of requests being served, for the whole server and for each account.
pub(crate) struct LoadGate {
    /// Requests served at once, all accounts together; 0 for no limit.
    max: usize,
    /// Requests served at once for one account; 0 for no limit.
    per_account: usize,
    in_flight: AtomicUsize,
    accounts: Mutex<HashMap<String, usize>>,
    shed_server: AtomicU64,
    shed_account: AtomicU64,
}

/// Why a request was refused.
pub(crate) enum Refusal {
    ServerFull,
    AccountFull,
}

/// A place among the requests being served; freed when dropped, however the request ends.
pub(crate) struct Place {
    gate: std::sync::Arc<LoadGate>,
    account: Option<String>,
}

impl LoadGate {
    pub(crate) fn new(max: usize, per_account: usize) -> Self {
        Self {
            max,
            per_account,
            in_flight: AtomicUsize::new(0),
            accounts: Mutex::new(HashMap::new()),
            shed_server: AtomicU64::new(0),
            shed_account: AtomicU64::new(0),
        }
    }

    /// Takes a place for a request, or says why not. `account` is who the request is from, when that
    /// is known without asking the database.
    pub(crate) fn enter(self: &std::sync::Arc<Self>, account: Option<String>) -> Result<Place, Refusal> {
        if self.max > 0 && self.in_flight.fetch_add(1, Ordering::SeqCst) >= self.max {
            self.in_flight.fetch_sub(1, Ordering::SeqCst);
            self.shed_server.fetch_add(1, Ordering::Relaxed);
            return Err(Refusal::ServerFull);
        }
        if self.max == 0 {
            self.in_flight.fetch_add(1, Ordering::SeqCst);
        }
        if let (Some(key), true) = (account.as_ref(), self.per_account > 0) {
            let mut accounts = self.accounts.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            let held = accounts.entry(key.clone()).or_insert(0);
            if *held >= self.per_account {
                drop(accounts);
                self.in_flight.fetch_sub(1, Ordering::SeqCst);
                self.shed_account.fetch_add(1, Ordering::Relaxed);
                return Err(Refusal::AccountFull);
            }
            *held += 1;
        }
        Ok(Place {
            gate: self.clone(),
            account: account.filter(|_| self.per_account > 0),
        })
    }

    /// Requests being served now.
    pub(crate) fn in_flight(&self) -> usize {
        self.in_flight.load(Ordering::SeqCst)
    }

    /// `(refused because the server was full, refused because the account was)` since the start.
    pub(crate) fn shed(&self) -> (u64, u64) {
        (
            self.shed_server.load(Ordering::Relaxed),
            self.shed_account.load(Ordering::Relaxed),
        )
    }
}

impl Drop for Place {
    fn drop(&mut self) {
        self.gate.in_flight.fetch_sub(1, Ordering::SeqCst);
        if let Some(key) = self.account.take() {
            let mut accounts = self
                .gate
                .accounts
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Some(held) = accounts.get_mut(&key) {
                *held = held.saturating_sub(1);
                if *held == 0 {
                    accounts.remove(&key);
                }
            }
        }
    }
}

/// Requests that are never counted: health checks, sign-in and the like (so a server that is busy
/// can still be checked and signed in to) and the event streams, which stay open on purpose.
fn outside_the_count(path: &str) -> bool {
    path.starts_with("/health")
        || path.starts_with("/v1/auth/")
        || path.starts_with("/v1/setup")
        || path.starts_with("/v1/invitations/")
        || path.ends_with("/events")
}

/// Who a request is from, without asking the database: the user of a session token (its signature
/// and expiry are checked, so nobody can spend another account's share) or a personal access token.
fn account_of(state: &AppState, request: &Request) -> Option<String> {
    let token = request
        .headers()
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")?;
    if token.starts_with(repomemo_storage::PERSONAL_TOKEN_PREFIX) {
        return Some(format!("token:{token}"));
    }
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_issuer(&[JWT_ISSUER]);
    decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(state.jwt_secret.as_bytes()),
        &validation,
    )
    .ok()
    .map(|data| format!("user:{}", data.claims.sub))
}

/// The middleware: takes a place for the request or refuses it at once.
pub(crate) async fn shed_load(State(state): State<AppState>, request: Request, next: Next) -> Response {
    if outside_the_count(request.uri().path()) {
        return next.run(request).await;
    }
    let account = account_of(&state, &request);
    match state.load.enter(account) {
        Ok(_place) => next.run(request).await,
        Err(Refusal::ServerFull) => {
            let mut error = ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "overloaded",
                "The server is serving as many requests as it can. Try again in a moment.",
            );
            error.retry_after = Some(1);
            error.into_response()
        }
        Err(Refusal::AccountFull) => security::too_many_requests(
            "You have too many requests running at once. Wait for some to finish, then try again.",
            Duration::from_secs(1),
        )
        .into_response(),
    }
}
