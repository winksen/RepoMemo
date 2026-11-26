//! An organization's CAIRN settings (V0.9.82): the catalog models it uses, its default model and whether its workspaces start in
//! Agent mode. One versioned policy for each organization; a change names the version it read.

use serde::{Deserialize, Serialize};

use super::*;
use crate::cairn_catalog::CairnSaved;

/// The Agent-mode default of an organization's workspaces.
pub const AGENT_MODES: &[&str] = &["auto", "agent", "jobs"];
/// The most models an organization enables.
pub const ORGANIZATION_MODELS_MAX: usize = 50;

/// What CAIRN may do, see, send and spend (V0.9.91 on), set by an organization and tightened by a workspace: a workspace can only
/// turn more off or make a limit stricter, never loosen what its organization set. Rows add fields here, and an older policy reads with
/// their defaults.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Guardrails {
    /// CAIRN tools and jobs turned off, by name (V0.9.91).
    #[serde(default)]
    pub tools_off: Vec<String>,
    /// The most changes one confirmation may make (V0.9.92); none keeps the default of 200. A workspace's is never higher than its
    /// organization's.
    #[serde(default)]
    pub max_changes: Option<i64>,
    /// The number of changes above which the person types the number to confirm (V0.9.92). None keeps today's rule: only after CAIRN
    /// read content from outside the workspace, above ten. A workspace's is never higher than its organization's.
    #[serde(default)]
    pub type_to_confirm_from: Option<i64>,
}

/// The most changes one confirmation makes when nothing sets less.
pub const DEFAULT_MAX_CHANGES: i64 = 200;

/// What an organization decided about CAIRN. More fields arrive with the rows that add guardrails, retention and preferences, and
/// an older policy reads with their defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrganizationCairnPolicy {
    /// The catalog models the organization uses; none until it chooses.
    #[serde(default)]
    pub enabled_model_ids: Vec<String>,
    /// The model its workspaces use unless they choose another of those enabled.
    #[serde(default)]
    pub default_model_id: Option<String>,
    /// `auto` starts a workspace in Agent mode only for a model that took the live evaluation (V0.9.79), `agent` and `jobs` set the
    /// mode for every workspace that has not chosen its own.
    #[serde(default = "auto_mode")]
    pub agent_mode: String,
    /// The roles that may use a model, by model id (V0.9.87): *cloud models for members and up*. A model with no entry is open to
    /// everyone who may use AI. Owners and admins always may, so nobody is locked out of a model they administer.
    #[serde(default)]
    pub model_roles: std::collections::BTreeMap<String, Vec<String>>,
    /// What CAIRN may do in the organization's workspaces (V0.9.91 on).
    #[serde(default)]
    pub guardrails: Guardrails,
}

fn auto_mode() -> String {
    "auto".to_owned()
}

impl Default for OrganizationCairnPolicy {
    fn default() -> Self {
        Self {
            enabled_model_ids: Vec::new(),
            default_model_id: None,
            agent_mode: auto_mode(),
            model_roles: Default::default(),
            guardrails: Default::default(),
        }
    }
}

/// An organization's settings with the version to name when changing them. `version` is 0 for an organization that never saved any.
#[derive(Debug, Clone, Serialize)]
pub struct OrganizationCairnSettings {
    pub organization_id: String,
    pub version: i64,
    #[serde(flatten)]
    pub policy: OrganizationCairnPolicy,
    pub updated_at: Option<String>,
    /// Who saved them last; *Former member* once their account is anonymized.
    pub updated_by: Option<String>,
}

fn settings_from(organization_id: &str, row: Option<SqliteRow>) -> OrganizationCairnSettings {
    match row {
        None => OrganizationCairnSettings {
            organization_id: organization_id.to_owned(),
            version: 0,
            policy: OrganizationCairnPolicy::default(),
            updated_at: None,
            updated_by: None,
        },
        Some(row) => OrganizationCairnSettings {
            organization_id: organization_id.to_owned(),
            version: row.get("version"),
            policy: serde_json::from_str(&row.get::<String, _>("policy_json")).unwrap_or_default(),
            updated_at: Some(row.get("updated_at")),
            updated_by: row.get("updated_by_name"),
        },
    }
}

const SELECT_SETTINGS: &str = "SELECT s.version, s.policy_json, s.updated_at, u.display_name AS updated_by_name
   FROM organization_cairn_settings s LEFT JOIN users u ON u.id = s.updated_by WHERE s.organization_id = ?1";

