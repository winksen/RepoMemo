//! Chat delivery (V0.8.9): digests sent to a Slack channel an administrator approved.
//!
//! A digest is a **count and a link**, never a title or an excerpt: "3 items are due for review" and the address of the
//! signed-in app, so someone who is not a member of the workspace sees a number and a login page, not content. Each digest
//! kind goes to a destination at most once a day: a ledger takes the right to send before the call is made, and gives it back
//! when the call fails so the next pass retries without ever sending twice. The destination's address is a secret (a Slack
//! incoming webhook): it is sealed, never returned, and the host must be on the operator's list (`REPOMEMO_OUTBOUND_HOSTS`).
//! Turning a destination off or removing it stops the next pass at once.

use std::time::Duration;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use repomemo_domain::ChatDestination;
use repomemo_storage::{NewChatDestination, CHAT_DIGESTS};
use serde::{Deserialize, Serialize};

use crate::{record_workspace_activity, require_workspace_admin, ApiError, AppState, ManagingSubject};

const SEND_TIMEOUT: Duration = Duration::from_secs(10);

fn map_error(error: anyhow::Error) -> ApiError {
    crate::map_storage_error(error)
}

/// A Slack incoming-webhook address on an approved host (`http` to this machine is accepted for a test receiver).
fn check_slack_url(state: &AppState, raw: &str) -> Result<(String, String), ApiError> {
    let url = state.outbound.check_destination(raw)?;
    let parsed = reqwest::Url::parse(&url).map_err(|_| ApiError::bad_request("That is not a valid address."))?;
    let host = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
    let local = matches!(host.as_str(), "localhost" | "127.0.0.1" | "[::1]");
    if !local && (host != "hooks.slack.com" || !parsed.path().starts_with("/services/")) {
        return Err(ApiError::bad_request(
            "A Slack destination is an incoming webhook address: https://hooks.slack.com/services/...",
        ));
    }
    Ok((url, host))
}

fn check_digests(digests: &[String]) -> Result<Vec<String>, ApiError> {
    let mut chosen = Vec::new();
    for digest in digests {
        if !CHAT_DIGESTS.contains(&digest.as_str()) {
            return Err(ApiError::bad_request(format!(
                "{digest} is not a digest. Choose from: {}.",
                CHAT_DIGESTS.join(", ")
            )));
        }
        if !chosen.contains(digest) {
            chosen.push(digest.clone());
        }
    }
    if chosen.is_empty() {
        return Err(ApiError::bad_request("Choose at least one digest."));
    }
    Ok(chosen)
}

/// The words of a digest: a count and where to see it. No title, no excerpt, no name.
pub(crate) fn digest_text(kind: &str, count: i64, link: &str) -> String {
    let what = match kind {
        "review" => format!(
            "{count} {} due for review",
            if count == 1 { "item is" } else { "items are" }
        ),
        _ => format!(
            "{count} open {} overdue",
            if count == 1 { "task is" } else { "tasks are" }
        ),
    };
    format!("RepoMemo: {what}. Sign in to see them: {link}")
}

async fn post(state: &AppState, destination: &ChatDestination, text: &str) -> Result<(), String> {
    let Some(url) = state.storage.open_chat_url(&destination.id).await.ok().flatten() else {
        return Err("The destination's address could not be opened.".to_owned());
    };
    // The operator may have taken the host off the list since the destination was made.
    if state.outbound.check_destination(&url).is_err() {
        return Err("The address is no longer an approved destination.".to_owned());
    }
    crate::outbound::check_resolved(&url).await?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(SEND_TIMEOUT)
        .user_agent("RepoMemo-Chat")
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .post(&url)
        .json(&serde_json::json!({ "text": text }))
        .send()
        .await
        // The text can name the host, never the secret path.
        .map_err(|error| format!("The destination could not be reached: {}", error.without_url()))?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!("The destination answered {}.", response.status().as_u16()))
    }
}

/// Sends the digests a destination is owed in this window. Returns the kinds that were sent.
pub(crate) async fn deliver_digests(state: &AppState, destination: &ChatDestination) -> Vec<String> {
    let mut sent = Vec::new();
    let Some(link) = state
        .mail
        .link(&format!("/workspaces/{}/review", destination.workspace_id))
    else {
        let _ = state
            .storage
            .record_chat_result(
                &destination.id,
                Some("Set REPOMEMO_PUBLIC_URL so a digest can carry a link."),
            )
            .await;
        return sent;
    };
    let window = chrono::Utc::now().format("%Y-%m-%d").to_string();
    for kind in &destination.digests {
        let count = match kind.as_str() {
            "review" => state.storage.count_due_reviews(&destination.workspace_id).await,
            _ => state.storage.count_overdue_tasks(&destination.workspace_id).await,
        };
        let Ok(count) = count else { continue };
        if count == 0 {
            continue;
        }
        // The right to send is taken first; a failed call gives it back.
        if !state
            .storage
            .claim_chat_digest(&destination.id, kind, &window)
            .await
            .unwrap_or(false)
        {
            continue;
        }
        match post(state, destination, &digest_text(kind, count, &link)).await {
            Ok(()) => {
                let _ = state.storage.record_chat_result(&destination.id, None).await;
                sent.push(kind.clone());
            }
            Err(error) => {
                let _ = state.storage.release_chat_digest(&destination.id, kind, &window).await;
                let _ = state.storage.record_chat_result(&destination.id, Some(&error)).await;
            }
        }
    }
    sent
}

