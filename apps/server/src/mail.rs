//! Outgoing mail (roadmap F6a): the SMTP settings, an outbox with retries and
//! the System › Settings page that manages them.
//!
//! Nothing here sends inside a request. A feature that wants a mail sent puts
//! it in the outbox (`MailService::send_to`) and returns; one background task
//! hands due messages to the mail server, retries the ones the server asked to
//! wait, and gives up after a few tries. The password is stored sealed like AI
//! provider keys and never leaves the server. The mail server's address
//! follows the outbound address rules (see `repomemo_mail::OutboundPolicy`).

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use repomemo_domain::{MailOutboxEntry, MailOverview};
use repomemo_mail::{compose, valid_address, OutboundPolicy, Outgoing, Security, SendError, SmtpSettings};
use repomemo_storage::{MailSettingsUpdate, QueuedMail, StoredMailSettings};
use tokio::sync::Notify;

use crate::system::{audit, require_full_system_admin, require_system_admin, require_system_change};
use crate::*;

/// Messages handed to the mail server per pass.
const BATCH: i64 = 10;
/// A message is tried this many times in all.
const MAX_ATTEMPTS: i64 = 6;
/// Waits before the second to the sixth try.
const RETRY_DELAYS: [Duration; 5] = [
    Duration::from_secs(60),
    Duration::from_secs(5 * 60),
    Duration::from_secs(30 * 60),
    Duration::from_secs(2 * 60 * 60),
    Duration::from_secs(6 * 60 * 60),
];
/// How often the worker looks for due messages when nothing wakes it.
const POLL: Duration = Duration::from_secs(30);
/// Finished messages are kept this many days.
const KEEP_DAYS: i64 = 30;
/// A person asking for test mails cannot fill the outbox.
const MAX_WAITING_FOR_TEST: i64 = 50;

/// Why a mail was not queued.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NotQueued {
    /// No mail server is saved, or sending is switched off.
    NotSetUp,
    /// The address is not one this server will send to.
    BadAddress,
    /// The outbox itself failed.
    Storage(String),
}

/// The handle features use to send mail, cloned into the application state.
#[derive(Clone)]
pub(crate) struct MailService {
    storage: StorageEngine,
    policy: Arc<OutboundPolicy>,
    wake: Arc<Notify>,
    /// The address people use (`REPOMEMO_PUBLIC_URL`): links in mail are made from it.
    public_url: Option<String>,
}

impl MailService {
    /// Creates the service and starts its worker. The task must be stopped
    /// when the environment is detached.
    pub(crate) fn start(
        storage: StorageEngine,
        policy: OutboundPolicy,
        public_url: Option<String>,
    ) -> (Self, tokio::task::JoinHandle<()>) {
        let service = Self {
            storage,
            policy: Arc::new(policy),
            wake: Arc::new(Notify::new()),
            public_url,
        };
        let worker = service.clone();
        let task = tokio::spawn(async move { worker.run().await });
        (service, task)
    }

    pub(crate) fn policy(&self) -> &OutboundPolicy {
        &self.policy
    }

    /// An absolute link for a path of the web app, or `None` when the server
    /// does not know the address people use (mail cannot carry a relative link).
    pub(crate) fn link(&self, path: &str) -> Option<String> {
        self.public_url
            .as_deref()
            .map(|base| format!("{}{path}", base.trim_end_matches('/')))
    }

    /// Mail can be sent: a server and a sender are saved and sending is on.
    pub(crate) async fn is_ready(&self) -> bool {
        match self.storage.mail_settings().await {
            Ok(Some(settings)) => ready(&settings),
            _ => false,
        }
    }

