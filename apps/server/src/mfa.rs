//! Optional two-step sign-in for local password accounts (V0.5.16).
//!
//! A person turns it on by scanning a secret into an authenticator app and proving it with a first code; they are then
//! given ten recovery codes, shown once and kept only as hashes. From then on a correct password alone never opens a
//! session: sign-in, and the sign-in a password reset ends with, need a current time-based code or an unused recovery
//! code. A time step is accepted once, so a code that was seen cannot be replayed, and a recovery code works once.
//! Wrong codes count as failed sign-ins and lock the account like wrong passwords do.
//! Sign-in through the identity provider is not covered: the provider is responsible for its own second factor.
use crate::*;
use ring::hmac;
use ring::rand::{SecureRandom, SystemRandom};
use serde_json::{json, Value};

const STEP_SECONDS: i64 = 30;
const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
const RECOVERY_CODES: usize = 10;

fn base32(bytes: &[u8]) -> String {
    let mut out = String::new();
    let (mut buffer, mut bits) = (0u32, 0u32);
    for byte in bytes {
        buffer = (buffer << 8) | u32::from(*byte);
        bits += 8;
        while bits >= 5 {
            out.push(char::from(ALPHABET[((buffer >> (bits - 5)) & 31) as usize]));
            bits -= 5;
        }
    }
    if bits > 0 {
        out.push(char::from(ALPHABET[((buffer << (5 - bits)) & 31) as usize]));
    }
    out
}

pub(crate) fn base32_decode(text: &str) -> Option<Vec<u8>> {
    let (mut buffer, mut bits) = (0u32, 0u32);
    let mut out = Vec::new();
    for c in text.bytes().filter(|c| *c != b'=' && *c != b' ') {
        let value = ALPHABET.iter().position(|a| *a == c.to_ascii_uppercase())? as u32;
        buffer = (buffer << 5) | value;
        bits += 5;
        if bits >= 8 {
            out.push((buffer >> (bits - 8)) as u8);
            bits -= 8;
        }
    }
    Some(out)
}

/// RFC 6238 with SHA-1, six digits: what every authenticator app computes.
pub(crate) fn totp(secret: &[u8], step: i64) -> u32 {
    let key = hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, secret);
    let tag = hmac::sign(&key, &(step as u64).to_be_bytes());
    let digest = tag.as_ref();
    let offset = (digest[digest.len() - 1] & 0x0f) as usize;
    let value = u32::from_be_bytes([
        digest[offset] & 0x7f,
        digest[offset + 1],
        digest[offset + 2],
        digest[offset + 3],
    ]);
    value % 1_000_000
}

pub(crate) fn current_step() -> i64 {
    Utc::now().timestamp() / STEP_SECONDS
}

/// Equal without stopping at the first difference.
fn same_bytes(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// The newest step within a step either side of now whose code is `code` and which has not been used.
fn matching_step(secret: &[u8], code: &str, now_step: i64, last_step: i64) -> Option<i64> {
    let candidate = format!("{:06}", code.parse::<u32>().ok()?);
    if code.len() != 6 {
        return None;
    }
    [now_step + 1, now_step, now_step - 1].into_iter().find(|step| {
        *step > last_step && same_bytes(format!("{:06}", totp(secret, *step)).as_bytes(), candidate.as_bytes())
    })
}

fn new_secret() -> Result<String, ApiError> {
    let mut bytes = [0u8; 20];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| ApiError::internal("Random generation failed"))?;
    Ok(base32(&bytes))
}

fn new_recovery_codes() -> Result<Vec<String>, ApiError> {
    let rng = SystemRandom::new();
    (0..RECOVERY_CODES)
        .map(|_| {
            let mut bytes = [0u8; 10];
            rng.fill(&mut bytes)
                .map_err(|_| ApiError::internal("Random generation failed"))?;
            let text = base32(&bytes);
            Ok(format!("{}-{}", &text[..8], &text[8..16]))
        })
        .collect()
}

async fn record(state: &AppState, user_id: &str, action: &str, detail: &str) {
    if let Err(error) = state.storage.record_system_event(Some(user_id), action, detail).await {
        tracing::error!(error = %error, "Failed to record a system audit event");
    }
}

fn required() -> ApiError {
    ApiError::new(
        StatusCode::FORBIDDEN,
        "mfa_required",
        "Enter the code from your authenticator app, or one of your recovery codes.",
    )
}

pub(crate) fn wrong_code() -> ApiError {
    ApiError::new(
        StatusCode::UNAUTHORIZED,
        "invalid_mfa_code",
        "That code is not right, or it was already used. Wait for the next code, or use a recovery code.",
    )
}

