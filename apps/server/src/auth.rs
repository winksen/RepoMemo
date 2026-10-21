use crate::*;

/// Registration is open by configuration, or because nobody has an account
/// yet (so a server with registration closed can still get its first owner).
pub(crate) async fn registration_open(state: &AppState) -> Result<bool, ApiError> {
    // Nobody registers before the onboarding is done.
    if setup::setup_pending(state).await? {
        return Ok(false);
    }
    if state.settings().allow_registration {
        return Ok(true);
    }
    Ok(state.storage.count_users().await.map_err(ApiError::internal)? == 0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AuthAction {
    /// Signing in or registering: guessable, so tightly limited.
    SignIn,
    /// Refreshing a session: every open tab does it and the token cannot be
    /// guessed, so it only needs protecting from floods.
    Refresh,
}

/// Counts one authentication request against the client address's quota.
pub(crate) fn check_auth_quota(state: &AppState, client: &ClientIp, action: AuthAction) -> Result<(), ApiError> {
    let quota = match action {
        AuthAction::SignIn => state.settings().auth_quota,
        AuthAction::Refresh => Quota {
            limit: state.settings().auth_quota.limit.saturating_mul(4),
            ..state.settings().auth_quota
        },
    };
    state
        .guards
        .auth
        .check(&format!("{action:?}:{}", client.0), quota)
        .map_err(|wait| {
            tracing::warn!(target: "audit", client = %client.0, ?action, "Authentication rate limit reached");
            too_many_requests(
                format!(
                    "Too many sign-in attempts from this address. Try again in {}.",
                    wait_text(wait)
                ),
                wait,
            )
        })
}

/// Registration attempts for an address that already has an account, which
/// each mail its owner, per address per hour.
const KNOWN_ADDRESS_NOTICES_PER_HOUR: u32 = 2;

#[derive(Debug, Serialize)]
pub(crate) struct RegistrationNotice {
    message: &'static str,
}

/// An open registration without an invitation answers the same for an address
/// that is new and for one that has an account, when mail can carry the
/// difference: a new address gets its account (to sign in to, with the
/// password just chosen) and a confirmation link; a known one gets a message
/// to its owner and nothing changes. Without mail, or without the public
/// address, it keeps answering `201` with a session, and `409` for a known
/// address.
async fn registers_without_revealing(state: &AppState) -> bool {
    state.mail.is_ready().await
        && state.mail.link("/login").is_some()
        && state.storage.count_users().await.is_ok_and(|users| users > 0)
}

async fn register_without_revealing(state: &AppState, client: &ClientIp, request: RegisterRequest) -> Result<Response, ApiError> {
    validate_registration(&request)?;
    let address = request.email.trim().to_lowercase();
    if !repomemo_mail::valid_address(&address) {
        return Err(ApiError::bad_request("That is not a valid email address."));
    }
    // The password is hashed whatever the address, so the time taken does
    // not tell the two cases apart.
    let password_hash = hash_password(&request.password).await?;
    let existing = state
        .storage
        .find_user_for_auth(&address)
        .await
        .map_err(ApiError::internal)?;
    match existing {
        Some(account) => {
            tracing::info!(target: "audit", user_id = %account.user.id, client = %client.0, "Registration attempted for an address that has an account");
            let within_limit = state
                .guards
                .auth
                .check(
                    &format!("registered:{address}"),
                    crate::Quota::per_hour(KNOWN_ADDRESS_NOTICES_PER_HOUR),
                )
                .is_ok();
            let deactivated = state
                .storage
                .is_deactivated(&account.user.id)
                .await
                .unwrap_or(true);
            if within_limit && !deactivated {
                let link = state.mail.link("/login").unwrap_or_default();
                let body = format!(
                    "Someone tried to create a RepoMemo account with this email address, which already has one.\n\n\
                     Nothing changed. If it was you, sign in here, or choose \"Forgot your password?\" on that page:\n\n\
                     {link}\n\n\
                     If it was not you, you can ignore this message.\n"
                );
                let _ = state
                    .mail
                    .send_to(&address, "Someone tried to register with your address", &body, "registration_attempt")
                    .await;
            }
        }
        None => {
            let user = state
                .storage
                .create_user(&address, &request.display_name, &password_hash)
                .await
                .map_err(ApiError::internal)?;
            state
                .storage
                .touch_user_connection(&user.id)
                .await
                .map_err(ApiError::internal)?;
            tracing::info!(target: "audit", user_id = %user.id, client = %client.0, "Account registered");
            if let Err(error) = state.storage.mark_email_unverified(&user.id).await {
                tracing::error!(error = %error, "Failed to mark an address as unverified");
            }
            crate::verification::send_link(state, &user).await;
        }
    }
    Ok((
        StatusCode::ACCEPTED,
        Json(RegistrationNotice {
            message: "If this address can be used, the account is ready: sign in with the email and password you just chose. An address that already has an account gets a message instead, and nothing changes for it.",
        }),
    )
        .into_response())
}

pub(crate) async fn register(
    State(state): State<AppState>,
    client: ClientIp,
    Json(request): Json<RegisterRequest>,
) -> Result<Response, ApiError> {
    check_auth_quota(&state, &client, AuthAction::SignIn)?;
    if setup::setup_pending(&state).await? {
        return Err(ApiError::forbidden_because(
            "This server is not set up yet. Its administrator must finish the setup first.",
        ));
    }
    match request
        .invitation_token
        .as_deref()
        .filter(|token| !token.trim().is_empty())
    {
        Some(token) => access::invitation_allows_registration(&state, token, &request.email).await?,
        None => {
            if !registration_open(&state).await? {
                return Err(ApiError::forbidden_because(
                    "Registration is closed on this server. Ask an administrator for an account.",
                ));
            }
            if registers_without_revealing(&state).await {
                return register_without_revealing(&state, &client, request).await;
            }
        }
    }
    validate_registration(&request)?;
    if state
        .storage
        .find_user_for_auth(&request.email)
        .await
        .map_err(map_storage_error)?
        .is_some()
    {
        return Err(ApiError::conflict("An account already exists for this email address."));
    }
    let password_hash = hash_password(&request.password).await?;
    let user = state
        .storage
        .create_user(&request.email, &request.display_name, &password_hash)
        .await
        .map_err(ApiError::internal)?;
    state
        .storage
        .touch_user_connection(&user.id)
        .await
        .map_err(ApiError::internal)?;
    tracing::info!(target: "audit", user_id = %user.id, client = %client.0, "Account registered");
    // The first account of a new server administers it. Accounts listed in
    // REPOMEMO_SYSTEM_ADMIN_EMAILS are promoted only at startup, once they
    // exist: email addresses are not verified, so whoever registers a listed
    // address first must not get the role.
    let first_account = state.storage.count_users().await.map_err(ApiError::internal)? == 1
        && state.storage.system_admin_count().await.map_err(ApiError::internal)? == 0;
    if first_account {
        state
            .storage
            .set_system_admin(&user.id, true)
            .await
            .map_err(ApiError::internal)?;
        // With the onboarding off, the first account sets the server up.
        state
            .storage
            .complete_setup(Some(&user.id))
            .await
            .map_err(ApiError::internal)?;
        let detail = format!(
            "{} became a system administrator as the first account on this server.",
            user.email.as_deref().unwrap_or("A new account")
        );
        tracing::info!(target: "audit", user_id = %user.id, "{detail}");
        if let Err(error) = state
            .storage
            .record_system_event(Some(&user.id), "system_admin_granted", &detail)
            .await
        {
            tracing::error!(error = %error, "Failed to record a system audit event");
        }
    } else if user.email.as_deref().is_some_and(|email| {
        state
            .settings()
            .system_admin_emails
            .iter()
            .any(|listed| listed == email)
    }) {
        tracing::warn!(target: "audit", user_id = %user.id, "An account registered with an address listed in REPOMEMO_SYSTEM_ADMIN_EMAILS. It was not made a system administrator; if it belongs to the intended person, grant the role in System › Users.");
    }
    // An invitation was sent to this address, and the first account administers
    // the server: both are known. Anyone else proves the address with a link.
    let invited = request
        .invitation_token
        .as_deref()
        .is_some_and(|token| !token.trim().is_empty());
    if !first_account && !invited {
        if let Err(error) = state.storage.mark_email_unverified(&user.id).await {
            tracing::error!(error = %error, "Failed to mark an address as unverified");
        }
        if crate::verification::enforced(&state).await {
            crate::verification::send_link(&state, &user).await;
        }
    }
    let response = issue_session(&state, user).await?;
    Ok((StatusCode::CREATED, Json(response)).into_response())
}

pub(crate) async fn login(
    State(state): State<AppState>,
    client: ClientIp,
    Json(request): Json<LoginRequest>,
) -> Result<Json<TokenResponse>, ApiError> {
    check_auth_quota(&state, &client, AuthAction::SignIn)?;
    let email = request.email.trim().to_lowercase();
    // Failures are counted per account and address, and per account across
    // all addresses with a higher bar, so one attacker cannot lock a victim
    // out from everywhere while a distributed one is still slowed down.
    let pair_key = format!("login:{email}|{}", client.0);
    let account_key = format!("login:{email}");
    let settings = state.settings();
    for key in [&pair_key, &account_key] {
        if let Some(wait) = state.guards.logins.locked_for(key) {
            // Anyone can lock an account by failing from many addresses, or
            // from one the owner shares; the owner's own device, which
            // still holds a session, gets through (review L10).
            if known_device(&state, &email, request.refresh_token.as_deref()).await {
                continue;
            }
            tracing::warn!(target: "audit", email = %email, client = %client.0, "Sign-in refused: account temporarily locked");
            return Err(too_many_requests(
                format!(
                    "Too many failed sign-in attempts for this account. Try again in {}.",
                    wait_text(wait)
                ),
                wait,
            ));
        }
    }
    let account = state
        .storage
        .find_user_for_auth(&request.email)
        .await
        .map_err(|_| invalid_credentials())?;
    // Verify against a fixed hash when the account does not exist, so the
    // response time does not reveal which email addresses have accounts.
    let (stored_hash, user) = match account {
        Some(account) => (account.password_hash, Some(account.user)),
        None => (dummy_password_hash().to_owned(), None),
    };
    let verified = verify_password(&request.password, &stored_hash).await;
    let Some(user) = user.filter(|_| verified) else {
        // `|`, not `||`: both counters must see every failure.
        let locked = state
            .guards
            .logins
            .record_failure(&pair_key, settings.login_max_failures, settings.login_lockout)
            | state.guards.logins.record_failure(
                &account_key,
                settings.login_max_failures.saturating_mul(5),
                settings.login_lockout,
            );
        tracing::warn!(target: "audit", email = %email, client = %client.0, locked, "Sign-in failed");
        return Err(invalid_credentials());
    };
    // Said only after the password is right, so it does not tell anyone
    // else which addresses have accounts.
    if state
        .storage
        .is_deactivated(&user.id)
        .await
        .map_err(ApiError::internal)?
    {
        tracing::warn!(target: "audit", user_id = %user.id, client = %client.0, "Sign-in refused: account deactivated");
        return Err(ApiError::forbidden_because(
            "This account has been deactivated. Ask a system administrator to reactivate it.",
        ));
    }
    state.guards.logins.clear(&pair_key);
    state
        .storage
        .touch_user_connection(&user.id)
        .await
        .map_err(ApiError::internal)?;
    tracing::info!(target: "audit", user_id = %user.id, client = %client.0, "Signed in");
    Ok(Json(issue_session(&state, user).await?))
}

/// Whether `refresh_token` is a usable session token of the account `email`.
pub(crate) async fn known_device(state: &AppState, email: &str, refresh_token: Option<&str>) -> bool {
    let Some(token) = refresh_token.filter(|token| !token.is_empty()) else {
        return false;
    };
    let (Ok(Some(owner)), Ok(Some(user))) = (
        state.storage.refresh_token_user(token).await,
        state.storage.find_user_by_email(email).await,
    ) else {
        return false;
    };
    owner == user.id
}

/// Exchanges a valid refresh token for a new access token and a rotated refresh token.
pub(crate) async fn refresh_session(
    State(state): State<AppState>,
    client: ClientIp,
    Json(request): Json<RefreshRequest>,
) -> Result<Json<TokenResponse>, ApiError> {
    check_auth_quota(&state, &client, AuthAction::Refresh)?;
    let user_id = state
        .storage
        .consume_refresh_token(&request.refresh_token)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(ApiError::unauthorized)?;
    let user = state
        .storage
        .find_user(&user_id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(ApiError::unauthorized)?;
    Ok(Json(issue_session(&state, user).await?))
}

pub(crate) async fn logout(
    State(state): State<AppState>,
    Json(request): Json<RefreshRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .storage
        .revoke_refresh_token(&request.refresh_token)
        .await
        .map_err(ApiError::internal)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Ends every session of the caller on every device: access tokens stop
/// working at once and refresh tokens are revoked.
pub(crate) async fn logout_everywhere(
    SessionSubject(subject): SessionSubject,
    State(state): State<AppState>,
) -> Result<StatusCode, ApiError> {
    state
        .storage
        .end_user_sessions(&subject.user_id)
        .await
        .map_err(ApiError::internal)?;
    tracing::info!(target: "audit", user_id = %subject.user_id, "Signed out of every session");
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn session(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<SharedSession>, ApiError> {
    let user = state
        .storage
        .find_user(&subject.user_id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(ApiError::unauthorized)?;
    let memberships = state
        .storage
        .workspace_memberships_for_user(&subject.user_id)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(SharedSession {
        user,
        authentication: "jwt".to_owned(),
        memberships,
        is_system_admin: subject.is_system_admin,
        is_app_admin: subject.is_app_admin,
        can_create_organizations: state.settings().organization_creators == "everyone"
            || subject.is_system_admin
            || subject.is_app_admin,
    }))
}

/// Changes the caller's password. Every existing session ends (other devices
/// are signed out at once) and a fresh session is returned for this one.
pub(crate) async fn change_profile_password(
    SessionSubject(subject): SessionSubject,
    State(state): State<AppState>,
    Json(request): Json<ChangePasswordRequest>,
) -> Result<Json<TokenResponse>, ApiError> {
    let account = state
        .storage
        .find_user(&subject.user_id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(ApiError::unauthorized)?;
    validate_password(&request.new_password, account.email.as_deref())?;
    let stored_account = state
        .storage
        .find_user_for_auth(account.email.as_deref().unwrap_or_default())
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(ApiError::unauthorized)?;
    if !verify_password(&request.current_password, &stored_account.password_hash).await {
        tracing::warn!(target: "audit", user_id = %subject.user_id, "Password change refused: wrong current password");
        return Err(ApiError::bad_request("Current password is incorrect."));
    }
    let password_hash = hash_password(&request.new_password).await?;
    state
        .storage
        .update_user_password(&subject.user_id, &password_hash)
        .await
        .map_err(map_storage_error)?;
    state
        .storage
        .end_user_sessions(&subject.user_id)
        .await
        .map_err(ApiError::internal)?;
    tracing::info!(target: "audit", user_id = %subject.user_id, "Password changed; other sessions ended");
    Ok(Json(issue_session(&state, account).await?))
}

pub(crate) fn validate_registration(request: &RegisterRequest) -> Result<(), ApiError> {
    if request.display_name.trim().is_empty() || request.display_name.trim().len() > 120 {
        return Err(ApiError::bad_request(
            "Display name must be between 1 and 120 characters.",
        ));
    }
    validate_password(&request.password, Some(&request.email))
}

/// Long passwords people still pick, compared ignoring case. Short ones are
/// already refused by the length rule.
pub(crate) const COMMON_PASSWORDS: &[&str] = &[
    "password1234",
    "password12345",
    "password123456",
    "password123!",
    "password1234!",
    "passwordpassword",
    "123456789012",
    "1234567890123",
    "12345678901234",
    "123456789abc",
    "1234567890ab",
    "1q2w3e4r5t6y",
    "qwertyuiop12",
    "qwertyuiopasdf",
    "qwerty123456",
    "qwertyqwerty",
    "asdfghjkl123",
    "1qaz2wsx3edc",
    "iloveyou1234",
    "letmein12345",
    "welcome12345",
    "welcome123456",
    "administrator",
    "changeme1234",
    "abcdefghijkl",
    "abc123abc123",
    "trustno11234",
    "football1234",
    "baseball1234",
    "superman1234",
    "monkey123456",
    "sunshine1234",
    "princess1234",
    "dragon123456",
    "master123456",
    "admin1234567",
];

/// The rules every new password follows (review L7): 12 to 1024 characters,
/// not a well-known password, not mostly the same few characters, and not
/// built from the account's email address.
pub(crate) fn validate_password(password: &str, email: Option<&str>) -> Result<(), ApiError> {
    if password.len() < 12 || password.len() > 1024 {
        return Err(ApiError::bad_request(
            "Password must be between 12 and 1024 characters.",
        ));
    }
    let lowered = password.to_lowercase();
    if COMMON_PASSWORDS.contains(&lowered.as_str()) {
        return Err(ApiError::bad_request(
            "This password is too common. Choose one that is not on lists of well-known passwords.",
        ));
    }
    if password.chars().collect::<BTreeSet<_>>().len() < 5 {
        return Err(ApiError::bad_request(
            "This password repeats too few characters. Use at least five different ones.",
        ));
    }
    let local_part = email
        .and_then(|email| email.trim().split('@').next())
        .map(str::to_lowercase)
        .filter(|part| part.chars().count() >= 4);
    if local_part.is_some_and(|part| lowered.contains(&part)) {
        return Err(ApiError::bad_request(
            "The password must not contain your email address.",
        ));
    }
    Ok(())
}

/// Argon2 is deliberately slow (tens of milliseconds), so it runs on the
/// blocking pool instead of holding up the async workers.
pub(crate) async fn hash_password(password: &str) -> Result<String, ApiError> {
    let password = password.to_owned();
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
    })
    .await
    .map_err(ApiError::internal)?
    .map_err(ApiError::internal)
}

pub(crate) async fn verify_password(password: &str, stored_hash: &str) -> bool {
    let password = password.to_owned();
    let stored_hash = stored_hash.to_owned();
    tokio::task::spawn_blocking(move || {
        PasswordHash::new(&stored_hash)
            .ok()
            .and_then(|hash| Argon2::default().verify_password(password.as_bytes(), &hash).ok())
            .is_some()
    })
    .await
    .unwrap_or(false)
}

/// A valid Argon2 hash of a random password nobody knows, checked when an
/// email has no account so that a miss costs as much time as a wrong password.
pub(crate) fn dummy_password_hash() -> &'static str {
    static HASH: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    HASH.get_or_init(|| {
        let salt = SaltString::generate(&mut OsRng);
        let secret = random_secret();
        Argon2::default()
            .hash_password(secret.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .unwrap_or_default()
    })
}

pub(crate) fn random_secret() -> String {
    let mut bytes = [0_u8; 32];
    rand_core::RngCore::fill_bytes(&mut OsRng, &mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) async fn issue_session(state: &AppState, user: SharedUser) -> Result<TokenResponse, ApiError> {
    let version = state
        .storage
        .user_session_version(&user.id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(ApiError::unauthorized)?;
    let refresh_token = state
        .storage
        .create_refresh_token(&user.id, state.settings().refresh_token_ttl_days)
        .await
        .map_err(ApiError::internal)?;
    issue_token(
        &state.jwt_secret,
        user,
        refresh_token,
        version,
        state.settings().access_token_ttl_minutes,
    )
}

pub(crate) fn issue_token(
    secret: &str,
    user: SharedUser,
    refresh_token: String,
    session_version: i64,
    ttl_minutes: i64,
) -> Result<TokenResponse, ApiError> {
    let now = Utc::now();
    let expires_at = now + ChronoDuration::minutes(ttl_minutes);
    let claims = JwtClaims {
        sub: user.id.clone(),
        email: user.email.clone().unwrap_or_default(),
        iss: JWT_ISSUER.to_owned(),
        iat: now.timestamp() as usize,
        exp: expires_at.timestamp() as usize,
        ver: session_version,
    };
    let access_token = encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(ApiError::internal)?;
    Ok(TokenResponse {
        access_token,
        refresh_token,
        token_type: "Bearer",
        expires_in: Duration::from_secs(ttl_minutes.max(0) as u64 * 60).as_secs(),
        user,
    })
}
