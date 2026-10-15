//! Push webhooks for repository links (roadmap F16a, V0.2.23).
//!
//! A repository has one webhook secret, made by a workspace administrator and
//! shown once. Hosts call `POST /v1/webhooks/repositories/{id}` when someone
//! pushes, and a correctly signed call queues one sync (a sync that is already
//! running absorbs it), so a workspace follows its repository within seconds
//! instead of at the next poll. Three senders are understood:
//!
//! - **GitHub**: `X-Hub-Signature-256: sha256=<hex>`, an HMAC-SHA256 of the
//!   body, and `X-GitHub-Event` (`ping` answers, `push` syncs);
//! - **GitLab**: `X-Gitlab-Token`, the secret itself (GitLab sends no
//!   signature), and `X-Gitlab-Event: Push Hook`;
//! - **anything else**: `X-RepoMemo-Signature: sha256=<hex>`, the same HMAC.
//!
//! Every comparison is constant time. A call that cannot be verified is
//! answered `401` the same way whether or not the repository exists, and
//! failures are limited per client address so the endpoint cannot be used to
//! guess or to flood.

use axum::{body::Bytes, http::HeaderMap};
use ring::hmac;
use serde_json::Value;

use crate::*;

/// The largest body accepted: push payloads are a few KiB, and a sync never
/// needs more than the branch name from them.
const MAX_BODY_BYTES: usize = 1024 * 1024;
/// Failed calls per client address per minute.
const FAILURES_PER_MINUTE: u32 = 20;
/// Verified calls per repository per minute.
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
        Sender::GitLab => {
            same_secret(header(headers, "x-gitlab-token").as_bytes(), secret.as_bytes())
        }
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

/// Public. The same `401` for an unknown repository, one without a webhook
/// and a call that is not signed right.
pub(crate) async fn receive_repository_webhook(
    State(state): State<AppState>,
    client: ClientIp,
    Path(source_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let fail = |state: &AppState| -> ApiError {
        // Only failures are counted against the address, so a host that
        // pushes a lot is never slowed by its own successes.
        match state
            .guards
            .auth
            .check(&format!("webhook-fail:{}", client.0), Quota::per_minute(FAILURES_PER_MINUTE))
        {
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
    let Some(sender) = sender_of(&headers) else {
        return Err(fail(&state));
    };
    let repository = state.core.repo_source(&source_id).await.ok();
    let secret = match &repository {
        Some(_) => state
            .storage
            .open_repo_webhook_secret(&source_id)
            .await
            .map_err(ApiError::internal)?,
        None => None,
    };
    // Verify against *something* even when there is no secret to compare, so
    // the time taken does not tell the cases apart.
    let candidate = secret.clone().unwrap_or_else(|| "-".repeat(64));
    let signed = verified(&sender, &headers, &body, &candidate);
    let (Some(repository), Some(_), true) = (repository, secret, signed) else {
        return Err(fail(&state));
    };

    state
        .guards
        .auth
        .check(&format!("webhook:{source_id}"), Quota::per_minute(CALLS_PER_MINUTE))
        .map_err(|wait| {
            crate::security::too_many_requests(
                format!("This repository gets webhook calls too fast. Try again in {}.", crate::security::wait_text(wait)),
                wait,
            )
        })?;
    if let Err(error) = state.storage.touch_repo_webhook(&source_id).await {
        tracing::warn!(error = %format!("{error:#}"), "A webhook's last call could not be noted");
    }
    let answer = |status: StatusCode, queued: bool, note: &str| {
        (status, Json(json!({"queued": queued, "note": note})))
    };
    match wish_of(&sender, &headers, &body, repository.settings.branch.as_deref()) {
        Wish::Pong => Ok(answer(StatusCode::OK, false, "pong")),
        Wish::Ignore(reason) => Ok(answer(StatusCode::ACCEPTED, false, reason)),
        Wish::Sync => {
            tracing::info!(target: "audit", repository = %source_id, "A push webhook asked for a sync");
            match state.repo_sync.start(&repository, None).await {
                Ok(Some(_)) => Ok(answer(StatusCode::ACCEPTED, true, "sync queued")),
                Ok(None) => Ok(answer(StatusCode::ACCEPTED, false, "already syncing")),
                // No room on the disk or in the allowance: the sender is told
                // so, and the next poll tries again.
                Err(error) => Ok(answer(StatusCode::ACCEPTED, false, &format!("{error:#}"))),
            }
        }
    }
}

#[derive(serde::Serialize)]
pub(crate) struct WebhookCreated {
    /// Shown once: the secret the sender signs with.
    secret: String,
    /// The path to call, and the full address when the server knows its own.
    path: String,
    url: Option<String>,
}

/// Makes or replaces a repository's webhook secret (workspace administrators).
pub(crate) async fn create_repository_webhook(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(source_id): Path<String>,
) -> Result<(StatusCode, Json<WebhookCreated>), ApiError> {
    let repository = state
        .core
        .repo_source(&source_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_admin(&state, &subject, &repository.workspace_id).await?;
    let secret = state
        .storage
        .create_repo_webhook(&source_id, Some(&subject.user_id))
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &repository.workspace_id,
        &subject.user_id,
        "repository_webhook_created",
        "repository",
        Some(&source_id),
        format!("Set up push webhooks for repository {}.", repository.name),
    )
    .await;
    let path = format!("/v1/webhooks/repositories/{source_id}");
    Ok((
        StatusCode::CREATED,
        Json(WebhookCreated {
            url: state.mail.link(&path),
            path,
            secret,
        }),
    ))
}

pub(crate) async fn delete_repository_webhook(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(source_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let repository = state
        .core
        .repo_source(&source_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_admin(&state, &subject, &repository.workspace_id).await?;
    if !state
        .storage
        .clear_repo_webhook(&source_id)
        .await
        .map_err(map_storage_error)?
    {
        return Err(ApiError::not_found("This repository has no webhook."));
    }
    record_workspace_activity(
        &state,
        &repository.workspace_id,
        &subject.user_id,
        "repository_webhook_removed",
        "repository",
        Some(&source_id),
        format!("Removed the push webhooks of repository {}.", repository.name),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
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
        assert!(!verified(&Sender::GitHub, &github, b"{}", secret), "the body is part of it");
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
        assert_eq!(wish_of(&Sender::GitHub, &github("push"), main, Some("main")), Wish::Sync);
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
}
