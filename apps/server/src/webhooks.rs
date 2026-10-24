//! Push webhooks for repository links (roadmap F16a, V0.2.23) and connector
//! webhooks (F17e, V0.2.31).
//!
//! A repository or connector has one webhook secret, made by a workspace
//! administrator and shown once. Hosts call `POST /v1/webhooks/repositories/{id}`
//! when someone pushes, or `POST /v1/webhooks/connectors/{id}` when an issue,
//! a comment or a pull request changes, and a correctly signed call queues
//! one sync (a sync that is already running absorbs it), so a workspace
//! follows its sources within seconds instead of at the next poll. Three
//! senders are understood:
//!
//! - **GitHub**: `X-Hub-Signature-256: sha256=<hex>`, an HMAC-SHA256 of the
//!   body, and `X-GitHub-Event` (`ping` answers, the events of the source
//!   sync);
//! - **GitLab**: `X-Gitlab-Token`, the secret itself (GitLab sends no
//!   signature), and `X-Gitlab-Event`;
//! - **anything else**: `X-RepoMemo-Signature: sha256=<hex>`, the same HMAC.
//!
//! Every comparison is constant time. A call that cannot be verified is
//! answered `401` the same way whether or not the source exists, and
//! failures are limited per client address so the endpoint cannot be used to
//! guess or to flood. A delivery id a source has already been sent (GitHub's
//! `X-GitHub-Delivery`, GitLab's `X-Gitlab-Event-UUID`, a generic sender's
//! `X-RepoMemo-Delivery`) is not acted on twice within the hour, so a signed
//! call that was captured cannot be replayed to keep a source syncing.
//!
//! Since V0.3.22 every call that was signed right is kept as a *delivery* (when, sender, event, what
//! became of it and why not), the newest hundred per source. A delivery that asked for a sync, whatever
//! came of it, can be sent again by a writer from the source's page: that queues the same sync again
//! and is kept as a delivery of its own.

use axum::{body::Bytes, http::HeaderMap};
use ring::hmac;
use serde_json::Value;

use crate::*;

/// The largest body accepted: push payloads are a few KiB, and a sync never
/// needs more than the branch name from them.
pub(crate) const MAX_BODY_BYTES: usize = 1024 * 1024;
/// Failed calls per client address per minute.
const FAILURES_PER_MINUTE: u32 = 20;
/// Verified calls per source per minute.
const CALLS_PER_MINUTE: u32 = 60;

#[derive(Debug, PartialEq, Eq)]
enum Sender {
    GitHub,
    GitLab,
    Generic,
}

fn sender_of(headers: &HeaderMap) -> Option<Sender> {
    if headers.contains_key("x-hub-signature-256") {
        Some(Sender::GitHub)
    } else if headers.contains_key("x-gitlab-token") {
        Some(Sender::GitLab)
    } else if headers.contains_key("x-repomemo-signature") {
        Some(Sender::Generic)
    } else {
        None
    }
}

/// Compares two secrets without the time taken depending on where they
/// differ or how long they are: each is signed with a throwaway key and the
/// signatures are compared by `ring`'s constant-time verify.
fn same_secret(left: &[u8], right: &[u8]) -> bool {
    let key = hmac::Key::new(hmac::HMAC_SHA256, b"repomemo webhook comparison");
    hmac::verify(&key, left, hmac::sign(&key, right).as_ref()).is_ok()
}

/// The id a sender gives this delivery, or empty when it gives none.
fn delivery_id<'a>(sender: &Sender, headers: &'a HeaderMap) -> &'a str {
    header(
        headers,
        match sender {
            Sender::GitHub => "x-github-delivery",
            Sender::GitLab => "x-gitlab-event-uuid",
            Sender::Generic => "x-repomemo-delivery",
        },
    )
}

/// What `authenticate` made of a call that is signed right.
enum Delivery {
    Fresh(Sender),
    /// The delivery id was already received in the last hour.
    Repeated,
}

fn header<'a>(headers: &'a HeaderMap, name: &str) -> &'a str {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .unwrap_or_default()
}