/// Whether the second factor is satisfied: `Ok(true)` when the account has none or the code is right (and now used),
/// `Ok(false)` for a wrong or reused code, and the `mfa_required` error when a code is needed and none was sent.
pub(crate) async fn check_second_factor(state: &AppState, user_id: &str, code: Option<&str>) -> Result<bool, ApiError> {
    let Some((secret, enabled, last_step)) = state.storage.mfa_secret(user_id).await.map_err(ApiError::internal)?
    else {
        return Ok(true);
    };
    if !enabled {
        return Ok(true);
    }
    let Some(code) = code.map(str::trim).filter(|c| !c.is_empty() && c.len() <= 32) else {
        return Err(required());
    };
    if code.len() == 6 && code.bytes().all(|c| c.is_ascii_digit()) {
        let Some(bytes) = base32_decode(&secret) else {
            return Ok(false);
        };
        return match matching_step(&bytes, code, current_step(), last_step) {
            Some(step) => state
                .storage
                .accept_mfa_step(user_id, step)
                .await
                .map_err(ApiError::internal),
            None => Ok(false),
        };
    }
    state
        .storage
        .use_recovery_code(user_id, &repomemo_storage::recovery_code_hash(code))
        .await
        .map_err(ApiError::internal)
}

/// Counts a wrong code like a wrong password, so six digits cannot be guessed freely.
fn throttle(state: &AppState, user_id: &str) -> Result<String, ApiError> {
    let key = format!("mfa:{user_id}");
    if let Some(wait) = state.guards.logins.locked_for(&key) {
        return Err(too_many_requests(
            format!("Too many wrong codes. Try again in {}.", wait_text(wait)),
            wait,
        ));
    }
    Ok(key)
}

fn miss(state: &AppState, key: &str) {
    let settings = state.settings();
    state
        .guards
        .logins
        .record_failure(key, settings.login_max_failures, settings.login_lockout);
}

pub(crate) async fn status(
    SessionSubject(subject): SessionSubject,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let (enabled, remaining) = state
        .storage
        .mfa_status(&subject.user_id)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(json!({"enabled":enabled,"recovery_codes_remaining":remaining})))
}

