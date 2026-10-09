//! First-run onboarding of a new server.
//!
//! A brand-new server (no account yet) shows an onboarding in the web app
//! instead of sign-in. It creates the first system administrator, checks the
//! server, takes the first settings and is then completed, after which it
//! never shows again. Guardrails:
//! - creating the first administrator needs a **one-time setup code** printed
//!   on the server console at startup (or set with `REPOMEMO_SETUP_CODE`), so
//!   whoever reaches a freshly exposed server first cannot claim it; failed
//!   codes are counted per address and locked out like failed sign-ins;
//! - the administrator can only be created while no account exists, in one
//!   transaction, so two requests cannot both succeed;
//! - public registration is refused until the onboarding is complete;
//! - the remaining steps need the new system administrator's session;
//! - completion is permanent: every setup endpoint refuses afterwards, and a
//!   server that already has accounts is complete from the start.
//!
//! `REPOMEMO_SETUP_WIZARD=false` turns the onboarding off for scripted
//! installs; the first account to register then becomes system administrator.

use std::{path::PathBuf, time::Duration};

use axum::{extract::State, http::StatusCode, Json};
use rand_core::{OsRng, RngCore};
use repomemo_storage::SetupState;
use serde::{Deserialize, Serialize};

use crate::{
    check_auth_quota, hash_password, issue_session, logs, map_storage_error,
    security::{too_many_requests, wait_text},
    validate_registration, ApiError, AppState, AuthAction, AuthenticatedSubject, ClientIp,
    RegisterRequest, TokenResponse,
};

/// Failed setup codes from one address before it must wait.
const MAX_CODE_FAILURES: u32 = 10;
const CODE_LOCKOUT: Duration = Duration::from_secs(15 * 60);
/// Letters and digits that cannot be mistaken for one another.
const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// What the onboarding needs at run time, fixed at startup.
pub(crate) struct SetupGate {
    /// The onboarding is on (`REPOMEMO_SETUP_WIZARD`, default true).
    pub wizard: bool,
    /// The one-time code, normalised; only while the server is new.
    code: Option<String>,
    facts: SetupFacts,
}

/// Facts about the configuration the server checks report on.
pub(crate) struct SetupFacts {
    pub secret_key_from_env: bool,
    pub allowed_origins: Vec<String>,
    pub repo_roots: Vec<String>,
    pub ai_hosts_restricted: bool,
    pub trust_proxy: bool,
    pub data_dir: PathBuf,
}

impl SetupGate {
    /// Prepares the onboarding. On a new server with the onboarding on, this
    /// takes the code from the configuration or makes one, and prints it on
    /// the console (not into the log files).
    pub(crate) fn prepare(wizard: bool, configured_code: Option<&str>, state: &SetupState, facts: SetupFacts) -> Self {
        let code = (wizard && *state == SetupState::New).then(|| match configured_code {
            Some(code) => {
                tracing::warn!(target: "audit", "This server is not set up yet: open the web app and enter the setup code from REPOMEMO_SETUP_CODE");
                normalise_code(code)
            }
            None => {
                let code = generate_code();
                eprintln!(
                    "\n================================================================\n  RepoMemo is not set up yet.\n  Open the web app and enter this one-time setup code:\n\n      {}\n\n  It changes every time the server starts until setup is done.\n================================================================\n",
                    display_code(&code)
                );
                tracing::warn!(target: "audit", "This server is not set up yet: open the web app and enter the setup code printed on the console");
                code
            }
        });
        if wizard && matches!(state, SetupState::Finishing { .. }) {
            tracing::warn!("Server setup is not finished: sign in as the system administrator to complete it");
        }
        Self { wizard, code, facts }
    }

    fn code_matches(&self, candidate: &str) -> bool {
        self.code.as_deref().is_some_and(|expected| codes_match(expected, candidate))
    }
}

/// Whether `candidate` is the normalised `expected` code, ignoring case and
/// separators.
pub(crate) fn codes_match(expected: &str, candidate: &str) -> bool {
    let candidate = normalise_code(candidate);
    // Compare every byte so the time taken does not reveal how much matched.
    expected.len() == candidate.len()
        && expected
            .bytes()
            .zip(candidate.bytes())
            .fold(0_u8, |difference, (left, right)| difference | (left ^ right))
            == 0
}

pub(crate) fn generate_code() -> String {
    let mut bytes = [0_u8; 12];
    OsRng.fill_bytes(&mut bytes);
    bytes
        .iter()
        .map(|byte| CODE_ALPHABET[usize::from(*byte) % CODE_ALPHABET.len()] as char)
        .collect()
}