    /// Puts a message in the outbox and wakes the worker. Does no network
    /// work: a slow or broken mail server cannot hold the caller.
    pub(crate) async fn send_to(
        &self,
        to: &str,
        subject: &str,
        body: &str,
        kind: &str,
    ) -> Result<MailOutboxEntry, NotQueued> {
        let to = to.trim();
        if !valid_address(to) {
            return Err(NotQueued::BadAddress);
        }
        if !self.is_ready().await {
            return Err(NotQueued::NotSetUp);
        }
        let entry = self
            .storage
            .enqueue_mail(to, subject, body, kind)
            .await
            .map_err(|error| NotQueued::Storage(format!("{error:#}")))?;
        self.wake.notify_one();
        Ok(entry)
    }

    async fn run(self) {
        match self.storage.requeue_interrupted_mail().await {
            Ok(0) => {}
            Ok(count) => tracing::info!(count, "Messages a stopped server was sending are due again"),
            Err(error) => tracing::warn!(error = %error, "Could not requeue interrupted mail"),
        }
        let mut last_prune = std::time::Instant::now();
        loop {
            self.deliver_due().await;
            if last_prune.elapsed() >= Duration::from_secs(3600) {
                last_prune = std::time::Instant::now();
                if let Err(error) = self.storage.prune_mail_outbox(KEEP_DAYS).await {
                    tracing::warn!(error = %error, "Could not remove old mail from the outbox");
                }
            }
            let wait = match self.storage.next_mail_due().await {
                Ok(Some(due)) => chrono::DateTime::parse_from_rfc3339(&due)
                    .ok()
                    .and_then(|due| (due.with_timezone(&Utc) - Utc::now()).to_std().ok())
                    .map_or(Duration::from_secs(1), |left| left.max(Duration::from_secs(1)))
                    .min(POLL),
                _ => POLL,
            };
            tokio::select! {
                _ = self.wake.notified() => {}
                _ = tokio::time::sleep(wait) => {}
            }
        }
    }

    async fn deliver_due(&self) {
        let due = match self.storage.claim_due_mail(BATCH).await {
            Ok(due) => due,
            Err(error) => {
                tracing::warn!(error = %error, "Could not read the mail outbox");
                return;
            }
        };
        if due.is_empty() {
            return;
        }
        let settings = match self.storage.mail_settings().await {
            Ok(settings) => settings,
            Err(error) => {
                tracing::warn!(error = %error, "Could not read the mail settings");
                None
            }
        };
        // Deliveries in a batch run side by side: one slow server connection
        // does not make the others wait.
        let mut running = tokio::task::JoinSet::new();
        for mail in due {
            let (storage, policy, settings) = (self.storage.clone(), self.policy.clone(), settings.clone());
            running.spawn(async move { deliver_one(&storage, &policy, settings.as_ref(), mail).await });
        }
        while running.join_next().await.is_some() {}
    }
}

fn ready(settings: &StoredMailSettings) -> bool {
    settings.enabled && !settings.host.trim().is_empty() && valid_address(settings.from_address.trim())
}

/// What happens after an attempt that failed: when to try again, or `None` to give up.
fn next_attempt(error: &SendError, attempts_before: i64) -> Option<Duration> {
    if !error.temporary {
        return None;
    }
    let made = attempts_before + 1;
    if made >= MAX_ATTEMPTS {
        return None;
    }
    RETRY_DELAYS.get((made - 1) as usize).copied()
}

