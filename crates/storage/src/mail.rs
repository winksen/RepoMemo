//! Outgoing mail: the SMTP settings (password sealed) and the outbox.

use super::*;
use repomemo_domain::{MailOutboxEntry, MailSettingsView};

/// The settings with the password opened, for the sender. Never serialised.
#[derive(Clone)]
pub struct StoredMailSettings {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub security: String,
    pub username: String,
    pub password: Option<String>,
    pub from_address: String,
    pub from_name: String,
}

impl std::fmt::Debug for StoredMailSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoredMailSettings")
            .field("enabled", &self.enabled)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("password", &self.password.as_ref().map(|_| "…"))
            .finish_non_exhaustive()
    }
}

/// What an administrator may change. `password`: `None` keeps the saved one,
/// `Some("")` removes it.
#[derive(Debug, Clone)]
pub struct MailSettingsUpdate {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub security: String,
    pub username: String,
    pub password: Option<String>,
    pub from_address: String,
    pub from_name: String,
}

/// A message the sender claimed, with its body.
#[derive(Debug, Clone)]
pub struct QueuedMail {
    pub id: String,
    pub to_address: String,
    pub subject: String,
    pub body: String,
    pub kind: String,
    /// Attempts made before this one.
    pub attempts: i64,
}

/// Where the outbox stands.
#[derive(Debug, Clone, Copy, Default)]
pub struct MailOutboxCounts {
    pub queued: i64,
    pub sent_last_day: i64,
    pub failed_last_day: i64,
}

/// A person who asked for the daily digest and is due one.
#[derive(Debug, Clone)]
pub struct DigestCandidate {
    pub user_id: String,
    pub email: String,
    pub display_name: String,
    /// Notices created after this are the digest's; the last digest, or a day ago.
    pub since: String,
}

/// One unread notice, as the digest lists it.
#[derive(Debug, Clone)]
pub struct DigestNotice {
    pub title: String,
    pub body: String,
}

const MAIL_SECRET_CONTEXT: &str = "mail-settings:smtp-password";

fn entry_from_row(row: &sqlx::sqlite::SqliteRow) -> MailOutboxEntry {
    MailOutboxEntry {
        id: row.get("id"),
        to_address: row.get("to_address"),
        subject: row.get("subject"),
        kind: row.get("kind"),
        status: row.get("status"),
        attempts: row.get("attempts"),
        last_error: row.get("last_error"),
        created_at: row.get("created_at"),
        next_attempt_at: row.get("next_attempt_at"),
        sent_at: row.get("sent_at"),
    }
}

impl StorageEngine {
    /// The saved settings, password opened; `None` before anything was saved.
    pub async fn mail_settings(&self) -> Result<Option<StoredMailSettings>> {
        let row = sqlx::query(
            "SELECT enabled, host, port, security, username, password_sealed, from_address, from_name FROM mail_settings WHERE id = 1",
        )
        .fetch_optional(self.reader())
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let password = match row.get::<Option<String>, _>("password_sealed") {
            Some(sealed) if !sealed.is_empty() => match self.secrets.open(&sealed, MAIL_SECRET_CONTEXT) {
                Ok(password) => Some(password),
                Err(error) => {
                    tracing::warn!(error = %error, "The saved mail password could not be opened; save it again");
                    None
                }
            },
            _ => None,
        };
        Ok(Some(StoredMailSettings {
            enabled: row.get::<i64, _>("enabled") != 0,
            host: row.get("host"),
            port: row.get::<i64, _>("port").clamp(1, 65_535) as u16,
            security: row.get("security"),
            username: row.get("username"),
            password,
            from_address: row.get("from_address"),
            from_name: row.get("from_name"),
        }))
    }

    /// The settings as administrators see them.
    pub async fn mail_settings_view(&self) -> Result<MailSettingsView> {
        let stored = self.mail_settings().await?;
        let updated_at = sqlx::query_scalar::<_, String>("SELECT updated_at FROM mail_settings WHERE id = 1")
            .fetch_optional(self.reader())
            .await?;
        Ok(match stored {
            Some(settings) => MailSettingsView {
                configured: !settings.host.is_empty() && !settings.from_address.is_empty(),
                enabled: settings.enabled,
                host: settings.host,
                port: i64::from(settings.port),
                security: settings.security,
                username: settings.username,
                password_set: settings.password.is_some(),
                from_address: settings.from_address,
                from_name: settings.from_name,
                updated_at,
            },
            None => MailSettingsView {
                configured: false,
                enabled: false,
                host: String::new(),
                port: 587,
                security: "starttls".to_owned(),
                username: String::new(),
                password_set: false,
                from_address: String::new(),
                from_name: String::new(),
                updated_at: None,
            },
        })
    }

