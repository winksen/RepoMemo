//! Credentials of remote repository links (roadmap S6a, V0.2.19).
//!
//! A credential is a read-only token (`https`) or a deploy key (`ssh`). It is
//! sealed with the same key as AI provider keys, bound to its link, and only
//! the sync engine ever opens it: nothing in the API returns it, and
//! [`RepoSecret`] cannot be printed by accident.

use super::*;

/// What can be said about a stored credential without opening it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoCredentialInfo {
    /// `token` or `deploy_key`.
    pub kind: String,
    /// The account name that goes with a token, if one was given.
    pub username: Option<String>,
    pub updated_at: String,
}

/// An opened credential, for handing to git and nothing else. `Debug` hides
/// the secret, and there is no `Serialize`.
#[derive(Clone)]
pub struct RepoSecret {
    pub kind: String,
    pub username: Option<String>,
    secret: String,
}

impl RepoSecret {
    /// The secret itself. Call only to pass it to the process that needs it.
    pub fn expose(&self) -> &str {
        &self.secret
    }
}

impl std::fmt::Debug for RepoSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RepoSecret")
            .field("kind", &self.kind)
            .field("username", &self.username)
            .field("secret", &"<hidden>")
            .finish()
    }
}

/// The longest secret kept: a deploy key is a few KiB at most.
const MAX_SECRET_BYTES: usize = 16 * 1024;

fn credential_context(source_id: &str) -> String {
    format!("repo_credentials:{source_id}:secret")
}

fn webhook_context(source_id: &str) -> String {
    format!("repo_webhooks:{source_id}:secret")
}

/// What can be said about a repository's webhook without its secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoWebhookInfo {
    pub created_at: String,
    pub last_received_at: Option<String>,
}