/// Upper case without separators, so `abcd-efgh` and `ABCDEFGH` match.
pub(crate) fn normalise_code(code: &str) -> String {
    code.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_uppercase())
        .collect()
}

pub(crate) fn display_code(code: &str) -> String {
    code.as_bytes()
        .chunks(4)
        .map(|chunk| String::from_utf8_lossy(chunk).into_owned())
        .collect::<Vec<_>>()
        .join("-")
}

/// Whether the onboarding is still to be done (it is on and not complete).
pub(crate) async fn setup_pending(state: &AppState) -> Result<bool, ApiError> {
    if !state.setup.wizard {
        return Ok(false);
    }
    Ok(!state.storage.setup_state().await.map_err(map_storage_error)?.is_complete())
}

#[derive(Debug, Serialize)]
pub(crate) struct SetupStatus {
    /// `new`, `finishing` or `complete`.
    state: &'static str,
    /// The web app should show the onboarding.
    onboarding: bool,
}

async fn status(state: &AppState) -> Result<SetupStatus, ApiError> {
    let current = state.storage.setup_state().await.map_err(map_storage_error)?;
    Ok(SetupStatus {
        state: match current {
            SetupState::New => "new",
            SetupState::Finishing { .. } => "finishing",
            SetupState::Complete => "complete",
        },
        onboarding: state.setup.wizard && !current.is_complete(),
    })
}

/// Public: whether this server still has to be set up.
pub(crate) async fn get_status(State(state): State<AppState>) -> Result<Json<SetupStatus>, ApiError> {
    status(&state).await.map(Json)
}

#[derive(Debug, Deserialize)]
pub(crate) struct VerifyCodeRequest {
    code: String,
}

/// Refuses unless the server is new, the onboarding is on and the code is
/// right. Failures count towards a lockout of the caller's address.
async fn check_new_server_and_code(state: &AppState, client: &ClientIp, code: &str) -> Result<(), ApiError> {
    if !state.setup.wizard {
        return Err(ApiError::not_found("The onboarding is turned off on this server."));
    }
    if state.storage.setup_state().await.map_err(map_storage_error)? != SetupState::New {
        return Err(ApiError::conflict("This server already has its system administrator."));
    }
    let key = format!("setup:{}", client.0);
    if let Some(wait) = state.guards.logins.locked_for(&key) {
        return Err(too_many_requests(
            format!("Too many wrong setup codes from this address. Try again in {}.", wait_text(wait)),
            wait,
        ));
    }
    if !state.setup.code_matches(code) {
        let locked = state.guards.logins.record_failure(&key, MAX_CODE_FAILURES, CODE_LOCKOUT);
        tracing::warn!(target: "audit", client = %client.0, locked, "Wrong server setup code");
        return Err(ApiError::forbidden_because(
            "The setup code is not correct. It is printed on the server console when the server starts.",
        ));
    }
    state.guards.logins.clear(&key);
    Ok(())
}