    pub async fn save_mail_settings(&self, update: MailSettingsUpdate, actor_user_id: Option<&str>) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let current = sqlx::query_scalar::<_, Option<String>>("SELECT password_sealed FROM mail_settings WHERE id = 1")
            .fetch_optional(&self.pool)
            .await?
            .flatten();
        let sealed = match update.password.as_deref() {
            None => current,
            Some("") => None,
            Some(password) => Some(self.secrets.seal(password, MAIL_SECRET_CONTEXT)?),
        };
        sqlx::query(
            r#"INSERT INTO mail_settings
                 (id, enabled, host, port, security, username, password_sealed, from_address, from_name, updated_at, updated_by_user_id)
               VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
               ON CONFLICT(id) DO UPDATE SET enabled = excluded.enabled, host = excluded.host, port = excluded.port,
                 security = excluded.security, username = excluded.username, password_sealed = excluded.password_sealed,
                 from_address = excluded.from_address, from_name = excluded.from_name,
                 updated_at = excluded.updated_at, updated_by_user_id = excluded.updated_by_user_id"#,
        )
        .bind(update.enabled)
        .bind(&update.host)
        .bind(i64::from(update.port))
        .bind(&update.security)
        .bind(&update.username)
        .bind(sealed)
        .bind(&update.from_address)
        .bind(&update.from_name)
        .bind(&now)
        .bind(actor_user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Puts a message in the outbox, due at once.
    pub async fn enqueue_mail(
        &self,
        to_address: &str,
        subject: &str,
        body: &str,
        kind: &str,
    ) -> Result<MailOutboxEntry> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"INSERT INTO mail_outbox (id, to_address, subject, body, kind, status, attempts, created_at, next_attempt_at)
               VALUES (?1, ?2, ?3, ?4, ?5, 'queued', 0, ?6, ?6)"#,
        )
        .bind(&id)
        .bind(to_address)
        .bind(subject)
        .bind(body)
        .bind(kind)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.mail_entry(&id).await
    }

    pub async fn mail_entry(&self, id: &str) -> Result<MailOutboxEntry> {
        let row = sqlx::query(
            "SELECT id, to_address, subject, kind, status, attempts, last_error, created_at, next_attempt_at, sent_at FROM mail_outbox WHERE id = ?1",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;
        Ok(entry_from_row(&row))
    }

    /// Takes the messages that are due (oldest first) and marks them sending,
    /// so no other pass takes them too.
    pub async fn claim_due_mail(&self, limit: i64) -> Result<Vec<QueuedMail>> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        let rows = sqlx::query(
            r#"SELECT id, to_address, subject, body, kind, attempts FROM mail_outbox
               WHERE status = 'queued' AND (next_attempt_at IS NULL OR next_attempt_at <= ?1)
               ORDER BY created_at LIMIT ?2"#,
        )
        .bind(&now)
        .bind(limit)
        .fetch_all(&mut *tx)
        .await?;
        let mut claimed = Vec::with_capacity(rows.len());
        for row in rows {
            let id = row.get::<String, _>("id");
            sqlx::query("UPDATE mail_outbox SET status = 'sending' WHERE id = ?1")
                .bind(&id)
                .execute(&mut *tx)
                .await?;
            claimed.push(QueuedMail {
                id,
                to_address: row.get("to_address"),
                subject: row.get("subject"),
                body: row.get("body"),
                kind: row.get("kind"),
                attempts: row.get("attempts"),
            });
        }
        tx.commit().await?;
        Ok(claimed)
    }

    /// The message was accepted by the mail server. Its body is dropped.
    pub async fn mark_mail_sent(&self, id: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE mail_outbox SET status = 'sent', body = '', attempts = attempts + 1, last_error = NULL, next_attempt_at = NULL, sent_at = ?2 WHERE id = ?1",
        )
        .bind(id)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// The attempt failed and a later one is planned.
    pub async fn retry_mail_later(&self, id: &str, error: &str, next_attempt_at: &str) -> Result<()> {
        sqlx::query(
            "UPDATE mail_outbox SET status = 'queued', attempts = attempts + 1, last_error = ?2, next_attempt_at = ?3 WHERE id = ?1",
        )
        .bind(id)
        .bind(error)
        .bind(next_attempt_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Gives up on a message. Its body is dropped.
    pub async fn mark_mail_failed(&self, id: &str, error: &str) -> Result<()> {
        sqlx::query(
            "UPDATE mail_outbox SET status = 'failed', body = '', attempts = attempts + 1, last_error = ?2, next_attempt_at = NULL WHERE id = ?1",
        )
        .bind(id)
        .bind(error)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// After a restart: messages a stopped process was sending are due again.
    pub async fn requeue_interrupted_mail(&self) -> Result<u64> {
        let now = Utc::now().to_rfc3339();
        Ok(
            sqlx::query("UPDATE mail_outbox SET status = 'queued', next_attempt_at = ?1 WHERE status = 'sending'")
                .bind(now)
                .execute(&self.pool)
                .await?
                .rows_affected(),
        )
    }

    /// When the next queued message is due, if any.
    pub async fn next_mail_due(&self) -> Result<Option<String>> {
        Ok(sqlx::query_scalar::<_, Option<String>>(
            "SELECT MIN(next_attempt_at) FROM mail_outbox WHERE status = 'queued'",
        )
        .fetch_one(self.reader())
        .await?)
    }

    pub async fn list_mail_outbox(&self, limit: i64) -> Result<Vec<MailOutboxEntry>> {
        let rows = sqlx::query(
            "SELECT id, to_address, subject, kind, status, attempts, last_error, created_at, next_attempt_at, sent_at FROM mail_outbox ORDER BY created_at DESC LIMIT ?1",
        )
        .bind(limit.clamp(1, 200))
        .fetch_all(self.reader())
        .await?;
        Ok(rows.iter().map(entry_from_row).collect())
    }

    pub async fn mail_outbox_counts(&self) -> Result<MailOutboxCounts> {
        let since = (Utc::now() - chrono::Duration::hours(24)).to_rfc3339();
        let row = sqlx::query(
            r#"SELECT
                 COALESCE(SUM(CASE WHEN status IN ('queued', 'sending') THEN 1 ELSE 0 END), 0) AS queued,
                 COALESCE(SUM(CASE WHEN status = 'sent' AND sent_at >= ?1 THEN 1 ELSE 0 END), 0) AS sent,
                 COALESCE(SUM(CASE WHEN status = 'failed' AND created_at >= ?1 THEN 1 ELSE 0 END), 0) AS failed
               FROM mail_outbox"#,
        )
        .bind(since)
        .fetch_one(self.reader())
        .await?;
        Ok(MailOutboxCounts {
            queued: row.get("queued"),
            sent_last_day: row.get("sent"),
            failed_last_day: row.get("failed"),
        })
    }

    /// People who asked for the digest, whose address is confirmed and who were
    /// not sent one in the last day.
    pub async fn digest_candidates(&self) -> Result<Vec<DigestCandidate>> {
        let day_ago = (Utc::now() - chrono::Duration::hours(24)).to_rfc3339();
        let rows = sqlx::query(
            r#"SELECT u.id, u.email, u.display_name, COALESCE(d.last_sent_at, ?1) AS since
               FROM notification_preferences p
               JOIN users u ON u.id = p.user_id
               LEFT JOIN mail_digests d ON d.user_id = u.id
               WHERE p.kind = 'daily_digest' AND p.channel = 'mail'
                 AND u.email_unverified_at IS NULL AND u.deactivated_at IS NULL
                 AND (d.last_sent_at IS NULL OR d.last_sent_at <= ?1)"#,
        )
        .bind(day_ago)
        .fetch_all(self.reader())
        .await?;
        Ok(rows
            .iter()
            .map(|row| DigestCandidate {
                user_id: row.get("id"),
                email: row.get("email"),
                display_name: row.get("display_name"),
                since: row.get("since"),
            })
            .collect())
    }

    /// The notices a person has not read that came after `since`, newest first.
    pub async fn unread_notices_since(
        &self,
        user_id: &str,
        since: &str,
        limit: i64,
    ) -> Result<(i64, Vec<DigestNotice>)> {
        let total = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM workspace_notifications WHERE user_id = ?1 AND read_at IS NULL AND created_at > ?2",
        )
        .bind(user_id)
        .bind(since)
        .fetch_one(self.reader())
        .await?;
        let rows = sqlx::query(
            r#"SELECT title, body FROM workspace_notifications
               WHERE user_id = ?1 AND read_at IS NULL AND created_at > ?2
               ORDER BY created_at DESC LIMIT ?3"#,
        )
        .bind(user_id)
        .bind(since)
        .bind(limit)
        .fetch_all(self.reader())
        .await?;
        Ok((
            total,
            rows.iter()
                .map(|row| DigestNotice {
                    title: row.get("title"),
                    body: row.get("body"),
                })
                .collect(),
        ))
    }

    pub async fn record_digest_sent(&self, user_id: &str) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO mail_digests (user_id, last_sent_at) VALUES (?1, ?2)
               ON CONFLICT(user_id) DO UPDATE SET last_sent_at = excluded.last_sent_at"#,
        )
        .bind(user_id)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Removes finished messages older than `older_than_days`.
    pub async fn prune_mail_outbox(&self, older_than_days: i64) -> Result<u64> {
        let cutoff = (Utc::now() - chrono::Duration::days(older_than_days)).to_rfc3339();
        Ok(
            sqlx::query("DELETE FROM mail_outbox WHERE status IN ('sent', 'failed') AND created_at < ?1")
                .bind(cutoff)
                .execute(&self.pool)
                .await?
                .rows_affected(),
        )
    }
}