/// Whether the call is signed with `secret`, in the way its sender signs.
fn verified(sender: &Sender, headers: &HeaderMap, body: &[u8], secret: &str) -> bool {
    match sender {
        Sender::GitLab => same_secret(header(headers, "x-gitlab-token").as_bytes(), secret.as_bytes()),
        Sender::GitHub | Sender::Generic => {
            let name = if *sender == Sender::GitHub {
                "x-hub-signature-256"
            } else {
                "x-repomemo-signature"
            };
            let Some(signature) = header(headers, name)
                .strip_prefix("sha256=")
                .and_then(|hex| hex::decode(hex).ok())
            else {
                return false;
            };
            let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
            hmac::verify(&key, body, &signature).is_ok()
        }
    }
}

/// What came of asking for a sync.
enum Asked {
    Queued,
    /// A sync was already running and takes the change up.
    Absorbed,
    Failed(String),
}

impl Asked {
    fn of(result: anyhow::Result<Option<IndexingJobStatus>>) -> Self {
        match result {
            Ok(Some(_)) => Self::Queued,
            Ok(None) => Self::Absorbed,
            // No room on the disk or in the allowance, or paused for the tracker's rate limit: told so, and
            // the next poll tries again.
            Err(error) => Self::Failed(format!("{error:#}")),
        }
    }

    /// The outcome and reason a delivery is kept with.
    fn outcome(&self) -> (&'static str, &str) {
        match self {
            Self::Queued => ("queued", ""),
            Self::Absorbed => ("absorbed", "A sync was already running."),
            Self::Failed(reason) => ("failed", reason),
        }
    }

    fn answer(&self) -> (StatusCode, Json<Value>) {
        match self {
            Self::Queued => answer(StatusCode::ACCEPTED, true, "sync queued"),
            Self::Absorbed => answer(StatusCode::ACCEPTED, false, "already syncing"),
            Self::Failed(reason) => answer(StatusCode::ACCEPTED, false, reason),
        }
    }
}

fn sender_name(sender: &Sender) -> &'static str {
    match sender {
        Sender::GitHub => "github",
        Sender::GitLab => "gitlab",
        Sender::Generic => "other",
    }
}

/// The event the sender names, or `call` for a sender that names none.
fn event_of(sender: &Sender, headers: &HeaderMap) -> String {
    let event = match sender {
        Sender::GitHub => header(headers, "x-github-event"),
        Sender::GitLab => header(headers, "x-gitlab-event"),
        Sender::Generic => "",
    };
    if event.is_empty() {
        "call".to_owned()
    } else {
        event.to_owned()
    }
}

/// Keeps a call that was signed right, with what became of it. A failure to keep it only costs the history.
async fn note_delivery(
    state: &AppState,
    source_id: &str,
    sender: &Sender,
    headers: &HeaderMap,
    outcome: &str,
    reason: &str,
) {
    let delivery = delivery_id(sender, headers);
    let delivery = if delivery.len() <= 128 { delivery } else { "" };
    if let Err(error) = state
        .storage
        .record_webhook_delivery(
            source_id,
            sender_name(sender),
            &event_of(sender, headers),
            delivery,
            outcome,
            reason,
        )
        .await
    {
        tracing::warn!(error = %format!("{error:#}"), "A webhook delivery could not be noted");
    }
}

/// What a verified call asks for.
#[derive(Debug, PartialEq, Eq)]
enum Wish {
    Pong,
    Sync,
    Ignore(&'static str),
}

fn wish_of(sender: &Sender, headers: &HeaderMap, body: &[u8], followed_branch: Option<&str>) -> Wish {
    match sender {
        Sender::GitHub => match header(headers, "x-github-event") {
            "ping" => return Wish::Pong,
            "push" => {}
            _ => return Wish::Ignore("not a push"),
        },
        Sender::GitLab => {
            if header(headers, "x-gitlab-event") != "Push Hook" {
                return Wish::Ignore("not a push");
            }
        }
        Sender::Generic => {}
    }
    let reference = serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|payload| payload.get("ref").and_then(Value::as_str).map(str::to_owned));
    match reference.as_deref() {
        Some(reference) if !reference.starts_with("refs/heads/") => Wish::Ignore("not a branch"),
        Some(reference) => match followed_branch {
            Some(branch) if reference != format!("refs/heads/{branch}") => Wish::Ignore("another branch"),
            _ => Wish::Sync,
        },
        None => Wish::Sync,
    }
}