async fn deliver_one(
    storage: &StorageEngine,
    policy: &OutboundPolicy,
    settings: Option<&StoredMailSettings>,
    mail: QueuedMail,
) {
    let Some(settings) = settings.filter(|settings| ready(settings)) else {
        let _ = storage
            .mark_mail_failed(&mail.id, "Mail delivery is not set up, or is switched off.")
            .await;
        return;
    };
    let Some(security) = Security::parse(&settings.security) else {
        let _ = storage
            .mark_mail_failed(&mail.id, "The saved connection security is not valid.")
            .await;
        return;
    };
    let from_name = Some(settings.from_name.as_str()).filter(|name| !name.trim().is_empty());
    let composed = compose(&Outgoing {
        from_address: settings.from_address.trim(),
        from_name,
        to: &mail.to_address,
        subject: &mail.subject,
        body: &mail.body,
    });
    let data = match composed {
        Ok(data) => data,
        Err(error) => {
            let _ = storage.mark_mail_failed(&mail.id, &error.to_string()).await;
            return;
        }
    };
    let smtp = SmtpSettings {
        host: settings.host.clone(),
        port: settings.port,
        security,
        username: Some(settings.username.clone()).filter(|name| !name.is_empty()),
        password: settings.password.clone(),
    };
    match repomemo_mail::send(&smtp, policy, settings.from_address.trim(), &mail.to_address, &data).await {
        Ok(()) => {
            tracing::info!(target: "mail", id = %mail.id, kind = %mail.kind, "Mail sent");
            if let Err(error) = storage.mark_mail_sent(&mail.id).await {
                tracing::warn!(error = %error, "Could not record a sent mail");
            }
        }
        Err(error) => {
            let reason = error.message.chars().take(500).collect::<String>();
            match next_attempt(&error, mail.attempts) {
                Some(delay) => {
                    tracing::warn!(target: "mail", id = %mail.id, kind = %mail.kind, error = %reason, retry_in_secs = delay.as_secs(), "Mail not sent yet");
                    let next = (Utc::now() + chrono::Duration::from_std(delay).unwrap_or_default()).to_rfc3339();
                    let _ = storage.retry_mail_later(&mail.id, &reason, &next).await;
                }
                None => {
                    tracing::warn!(target: "mail", id = %mail.id, kind = %mail.kind, error = %reason, "Mail given up on");
                    let _ = storage.mark_mail_failed(&mail.id, &reason).await;
                }
            }
        }
    }
}

/// A person's or a place's name as it may appear in a subject: control
/// characters (a crafted name with a line break) become spaces, runs of spaces
/// collapse and the length is bounded.
pub(crate) fn plain(text: &str, limit: usize) -> String {
    let cleaned = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>();
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= limit {
        collapsed
    } else {
        format!(
            "{}…",
            collapsed.chars().take(limit.saturating_sub(1)).collect::<String>()
        )
    }
}

/// The subject and body of an invitation mail.
pub(crate) fn invitation_message(
    inviter: &str,
    target_kind: &str,
    target_name: &str,
    role: &str,
    link: &str,
    expires_on: &str,
) -> (String, String) {
    let inviter = plain(inviter, 80);
    let target = plain(target_name, 80);
    let subject = format!("{inviter} invited you to {target} on RepoMemo");
    let body = format!(
        "{inviter} invited you to join the {target} {target_kind} on RepoMemo as {role}.\n\n\
         Open this link to accept. It works once, for this email address only, and expires on {expires_on}:\n\n\
         {link}\n\n\
         If you were not expecting this, you can ignore this message.\n"
    );
    (subject, body)
}

/// Sends the invitation by mail when mail is set up and the public address is
/// known. Returns what happened, for the answer to the person inviting:
/// `queued`, `not_set_up`, `no_public_address` or `failed`. The link is shown
/// to the inviter in every case, so a mail that does not go changes nothing.
pub(crate) async fn mail_invitation(
    state: &AppState,
    inviter_id: &str,
    invitation: &repomemo_domain::Invitation,
    target_name: &str,
    path: &str,
) -> &'static str {
    let Some(link) = state.mail.link(path) else {
        return if state.mail.is_ready().await {
            "no_public_address"
        } else {
            "not_set_up"
        };
    };
    let inviter = match state.storage.find_user(inviter_id).await {
        Ok(Some(user)) => user.display_name,
        _ => "Someone".to_owned(),
    };
    let expires_on = invitation
        .expires_at
        .get(..10)
        .unwrap_or(&invitation.expires_at)
        .to_owned();
    let (subject, body) = invitation_message(
        &inviter,
        &invitation.target_type,
        target_name,
        &invitation.role,
        &link,
        &expires_on,
    );
    match state
        .mail
        .send_to(&invitation.email, &subject, &body, "invitation")
        .await
    {
        Ok(_) => "queued",
        Err(NotQueued::NotSetUp) => "not_set_up",
        Err(_) => "failed",
    }
}

