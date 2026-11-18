//! Opt-in signed outbound webhooks (V0.8.4).
//!
//! An administrator adds an endpoint; its address must be on the operator's list (`REPOMEMO_OUTBOUND_HOSTS`, empty means
//! off) and is checked again at every send. Each event is signed with the endpoint's secret
//! (`X-RepoMemo-Signature: v1=<hmac-sha256 of "{timestamp}.{body}">`) and carries identifiers and counts only. An event has one
//! id for every endpoint and every retry, so a receiver can tell a repeat from a new event. A failed delivery is retried after
//! 30 s, 2 min, 10 min and 1 h and then given up. A disabled endpoint, a deleted one or an address that is no longer approved
//! receives nothing that was still queued.

use std::{sync::Arc, time::Duration};

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use repomemo_api::{RemoteGitPolicy, RepoMemoCore};
use repomemo_domain::{OutboundDelivery, OutboundEndpoint};
use repomemo_storage::{DueDelivery, OUTBOUND_EVENTS};
use ring::hmac;
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;

use crate::{record_workspace_activity, require_workspace_admin, ApiError, AppState, ManagingSubject};

const SEND_TIMEOUT: Duration = Duration::from_secs(10);
const POLL: Duration = Duration::from_secs(3);
const BATCH: i64 = 50;
const MAX_NAME_CHARS: usize = 80;
const MAX_URL_CHARS: usize = 2_000;

#[derive(Clone)]
pub(crate) struct OutboundService {
    core: RepoMemoCore,
    policy: Arc<RemoteGitPolicy>,
    http: reqwest::Client,
    wake: Arc<Notify>,
}

impl OutboundService {
    pub(crate) fn new(core: RepoMemoCore, allowed_hosts: Vec<String>) -> anyhow::Result<Self> {
        Ok(Self {
            core,
            policy: Arc::new(RemoteGitPolicy::with_allowed_hosts(allowed_hosts)),
            http: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(SEND_TIMEOUT)
                .user_agent("RepoMemo-Webhook")
                .build()?,
            wake: Arc::new(Notify::new()),
        })
    }

    /// Whether the operator has named any host.
    pub(crate) fn is_available(&self) -> bool {
        self.policy.is_enabled()
    }

    /// Checks an address and returns it in a normal form: `https` (`http` only to this machine), no credentials, on the list.
    pub(crate) fn check_destination(&self, raw: &str) -> Result<String, ApiError> {
        if !self.policy.is_enabled() {
            return Err(ApiError::bad_request(
                "Outbound webhooks are off on this server. Its operator turns them on by setting REPOMEMO_OUTBOUND_HOSTS to the hosts they may use.",
            ));
        }
        let raw = raw.trim();
        if raw.is_empty() || raw.len() > MAX_URL_CHARS {
            return Err(ApiError::bad_request("Enter the address events are sent to."));
        }
        let url = reqwest::Url::parse(raw).map_err(|_| ApiError::bad_request("That is not a valid address."))?;
        let host = url
            .host_str()
            .map(|host| host.trim_matches(['[', ']']).to_ascii_lowercase())
            .ok_or_else(|| ApiError::bad_request("That is not a valid address."))?;
        let local = matches!(host.as_str(), "localhost" | "127.0.0.1" | "::1");
        if url.scheme() != "https" && !(url.scheme() == "http" && local) {
            return Err(ApiError::bad_request("Only https addresses can receive events."));
        }
        if !url.username().is_empty() || url.password().is_some() || url.fragment().is_some() {
            return Err(ApiError::bad_request(
                "The address cannot carry a name, a password or a fragment. The signature authenticates the sender.",
            ));
        }
        self.policy
            .check_destination(&host, url.port())
            .map_err(|error| ApiError::bad_request(format!("{error:#}")))?;
        Ok(url.to_string())
    }

    /// Sends what is due, once. Returns how many deliveries were attempted.
    pub(crate) async fn dispatch_due(&self) -> usize {
        let due = match self.core.storage().due_outbound_deliveries(BATCH).await {
            Ok(due) => due,
            Err(error) => {
                tracing::warn!(error = %error, "Outbound deliveries could not be read");
                return 0;
            }
        };
        let mut attempted = 0;
        for delivery in due {
            if self.send(&delivery).await {
                attempted += 1;
            }
        }
        attempted
    }

