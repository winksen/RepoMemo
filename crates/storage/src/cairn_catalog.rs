//! The model catalog (V0.9.75): connections to model servers and the models they offer, kept once for the whole server. A
//! connection holds the address and the encrypted key; a model holds what CAIRN needs to choose it. Every change is checked against
//! the version the caller read, so two administrators cannot overwrite each other unseen. The key is write-only: no read returns it.

use serde::Serialize;
use serde_json::Value;

use super::*;

/// The connection kinds the catalog holds, as providers name them.
pub const CONNECTION_KINDS: &[&str] = &["ollama", "openrouter", "openai_compatible"];
/// What a model is for.
pub const MODEL_PURPOSES: &[&str] = &["text", "vision", "embedding"];
/// How a model takes tools, as the model traits name them.
pub const TOOL_MODES: &[&str] = &["none", "json", "native"];
/// The longest name of a connection or a model.
pub const CATALOG_NAME_CHARS: usize = 80;

/// A connection to a model server. The key is never part of it, only whether there is one.
#[derive(Debug, Clone, Serialize)]
pub struct CairnConnection {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub base_url: String,
    pub has_key: bool,
    /// `local` or `cloud`: where the model server runs, so what may be sent to it.
    pub locality: String,
    pub enabled: bool,
    pub cloud_acknowledged_at: Option<String>,
    pub cloud_acknowledged_by: Option<String>,
    pub version: i64,
    pub created_at: String,
    pub updated_at: String,
    pub updated_by: Option<String>,
}

/// A connection to make.
pub struct NewCairnConnection {
    pub kind: String,
    pub name: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub locality: String,
    pub enabled: bool,
}

/// What an edit of a connection sets. `api_key` is `None` to keep the stored key, which is kept only while the connection keeps
/// pointing at the same kind and address; an empty text removes it.
pub struct CairnConnectionChange {
    pub name: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub locality: String,
    pub enabled: bool,
}

/// A model of a connection.
#[derive(Debug, Clone, Serialize)]
pub struct CairnModel {
    pub id: String,
    pub connection_id: String,
    /// The name the model server knows it by.
    pub model_id: String,
    pub display_name: String,
    pub purpose: String,
    pub tool_mode: String,
    pub context_tokens: i64,
    pub vision: bool,
    /// The newest live evaluation report attached to the model, if any.
    pub evaluation: Option<Value>,
    pub fallback_model_id: Option<String>,
    /// The roles allowed to use the model; `None` leaves it to everyone who may use AI.
    pub roles: Option<Vec<String>>,
    pub enabled: bool,
    pub version: i64,
    pub created_at: String,
    pub updated_at: String,
    pub updated_by: Option<String>,
}

/// A model to add.
pub struct NewCairnModel {
    pub connection_id: String,
    pub model_id: String,
    pub display_name: String,
    pub purpose: String,
    pub tool_mode: String,
    pub context_tokens: i64,
    pub vision: bool,
    pub evaluation: Option<Value>,
    pub fallback_model_id: Option<String>,
    pub roles: Option<Vec<String>>,
    pub enabled: bool,
}

/// What an edit of a model sets: everything but where it comes from.
pub struct CairnModelChange {
    pub display_name: String,
    pub tool_mode: String,
    pub context_tokens: i64,
    pub vision: bool,
    pub evaluation: Option<Value>,
    pub fallback_model_id: Option<String>,
    pub roles: Option<Vec<String>>,
    pub enabled: bool,
}

/// The outcome of a change checked against a version: saved, or the current state when someone else changed it first.
#[derive(Debug)]
pub enum CairnSaved<T> {
    Saved(T),
    Stale(T),
}

fn connection_secret_context(connection_id: &str) -> String {
    format!("cairn_connections:{connection_id}:api_key")
}