/// Public, new server only: checks the setup code before the account form.
pub(crate) async fn verify_code(
    State(state): State<AppState>,
    client: ClientIp,
    Json(request): Json<VerifyCodeRequest>,
) -> Result<StatusCode, ApiError> {
    check_auth_quota(&state, &client, AuthAction::SignIn)?;
    check_new_server_and_code(&state, &client, &request.code).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateAdminRequest {
    code: String,
    email: String,
    display_name: String,
    password: String,
}

/// Public, new server only: creates the first system administrator and signs
/// them in.
pub(crate) async fn create_admin(
    State(state): State<AppState>,
    client: ClientIp,
    Json(request): Json<CreateAdminRequest>,
) -> Result<(StatusCode, Json<TokenResponse>), ApiError> {
    check_auth_quota(&state, &client, AuthAction::SignIn)?;
    check_new_server_and_code(&state, &client, &request.code).await?;
    validate_registration(&RegisterRequest {
        email: request.email.clone(),
        display_name: request.display_name.clone(),
        password: request.password.clone(),
    })?;
    let password_hash = hash_password(&request.password).await?;
    let user = state
        .storage
        .create_setup_admin(&request.email, &request.display_name, &password_hash)
        .await
        .map_err(|error| {
            let message = error.to_string();
            if message.contains("valid email") {
                ApiError::bad_request(message)
            } else {
                ApiError::internal(error)
            }
        })?
        .ok_or_else(|| ApiError::conflict("This server already has its system administrator."))?;
    state
        .storage
        .touch_user_connection(&user.id)
        .await
        .map_err(ApiError::internal)?;
    let detail = format!(
        "{} was created as the first system administrator during server setup.",
        user.email.as_deref().unwrap_or("An account")
    );
    tracing::info!(target: "audit", user_id = %user.id, client = %client.0, "{detail}");
    if let Err(error) = state.storage.record_system_event(Some(&user.id), "server_setup_admin_created", &detail).await {
        tracing::error!(error = %error, "Failed to record a system audit event");
    }
    let session = issue_session(&state, user).await?;
    Ok((StatusCode::CREATED, Json(session)))
}

/// The remaining steps belong to the system administrator, while the server
/// is still being set up.
async fn require_finishing_admin(state: &AppState, subject: &AuthenticatedSubject) -> Result<(), ApiError> {
    if !state.setup.wizard {
        return Err(ApiError::not_found("The onboarding is turned off on this server."));
    }
    if !subject.is_system_admin {
        return Err(ApiError::forbidden_because("Only the system administrator can finish setting up the server."));
    }
    match state.storage.setup_state().await.map_err(map_storage_error)? {
        SetupState::Finishing { .. } => Ok(()),
        SetupState::Complete => Err(ApiError::conflict("This server is already set up.")),
        SetupState::New => Err(ApiError::conflict("Create the system administrator first.")),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum CheckStatus {
    Ok,
    Info,
    Warning,
    Error,
}

#[derive(Debug, Serialize)]
pub(crate) struct SetupCheck {
    key: &'static str,
    label: &'static str,
    status: CheckStatus,
    detail: String,
    /// What to do about a warning or an error.
    advice: Option<&'static str>,
}

/// Checks the server's environment for the onboarding.
pub(crate) async fn checks(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<Vec<SetupCheck>>, ApiError> {
    require_finishing_admin(&state, &subject).await?;
    let facts = &state.setup.facts;
    let mut checks = Vec::new();

    checks.push(match state.storage.ping().await {
        Ok(()) => SetupCheck { key: "database", label: "Database", status: CheckStatus::Ok, detail: "The database answers.".to_owned(), advice: None },
        Err(error) => SetupCheck { key: "database", label: "Database", status: CheckStatus::Error, detail: format!("The database does not answer: {error}"), advice: Some("Check that the data folder is on a local disk with free space.") },
    });

    let probe = facts.data_dir.join(format!(".setup-check-{}", uuid_like()));
    let writable = tokio::fs::write(&probe, b"ok").await.is_ok() && tokio::fs::remove_file(&probe).await.is_ok();
    checks.push(SetupCheck {
        key: "data_folder",
        label: "Data folder",
        status: if writable { CheckStatus::Ok } else { CheckStatus::Error },
        detail: format!("{} {}", facts.data_dir.display(), if writable { "is writable." } else { "cannot be written." }),
        advice: (!writable).then_some("Give the server's account write access to its data folder (REPOMEMO_SERVER_DATA_DIR)."),
    });

    checks.push(if facts.secret_key_from_env {
        SetupCheck { key: "secret_key", label: "Key for stored credentials", status: CheckStatus::Ok, detail: "AI provider keys are encrypted with REPOMEMO_SECRET_KEY.".to_owned(), advice: None }
    } else {
        SetupCheck { key: "secret_key", label: "Key for stored credentials", status: CheckStatus::Warning, detail: "AI provider keys are encrypted with the key file secret.key in the data folder.".to_owned(), advice: Some("Back up secret.key together with the database, or set REPOMEMO_SECRET_KEY from a secret store so a copy of the data folder alone reveals nothing.") }
    });

    let git = tokio::time::timeout(
        Duration::from_secs(5),
        tokio::process::Command::new("git").arg("--version").kill_on_drop(true).output(),
    )
    .await;
    checks.push(match git {
        Ok(Ok(output)) if output.status.success() => SetupCheck { key: "git", label: "git", status: CheckStatus::Ok, detail: String::from_utf8_lossy(&output.stdout).trim().to_owned(), advice: None },
        _ => SetupCheck { key: "git", label: "git", status: CheckStatus::Warning, detail: "git was not found.".to_owned(), advice: Some("Install git and add it to the server's PATH to link repositories.") },
    });

    checks.push(if state.converter.enabled() {
        SetupCheck { key: "libreoffice", label: "LibreOffice", status: CheckStatus::Ok, detail: "Word, Excel and PowerPoint files get layout-accurate previews.".to_owned(), advice: None }
    } else {
        SetupCheck { key: "libreoffice", label: "LibreOffice", status: CheckStatus::Info, detail: "Not installed: Office files show extracted text and tables.".to_owned(), advice: Some("Optional. Install LibreOffice (or set REPOMEMO_SOFFICE) for faithful previews.") }
    });

    checks.push(SetupCheck {
        key: "origins",
        label: "Browser origins",
        status: if facts.allowed_origins.is_empty() { CheckStatus::Warning } else { CheckStatus::Ok },
        detail: if facts.allowed_origins.is_empty() { "No browser origin is allowed.".to_owned() } else { facts.allowed_origins.join(", ") },
        advice: Some("Only these addresses can use the web app. Set REPOMEMO_ALLOWED_ORIGIN to the address people open, and serve it over HTTPS."),
    });

    checks.push(if facts.repo_roots.is_empty() {
        SetupCheck { key: "repo_roots", label: "Repository folders", status: CheckStatus::Warning, detail: "Any local folder the server can read can be linked as a repository.".to_owned(), advice: Some("Set REPOMEMO_REPO_ROOTS to the folders that hold repositories.") }
    } else {
        SetupCheck { key: "repo_roots", label: "Repository folders", status: CheckStatus::Ok, detail: facts.repo_roots.join(", "), advice: None }
    });

    checks.push(SetupCheck {
        key: "ai_hosts",
        label: "AI provider addresses",
        status: if facts.ai_hosts_restricted { CheckStatus::Ok } else { CheckStatus::Info },
        detail: if facts.ai_hosts_restricted { "Limited to REPOMEMO_AI_ALLOWED_HOSTS.".to_owned() } else { "Any address except link-local and cloud-metadata ones.".to_owned() },
        advice: (!facts.ai_hosts_restricted).then_some("Optional. Set REPOMEMO_AI_ALLOWED_HOSTS to the AI services you use."),
    });

    checks.push(SetupCheck {
        key: "proxy",
        label: "Client addresses",
        status: CheckStatus::Info,
        detail: if facts.trust_proxy { "Taken from X-Forwarded-For (behind a trusted proxy).".to_owned() } else { "Taken from the connection.".to_owned() },
        advice: Some("Behind a reverse proxy, set REPOMEMO_TRUST_PROXY=true so sign-in limits apply per person, not per proxy."),
    });

    checks.push(SetupCheck {
        key: "log_files",
        label: "Log files",
        status: if logs::writing_files() { CheckStatus::Ok } else { CheckStatus::Info },
        detail: if logs::writing_files() { "Daily log files are written to the data folder.".to_owned() } else { "Logs go to the console only.".to_owned() },
        advice: None,
    });

    Ok(Json(checks))
}

fn uuid_like() -> String {
    let mut bytes = [0_u8; 8];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Completes the onboarding for good.
pub(crate) async fn complete(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<SetupStatus>, ApiError> {
    require_finishing_admin(&state, &subject).await?;
    if !state
        .storage
        .complete_setup(Some(&subject.user_id))
        .await
        .map_err(map_storage_error)?
    {
        return Err(ApiError::conflict("This server is already set up."));
    }
    tracing::info!(target: "audit", user_id = %subject.user_id, "Server setup completed");
    if let Err(error) = state
        .storage
        .record_system_event(Some(&subject.user_id), "server_setup_completed", "Completed the server setup.")
        .await
    {
        tracing::error!(error = %error, "Failed to record a system audit event");
    }
    status(&state).await.map(Json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_unambiguous_and_compared_without_separators() {
        let code = generate_code();
        assert_eq!(code.len(), 12);
        assert!(code.bytes().all(|byte| CODE_ALPHABET.contains(&byte)));
        let gate = SetupGate {
            wizard: true,
            code: Some(code.clone()),
            facts: SetupFacts { secret_key_from_env: false, allowed_origins: Vec::new(), repo_roots: Vec::new(), ai_hosts_restricted: false, trust_proxy: false, data_dir: PathBuf::new() },
        };
        assert!(gate.code_matches(&display_code(&code).to_lowercase()));
        assert!(gate.code_matches(&format!(" {code} ")));
        assert!(!gate.code_matches(&code[..11]));
        assert!(!gate.code_matches(""));
        let closed = SetupGate { code: None, ..gate };
        assert!(!closed.code_matches(&code), "no code once the server has an administrator");
    }
}
