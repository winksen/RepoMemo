use super::*;

/// Public configuration only. Credentials are sealed separately and never serialized.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default, PartialEq)]
pub struct IdentityProvider {
    pub name: String,
    pub issuer: String,
    pub client_id: String,
    pub enabled: bool,
    #[serde(default)]
    pub allowed_domains: Vec<String>,
    #[serde(default)]
    pub authorization_endpoint: String,
    #[serde(default)]
    pub token_endpoint: String,
    #[serde(default)]
    pub jwks_uri: String,
    #[serde(default)]
    pub end_session_endpoint: Option<String>,
    #[serde(default)]
    pub checked_at: Option<String>,
}

impl StorageEngine {
    pub async fn identity_provider(&self) -> Result<Option<IdentityProvider>> {
        let row: Option<String> = sqlx::query_scalar("SELECT configuration FROM identity_provider WHERE id=1")
            .fetch_optional(self.reader())
            .await?;
        row.map(|s| serde_json::from_str(&s).map_err(Into::into)).transpose()
    }

    pub async fn identity_secret(&self) -> Result<Option<String>> {
        let row: Option<String> = sqlx::query_scalar("SELECT client_secret FROM identity_provider WHERE id=1")
            .fetch_optional(self.reader())
            .await?;
        row.map(|s| self.secrets.open(&s, "identity_provider:1:client_secret"))
            .transpose()
    }

    pub async fn save_identity_provider(&self, provider: &IdentityProvider, secret: &str, actor: &str) -> Result<()> {
        let sealed = self.secrets.seal(secret, "identity_provider:1:client_secret")?;
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO identity_provider (id,configuration,client_secret,updated_at) VALUES (1,?1,?2,?3) ON CONFLICT(id) DO UPDATE SET configuration=excluded.configuration,client_secret=excluded.client_secret,updated_at=excluded.updated_at")
            .bind(serde_json::to_string(provider)?).bind(sealed).bind(Utc::now().to_rfc3339())
            .execute(&mut *tx).await?;
        sqlx::query("INSERT INTO system_audit_events (id,actor_user_id,action,detail,created_at) VALUES (?1,?2,'identity_provider_updated','Identity provider configuration changed',?3)")
            .bind(Uuid::new_v4().to_string()).bind(actor).bind(Utc::now().to_rfc3339())
            .execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn identity_secret_is_sealed_and_configuration_survives_reopen() {
        let dir = std::env::temp_dir().join(format!("repomemo-identity-{}", Uuid::new_v4()));
        let storage = StorageEngine::open(StorageConfig::new(dir.clone())).await.unwrap();
        let user = storage
            .create_user("admin@example.com", "Admin", "unused")
            .await
            .unwrap();
        let provider = IdentityProvider {
            issuer: "https://id.example.com".into(),
            ..Default::default()
        };
        storage
            .save_identity_provider(&provider, "secret-to-seal", &user.id)
            .await
            .unwrap();
        let stored: String = sqlx::query_scalar("SELECT client_secret FROM identity_provider")
            .fetch_one(storage.reader())
            .await
            .unwrap();
        assert!(SecretBox::is_sealed(&stored));
        assert!(!stored.contains("secret-to-seal"));
        assert_eq!(
            storage.identity_secret().await.unwrap().as_deref(),
            Some("secret-to-seal")
        );
        assert_eq!(storage.identity_provider().await.unwrap(), Some(provider));
    }
}