impl StorageEngine {
    /// The settings of an organization, or the defaults at version 0 when it never saved any.
    pub async fn get_organization_cairn_settings(&self, organization_id: &str) -> Result<OrganizationCairnSettings> {
        let row = sqlx::query(SELECT_SETTINGS)
            .bind(organization_id)
            .fetch_optional(self.reader())
            .await?;
        Ok(settings_from(organization_id, row))
    }

    /// Saves the settings an administrator read at `expected_version` (0 for none yet). A save made on an older version is returned
    /// with the current settings instead of overwriting them.
    pub async fn save_organization_cairn_settings(
        &self,
        organization_id: &str,
        expected_version: i64,
        policy: &OrganizationCairnPolicy,
        actor: &str,
    ) -> Result<CairnSaved<OrganizationCairnSettings>> {
        let now = Utc::now().to_rfc3339();
        let json = serde_json::to_string(policy)?;
        let saved = if expected_version == 0 {
            sqlx::query(
                "INSERT OR IGNORE INTO organization_cairn_settings (organization_id, policy_json, version, updated_at, updated_by)
                 VALUES (?1, ?2, 1, ?3, ?4)",
            )
            .bind(organization_id)
            .bind(&json)
            .bind(&now)
            .bind(actor)
            .execute(&self.pool)
            .await?
        } else {
            sqlx::query(
                "UPDATE organization_cairn_settings SET policy_json = ?2, version = version + 1, updated_at = ?3, updated_by = ?4
                 WHERE organization_id = ?1 AND version = ?5",
            )
            .bind(organization_id)
            .bind(&json)
            .bind(&now)
            .bind(actor)
            .bind(expected_version)
            .execute(&self.pool)
            .await?
        };
        let current = self.get_organization_cairn_settings(organization_id).await?;
        Ok(if saved.rows_affected() == 0 {
            CairnSaved::Stale(current)
        } else {
            CairnSaved::Saved(current)
        })
    }

    /// How many organizations enable a model or make it their default.
    pub async fn organizations_using_cairn_model(&self, model_id: &str) -> Result<i64> {
        Ok(sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM organization_cairn_settings s
             WHERE EXISTS (SELECT 1 FROM json_each(s.policy_json, '$.enabled_model_ids') WHERE value = ?1)
                OR json_extract(s.policy_json, '$.default_model_id') = ?1",
        )
        .bind(model_id)
        .fetch_one(self.reader())
        .await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn settings_are_versioned_and_a_stale_save_returns_the_current_ones() {
        let dir = std::env::temp_dir().join(format!("repomemo-org-cairn-{}", Uuid::new_v4()));
        let storage = StorageEngine::open(crate::StorageConfig::new(dir.clone()))
            .await
            .unwrap();
        let owner = storage.create_user("owner@example.com", "Owner", "hash").await.unwrap();
        let organization = storage
            .create_organization_with(&owner.id, "Org", None, None)
            .await
            .unwrap();

        // An organization that never saved any has the defaults at version 0.
        let none = storage.get_organization_cairn_settings(&organization.id).await.unwrap();
        assert_eq!(
            (
                none.version,
                none.policy.agent_mode.as_str(),
                none.policy.enabled_model_ids.len()
            ),
            (0, "auto", 0)
        );

        let policy = OrganizationCairnPolicy {
            enabled_model_ids: vec!["a".to_owned(), "b".to_owned()],
            default_model_id: Some("a".to_owned()),
            agent_mode: "jobs".to_owned(),
            model_roles: Default::default(),
            guardrails: Default::default(),
        };
        let CairnSaved::Saved(first) = storage
            .save_organization_cairn_settings(&organization.id, 0, &policy, &owner.id)
            .await
            .unwrap()
        else {
            panic!("saved")
        };
        assert_eq!((first.version, first.updated_by.as_deref()), (1, Some("Owner")));
        assert_eq!(first.policy, policy);

        // A second first save, and a save on an old version, are stale and change nothing.
        for old in [0, 5] {
            let stale = storage
                .save_organization_cairn_settings(&organization.id, old, &OrganizationCairnPolicy::default(), &owner.id)
                .await
                .unwrap();
            assert!(matches!(stale, CairnSaved::Stale(current) if current.version == 1 && current.policy == policy));
        }
        let CairnSaved::Saved(second) = storage
            .save_organization_cairn_settings(&organization.id, 1, &OrganizationCairnPolicy::default(), &owner.id)
            .await
            .unwrap()
        else {
            panic!("saved")
        };
        assert_eq!(second.version, 2);
        assert_eq!(storage.organizations_using_cairn_model("a").await.unwrap(), 0);
        let _ = std::fs::remove_dir_all(dir);
    }
}
