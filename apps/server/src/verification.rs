//! Email verification (roadmap F13, decision 9).
//!
//! Registration stays closed by default. When an operator opens it and mail
//! is set up, an account made without an invitation is *unverified*: it can
//! sign in and look at what it was given, but it cannot create an
//! organization or a workspace until it has used a link sent to its address.
//! An invitation counts as verification (it was sent to that address), and so
//! does the first account of a server. A changed address is unverified again.
//! Without mail nobody could ever verify, so nothing is withheld then.

use crate::*;

const VERIFICATION_TTL_HOURS: i64 = 24;
/// Links asked for per account per hour.
const RESENDS_PER_HOUR: u32 = 3;

/// Unverified accounts are held back: registration is open and mail can be sent.
pub(crate) async fn enforced(state: &AppState) -> bool {
    matches!(crate::auth::registration_open(state).await, Ok(true)) && state.mail.is_ready().await
}

/// Refuses an unverified account the things a verified one may do.
pub(crate) async fn require_verified(state: &AppState, user_id: &str) -> Result<(), ApiError> {
    if state
        .storage
        .email_is_verified(user_id)
        .await
        .map_err(map_storage_error)?
        || !enforced(state).await
    {
        return Ok(());
    }
    Err(ApiError::forbidden_because(
        "Verify your email address first: a link was sent to it. You can ask for another in your profile.",
    ))
}

/// Mails the verification link of an account's current address.
pub(crate) async fn send_link(state: &AppState, user: &repomemo_domain::SharedUser) -> bool {
    let Some(address) = user.email.as_deref() else {
        return false;
    };
    let Ok((token, _)) = state
        .storage
        .create_email_verification(&user.id, address, VERIFICATION_TTL_HOURS)
        .await
    else {
        return false;
    };
    let Some(link) = state.mail.link(&format!("/verify-email#{token}")) else {
        return false;
    };
    let body = format!(
        "Open this link to confirm that this is your email address on RepoMemo. It works once and for 24 hours:\
\
{link}\
\
If you did not create an account, ignore this message.\
"
    );
    state
        .mail
        .send_to(
            address,
            "Confirm your RepoMemo email address",
            &body,
            "email_verification",
        )
        .await
        .is_ok()
}

#[derive(serde::Deserialize)]
pub(crate) struct VerifyRequest {
    token: String,
}

#[derive(serde::Serialize)]
pub(crate) struct VerifiedAnswer {
    email: String,
}

/// Public: uses a verification link. The same refusal for an unknown, used,
/// expired or outdated link (the address was changed since).
pub(crate) async fn verify_email(
    State(state): State<AppState>,
    client: ClientIp,
    Json(request): Json<VerifyRequest>,
) -> Result<Json<VerifiedAnswer>, ApiError> {
    check_auth_quota(&state, &client, AuthAction::SignIn)?;
    let verified = state
        .storage
        .consume_email_verification(&request.token)
        .await
        .map_err(map_storage_error)?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::GONE,
                "gone",
                "This link cannot be used: it was used already, has expired or belongs to an address that was changed.",
            )
        })?;
    tracing::info!(target: "audit", user_id = %verified.0, "Email address verified");
    Ok(Json(VerifiedAnswer { email: verified.1 }))
}

#[derive(serde::Serialize)]
pub(crate) struct ResendAnswer {
    /// The address already is verified: nothing was sent.
    verified: bool,
    /// A message was queued.
    sent: bool,
}

/// Sends the account's verification link again (three an hour).
pub(crate) async fn resend(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<ResendAnswer>, ApiError> {
    if state
        .storage
        .email_is_verified(&subject.user_id)
        .await
        .map_err(map_storage_error)?
    {
        return Ok(Json(ResendAnswer {
            verified: true,
            sent: false,
        }));
    }
    state
        .guards
        .auth
        .check(
            &format!("verify:{}", subject.user_id),
            Quota::per_hour(RESENDS_PER_HOUR),
        )
        .map_err(|wait| {
            crate::security::too_many_requests(
                format!(
                    "A link was sent a few times already. Try again in {}.",
                    crate::security::wait_text(wait)
                ),
                wait,
            )
        })?;
    let user = state
        .storage
        .find_user(&subject.user_id)
        .await
        .map_err(map_storage_error)?
        .ok_or_else(ApiError::unauthorized)?;
    let sent = send_link(&state, &user).await;
    Ok(Json(ResendAnswer { verified: false, sent }))
}

#[derive(serde::Deserialize)]
pub(crate) struct ChangeEmailRequest {
    email: String,
    current_password: String,
}

/// Changes the account's address. The password is asked again, the new
/// address starts unverified and gets a link; tokens and sessions stay.
pub(crate) async fn change_email(
    SessionSubject(subject): SessionSubject,
    State(state): State<AppState>,
    Json(request): Json<ChangeEmailRequest>,
) -> Result<Json<repomemo_domain::SharedUser>, ApiError> {
    let email = request.email.trim().to_lowercase();
    if !repomemo_mail::valid_address(&email) {
        return Err(ApiError::bad_request("That is not a valid email address."));
    }
    let current = state
        .storage
        .find_user(&subject.user_id)
        .await
        .map_err(map_storage_error)?
        .ok_or_else(ApiError::unauthorized)?;
    let stored = state
        .storage
        .find_user_for_auth(current.email.as_deref().unwrap_or_default())
        .await
        .map_err(map_storage_error)?
        .ok_or_else(ApiError::unauthorized)?;
    if !crate::auth::verify_password(&request.current_password, &stored.password_hash).await {
        return Err(ApiError::forbidden_because("The current password is not right."));
    }
    let user = state
        .storage
        .change_user_email(&subject.user_id, &email)
        .await
        .map_err(map_storage_error)?;
    tracing::info!(target: "audit", user_id = %user.id, "Email address changed");
    send_link(&state, &user).await;
    Ok(Json(user))
}
