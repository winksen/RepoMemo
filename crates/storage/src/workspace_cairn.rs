//! A workspace's CAIRN settings (V0.9.85): which of its organization's models the workspace offers, and which is its default. One
//! versioned policy for each workspace; a change names the version it read.

use serde::{Deserialize, Serialize};

use super::*;
use crate::cairn_catalog::CairnSaved;

/// What a workspace decided about CAIRN. More fields arrive with the rows that add guardrails, closed folders and abilities.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceCairnPolicy {
    /// The organization's models the workspace offers; a workspace that never saved settings offers all of its organization's.
    #[serde(default)]
    pub offered_model_ids: Vec<String>,
    /// The model the workspace uses unless a person chooses another of those offered.
    #[serde(default)]
    pub default_model_id: Option<String>,
    /// The roles that may use a model in this workspace, by model id, within what the organization allows (V0.9.87).
    #[serde(default)]
    pub model_roles: std::collections::BTreeMap<String, Vec<String>>,
}

/// A workspace's settings with the version to name when changing them. `version` is 0 for a workspace that never saved any.
#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceCairnSettings {
    pub workspace_id: String,
    pub version: i64,
    #[serde(flatten)]
    pub policy: WorkspaceCairnPolicy,
    pub updated_at: Option<String>,
    pub updated_by: Option<String>,
}

fn settings_from(workspace_id: &str, row: Option<SqliteRow>) -> WorkspaceCairnSettings {
    match row {
        None => WorkspaceCairnSettings {
            workspace_id: workspace_id.to_owned(),
            version: 0,
            policy: WorkspaceCairnPolicy::default(),
            updated_at: None,
            updated_by: None,
        },
        Some(row) => WorkspaceCairnSettings {
            workspace_id: workspace_id.to_owned(),
            version: row.get("version"),
            policy: serde_json::from_str(&row.get::<String, _>("policy_json")).unwrap_or_default(),
            updated_at: Some(row.get("updated_at")),
            updated_by: row.get("updated_by_name"),
        },
    }
}

impl StorageEngine {
    /// The settings of a workspace, or none at version 0 when it never saved any.
    pub async fn get_workspace_cairn_settings(&self, workspace_id: &str) -> Result<WorkspaceCairnSettings> {
        let row = sqlx::query(
            "SELECT s.version, s.policy_json, s.updated_at, u.display_name AS updated_by_name
             FROM workspace_cairn_settings s LEFT JOIN users u ON u.id = s.updated_by WHERE s.workspace_id = ?1",
        )
        .bind(workspace_id)
        .fetch_optional(self.reader())
        .await?;
        Ok(settings_from(workspace_id, row))
    }

    /// Saves the settings a workspace administrator read at `expected_version` (0 for none yet). A save made on an older version is
    /// returned with the current settings instead of overwriting them. `actor` is none when the server itself saves them, as an import
    /// does.
    pub async fn save_workspace_cairn_settings(
        &self,
        workspace_id: &str,
        expected_version: i64,
        policy: &WorkspaceCairnPolicy,
        actor: Option<&str>,
    ) -> Result<CairnSaved<WorkspaceCairnSettings>> {
        let now = Utc::now().to_rfc3339();
        let json = serde_json::to_string(policy)?;
        let saved = if expected_version == 0 {
            sqlx::query(
                "INSERT OR IGNORE INTO workspace_cairn_settings (workspace_id, policy_json, version, updated_at, updated_by)
                 VALUES (?1, ?2, 1, ?3, ?4)",
            )
            .bind(workspace_id)
            .bind(&json)
            .bind(&now)
            .bind(actor)
            .execute(&self.pool)
            .await?
        } else {
            sqlx::query(
                "UPDATE workspace_cairn_settings SET policy_json = ?2, version = version + 1, updated_at = ?3, updated_by = ?4
                 WHERE workspace_id = ?1 AND version = ?5",
            )
            .bind(workspace_id)
            .bind(&json)
            .bind(&now)
            .bind(actor)
            .bind(expected_version)
            .execute(&self.pool)
            .await?
        };
        let current = self.get_workspace_cairn_settings(workspace_id).await?;
        Ok(if saved.rows_affected() == 0 {
            CairnSaved::Stale(current)
        } else {
            CairnSaved::Saved(current)
        })
    }

    /// How many workspaces offer a model or make it their default.
    pub async fn workspaces_using_cairn_model(&self, model_id: &str) -> Result<i64> {
        Ok(sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM workspace_cairn_settings s
             WHERE EXISTS (SELECT 1 FROM json_each(s.policy_json, '$.offered_model_ids') WHERE value = ?1)
                OR json_extract(s.policy_json, '$.default_model_id') = ?1",
        )
        .bind(model_id)
        .fetch_one(self.reader())
        .await?)
    }
}