fn connection_from(row: &SqliteRow) -> CairnConnection {
    CairnConnection {
        id: row.get("id"),
        kind: row.get("kind"),
        name: row.get("name"),
        base_url: row.get("base_url"),
        has_key: row.get::<Option<String>, _>("api_key_sealed").is_some(),
        locality: row.get("locality"),
        enabled: row.get::<i64, _>("enabled") != 0,
        cloud_acknowledged_at: row.get("cloud_acknowledged_at"),
        cloud_acknowledged_by: row.get("cloud_acknowledged_by"),
        version: row.get("version"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        updated_by: row.get("updated_by"),
    }
}

fn model_from(row: &SqliteRow) -> CairnModel {
    CairnModel {
        id: row.get("id"),
        connection_id: row.get("connection_id"),
        model_id: row.get("model_id"),
        display_name: row.get("display_name"),
        purpose: row.get("purpose"),
        tool_mode: row.get("tool_mode"),
        context_tokens: row.get("context_tokens"),
        vision: row.get::<i64, _>("vision") != 0,
        evaluation: row
            .get::<Option<String>, _>("evaluation_json")
            .and_then(|text| serde_json::from_str(&text).ok()),
        fallback_model_id: row.get("fallback_model_id"),
        roles: row
            .get::<Option<String>, _>("roles_json")
            .and_then(|text| serde_json::from_str(&text).ok()),
        enabled: row.get::<i64, _>("enabled") != 0,
        version: row.get("version"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        updated_by: row.get("updated_by"),
    }
}

fn checked_name(name: &str, what: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        repomemo_domain::bail_invalid!("A {what} needs a name.");
    }
    if name.chars().count() > CATALOG_NAME_CHARS {
        repomemo_domain::bail_invalid!("A {what}'s name is at most {CATALOG_NAME_CHARS} characters.");
    }
    Ok(name.to_owned())
}

fn checked_choice(value: &str, allowed: &[&str], what: &str) -> Result<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        repomemo_domain::bail_invalid!("{value} is not a {what}; use {}.", allowed.join(", "))
    }
}

fn checked_base_url(base_url: &str) -> Result<String> {
    let base_url = base_url.trim().trim_end_matches('/').to_owned();
    if base_url.is_empty() {
        repomemo_domain::bail_invalid!("A connection needs an address.");
    }
    Ok(base_url)
}

fn roles_json(roles: &Option<Vec<String>>) -> Result<Option<String>> {
    match roles {
        None => Ok(None),
        Some(roles) => {
            for role in roles {
                checked_choice(role, &["owner", "admin", "member", "viewer"], "role")?;
            }
            Ok(Some(serde_json::to_string(roles)?))
        }
    }
}

impl StorageEngine {
    /// Every connection, by name.
    pub async fn list_cairn_connections(&self) -> Result<Vec<CairnConnection>> {
        let rows = sqlx::query("SELECT * FROM cairn_connections ORDER BY name COLLATE NOCASE ASC, id ASC")
            .fetch_all(self.reader())
            .await?;
        Ok(rows.iter().map(connection_from).collect())
    }

    pub async fn get_cairn_connection(&self, id: &str) -> Result<CairnConnection> {
        let row = sqlx::query("SELECT * FROM cairn_connections WHERE id = ?1")
            .bind(id)
            .fetch_optional(self.reader())
            .await?
            .ok_or_else(|| repomemo_domain::not_found!("The connection was not found."))?;
        Ok(connection_from(&row))
    }

    /// The key of a connection, for the server to call the model server with. It is never part of a read the caller serializes.
    pub async fn cairn_connection_key(&self, id: &str) -> Result<Option<String>> {
        let sealed =
            sqlx::query_scalar::<_, Option<String>>("SELECT api_key_sealed FROM cairn_connections WHERE id = ?1")
                .bind(id)
                .fetch_optional(self.reader())
                .await?
                .ok_or_else(|| repomemo_domain::not_found!("The connection was not found."))?;
        sealed
            .map(|sealed| self.secrets.open(&sealed, &connection_secret_context(id)))
            .transpose()
    }

