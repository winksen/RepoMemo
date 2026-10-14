use super::*;

impl StorageEngine {
    pub async fn list_provider_settings(&self, workspace_id: &str) -> Result<Vec<ProviderSettings>> {
        let rows = sqlx::query_as::<_, ProviderSettingsRow>(
            r#"
            SELECT id, workspace_id, provider_type, name, base_url, model,
                   embedding_model, enabled, metadata_json
            FROM provider_settings
            WHERE workspace_id = ?1
            ORDER BY enabled DESC, updated_at DESC, name
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| self.provider_settings_from_row(row))
            .collect())
    }

    pub async fn get_provider_settings(&self, provider_id: &str) -> Result<ProviderSettings> {
        let row = sqlx::query_as::<_, ProviderSettingsRow>(
            r#"
            SELECT id, workspace_id, provider_type, name, base_url, model,
                   embedding_model, enabled, metadata_json
            FROM provider_settings
            WHERE id = ?1
            "#,
        )
        .bind(provider_id)
        .fetch_optional(self.reader())
        .await?
        .ok_or_else(|| repomemo_domain::not_found!("provider settings were not found"))?;
        Ok(self.provider_settings_from_row(row))
    }

    /// Saves a provider. The API key is encrypted before it is stored. When
    /// `settings.api_key` is empty the stored key is kept, but only while the
    /// provider keeps pointing at the same service: a key is never carried over
    /// to another provider type or base URL, so changing where requests go
    /// cannot send the stored key to a new address.
    pub async fn save_provider_settings(&self, settings: ProviderSettings) -> Result<ProviderSettings> {
        let id = if settings.id.trim().is_empty() {
            Uuid::new_v4().to_string()
        } else {
            settings.id.clone()
        };
        let now = Utc::now().to_rfc3339();
        let mut metadata = settings.metadata.clone();
        if !metadata.is_object() {
            metadata = Value::Object(Default::default());
        }
        if let Some(object) = metadata.as_object_mut() {
            object.remove(PROVIDER_API_KEY_FIELD);
            object.remove(PROVIDER_SEALED_API_KEY_FIELD);
        }
        let api_key = match settings.api_key.clone().filter(|value| !value.trim().is_empty()) {
            Some(api_key) => Some(api_key),
            None => match self.get_provider_settings(&id).await {
                Ok(existing) if same_provider_endpoint(&existing, &settings) => existing.api_key,
                _ => None,
            },
        };
        if let Some(api_key) = api_key {
            metadata[PROVIDER_SEALED_API_KEY_FIELD] =
                Value::String(self.secrets.seal(api_key.trim(), &provider_secret_context(&id))?);
        }
        let metadata_json = serde_json::to_string(&metadata)?;
        sqlx::query(
            r#"
            INSERT INTO provider_settings (
              id, workspace_id, provider_type, name, base_url, model, embedding_model,
              enabled, metadata_json, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)
            ON CONFLICT(id) DO UPDATE SET
              workspace_id = excluded.workspace_id, provider_type = excluded.provider_type,
              name = excluded.name, base_url = excluded.base_url, model = excluded.model,
              embedding_model = excluded.embedding_model, enabled = excluded.enabled,
              metadata_json = excluded.metadata_json, updated_at = excluded.updated_at
            "#,
        )
        .bind(&id)
        .bind(&settings.workspace_id)
        .bind(&settings.provider_type)
        .bind(&settings.name)
        .bind(&settings.base_url)
        .bind(&settings.model)
        .bind(&settings.embedding_model)
        .bind(settings.enabled)
        .bind(metadata_json)
        .bind(now)
        .execute(&self.pool)
        .await?;
        self.get_provider_settings(&id).await
    }

    /// Rebuilds a provider from its row, decrypting its API key. A key that no
    /// longer decrypts (the secret key changed) is reported once in the log and
    /// treated as missing, so an administrator can enter it again.
    pub(crate) fn provider_settings_from_row(&self, row: ProviderSettingsRow) -> ProviderSettings {
        let mut metadata = serde_json::from_str::<Value>(&row.metadata_json)
            .ok()
            .filter(Value::is_object)
            .unwrap_or_else(|| Value::Object(Default::default()));
        let object = metadata.as_object_mut().expect("metadata is an object");
        let sealed = object
            .remove(PROVIDER_SEALED_API_KEY_FIELD)
            .and_then(|value| value.as_str().map(str::to_owned));
        let legacy = object
            .remove(PROVIDER_API_KEY_FIELD)
            .and_then(|value| value.as_str().map(str::to_owned));
        let api_key = match sealed {
            Some(sealed) => match self.secrets.open(&sealed, &provider_secret_context(&row.id)) {
                Ok(api_key) => Some(api_key),
                Err(error) => {
                    tracing::warn!(provider_id = %row.id, error = %error, "An AI provider API key could not be decrypted");
                    None
                }
            },
            None => legacy,
        };
        ProviderSettings {
            id: row.id,
            workspace_id: row.workspace_id,
            provider_type: row.provider_type,
            name: row.name,
            base_url: row.base_url,
            model: row.model,
            embedding_model: row.embedding_model,
            enabled: row.enabled,
            metadata,
            api_key,
        }
    }

    /// Encrypts API keys saved before keys were encrypted at rest. Runs at
    /// startup; rows that are already sealed are left alone.
    pub(crate) async fn seal_plaintext_provider_keys(&self) -> Result<u64> {
        let rows = sqlx::query_as::<_, (String, String)>(
            "SELECT id, metadata_json FROM provider_settings WHERE metadata_json LIKE '%\"api_key\"%'",
        )
        .fetch_all(self.reader())
        .await?;
        let mut sealed = 0;
        for (id, metadata_json) in rows {
            let Ok(mut metadata) = serde_json::from_str::<Value>(&metadata_json) else {
                continue;
            };
            let Some(object) = metadata.as_object_mut() else {
                continue;
            };
            let Some(plaintext) = object
                .remove(PROVIDER_API_KEY_FIELD)
                .and_then(|value| value.as_str().map(str::to_owned))
            else {
                continue;
            };
            if !plaintext.trim().is_empty() && !object.contains_key(PROVIDER_SEALED_API_KEY_FIELD) {
                object.insert(
                    PROVIDER_SEALED_API_KEY_FIELD.to_owned(),
                    Value::String(self.secrets.seal(plaintext.trim(), &provider_secret_context(&id))?),
                );
            }
            sqlx::query("UPDATE provider_settings SET metadata_json = ?1 WHERE id = ?2")
                .bind(serde_json::to_string(&metadata)?)
                .bind(&id)
                .execute(&self.pool)
                .await?;
            sealed += 1;
        }
        Ok(sealed)
    }
}