/// What a verified call to a connector's webhook asks for: any change to an
/// issue, a pull or merge request, or their discussion.
fn connector_wish_of(sender: &Sender, headers: &HeaderMap) -> Wish {
    match sender {
        Sender::GitHub => match header(headers, "x-github-event") {
            "ping" => Wish::Pong,
            "issues" | "issue_comment" | "pull_request" | "pull_request_review" | "pull_request_review_comment" => {
                Wish::Sync
            }
            _ => Wish::Ignore("not an issue or pull request event"),
        },
        Sender::GitLab => match header(headers, "x-gitlab-event") {
            "Issue Hook"
            | "Confidential Issue Hook"
            | "Note Hook"
            | "Confidential Note Hook"
            | "Merge Request Hook" => Wish::Sync,
            _ => Wish::Ignore("not an issue or merge request event"),
        },
        Sender::Generic => Wish::Sync,
    }
}

/// Checks that a call is signed right for `source_id` and within the limits,
/// and notes that it arrived. `exists` is whether the source is one this
/// endpoint serves; the answer is the same `401` when it is not, when it has
/// no webhook and when the signature is wrong.
async fn authenticate(
    state: &AppState,
    client: &ClientIp,
    source_id: &str,
    exists: bool,
    headers: &HeaderMap,
    body: &[u8],
) -> Result<Delivery, ApiError> {
    let fail = |state: &AppState| -> ApiError {
        // Only failures are counted against the address, so a host that
        // pushes a lot is never slowed by its own successes.
        match state.guards.auth.check(
            &format!("webhook-fail:{}", client.limit_key()),
            Quota::per_minute(FAILURES_PER_MINUTE),
        ) {
            Ok(()) => ApiError::unauthorized(),
            Err(wait) => crate::security::too_many_requests(
                format!(
                    "Too many calls that could not be verified. Try again in {}.",
                    crate::security::wait_text(wait)
                ),
                wait,
            ),
        }
    };
    if body.len() > MAX_BODY_BYTES {
        return Err(ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "payload_too_large",
            "That payload is too large for a webhook.",
        ));
    }
    let Some(sender) = sender_of(headers) else {
        return Err(fail(state));
    };
    let secret = if exists {
        state
            .storage
            .open_repo_webhook_secret(source_id)
            .await
            .map_err(ApiError::internal)?
    } else {
        None
    };
    // Verify against *something* even when there is no secret to compare, so
    // the time taken does not tell the cases apart.
    let candidate = secret.clone().unwrap_or_else(|| "-".repeat(64));
    let signed = verified(&sender, headers, body, &candidate);
    if !(exists && secret.is_some() && signed) {
        return Err(fail(state));
    }
    // A repeat is answered without counting against the source's calls, so replaying a captured
    // call cannot use up the quota the real sender needs.
    let delivery = delivery_id(&sender, headers);
    if !delivery.is_empty()
        && delivery.len() <= 128
        && state
            .guards
            .auth
            .check(&format!("delivery:{source_id}:{delivery}"), Quota::per_hour(1))
            .is_err()
    {
        return Ok(Delivery::Repeated);
    }
    state
        .guards
        .auth
        .check(&format!("webhook:{source_id}"), Quota::per_minute(CALLS_PER_MINUTE))
        .map_err(|wait| {
            crate::security::too_many_requests(
                format!(
                    "This source gets webhook calls too fast. Try again in {}.",
                    crate::security::wait_text(wait)
                ),
                wait,
            )
        })?;
    if let Err(error) = state.storage.touch_repo_webhook(source_id).await {
        tracing::warn!(error = %format!("{error:#}"), "A webhook's last call could not be noted");
    }
    Ok(Delivery::Fresh(sender))
}