// ---------------------------------------------------------------------------
// System › Settings › Mail

#[derive(Debug, serde::Deserialize)]
pub(crate) struct MailSettingsRequest {
    enabled: bool,
    host: String,
    port: i64,
    security: String,
    #[serde(default)]
    username: String,
    /// Absent keeps the saved password; an empty string removes it.
    #[serde(default)]
    password: Option<String>,
    from_address: String,
    #[serde(default)]
    from_name: String,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct OutboxQuery {
    limit: Option<i64>,
}

#[derive(Debug, serde::Deserialize, Default)]
pub(crate) struct TestMailRequest {
    /// Where to send it; the caller's own address when absent.
    #[serde(default)]
    to: Option<String>,
}

fn has_control(text: &str) -> bool {
    text.chars().any(char::is_control)
}

/// Checks what an administrator typed. Does no network work.
fn validate(request: &MailSettingsRequest, policy: &OutboundPolicy) -> Result<MailSettingsUpdate, ApiError> {
    let host = request.host.trim().to_ascii_lowercase();
    let port = u16::try_from(request.port)
        .ok()
        .filter(|port| *port > 0)
        .ok_or_else(|| ApiError::bad_request("The port is a number from 1 to 65535."))?;
    if Security::parse(&request.security).is_none() {
        return Err(ApiError::bad_request(
            "Connection security is TLS (from the first byte) or STARTTLS; mail is not sent in plain text.",
        ));
    }
    if !host.is_empty() {
        if host.chars().count() > 253 {
            return Err(ApiError::bad_request("The mail server name is too long."));
        }
        repomemo_mail::check_host(&host, policy).map_err(|error| ApiError::bad_request(error.message))?;
    }
    let from_address = request.from_address.trim().to_owned();
    if !from_address.is_empty() && !valid_address(&from_address) {
        return Err(ApiError::bad_request("The sender is not a valid email address."));
    }
    if request.from_name.chars().count() > 120 || has_control(&request.from_name) {
        return Err(ApiError::bad_request(
            "The sender name is at most 120 characters, without line breaks.",
        ));
    }
    if request.username.chars().count() > 255 || has_control(&request.username) {
        return Err(ApiError::bad_request(
            "The user name is at most 255 characters, without line breaks.",
        ));
    }
    if let Some(password) = &request.password {
        if password.chars().count() > 1024 || has_control(password) {
            return Err(ApiError::bad_request(
                "The password is at most 1024 characters, without line breaks.",
            ));
        }
    }
    if request.enabled && (host.is_empty() || from_address.is_empty()) {
        return Err(ApiError::bad_request(
            "Name the mail server and the sender before switching mail on.",
        ));
    }
    Ok(MailSettingsUpdate {
        enabled: request.enabled,
        host,
        port,
        security: request.security.clone(),
        username: request.username.trim().to_owned(),
        password: request.password.clone(),
        from_address,
        from_name: request.from_name.trim().to_owned(),
    })
}

pub(crate) async fn overview(state: &AppState, limit: i64) -> Result<MailOverview, ApiError> {
    let settings = state.storage.mail_settings_view().await.map_err(map_storage_error)?;
    let counts = state.storage.mail_outbox_counts().await.map_err(map_storage_error)?;
    let entries = state.storage.list_mail_outbox(limit).await.map_err(map_storage_error)?;
    Ok(MailOverview {
        settings,
        queued_count: counts.queued,
        sent_last_day: counts.sent_last_day,
        failed_last_day: counts.failed_last_day,
        entries,
    })
}

/// The mail page: settings (never the password), what the outbox holds and the latest messages.
pub(crate) async fn get_mail(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Query(query): Query<OutboxQuery>,
) -> Result<Json<MailOverview>, ApiError> {
    require_system_admin(&subject)?;
    overview(&state, query.limit.unwrap_or(20)).await.map(Json)
}

/// Saves the mail server. A change to what protects an account's credentials
/// needs a signed-in session, as the run-time settings do.
pub(crate) async fn put_mail(
    SessionSubject(subject): SessionSubject,
    State(state): State<AppState>,
    Json(request): Json<MailSettingsRequest>,
) -> Result<Json<MailOverview>, ApiError> {
    require_full_system_admin(&subject)?;
    let update = validate(&request, state.mail.policy())?;
    let (enabled, host, port, security, from) = (
        update.enabled,
        update.host.clone(),
        update.port,
        update.security.clone(),
        update.from_address.clone(),
    );
    let password_changed = match &update.password {
        None => "kept the password",
        Some(password) if password.is_empty() => "removed the password",
        Some(_) => "set a new password",
    };
    state
        .storage
        .save_mail_settings(update, Some(&subject.user_id))
        .await
        .map_err(map_storage_error)?;
    audit(
        &state,
        &subject,
        "mail_settings_changed",
        format!(
            "Saved the mail settings: {} through {host}:{port} ({security}) from {from}; {password_changed}.",
            if enabled { "on" } else { "off" }
        ),
    )
    .await;
    overview(&state, 20).await.map(Json)
}

/// Queues a test mail to the caller, or to the address given, and answers at
/// once: the result is on the outbox entry.
pub(crate) async fn test_mail(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    body: Option<Json<TestMailRequest>>,
) -> Result<(StatusCode, Json<MailOutboxEntry>), ApiError> {
    require_full_system_admin(&subject)?;
    require_system_change(&subject)?;
    let request = body.map(|Json(request)| request).unwrap_or_default();
    let to = match request.to.map(|to| to.trim().to_owned()).filter(|to| !to.is_empty()) {
        Some(to) => to,
        None => state
            .storage
            .find_user(&subject.user_id)
            .await
            .map_err(map_storage_error)?
            .and_then(|user| user.email)
            .ok_or_else(|| ApiError::bad_request("Your account has no email address; give one to send the test to."))?,
    };
    let entry = send_test(&state, &subject, &to).await?;
    Ok((StatusCode::ACCEPTED, Json(entry)))
}

/// Queues the test mail; shared by the page and the console.
pub(crate) async fn send_test(
    state: &AppState,
    subject: &AuthenticatedSubject,
    to: &str,
) -> Result<MailOutboxEntry, ApiError> {
    let counts = state.storage.mail_outbox_counts().await.map_err(map_storage_error)?;
    if counts.queued > MAX_WAITING_FOR_TEST {
        return Err(ApiError::conflict(format!(
            "{} messages are waiting in the outbox; wait for them before sending a test.",
            counts.queued
        )));
    }
    let body =
        "This is a test message from your RepoMemo server.\n\nIf you can read it, the server can send mail.".to_owned();
    let entry = state
        .mail
        .send_to(to, "RepoMemo test message", &body, "test")
        .await
        .map_err(|refusal| match refusal {
            NotQueued::BadAddress => ApiError::bad_request("That is not a valid email address."),
            NotQueued::NotSetUp => ApiError::conflict(
                "Mail is not set up: save the mail server and the sender, and switch mail on, then try again.",
            ),
            NotQueued::Storage(message) => ApiError::internal(message),
        })?;
    audit(
        state,
        subject,
        "mail_test_requested",
        format!("Asked for a test mail to {to}."),
    )
    .await;
    Ok(entry)
}

/// The latest messages of the outbox, newest first (administrators).
pub(crate) async fn list_outbox(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Query(query): Query<OutboxQuery>,
) -> Result<Json<Vec<MailOutboxEntry>>, ApiError> {
    require_system_admin(&subject)?;
    state
        .storage
        .list_mail_outbox(query.limit.unwrap_or(50))
        .await
        .map(Json)
        .map_err(map_storage_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(host: &str) -> MailSettingsRequest {
        MailSettingsRequest {
            enabled: true,
            host: host.to_owned(),
            port: 587,
            security: "starttls".to_owned(),
            username: "memo".to_owned(),
            password: Some("secret".to_owned()),
            from_address: "memo@example.com".to_owned(),
            from_name: "RepoMemo".to_owned(),
        }
    }

    #[test]
    fn what_an_administrator_types_is_checked_before_it_is_saved() {
        let policy = OutboundPolicy::default();
        assert!(validate(&request("smtp.example.com"), &policy).is_ok());
        for (label, mutate) in [
            (
                "metadata host",
                Box::new(|r: &mut MailSettingsRequest| r.host = "169.254.169.254".to_owned())
                    as Box<dyn Fn(&mut MailSettingsRequest)>,
            ),
            ("private address", Box::new(|r| r.host = "10.1.2.3".to_owned())),
            ("port zero", Box::new(|r| r.port = 0)),
            ("port too high", Box::new(|r| r.port = 70_000)),
            ("plain text", Box::new(|r| r.security = "none".to_owned())),
            (
                "bad sender",
                Box::new(|r| r.from_address = "memo@@example.com".to_owned()),
            ),
            (
                "sender name with a line break",
                Box::new(|r| r.from_name = "A\r\nBcc: x@example.com".to_owned()),
            ),
            ("on without a server", Box::new(|r| r.host = String::new())),
            ("on without a sender", Box::new(|r| r.from_address = String::new())),
            (
                "password with a line break",
                Box::new(|r| r.password = Some("a\nb".to_owned())),
            ),
        ] {
            let mut crafted = request("smtp.example.com");
            mutate(&mut crafted);
            assert!(validate(&crafted, &policy).is_err(), "{label}");
        }
        // Switched off, half-filled settings can still be saved.
        let mut draft = request("");
        draft.enabled = false;
        draft.from_address = String::new();
        assert!(validate(&draft, &policy).is_ok());
        // A host the operator listed may be private.
        let listed = OutboundPolicy {
            allowed_hosts: vec!["10.1.2.3".to_owned()],
            ..OutboundPolicy::default()
        };
        assert!(validate(&request("10.1.2.3"), &listed).is_ok());
    }

    #[test]
    fn an_invitation_names_who_where_and_for_how_long_and_a_crafted_name_adds_nothing() {
        let (subject, body) = invitation_message(
            "Ada Lovelace",
            "workspace",
            "Atlas",
            "member",
            "https://memo.example.com/invite#abc",
            "2026-10-21",
        );
        assert_eq!(subject, "Ada Lovelace invited you to Atlas on RepoMemo");
        assert!(body.contains("https://memo.example.com/invite#abc"));
        assert!(body.contains("expires on 2026-10-21") && body.contains("as member"));
        let (subject, _) = invitation_message(
            "Eve\r\nBcc: boss@example.com",
            "workspace",
            &"Long ".repeat(60),
            "member",
            "https://memo.example.com/invite#abc",
            "2026-10-21",
        );
        assert!(!subject.chars().any(char::is_control), "{subject:?}");
        assert!(subject.chars().count() < 200);
        assert_eq!(plain("a \t b\n c", 20), "a b c");
        assert_eq!(plain("abcdef", 4), "abc…");
    }

    #[test]
    fn retries_follow_the_schedule_and_stop() {
        let temporary = SendError {
            temporary: true,
            message: "busy".to_owned(),
        };
        let permanent = SendError {
            temporary: false,
            message: "no such user".to_owned(),
        };
        assert_eq!(next_attempt(&temporary, 0), Some(Duration::from_secs(60)));
        assert_eq!(next_attempt(&temporary, 1), Some(Duration::from_secs(300)));
        assert_eq!(next_attempt(&temporary, 4), Some(Duration::from_secs(6 * 3600)));
        assert_eq!(next_attempt(&temporary, 5), None, "the sixth failure ends it");
        assert_eq!(next_attempt(&permanent, 0), None, "a refusal is not retried");
    }
}