    pub async fn create_cairn_connection(&self, new: NewCairnConnection, actor: &str) -> Result<CairnConnection> {
        self.insert_cairn_connection(new, Some(actor), false).await
    }

    /// A connection made by the server itself, from what a workspace's provider held (V0.9.78): no person made it, and it may carry
    /// the acknowledgement the workspace's provider already had.
    pub async fn import_cairn_connection(
        &self,
        new: NewCairnConnection,
        acknowledged: bool,
    ) -> Result<CairnConnection> {
        self.insert_cairn_connection(new, None, acknowledged).await
    }

    async fn insert_cairn_connection(
        &self,
        new: NewCairnConnection,
        actor: Option<&str>,
        acknowledged: bool,
    ) -> Result<CairnConnection> {
        checked_choice(&new.kind, CONNECTION_KINDS, "kind of connection")?;
        checked_choice(&new.locality, &["local", "cloud"], "locality")?;
        let name = checked_name(&new.name, "connection")?;
        let base_url = checked_base_url(&new.base_url)?;
        let id = Uuid::new_v4().to_string();
        let sealed = match new.api_key.as_deref().map(str::trim).filter(|key| !key.is_empty()) {
            Some(key) => Some(self.secrets.seal(key, &connection_secret_context(&id))?),
            None => None,
        };
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO cairn_connections (id, kind, name, base_url, api_key_sealed, locality, enabled, created_at, updated_at, updated_by,
               cloud_acknowledged_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8, ?9, ?10)",
        )
        .bind(&id)
        .bind(&new.kind)
        .bind(name)
        .bind(base_url)
        .bind(sealed)
        .bind(&new.locality)
        .bind(new.enabled)
        .bind(&now)
        .bind(actor)
        .bind(acknowledged.then(|| now.clone()))
        .execute(&self.pool)
        .await?;
        self.get_cairn_connection(&id).await
    }

    /// Edits a connection that was read at `expected_version`. The key is kept only while the address does not change, so changing
    /// where requests go cannot send the stored key to a new address.
    pub async fn update_cairn_connection(
        &self,
        id: &str,
        expected_version: i64,
        change: CairnConnectionChange,
        actor: &str,
    ) -> Result<CairnSaved<CairnConnection>> {
        checked_choice(&change.locality, &["local", "cloud"], "locality")?;
        let name = checked_name(&change.name, "connection")?;
        let base_url = checked_base_url(&change.base_url)?;
        let existing = self.get_cairn_connection(id).await?;
        if existing.version != expected_version {
            return Ok(CairnSaved::Stale(existing));
        }
        let sealed = match change.api_key.as_deref().map(str::trim) {
            Some("") => None,
            Some(key) => Some(self.secrets.seal(key, &connection_secret_context(id))?),
            None if existing.base_url == base_url => {
                sqlx::query_scalar::<_, Option<String>>("SELECT api_key_sealed FROM cairn_connections WHERE id = ?1")
                    .bind(id)
                    .fetch_one(self.reader())
                    .await?
            }
            None => None,
        };
        let saved = sqlx::query(
            "UPDATE cairn_connections SET name = ?1, base_url = ?2, api_key_sealed = ?3, locality = ?4, enabled = ?5,
               version = version + 1, updated_at = ?6, updated_by = ?7
             WHERE id = ?8 AND version = ?9",
        )
        .bind(name)
        .bind(base_url)
        .bind(sealed)
        .bind(&change.locality)
        .bind(change.enabled)
        .bind(Utc::now().to_rfc3339())
        .bind(actor)
        .bind(id)
        .bind(expected_version)
        .execute(&self.pool)
        .await?;
        let current = self.get_cairn_connection(id).await?;
        Ok(if saved.rows_affected() == 0 {
            CairnSaved::Stale(current)
        } else {
            CairnSaved::Saved(current)
        })
    }

    /// Records that an administrator accepted that a cloud connection's models receive what CAIRN sends them.
    pub async fn acknowledge_cairn_connection(&self, id: &str, actor: &str) -> Result<CairnConnection> {
        let saved = sqlx::query(
            "UPDATE cairn_connections SET cloud_acknowledged_at = ?1, cloud_acknowledged_by = ?2, version = version + 1, updated_at = ?1
             WHERE id = ?3",
        )
        .bind(Utc::now().to_rfc3339())
        .bind(actor)
        .bind(id)
        .execute(&self.pool)
        .await?;
        if saved.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("The connection was not found.");
        }
        self.get_cairn_connection(id).await
    }

    /// Deletes a connection and the models that nothing uses. Refused while any of its models is used.
    pub async fn delete_cairn_connection(&self, id: &str) -> Result<()> {
        self.get_cairn_connection(id).await?;
        for model in self.list_cairn_models(Some(id)).await? {
            let uses = self.cairn_model_uses(&model.id).await?;
            if uses > 0 {
                repomemo_domain::bail_conflict!(
                    "{} is used in {uses} place{}; stop using it before the connection is deleted.",
                    model.display_name,
                    if uses == 1 { "" } else { "s" }
                );
            }
        }
        let mut transaction = self.pool.begin().await?;
        sqlx::query("UPDATE cairn_models SET fallback_model_id = NULL WHERE fallback_model_id IN (SELECT id FROM cairn_models WHERE connection_id = ?1)")
            .bind(id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("DELETE FROM cairn_models WHERE connection_id = ?1")
            .bind(id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("DELETE FROM cairn_connections WHERE id = ?1")
            .bind(id)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(())
    }

    /// The models of one connection, or of all, by name.
    pub async fn list_cairn_models(&self, connection_id: Option<&str>) -> Result<Vec<CairnModel>> {
        let rows = sqlx::query(
            "SELECT * FROM cairn_models WHERE (?1 IS NULL OR connection_id = ?1) ORDER BY display_name COLLATE NOCASE ASC, id ASC",
        )
        .bind(connection_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows.iter().map(model_from).collect())
    }

    pub async fn get_cairn_model(&self, id: &str) -> Result<CairnModel> {
        let row = sqlx::query("SELECT * FROM cairn_models WHERE id = ?1")
            .bind(id)
            .fetch_optional(self.reader())
            .await?
            .ok_or_else(|| repomemo_domain::not_found!("The model was not found."))?;
        Ok(model_from(&row))
    }

    pub async fn create_cairn_model(&self, new: NewCairnModel, actor: &str) -> Result<CairnModel> {
        self.insert_cairn_model(new, Some(actor)).await
    }

    /// A model added by the server itself, from what a workspace's provider named (V0.9.78).
    pub async fn import_cairn_model(&self, new: NewCairnModel) -> Result<CairnModel> {
        self.insert_cairn_model(new, None).await
    }

    async fn insert_cairn_model(&self, new: NewCairnModel, actor: Option<&str>) -> Result<CairnModel> {
        checked_choice(&new.purpose, MODEL_PURPOSES, "purpose")?;
        checked_choice(&new.tool_mode, TOOL_MODES, "tool mode")?;
        let display_name = checked_name(&new.display_name, "model")?;
        if new.model_id.trim().is_empty() {
            repomemo_domain::bail_invalid!("A model needs the name its server knows it by.");
        }
        if new.context_tokens < 0 {
            repomemo_domain::bail_invalid!("The context size cannot be negative.");
        }
        self.get_cairn_connection(&new.connection_id).await?;
        self.check_fallback(None, new.fallback_model_id.as_deref()).await?;
        let roles = roles_json(&new.roles)?;
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let inserted = sqlx::query(
            "INSERT INTO cairn_models (id, connection_id, model_id, display_name, purpose, tool_mode, context_tokens, vision,
               evaluation_json, fallback_model_id, roles_json, enabled, created_at, updated_at, updated_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13, ?14)",
        )
        .bind(&id)
        .bind(&new.connection_id)
        .bind(new.model_id.trim())
        .bind(display_name)
        .bind(&new.purpose)
        .bind(&new.tool_mode)
        .bind(new.context_tokens)
        .bind(new.vision)
        .bind(new.evaluation.as_ref().map(Value::to_string))
        .bind(&new.fallback_model_id)
        .bind(roles)
        .bind(new.enabled)
        .bind(now)
        .bind(actor)
        .execute(&self.pool)
        .await;
        match inserted {
            Ok(_) => self.get_cairn_model(&id).await,
            Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
                repomemo_domain::bail_conflict!("That model is already in the catalog for this connection and purpose.")
            }
            Err(error) => Err(error.into()),
        }
    }

    /// Edits a model that was read at `expected_version`.
    pub async fn update_cairn_model(
        &self,
        id: &str,
        expected_version: i64,
        change: CairnModelChange,
        actor: &str,
    ) -> Result<CairnSaved<CairnModel>> {
        checked_choice(&change.tool_mode, TOOL_MODES, "tool mode")?;
        let display_name = checked_name(&change.display_name, "model")?;
        if change.context_tokens < 0 {
            repomemo_domain::bail_invalid!("The context size cannot be negative.");
        }
        let existing = self.get_cairn_model(id).await?;
        if existing.version != expected_version {
            return Ok(CairnSaved::Stale(existing));
        }
        self.check_fallback(Some(id), change.fallback_model_id.as_deref())
            .await?;
        let roles = roles_json(&change.roles)?;
        let saved = sqlx::query(
            "UPDATE cairn_models SET display_name = ?1, tool_mode = ?2, context_tokens = ?3, vision = ?4, evaluation_json = ?5,
               fallback_model_id = ?6, roles_json = ?7, enabled = ?8, version = version + 1, updated_at = ?9, updated_by = ?10
             WHERE id = ?11 AND version = ?12",
        )
        .bind(display_name)
        .bind(&change.tool_mode)
        .bind(change.context_tokens)
        .bind(change.vision)
        .bind(change.evaluation.as_ref().map(Value::to_string))
        .bind(&change.fallback_model_id)
        .bind(roles)
        .bind(change.enabled)
        .bind(Utc::now().to_rfc3339())
        .bind(actor)
        .bind(id)
        .bind(expected_version)
        .execute(&self.pool)
        .await?;
        let current = self.get_cairn_model(id).await?;
        Ok(if saved.rows_affected() == 0 {
            CairnSaved::Stale(current)
        } else {
            CairnSaved::Saved(current)
        })
    }

    /// A fallback is another model of the catalog, never the model itself.
    async fn check_fallback(&self, model_id: Option<&str>, fallback: Option<&str>) -> Result<()> {
        let Some(fallback) = fallback else { return Ok(()) };
        if model_id == Some(fallback) {
            repomemo_domain::bail_invalid!("A model cannot be its own fallback.");
        }
        self.get_cairn_model(fallback)
            .await
            .map_err(|_| repomemo_domain::invalid!("The fallback model is not in the catalog."))?;
        Ok(())
    }

    /// In how many places a model is used: workspace providers that name it and models that name it as their fallback. The
    /// settings of organizations and workspaces that choose models add to this as they arrive.
    pub async fn cairn_model_uses(&self, model_id: &str) -> Result<i64> {
        let providers = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM provider_settings WHERE json_extract(metadata_json, '$.cairn_model_id') = ?1",
        )
        .bind(model_id)
        .fetch_one(self.reader())
        .await?;
        let fallbacks = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM cairn_models WHERE fallback_model_id = ?1")
            .bind(model_id)
            .fetch_one(self.reader())
            .await?;
        Ok(providers + fallbacks)
    }

    /// Deletes a model. Refused while it is used.
    pub async fn delete_cairn_model(&self, id: &str) -> Result<()> {
        let model = self.get_cairn_model(id).await?;
        let uses = self.cairn_model_uses(id).await?;
        if uses > 0 {
            repomemo_domain::bail_conflict!(
                "{} is used in {uses} place{}; stop using it before it is deleted.",
                model.display_name,
                if uses == 1 { "" } else { "s" }
            );
        }
        sqlx::query("DELETE FROM cairn_models WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn storage() -> (StorageEngine, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("repomemo-catalog-{}", Uuid::new_v4()));
        let storage = StorageEngine::open(crate::StorageConfig::new(dir.clone()))
            .await
            .unwrap();
        (storage, dir)
    }

    fn connection(key: Option<&str>) -> NewCairnConnection {
        NewCairnConnection {
            kind: "openrouter".to_owned(),
            name: "OpenRouter".to_owned(),
            base_url: "https://openrouter.ai/".to_owned(),
            api_key: key.map(str::to_owned),
            locality: "cloud".to_owned(),
            enabled: true,
        }
    }

    fn model(connection_id: &str, model_id: &str) -> NewCairnModel {
        NewCairnModel {
            connection_id: connection_id.to_owned(),
            model_id: model_id.to_owned(),
            display_name: model_id.to_owned(),
            purpose: "text".to_owned(),
            tool_mode: "native".to_owned(),
            context_tokens: 32_000,
            vision: false,
            evaluation: None,
            fallback_model_id: None,
            roles: None,
            enabled: true,
        }
    }

    #[tokio::test]
    async fn a_key_is_stored_encrypted_and_never_appears_in_a_read() {
        let (storage, dir) = storage().await;
        let actor = storage.create_user("admin@example.com", "Admin", "hash").await.unwrap();
        let made = storage
            .create_cairn_connection(connection(Some("sk-secret-value")), &actor.id)
            .await
            .unwrap();
        assert!(made.has_key);
        assert_eq!(made.base_url, "https://openrouter.ai", "a trailing slash is dropped");
        // No read serializes the key.
        for read in [
            serde_json::to_string(&made).unwrap(),
            serde_json::to_string(&storage.list_cairn_connections().await.unwrap()).unwrap(),
            serde_json::to_string(&storage.get_cairn_connection(&made.id).await.unwrap()).unwrap(),
        ] {
            assert!(!read.contains("sk-secret-value"), "{read}");
            assert!(!read.contains("sealed"), "{read}");
        }
        // It is sealed at rest and opens only for the server.
        let stored =
            sqlx::query_scalar::<_, Option<String>>("SELECT api_key_sealed FROM cairn_connections WHERE id = ?1")
                .bind(&made.id)
                .fetch_one(&storage.pool)
                .await
                .unwrap()
                .unwrap();
        assert!(!stored.contains("sk-secret-value"));
        assert_eq!(
            storage.cairn_connection_key(&made.id).await.unwrap().as_deref(),
            Some("sk-secret-value")
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn a_key_is_kept_while_the_address_stays_and_dropped_when_it_moves() {
        let (storage, dir) = storage().await;
        let actor = storage.create_user("admin@example.com", "Admin", "hash").await.unwrap();
        let made = storage
            .create_cairn_connection(connection(Some("sk-kept")), &actor.id)
            .await
            .unwrap();
        let rename = |name: &str, base_url: &str| CairnConnectionChange {
            name: name.to_owned(),
            base_url: base_url.to_owned(),
            api_key: None,
            locality: "cloud".to_owned(),
            enabled: true,
        };
        let CairnSaved::Saved(renamed) = storage
            .update_cairn_connection(&made.id, 1, rename("Renamed", "https://openrouter.ai"), &actor.id)
            .await
            .unwrap()
        else {
            panic!("saved")
        };
        assert_eq!((renamed.version, renamed.has_key), (2, true));
        let CairnSaved::Saved(moved) = storage
            .update_cairn_connection(&made.id, 2, rename("Moved", "https://elsewhere.example"), &actor.id)
            .await
            .unwrap()
        else {
            panic!("saved")
        };
        assert!(!moved.has_key, "a key is never carried to another address");
        // A stale save is refused with the current state.
        let stale = storage
            .update_cairn_connection(&made.id, 1, rename("Late", "https://elsewhere.example"), &actor.id)
            .await
            .unwrap();
        assert!(matches!(stale, CairnSaved::Stale(current) if current.name == "Moved"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn a_used_connection_and_a_used_model_cannot_be_deleted() {
        let (storage, dir) = storage().await;
        let actor = storage.create_user("admin@example.com", "Admin", "hash").await.unwrap();
        let connection = storage
            .create_cairn_connection(connection(None), &actor.id)
            .await
            .unwrap();
        let primary = storage
            .create_cairn_model(model(&connection.id, "vendor/primary"), &actor.id)
            .await
            .unwrap();
        let mut fallback = model(&connection.id, "vendor/fallback");
        fallback.purpose = "text".to_owned();
        let fallback = storage.create_cairn_model(fallback, &actor.id).await.unwrap();

        // A model another names as its fallback is in use, and so is the connection that holds it.
        let CairnSaved::Saved(_) = storage
            .update_cairn_model(
                &primary.id,
                1,
                CairnModelChange {
                    display_name: "Primary".to_owned(),
                    tool_mode: "native".to_owned(),
                    context_tokens: 32_000,
                    vision: false,
                    evaluation: None,
                    fallback_model_id: Some(fallback.id.clone()),
                    roles: Some(vec!["member".to_owned()]),
                    enabled: true,
                },
                &actor.id,
            )
            .await
            .unwrap()
        else {
            panic!("saved")
        };
        assert_eq!(storage.cairn_model_uses(&fallback.id).await.unwrap(), 1);
        assert!(storage.delete_cairn_model(&fallback.id).await.is_err());
        assert!(storage.delete_cairn_connection(&connection.id).await.is_err());
        assert_eq!(storage.list_cairn_models(Some(&connection.id)).await.unwrap().len(), 2);

        // A workspace provider that names a model uses it too.
        let workspace = storage.create_workspace("Space").await.unwrap();
        let provider = ProviderSettings {
            id: String::new(),
            workspace_id: Some(workspace.id.clone()),
            provider_type: "ollama".to_owned(),
            name: "Local".to_owned(),
            base_url: Some("http://localhost:11434".to_owned()),
            model: Some("vendor/primary".to_owned()),
            embedding_model: None,
            enabled: true,
            metadata: serde_json::json!({"cairn_model_id": primary.id}),
            api_key: None,
        };
        storage.save_provider_settings(provider).await.unwrap();
        assert_eq!(storage.cairn_model_uses(&primary.id).await.unwrap(), 1);
        assert!(storage.delete_cairn_model(&primary.id).await.is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn an_unused_connection_is_deleted_with_its_models_and_a_model_is_added_once() {
        let (storage, dir) = storage().await;
        let actor = storage.create_user("admin@example.com", "Admin", "hash").await.unwrap();
        let connection = storage
            .create_cairn_connection(connection(None), &actor.id)
            .await
            .unwrap();
        storage
            .create_cairn_model(model(&connection.id, "vendor/once"), &actor.id)
            .await
            .unwrap();
        assert!(
            storage
                .create_cairn_model(model(&connection.id, "vendor/once"), &actor.id)
                .await
                .is_err(),
            "the same model, connection and purpose twice"
        );
        let mut invalid = model(&connection.id, "vendor/bad");
        invalid.purpose = "audio".to_owned();
        assert!(storage.create_cairn_model(invalid, &actor.id).await.is_err());
        storage.delete_cairn_connection(&connection.id).await.unwrap();
        assert!(storage.list_cairn_connections().await.unwrap().is_empty());
        assert!(storage.list_cairn_models(None).await.unwrap().is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}