fn answer(status: StatusCode, queued: bool, note: &str) -> (StatusCode, Json<Value>) {
    (status, Json(json!({"queued": queued, "note": note})))
}

/// Public. The same `401` for an unknown repository, one without a webhook
/// and a call that is not signed right.
pub(crate) async fn receive_repository_webhook(
    State(state): State<AppState>,
    client: ClientIp,
    Path(source_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let repository = state.core.repo_source(&source_id).await.ok();
    let sender = match authenticate(&state, &client, &source_id, repository.is_some(), &headers, &body).await? {
        Delivery::Fresh(sender) => sender,
        Delivery::Repeated => return Ok(answer(StatusCode::ACCEPTED, false, "already received")),
    };
    let Some(repository) = repository else {
        return Err(ApiError::unauthorized());
    };
    match wish_of(&sender, &headers, &body, repository.settings.branch.as_deref()) {
        Wish::Pong => {
            note_delivery(&state, &source_id, &sender, &headers, "pong", "").await;
            Ok(answer(StatusCode::OK, false, "pong"))
        }
        Wish::Ignore(reason) => {
            note_delivery(&state, &source_id, &sender, &headers, "ignored", reason).await;
            Ok(answer(StatusCode::ACCEPTED, false, reason))
        }
        Wish::Sync => {
            tracing::info!(target: "audit", repository = %source_id, "A push webhook asked for a sync");
            let asked = Asked::of(state.repo_sync.start(&repository, None).await);
            let (outcome, reason) = asked.outcome();
            note_delivery(&state, &source_id, &sender, &headers, outcome, reason).await;
            Ok(asked.answer())
        }
    }
}

/// Public. A tracker's webhook: any change to an issue or a pull request
/// queues one sync of the connector.
pub(crate) async fn receive_connector_webhook(
    State(state): State<AppState>,
    client: ClientIp,
    Path(source_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let connector = state.core.connector_source(&source_id).await.ok();
    let sender = match authenticate(&state, &client, &source_id, connector.is_some(), &headers, &body).await? {
        Delivery::Fresh(sender) => sender,
        Delivery::Repeated => return Ok(answer(StatusCode::ACCEPTED, false, "already received")),
    };
    let Some(connector) = connector else {
        return Err(ApiError::unauthorized());
    };
    match connector_wish_of(&sender, &headers) {
        Wish::Pong => {
            note_delivery(&state, &source_id, &sender, &headers, "pong", "").await;
            Ok(answer(StatusCode::OK, false, "pong"))
        }
        Wish::Ignore(reason) => {
            note_delivery(&state, &source_id, &sender, &headers, "ignored", reason).await;
            Ok(answer(StatusCode::ACCEPTED, false, reason))
        }
        Wish::Sync => {
            tracing::info!(target: "audit", connector = %source_id, "A webhook asked for a connector sync");
            let asked = Asked::of(state.connector_sync.start(&connector, None).await);
            let (outcome, reason) = asked.outcome();
            note_delivery(&state, &source_id, &sender, &headers, outcome, reason).await;
            Ok(asked.answer())
        }
    }
}

/// Resends allowed per source per minute.
const RESENDS_PER_MINUTE: u32 = 10;

/// Most deliveries a list shows.
const DELIVERIES_LISTED: i64 = 100;

#[derive(serde::Serialize)]
pub(crate) struct ResendAnswer {
    queued: bool,
    note: String,
    /// The delivery this resend is kept as.
    delivery: repomemo_domain::WebhookDelivery,
}

async fn deliveries_of(
    state: &AppState,
    source_id: &str,
) -> Result<Json<Vec<repomemo_domain::WebhookDelivery>>, ApiError> {
    state
        .storage
        .list_webhook_deliveries(source_id, DELIVERIES_LISTED)
        .await
        .map(Json)
        .map_err(ApiError::internal)
}

/// Sends a delivery again: the sync it asked for is asked for once more, whatever came of it the first time.
/// `start` is how the source's own runner is asked.
async fn resend(
    state: &AppState,
    subject: &AuthenticatedSubject,
    source_id: &str,
    delivery_id: &str,
    start: impl std::future::Future<Output = anyhow::Result<Option<IndexingJobStatus>>>,
) -> Result<(StatusCode, Json<ResendAnswer>), ApiError> {
    let delivery = state
        .storage
        .webhook_delivery(source_id, delivery_id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("That delivery was not found. Only the newest hundred are kept."))?;
    if !matches!(delivery.outcome.as_str(), "queued" | "absorbed" | "failed") {
        return Err(ApiError::bad_request(
            "That delivery did not ask for a sync, so there is nothing to send again.",
        ));
    }
    state
        .guards
        .auth
        .check(
            &format!("webhook-resend:{source_id}"),
            Quota::per_minute(RESENDS_PER_MINUTE),
        )
        .map_err(|wait| {
            crate::security::too_many_requests(
                format!(
                    "Deliveries of this source are being sent again too fast. Try again in {}.",
                    crate::security::wait_text(wait)
                ),
                wait,
            )
        })?;
    tracing::info!(target: "audit", source = %source_id, "A webhook delivery was sent again");
    let asked = Asked::of(start.await);
    let (outcome, reason) = asked.outcome();
    let id = state
        .storage
        .record_webhook_delivery(
            source_id,
            "resend",
            &delivery.event,
            &delivery.delivery_id,
            outcome,
            reason,
        )
        .await
        .map_err(ApiError::internal)?;
    if let Err(error) = state
        .storage
        .mark_webhook_delivery_resent(source_id, delivery_id, &subject.user_id)
        .await
    {
        tracing::warn!(error = %format!("{error:#}"), "A resent delivery could not be marked");
    }
    let kept = state
        .storage
        .webhook_delivery(source_id, &id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("That delivery was not found."))?;
    let (status, Json(body)) = asked.answer();
    Ok((
        status,
        Json(ResendAnswer {
            queued: body.get("queued").and_then(Value::as_bool).unwrap_or(false),
            note: body.get("note").and_then(Value::as_str).unwrap_or_default().to_owned(),
            delivery: kept,
        }),
    ))
}

/// The calls a connector's webhook received (writers).
pub(crate) async fn list_connector_webhook_deliveries(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(source_id): Path<String>,
) -> Result<Json<Vec<repomemo_domain::WebhookDelivery>>, ApiError> {
    let connector = state
        .core
        .connector_source(&source_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &connector.workspace_id).await?;
    deliveries_of(&state, &source_id).await
}

/// Sends a delivery to a connector again (writers).
pub(crate) async fn resend_connector_webhook_delivery(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path((source_id, delivery_id)): Path<(String, String)>,
) -> Result<(StatusCode, Json<ResendAnswer>), ApiError> {
    let connector = state
        .core
        .connector_source(&source_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &connector.workspace_id).await?;
    resend(
        &state,
        &subject,
        &source_id,
        &delivery_id,
        state.connector_sync.start(&connector, Some(subject.user_id.clone())),
    )
    .await
}

/// The calls a repository's webhook received (writers).
pub(crate) async fn list_repository_webhook_deliveries(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(source_id): Path<String>,
) -> Result<Json<Vec<repomemo_domain::WebhookDelivery>>, ApiError> {
    let repository = state.core.repo_source(&source_id).await.map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &repository.workspace_id).await?;
    deliveries_of(&state, &source_id).await
}

/// Sends a delivery to a repository again (writers).
pub(crate) async fn resend_repository_webhook_delivery(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path((source_id, delivery_id)): Path<(String, String)>,
) -> Result<(StatusCode, Json<ResendAnswer>), ApiError> {
    let repository = state.core.repo_source(&source_id).await.map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &repository.workspace_id).await?;
    resend(
        &state,
        &subject,
        &source_id,
        &delivery_id,
        state.repo_sync.start(&repository, Some(subject.user_id.clone())),
    )
    .await
}