    async fn send(&self, delivery: &DueDelivery) -> bool {
        let storage = self.core.storage();
        if !delivery.enabled {
            let _ = storage
                .cancel_outbound_delivery(&delivery.id, "The endpoint was turned off.")
                .await;
            return false;
        }
        // The operator may have taken the host off the list since the endpoint was made.
        if self.check_destination(&delivery.url).is_err() {
            let _ = storage
                .cancel_outbound_delivery(&delivery.id, "The address is no longer an approved destination.")
                .await;
            return false;
        }
        let Some(secret) = storage.open_outbound_secret(&delivery.endpoint_id).await.ok().flatten() else {
            let _ = storage
                .record_outbound_attempt(delivery, None, Some("The signing secret could not be opened."))
                .await;
            return true;
        };
        let timestamp = chrono::Utc::now().timestamp().to_string();
        let signature = sign(&secret, &timestamp, &delivery.payload);
        let attempt = delivery.attempts + 1;
        let outcome = self
            .http
            .post(&delivery.url)
            .header("Content-Type", "application/json")
            .header("X-RepoMemo-Event", &delivery.event_type)
            .header("X-RepoMemo-Event-Id", &delivery.event_id)
            .header("X-RepoMemo-Delivery", &delivery.id)
            .header("X-RepoMemo-Attempt", attempt.to_string())
            .header("X-RepoMemo-Timestamp", &timestamp)
            .header("X-RepoMemo-Signature", signature)
            .body(delivery.payload.clone())
            .send()
            .await;
        let (code, error) = match outcome {
            Ok(response) => {
                let code = response.status().as_u16();
                (
                    Some(code),
                    (!(200..300).contains(&code)).then(|| format!("The destination answered {code}.")),
                )
            }
            // The text can name the address but never the secret.
            Err(error) => (
                None,
                Some(format!("The destination could not be reached: {}", error.without_url())),
            ),
        };
        if let Err(error) = storage.record_outbound_attempt(delivery, code, error.as_deref()).await {
            tracing::warn!(error = %error, "An outbound delivery's outcome could not be recorded");
        }
        true
    }

    /// Sends what is due now and then every few seconds, or as soon as it is woken.
    pub(crate) fn start(&self) -> tokio::task::JoinHandle<()> {
        let service = self.clone();
        tokio::spawn(async move {
            loop {
                service.dispatch_due().await;
                tokio::select! {
                    _ = tokio::time::sleep(POLL) => {}
                    _ = service.wake.notified() => {}
                }
            }
        })
    }
}

/// `v1=` and the hex HMAC-SHA256 of `{timestamp}.{body}`: a receiver recomputes it with its secret and rejects an old timestamp.
pub(crate) fn sign(secret: &str, timestamp: &str, body: &str) -> String {
    let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
    format!(
        "v1={}",
        hex::encode(hmac::sign(&key, format!("{timestamp}.{body}").as_bytes()).as_ref())
    )
}