/// Starts an enrollment: a new secret, shown once as text and as the address an authenticator app scans.
pub(crate) async fn start(
    SessionSubject(subject): SessionSubject,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let user = state
        .storage
        .find_user(&subject.user_id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(ApiError::unauthorized)?;
    let secret = new_secret()?;
    state
        .storage
        .begin_mfa(&user.id, &secret)
        .await
        .map_err(map_storage_error)?;
    let account = user.email.unwrap_or(user.display_name);
    let issuer = "RepoMemo";
    let uri = format!(
        "otpauth://totp/{issuer}:{}?secret={secret}&issuer={issuer}&algorithm=SHA1&digits=6&period=30",
        url::form_urlencoded::byte_serialize(account.as_bytes()).collect::<String>()
    );
    Ok(Json(json!({"secret":secret,"otpauth_url":uri})))
}

#[derive(Deserialize)]
pub(crate) struct CodeRequest {
    code: String,
}

/// Turns it on after the first valid code and returns the recovery codes, this once.
pub(crate) async fn confirm(
    SessionSubject(subject): SessionSubject,
    State(state): State<AppState>,
    Json(request): Json<CodeRequest>,
) -> Result<Json<Value>, ApiError> {
    let key = throttle(&state, &subject.user_id)?;
    let Some((secret, enabled, last_step)) = state
        .storage
        .mfa_secret(&subject.user_id)
        .await
        .map_err(ApiError::internal)?
    else {
        return Err(ApiError::bad_request("Start setting up two-step sign-in first."));
    };
    if enabled {
        return Err(ApiError::conflict("Two-step sign-in is already on."));
    }
    let step =
        base32_decode(&secret).and_then(|bytes| matching_step(&bytes, request.code.trim(), current_step(), last_step));
    let Some(step) = step else {
        miss(&state, &key);
        return Err(wrong_code());
    };
    let codes = new_recovery_codes()?;
    let hashes: Vec<String> = codes.iter().map(|c| repomemo_storage::recovery_code_hash(c)).collect();
    if !state
        .storage
        .enable_mfa(&subject.user_id, step, &hashes)
        .await
        .map_err(ApiError::internal)?
    {
        return Err(wrong_code());
    }
    state.guards.logins.clear(&key);
    record(
        &state,
        &subject.user_id,
        "mfa_enabled",
        "Two-step sign-in was turned on",
    )
    .await;
    Ok(Json(json!({"recovery_codes":codes})))
}

#[derive(Deserialize)]
pub(crate) struct ProofRequest {
    password: String,
    code: String,
}

/// Password and a second factor together: what changing the second factor itself requires.
async fn prove(state: &AppState, user_id: &str, request: &ProofRequest) -> Result<(), ApiError> {
    let key = throttle(state, user_id)?;
    let user = state
        .storage
        .find_user(user_id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(ApiError::unauthorized)?;
    let account = state
        .storage
        .find_user_for_auth(user.email.as_deref().unwrap_or_default())
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(ApiError::unauthorized)?;
    if !auth::verify_password(&request.password, &account.password_hash).await? {
        miss(state, &key);
        return Err(ApiError::bad_request("Current password is incorrect."));
    }
    match check_second_factor(state, user_id, Some(&request.code)).await? {
        true => {
            state.guards.logins.clear(&key);
            Ok(())
        }
        false => {
            miss(state, &key);
            Err(wrong_code())
        }
    }
}

pub(crate) async fn disable(
    SessionSubject(subject): SessionSubject,
    State(state): State<AppState>,
    Json(request): Json<ProofRequest>,
) -> Result<StatusCode, ApiError> {
    prove(&state, &subject.user_id, &request).await?;
    state
        .storage
        .disable_mfa(&subject.user_id)
        .await
        .map_err(ApiError::internal)?;
    record(
        &state,
        &subject.user_id,
        "mfa_disabled",
        "Two-step sign-in was turned off",
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn regenerate_codes(
    SessionSubject(subject): SessionSubject,
    State(state): State<AppState>,
    Json(request): Json<ProofRequest>,
) -> Result<Json<Value>, ApiError> {
    prove(&state, &subject.user_id, &request).await?;
    let codes = new_recovery_codes()?;
    let hashes: Vec<String> = codes.iter().map(|c| repomemo_storage::recovery_code_hash(c)).collect();
    state
        .storage
        .replace_recovery_codes(&subject.user_id, &hashes)
        .await
        .map_err(ApiError::internal)?;
    record(
        &state,
        &subject.user_id,
        "mfa_recovery_codes_replaced",
        "Recovery codes were replaced",
    )
    .await;
    Ok(Json(json!({"recovery_codes":codes})))
}

/// A system administrator removes someone's second factor (a lost device with no recovery code left) and ends their
/// sessions. It is audited, and the person signs in with their password alone until they set it up again.
pub(crate) async fn admin_reset(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    crate::system::require_full_system_admin(&subject)?;
    crate::system::require_system_change(&subject)?;
    if user_id == subject.user_id {
        return Err(ApiError::conflict(
            "Turn off your own two-step sign-in from your profile.",
        ));
    }
    state
        .storage
        .find_user(&user_id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("User was not found."))?;
    let had = state.storage.disable_mfa(&user_id).await.map_err(ApiError::internal)?;
    state
        .storage
        .end_user_sessions(&user_id)
        .await
        .map_err(ApiError::internal)?;
    crate::system::audit(
        &state,
        &subject,
        "mfa_reset_by_administrator",
        format!(
            "Removed two-step sign-in ({}) of user {user_id} and ended their sessions.",
            if had { "it was on" } else { "it was off" }
        ),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn totp_matches_the_rfc_6238_sha1_vectors() {
        // RFC 6238 appendix B, SHA-1, the secret "12345678901234567890", last six digits.
        let secret = b"12345678901234567890";
        assert_eq!(totp(secret, 59 / 30), 287_082);
        assert_eq!(totp(secret, 1_111_111_109 / 30), 81_804);
        assert_eq!(totp(secret, 20_000_000_000 / 30), 353_130);
    }

    #[test]
    fn base32_round_trips_and_codes_are_accepted_once_within_a_step_either_side() {
        let bytes = [
            1u8, 2, 3, 250, 251, 252, 0, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19,
        ];
        assert_eq!(base32_decode(&base32(&bytes)).unwrap(), bytes);
        let secret = b"12345678901234567890";
        let code = format!("{:06}", totp(secret, 1000));
        assert_eq!(matching_step(secret, &code, 1000, 0), Some(1000));
        assert_eq!(matching_step(secret, &code, 1001, 0), Some(1000));
        assert_eq!(matching_step(secret, &code, 1002, 0), None);
        // The step is already used, or a later one was.
        assert_eq!(matching_step(secret, &code, 1000, 1000), None);
        assert_eq!(matching_step(secret, &code, 1000, 1005), None);
        assert_eq!(matching_step(secret, "12345", 1000, 0), None);
        assert_eq!(matching_step(secret, "abcdef", 1000, 0), None);
    }
}