#[derive(serde::Serialize)]
pub(crate) struct WebhookCreated {
    /// Shown once: the secret the sender signs with.
    secret: String,
    /// The path to call, and the full address when the server knows its own.
    path: String,
    url: Option<String>,
}

#[allow(clippy::too_many_arguments)]
async fn create_webhook(
    state: &AppState,
    subject: &AuthenticatedSubject,
    source_id: &str,
    workspace_id: &str,
    name: &str,
    what: &str,
    path: String,
    action: &str,
) -> Result<(StatusCode, Json<WebhookCreated>), ApiError> {
    require_workspace_admin(state, subject, workspace_id).await?;
    let secret = state
        .storage
        .create_repo_webhook(source_id, Some(&subject.user_id))
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        state,
        workspace_id,
        &subject.user_id,
        action,
        what,
        Some(source_id),
        format!("Set up webhooks for {name}."),
    )
    .await;
    Ok((
        StatusCode::CREATED,
        Json(WebhookCreated {
            url: state.mail.link(&path),
            path,
            secret,
        }),
    ))
}

async fn delete_webhook(
    state: &AppState,
    subject: &AuthenticatedSubject,
    source_id: &str,
    workspace_id: &str,
    name: &str,
    what: &str,
    action: &str,
) -> Result<StatusCode, ApiError> {
    require_workspace_admin(state, subject, workspace_id).await?;
    if !state
        .storage
        .clear_repo_webhook(source_id)
        .await
        .map_err(map_storage_error)?
    {
        return Err(ApiError::not_found("This source has no webhook."));
    }
    record_workspace_activity(
        state,
        workspace_id,
        &subject.user_id,
        action,
        what,
        Some(source_id),
        format!("Removed the webhooks of {name}."),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

/// Makes or replaces a repository's webhook secret (workspace administrators).
pub(crate) async fn create_repository_webhook(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(source_id): Path<String>,
) -> Result<(StatusCode, Json<WebhookCreated>), ApiError> {
    let repository = state.core.repo_source(&source_id).await.map_err(map_storage_error)?;
    create_webhook(
        &state,
        &subject,
        &source_id,
        &repository.workspace_id,
        &format!("repository {}", repository.name),
        "repository",
        format!("/v1/webhooks/repositories/{source_id}"),
        "repository_webhook_created",
    )
    .await
}

pub(crate) async fn delete_repository_webhook(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(source_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let repository = state.core.repo_source(&source_id).await.map_err(map_storage_error)?;
    delete_webhook(
        &state,
        &subject,
        &source_id,
        &repository.workspace_id,
        &format!("repository {}", repository.name),
        "repository",
        "repository_webhook_removed",
    )
    .await
}

/// Makes or replaces a connector's webhook secret (workspace administrators).
pub(crate) async fn create_connector_webhook(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(source_id): Path<String>,
) -> Result<(StatusCode, Json<WebhookCreated>), ApiError> {
    let connector = state
        .core
        .connector_source(&source_id)
        .await
        .map_err(map_storage_error)?;
    create_webhook(
        &state,
        &subject,
        &source_id,
        &connector.workspace_id,
        &connector.name,
        "connector",
        format!("/v1/webhooks/connectors/{source_id}"),
        "connector_webhook_created",
    )
    .await
}

pub(crate) async fn delete_connector_webhook(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(source_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let connector = state
        .core
        .connector_source(&source_id)
        .await
        .map_err(map_storage_error)?;
    delete_webhook(
        &state,
        &subject,
        &source_id,
        &connector.workspace_id,
        &connector.name,
        "connector",
        "connector_webhook_removed",
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn signed(secret: &str, body: &[u8]) -> String {
        let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
        format!("sha256={}", hex::encode(hmac::sign(&key, body).as_ref()))
    }

    fn headers(pairs: &[(&'static str, String)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.insert(*name, HeaderValue::from_str(value).unwrap());
        }
        map
    }

    #[test]
    fn signatures_are_checked_the_way_each_sender_makes_them() {
        let body = br#"{"ref":"refs/heads/main"}"#;
        let secret = "s3cret";
        let github = headers(&[("x-hub-signature-256", signed(secret, body))]);
        assert!(verified(&Sender::GitHub, &github, body, secret));
        assert!(!verified(&Sender::GitHub, &github, body, "other"));
        assert!(
            !verified(&Sender::GitHub, &github, b"{}", secret),
            "the body is part of it"
        );
        let generic = headers(&[("x-repomemo-signature", signed(secret, body))]);
        assert!(verified(&Sender::Generic, &generic, body, secret));
        let gitlab = headers(&[("x-gitlab-token", secret.to_owned())]);
        assert!(verified(&Sender::GitLab, &gitlab, body, secret));
        assert!(!verified(&Sender::GitLab, &gitlab, body, "other"));
        for broken in ["", "sha256=", "sha256=zz", "sha1=abcd", &signed("x", body)[..20]] {
            let headers = headers(&[("x-hub-signature-256", broken.to_owned())]);
            assert!(!verified(&Sender::GitHub, &headers, body, secret), "{broken}");
        }
        assert_eq!(sender_of(&HeaderMap::new()), None);
    }

    #[test]
    fn a_push_asks_for_a_sync_only_when_it_concerns_the_followed_branch() {
        let github = |event: &str| headers(&[("x-github-event", event.to_owned())]);
        let main = br#"{"ref":"refs/heads/main"}"#;
        assert_eq!(wish_of(&Sender::GitHub, &github("ping"), b"{}", None), Wish::Pong);
        assert_eq!(wish_of(&Sender::GitHub, &github("push"), main, None), Wish::Sync);
        assert_eq!(
            wish_of(&Sender::GitHub, &github("push"), main, Some("main")),
            Wish::Sync
        );
        assert_eq!(
            wish_of(&Sender::GitHub, &github("push"), main, Some("release")),
            Wish::Ignore("another branch")
        );
        assert_eq!(
            wish_of(&Sender::GitHub, &github("push"), br#"{"ref":"refs/tags/v1"}"#, None),
            Wish::Ignore("not a branch")
        );
        assert_eq!(
            wish_of(&Sender::GitHub, &github("issues"), main, None),
            Wish::Ignore("not a push")
        );
        let gitlab = headers(&[("x-gitlab-event", "Push Hook".to_owned())]);
        assert_eq!(wish_of(&Sender::GitLab, &gitlab, main, None), Wish::Sync);
        let tag = headers(&[("x-gitlab-event", "Tag Push Hook".to_owned())]);
        assert_eq!(wish_of(&Sender::GitLab, &tag, main, None), Wish::Ignore("not a push"));
        // A generic sender needs no payload at all.
        assert_eq!(wish_of(&Sender::Generic, &HeaderMap::new(), b"", None), Wish::Sync);
    }

    #[test]
    fn a_connector_webhook_syncs_on_item_events_only() {
        let github = |event: &str| headers(&[("x-github-event", event.to_owned())]);
        for event in [
            "issues",
            "issue_comment",
            "pull_request",
            "pull_request_review",
            "pull_request_review_comment",
        ] {
            assert_eq!(
                connector_wish_of(&Sender::GitHub, &github(event)),
                Wish::Sync,
                "{event}"
            );
        }
        assert_eq!(connector_wish_of(&Sender::GitHub, &github("ping")), Wish::Pong);
        assert!(matches!(
            connector_wish_of(&Sender::GitHub, &github("push")),
            Wish::Ignore(_)
        ));
        let gitlab = |event: &str| headers(&[("x-gitlab-event", event.to_owned())]);
        for event in ["Issue Hook", "Note Hook", "Merge Request Hook"] {
            assert_eq!(
                connector_wish_of(&Sender::GitLab, &gitlab(event)),
                Wish::Sync,
                "{event}"
            );
        }
        assert!(matches!(
            connector_wish_of(&Sender::GitLab, &gitlab("Push Hook")),
            Wish::Ignore(_)
        ));
        assert_eq!(connector_wish_of(&Sender::Generic, &HeaderMap::new()), Wish::Sync);
    }
}