#[derive(Debug, Serialize)]
pub(crate) struct EndpointsResponse {
    endpoints: Vec<OutboundEndpoint>,
    /// The events an endpoint may subscribe to.
    events: Vec<&'static str>,
    /// Whether the operator has named any host; without one nothing can be added or sent.
    available: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct EndpointCreated {
    endpoint: OutboundEndpoint,
    /// Shown once: it signs every delivery.
    secret: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EndpointRequest {
    name: String,
    url: String,
    events: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EndpointUpdate {
    name: Option<String>,
    events: Option<Vec<String>>,
    enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DeliveryQuery {
    limit: Option<i64>,
}

fn check_name(name: &str) -> Result<String, ApiError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME_CHARS {
        return Err(ApiError::bad_request(format!(
            "Name the endpoint in up to {MAX_NAME_CHARS} characters."
        )));
    }
    Ok(name.to_owned())
}

fn check_events(events: &[String]) -> Result<Vec<String>, ApiError> {
    let mut chosen = Vec::new();
    for event in events {
        if !OUTBOUND_EVENTS.contains(&event.as_str()) {
            return Err(ApiError::bad_request(format!(
                "{event} is not an event. Choose from: {}.",
                OUTBOUND_EVENTS.join(", ")
            )));
        }
        if !chosen.contains(event) {
            chosen.push(event.clone());
        }
    }
    if chosen.is_empty() {
        return Err(ApiError::bad_request("Choose at least one event."));
    }
    Ok(chosen)
}

fn map_error(error: anyhow::Error) -> ApiError {
    crate::map_storage_error(error)
}

async fn endpoint_of(state: &AppState, id: &str) -> Result<OutboundEndpoint, ApiError> {
    state.core.storage().outbound_endpoint(id).await.map_err(map_error)
}

pub(crate) async fn list_endpoints(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<EndpointsResponse>, ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    let endpoints = state
        .core
        .storage()
        .list_outbound_endpoints(&workspace_id)
        .await
        .map_err(map_error)?;
    Ok(Json(EndpointsResponse {
        endpoints,
        events: OUTBOUND_EVENTS.to_vec(),
        available: state.outbound.policy.is_enabled(),
    }))
}

pub(crate) async fn create_endpoint(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<EndpointRequest>,
) -> Result<(StatusCode, Json<EndpointCreated>), ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    let name = check_name(&request.name)?;
    let events = check_events(&request.events)?;
    let url = state.outbound.check_destination(&request.url)?;
    let (endpoint, secret) = state
        .core
        .storage()
        .create_outbound_endpoint(&workspace_id, &name, &url, &events, &subject.user_id)
        .await
        .map_err(map_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "outbound_endpoint_created",
        "outbound_endpoint",
        Some(&endpoint.id),
        format!("Approved {} to receive {} events.", endpoint.name, events.len()),
    )
    .await;
    Ok((StatusCode::CREATED, Json(EndpointCreated { endpoint, secret })))
}

pub(crate) async fn update_endpoint(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(endpoint_id): Path<String>,
    Json(request): Json<EndpointUpdate>,
) -> Result<Json<OutboundEndpoint>, ApiError> {
    let current = endpoint_of(&state, &endpoint_id).await?;
    require_workspace_admin(&state, &subject, &current.workspace_id).await?;
    let name = request.name.as_deref().map(check_name).transpose()?;
    let events = request.events.as_deref().map(check_events).transpose()?;
    let updated = state
        .core
        .storage()
        .update_outbound_endpoint(&endpoint_id, name.as_deref(), events.as_deref(), request.enabled)
        .await
        .map_err(map_error)?;
    if request.enabled.is_some() && request.enabled != Some(current.enabled) {
        record_workspace_activity(
            &state,
            &current.workspace_id,
            &subject.user_id,
            "outbound_endpoint_toggled",
            "outbound_endpoint",
            Some(&endpoint_id),
            format!(
                "Turned {} {}.",
                updated.name,
                if updated.enabled { "on" } else { "off" }
            ),
        )
        .await;
    }
    Ok(Json(updated))
}

pub(crate) async fn delete_endpoint(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(endpoint_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let current = endpoint_of(&state, &endpoint_id).await?;
    require_workspace_admin(&state, &subject, &current.workspace_id).await?;
    state
        .core
        .storage()
        .delete_outbound_endpoint(&endpoint_id)
        .await
        .map_err(map_error)?;
    record_workspace_activity(
        &state,
        &current.workspace_id,
        &subject.user_id,
        "outbound_endpoint_removed",
        "outbound_endpoint",
        Some(&endpoint_id),
        format!("Removed {}.", current.name),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize)]
pub(crate) struct SecretResponse {
    secret: String,
}

pub(crate) async fn rotate_secret(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(endpoint_id): Path<String>,
) -> Result<Json<SecretResponse>, ApiError> {
    let current = endpoint_of(&state, &endpoint_id).await?;
    require_workspace_admin(&state, &subject, &current.workspace_id).await?;
    let secret = state
        .core
        .storage()
        .rotate_outbound_secret(&endpoint_id)
        .await
        .map_err(map_error)?;
    Ok(Json(SecretResponse { secret }))
}

pub(crate) async fn list_deliveries(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(endpoint_id): Path<String>,
    Query(query): Query<DeliveryQuery>,
) -> Result<Json<Vec<OutboundDelivery>>, ApiError> {
    let current = endpoint_of(&state, &endpoint_id).await?;
    require_workspace_admin(&state, &subject, &current.workspace_id).await?;
    state
        .core
        .storage()
        .list_outbound_deliveries(&endpoint_id, query.limit.unwrap_or(50))
        .await
        .map(Json)
        .map_err(map_error)
}

#[derive(Debug, Serialize)]
pub(crate) struct TestQueued {
    event_id: String,
}

/// Queues a `ping` for one endpoint so its owner can see a delivery arrive.
pub(crate) async fn test_endpoint(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(endpoint_id): Path<String>,
) -> Result<(StatusCode, Json<TestQueued>), ApiError> {
    let current = endpoint_of(&state, &endpoint_id).await?;
    require_workspace_admin(&state, &subject, &current.workspace_id).await?;
    if !current.enabled {
        return Err(ApiError::conflict("Turn the endpoint on before sending it a test."));
    }
    let event_id = state
        .core
        .storage()
        .enqueue_outbound_ping(&endpoint_id)
        .await
        .map_err(map_error)?;
    state.outbound.wake.notify_one();
    Ok((StatusCode::ACCEPTED, Json(TestQueued { event_id })))
}

/// Queues an event for the workspace's endpoints. A failure is logged and never stops the work that caused it.
pub(crate) async fn emit(core: &RepoMemoCore, workspace_id: &str, event_type: &str, data: serde_json::Value) {
    if let Err(error) = core
        .storage()
        .enqueue_outbound_event(workspace_id, event_type, &data)
        .await
    {
        tracing::warn!(event = event_type, error = %error, "An outbound event could not be queued");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_signature_covers_the_time_and_the_body() {
        let a = sign("whsec_x", "1700000000", "{\"a\":1}");
        assert!(a.starts_with("v1="));
        assert_eq!(a, sign("whsec_x", "1700000000", "{\"a\":1}"));
        assert_ne!(a, sign("whsec_x", "1700000001", "{\"a\":1}"));
        assert_ne!(a, sign("whsec_x", "1700000000", "{\"a\":2}"));
        assert_ne!(a, sign("whsec_y", "1700000000", "{\"a\":1}"));
    }

    #[test]
    fn events_are_checked_and_deduplicated() {
        assert!(check_events(&[]).is_err());
        assert!(check_events(&["nope".to_owned()]).is_err());
        let chosen = check_events(&["ping".to_owned(), "ping".to_owned()]).unwrap();
        assert_eq!(chosen, vec!["ping".to_owned()]);
    }
}