impl StorageEngine {
    /// Saves (or replaces) the credential of a repository link.
    pub async fn set_repo_credential(
        &self,
        source_id: &str,
        kind: &str,
        username: Option<&str>,
        secret: &str,
        actor_user_id: Option<&str>,
    ) -> Result<RepoCredentialInfo> {
        if !matches!(kind, "token" | "deploy_key") {
            repomemo_domain::bail_invalid!("A credential is a token or a deploy key.");
        }
        let secret = secret.trim();
        if secret.is_empty() {
            repomemo_domain::bail_invalid!("Enter the token or key.");
        }
        if secret.len() > MAX_SECRET_BYTES {
            repomemo_domain::bail_invalid!("That token or key is too long.");
        }
        if kind == "token" && secret.chars().any(|c| c.is_control() || c.is_whitespace()) {
            repomemo_domain::bail_invalid!("A token has no spaces or line breaks.");
        }
        if kind == "deploy_key" && !secret.starts_with("-----BEGIN ") {
            repomemo_domain::bail_invalid!("A deploy key is a private key in PEM or OpenSSH form (it starts with -----BEGIN).");
        }
        let username = username.map(str::trim).filter(|name| !name.is_empty());
        if let Some(name) = username {
            if name.len() > 100 || name.chars().any(|c| c.is_control() || c.is_whitespace() || c == ':' || c == '@') {
                repomemo_domain::bail_invalid!("That account name is not valid.");
            }
        }
        let is_repository = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sources WHERE id = ?1 AND type = 'git_repo'")
            .bind(source_id)
            .fetch_one(self.reader())
            .await?;
        if is_repository == 0 {
            repomemo_domain::bail_not_found!("Repository was not found.");
        }
        let sealed = self.secrets.seal(secret, &credential_context(source_id))?;
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"INSERT INTO repo_credentials (source_id, kind, username, sealed_secret, created_by_user_id, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
               ON CONFLICT(source_id) DO UPDATE SET kind = excluded.kind, username = excluded.username,
                 sealed_secret = excluded.sealed_secret, created_by_user_id = excluded.created_by_user_id,
                 updated_at = excluded.updated_at"#,
        )
        .bind(source_id)
        .bind(kind)
        .bind(username)
        .bind(sealed)
        .bind(actor_user_id)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(RepoCredentialInfo {
            kind: kind.to_owned(),
            username: username.map(str::to_owned),
            updated_at: now,
        })
    }

    /// What is stored for a link, without the secret.
    pub async fn repo_credential_info(&self, source_id: &str) -> Result<Option<RepoCredentialInfo>> {
        let row = sqlx::query_as::<_, (String, Option<String>, String)>(
            "SELECT kind, username, updated_at FROM repo_credentials WHERE source_id = ?1",
        )
        .bind(source_id)
        .fetch_optional(self.reader())
        .await?;
        Ok(row.map(|(kind, username, updated_at)| RepoCredentialInfo {
            kind,
            username,
            updated_at,
        }))
    }

    /// Opens the credential of a link, for the sync engine to hand to git. A
    /// credential that no longer opens (the secret key changed) is reported in
    /// the log without its value and treated as missing, so it can be entered
    /// again.
    pub async fn open_repo_credential(&self, source_id: &str) -> Result<Option<RepoSecret>> {
        let row = sqlx::query_as::<_, (String, Option<String>, String)>(
            "SELECT kind, username, sealed_secret FROM repo_credentials WHERE source_id = ?1",
        )
        .bind(source_id)
        .fetch_optional(self.reader())
        .await?;
        let Some((kind, username, sealed)) = row else {
            return Ok(None);
        };
        match self.secrets.open(&sealed, &credential_context(source_id)) {
            Ok(secret) => Ok(Some(RepoSecret { kind, username, secret })),
            Err(error) => {
                tracing::warn!(repository = %source_id, error = %error, "A repository credential could not be decrypted");
                Ok(None)
            }
        }
    }

    /// Makes (or replaces) the secret that signs a repository's push
    /// webhooks and returns it. This is the only time it is shown.
    pub async fn create_repo_webhook(&self, source_id: &str, actor_user_id: Option<&str>) -> Result<String> {
        use ring::rand::{SecureRandom, SystemRandom};
        let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sources WHERE id = ?1 AND type = 'git_repo'")
            .bind(source_id)
            .fetch_one(self.reader())
            .await?;
        if exists == 0 {
            repomemo_domain::bail_not_found!("Repository was not found.");
        }
        let mut bytes = [0_u8; 32];
        SystemRandom::new()
            .fill(&mut bytes)
            .map_err(|_| anyhow::anyhow!("a webhook secret could not be generated"))?;
        let secret = hex::encode(bytes);
        let sealed = self.secrets.seal(&secret, &webhook_context(source_id))?;
        sqlx::query(
            r#"INSERT INTO repo_webhooks (source_id, sealed_secret, created_by_user_id, created_at, last_received_at)
               VALUES (?1, ?2, ?3, ?4, NULL)
               ON CONFLICT(source_id) DO UPDATE SET sealed_secret = excluded.sealed_secret,
                 created_by_user_id = excluded.created_by_user_id, created_at = excluded.created_at,
                 last_received_at = NULL"#,
        )
        .bind(source_id)
        .bind(sealed)
        .bind(actor_user_id)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(secret)
    }

    /// The secret to check a webhook's signature with.
    pub async fn open_repo_webhook_secret(&self, source_id: &str) -> Result<Option<String>> {
        let sealed = sqlx::query_scalar::<_, String>("SELECT sealed_secret FROM repo_webhooks WHERE source_id = ?1")
            .bind(source_id)
            .fetch_optional(self.reader())
            .await?;
        Ok(match sealed {
            Some(sealed) => match self.secrets.open(&sealed, &webhook_context(source_id)) {
                Ok(secret) => Some(secret),
                Err(error) => {
                    tracing::warn!(repository = %source_id, error = %error, "A webhook secret could not be decrypted");
                    None
                }
            },
            None => None,
        })
    }

    pub async fn repo_webhook_info(&self, source_id: &str) -> Result<Option<RepoWebhookInfo>> {
        let row = sqlx::query_as::<_, (String, Option<String>)>(
            "SELECT created_at, last_received_at FROM repo_webhooks WHERE source_id = ?1",
        )
        .bind(source_id)
        .fetch_optional(self.reader())
        .await?;
        Ok(row.map(|(created_at, last_received_at)| RepoWebhookInfo {
            created_at,
            last_received_at,
        }))
    }

    /// Notes that a correctly signed call arrived.
    pub async fn touch_repo_webhook(&self, source_id: &str) -> Result<()> {
        sqlx::query("UPDATE repo_webhooks SET last_received_at = ?1 WHERE source_id = ?2")
            .bind(Utc::now().to_rfc3339())
            .bind(source_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Removes a repository's webhook secret. Returns whether there was one.
    pub async fn clear_repo_webhook(&self, source_id: &str) -> Result<bool> {
        Ok(sqlx::query("DELETE FROM repo_webhooks WHERE source_id = ?1")
            .bind(source_id)
            .execute(&self.pool)
            .await?
            .rows_affected()
            > 0)
    }

    /// Removes a link's credential. Returns whether there was one.
    pub async fn clear_repo_credential(&self, source_id: &str) -> Result<bool> {
        Ok(sqlx::query("DELETE FROM repo_credentials WHERE source_id = ?1")
            .bind(source_id)
            .execute(&self.pool)
            .await?
            .rows_affected()
            > 0)
    }
}

#[cfg(test)]
mod tests {
    use crate::{StorageConfig, StorageEngine};

    #[tokio::test]
    async fn a_credential_is_sealed_never_shown_and_dies_with_its_link() {
        let data_dir = std::env::temp_dir().join(format!("repomemo-credentials-{}", uuid::Uuid::new_v4()));
        let storage = StorageEngine::open(StorageConfig::new(data_dir.clone())).await.unwrap();
        let owner = storage.create_user("cred@example.com", "Cred", "hash").await.unwrap();
        let organization = storage.create_organization(&owner.id, "Team").await.unwrap();
        let workspace = storage
            .create_shared_workspace(&owner.id, &organization.id, "Ledger")
            .await
            .unwrap();
        let source_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO sources (id, workspace_id, type, name, root_uri, created_at, updated_at) VALUES (?1, ?2, 'git_repo', 'app', 'https://github.com/acme/app', 'now', 'now')")
            .bind(&source_id)
            .bind(&workspace.workspace.id)
            .execute(&storage.pool)
            .await
            .unwrap();

        assert!(storage.set_repo_credential(&source_id, "token", None, "  ", None).await.is_err());
        assert!(storage.set_repo_credential(&source_id, "token", None, "has a space", None).await.is_err());
        assert!(storage.set_repo_credential(&source_id, "deploy_key", None, "not a key", None).await.is_err());
        assert!(storage.set_repo_credential("missing", "token", None, "ghp_secretvalue", None).await.is_err());

        let info = storage
            .set_repo_credential(&source_id, "token", Some("x-access-token"), "ghp_secretvalue", Some(&owner.id))
            .await
            .unwrap();
        assert_eq!((info.kind.as_str(), info.username.as_deref()), ("token", Some("x-access-token")));
        let stored: String = sqlx::query_scalar("SELECT sealed_secret FROM repo_credentials WHERE source_id = ?1")
            .bind(&source_id)
            .fetch_one(&storage.pool)
            .await
            .unwrap();
        assert!(!stored.contains("ghp_secretvalue"), "the secret is stored sealed");
        let opened = storage.open_repo_credential(&source_id).await.unwrap().unwrap();
        assert_eq!(opened.expose(), "ghp_secretvalue");
        assert!(!format!("{opened:?}").contains("ghp_secretvalue"), "it cannot be printed by accident");
        assert_eq!(
            storage.repo_credential_info(&source_id).await.unwrap().unwrap().kind,
            "token"
        );

        // A sealed value copied onto another link does not open there.
        let other_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO sources (id, workspace_id, type, name, root_uri, created_at, updated_at) VALUES (?1, ?2, 'git_repo', 'other', 'https://github.com/acme/other', 'now', 'now')")
            .bind(&other_id)
            .bind(&workspace.workspace.id)
            .execute(&storage.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO repo_credentials (source_id, kind, username, sealed_secret, created_at, updated_at) VALUES (?1, 'token', NULL, ?2, 'now', 'now')")
            .bind(&other_id)
            .bind(&stored)
            .execute(&storage.pool)
            .await
            .unwrap();
        assert!(storage.open_repo_credential(&other_id).await.unwrap().is_none());

        assert!(storage.clear_repo_credential(&source_id).await.unwrap());
        assert!(storage.open_repo_credential(&source_id).await.unwrap().is_none());

        // A webhook secret is made once, stored sealed, and dies with its link.
        assert!(storage.create_repo_webhook("missing", None).await.is_err());
        let secret = storage.create_repo_webhook(&source_id, Some(&owner.id)).await.unwrap();
        assert_eq!(secret.len(), 64);
        let stored: String = sqlx::query_scalar("SELECT sealed_secret FROM repo_webhooks WHERE source_id = ?1")
            .bind(&source_id)
            .fetch_one(&storage.pool)
            .await
            .unwrap();
        assert!(!stored.contains(&secret));
        assert_eq!(storage.open_repo_webhook_secret(&source_id).await.unwrap().as_deref(), Some(secret.as_str()));
        assert!(storage.repo_webhook_info(&source_id).await.unwrap().unwrap().last_received_at.is_none());
        storage.touch_repo_webhook(&source_id).await.unwrap();
        assert!(storage.repo_webhook_info(&source_id).await.unwrap().unwrap().last_received_at.is_some());
        let renewed = storage.create_repo_webhook(&source_id, None).await.unwrap();
        assert_ne!(renewed, secret);
        assert!(storage.repo_webhook_info(&source_id).await.unwrap().unwrap().last_received_at.is_none());
        assert!(storage.clear_repo_webhook(&source_id).await.unwrap());
        assert!(storage.open_repo_webhook_secret(&source_id).await.unwrap().is_none());
        storage.pool.close().await;
        let _ = std::fs::remove_dir_all(data_dir);
    }
}