/// One maintenance pass: every destination that is on gets what it is owed.
pub(crate) async fn send_due(state: &AppState) -> anyhow::Result<usize> {
    let mut sent = 0;
    for destination in state.storage.list_enabled_chat_destinations().await? {
        sent += deliver_digests(state, &destination).await.len();
    }
    Ok(sent)
}

#[derive(Debug, Serialize)]
pub(crate) struct DestinationsResponse {
    destinations: Vec<ChatDestination>,
    digests: Vec<&'static str>,
    /// Whether the operator has named any host.
    available: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DestinationRequest {
    name: String,
    /// The Slack incoming webhook address.
    url: String,
    digests: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DestinationUpdate {
    name: Option<String>,
    digests: Option<Vec<String>>,
    enabled: Option<bool>,
}

fn check_name(name: &str) -> Result<String, ApiError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(ApiError::bad_request("Name the destination in up to 80 characters."));
    }
    Ok(name.to_owned())
}

async fn destination_of(state: &AppState, id: &str) -> Result<ChatDestination, ApiError> {
    state.storage.chat_destination(id).await.map_err(map_error)
}

pub(crate) async fn list_destinations(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<DestinationsResponse>, ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    Ok(Json(DestinationsResponse {
        destinations: state
            .storage
            .list_chat_destinations(&workspace_id)
            .await
            .map_err(map_error)?,
        digests: CHAT_DIGESTS.to_vec(),
        available: state.outbound.is_available(),
    }))
}

pub(crate) async fn create_destination(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<DestinationRequest>,
) -> Result<(StatusCode, Json<ChatDestination>), ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    let name = check_name(&request.name)?;
    let digests = check_digests(&request.digests)?;
    let (url, host) = check_slack_url(&state, &request.url)?;
    let destination = state
        .storage
        .create_chat_destination(
            &workspace_id,
            NewChatDestination {
                provider: "slack",
                name: &name,
                url: &url,
                host: &host,
                digests: &digests,
            },
            &subject.user_id,
        )
        .await
        .map_err(map_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "chat_destination_created",
        "chat_destination",
        Some(&destination.id),
        format!("Approved {} to receive digests.", destination.name),
    )
    .await;
    Ok((StatusCode::CREATED, Json(destination)))
}

pub(crate) async fn update_destination(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<DestinationUpdate>,
) -> Result<Json<ChatDestination>, ApiError> {
    let current = destination_of(&state, &id).await?;
    require_workspace_admin(&state, &subject, &current.workspace_id).await?;
    let name = request.name.as_deref().map(check_name).transpose()?;
    let digests = request.digests.as_deref().map(check_digests).transpose()?;
    state
        .storage
        .update_chat_destination(&id, name.as_deref(), digests.as_deref(), request.enabled)
        .await
        .map(Json)
        .map_err(map_error)
}

pub(crate) async fn delete_destination(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let current = destination_of(&state, &id).await?;
    require_workspace_admin(&state, &subject, &current.workspace_id).await?;
    state.storage.delete_chat_destination(&id).await.map_err(map_error)?;
    record_workspace_activity(
        &state,
        &current.workspace_id,
        &subject.user_id,
        "chat_destination_removed",
        "chat_destination",
        Some(&id),
        format!("Disconnected {}.", current.name),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize)]
pub(crate) struct TestResult {
    delivered: bool,
    error: Option<String>,
}

/// Sends a short test message (no workspace content) so an administrator can see it arrive.
pub(crate) async fn test_destination(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<TestResult>, ApiError> {
    let destination = destination_of(&state, &id).await?;
    require_workspace_admin(&state, &subject, &destination.workspace_id).await?;
    let result = post(
        &state,
        &destination,
        "RepoMemo: this is a test. Digests will arrive here.",
    )
    .await;
    Ok(Json(TestResult {
        delivered: result.is_ok(),
        error: result.err(),
    }))
}

#[derive(Debug, Serialize)]
pub(crate) struct DigestResult {
    /// The digests that were sent now; one already sent today, or with nothing to report, is not sent again.
    sent: Vec<String>,
}

/// Runs the destination's digests now, under the same once-a-day ledger as the scheduled pass.
pub(crate) async fn send_digest_now(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DigestResult>, ApiError> {
    let destination = destination_of(&state, &id).await?;
    require_workspace_admin(&state, &subject, &destination.workspace_id).await?;
    if !destination.enabled {
        return Err(ApiError::conflict(
            "Turn the destination on before sending it a digest.",
        ));
    }
    Ok(Json(DigestResult {
        sent: deliver_digests(&state, &destination).await,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_digest_is_a_count_and_a_link() {
        let text = digest_text("review", 3, "https://repomemo.example/workspaces/w/review");
        assert_eq!(
            text,
            "RepoMemo: 3 items are due for review. Sign in to see them: https://repomemo.example/workspaces/w/review"
        );
        assert!(digest_text("projects", 1, "x").contains("1 open task is overdue"));
    }

    #[test]
    fn digests_are_checked() {
        assert!(check_digests(&[]).is_err());
        assert!(check_digests(&["titles".to_owned()]).is_err());
        assert_eq!(
            check_digests(&["review".to_owned(), "review".to_owned()]).unwrap(),
            vec!["review".to_owned()]
        );
    }
}
