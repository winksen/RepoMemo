//! Storage for system administration: the system-administrator flag, settings
//! changed at run time, the system audit trail, and server-wide statistics.

use std::collections::BTreeMap;

use anyhow::{bail, Context, Result};
use chrono::Utc;
use repomemo_domain::{
    CountByLabel, IndexingJobStatus, Organization, SharedUser, SharedWorkspace, SystemAuditEvent,
    SystemStatistics, SystemUser, SystemUserOrganization, SystemUserWorkspace, WorkspaceUsage,
};
use serde_json::Value;
use sqlx::Row;
use uuid::Uuid;

use super::{normalize_email, IndexingJobRow, OrganizationRow, SharedWorkspaceRow, StorageEngine};

/// A setting a system administrator changed, as stored.
#[derive(Debug, Clone)]
pub struct StoredSystemSetting {
    pub key: String,
    pub value: Value,
    pub updated_at: String,
    pub updated_by: Option<SharedUser>,
}

/// The state checked for every authenticated request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserAccess {
    pub session_version: i64,
    /// Administers the server and acts as an administrator everywhere.
    pub is_system_admin: bool,
    /// Uses the system pages and API, without the access to every organization
    /// and workspace.
    pub is_app_admin: bool,
}

impl StorageEngine {
    /// The session version and system-administrator flag of a user, or `None`
    /// when the user no longer exists.
    pub async fn user_access(&self, user_id: &str) -> Result<Option<UserAccess>> {
        let row = sqlx::query_as::<_, (i64, i64, i64)>(
            "SELECT session_version, is_system_admin, is_app_admin FROM users WHERE id = ?1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(session_version, is_system_admin, is_app_admin)| UserAccess {
            session_version,
            is_system_admin: is_system_admin != 0,
            is_app_admin: is_app_admin != 0,
        }))
    }

    /// Grants or removes the app-administrator role.
    pub async fn set_app_admin(&self, user_id: &str, enabled: bool) -> Result<()> {
        let changed = sqlx::query("UPDATE users SET is_app_admin = ?1 WHERE id = ?2")
            .bind(enabled)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        if changed.rows_affected() == 0 {
            bail!("User was not found.");
        }
        Ok(())
    }

    pub async fn system_admin_count(&self) -> Result<i64> {
        Ok(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE is_system_admin = 1")
                .fetch_one(&self.pool)
                .await?,
        )
    }

    /// Grants or removes the system-administrator role. The last system
    /// administrator cannot be removed, so the server always keeps one.
    pub async fn set_system_admin(&self, user_id: &str, enabled: bool) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        // Take the write lock first so two concurrent removals cannot both
        // see another administrator left.
        let changed = sqlx::query("UPDATE users SET is_system_admin = ?1 WHERE id = ?2")
            .bind(enabled)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        if changed.rows_affected() == 0 {
            bail!("User was not found.");
        }
        if !enabled {
            let remaining =
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE is_system_admin = 1")
                    .fetch_one(&mut *tx)
                    .await?;
            if remaining == 0 {
                bail!("The last system administrator cannot be removed; grant the role to someone else first.");
            }
        }
        tx.commit().await?;
        Ok(())
    }

    /// Makes the accounts with these emails system administrators. Returns
    /// how many accounts gained the role.
    pub async fn promote_system_admins(&self, emails: &[String]) -> Result<u64> {
        let mut promoted = 0;
        for email in emails {
            let Ok(email) = normalize_email(email) else {
                continue;
            };
            promoted += sqlx::query(
                "UPDATE users SET is_system_admin = 1 WHERE email = ?1 AND is_system_admin = 0",
            )
            .bind(&email)
            .execute(&self.pool)
            .await?
            .rows_affected();
        }
        Ok(promoted)
    }

    pub async fn organization_exists(&self, organization_id: &str) -> Result<bool> {
        Ok(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM organizations WHERE id = ?1")
                .bind(organization_id)
                .fetch_one(&self.pool)
                .await?
                > 0,
        )
    }

    /// Every organization, with the role a system administrator holds in it:
    /// owner where they own it, administrator everywhere else.
    pub async fn list_organizations_for_system_admin(&self, user_id: &str) -> Result<Vec<Organization>> {
        let rows = sqlx::query_as::<_, OrganizationRow>(
            r#"
            SELECT o.id, o.name,
                   CASE m.role WHEN 'owner' THEN 'owner' ELSE 'admin' END AS role,
                   o.created_at, o.updated_at
            FROM organizations o
            LEFT JOIN organization_memberships m ON m.organization_id = o.id AND m.user_id = ?1
            ORDER BY o.name ASC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Organization::from).collect())
    }

    /// Every workspace, with the role a system administrator holds in it.
    pub async fn list_shared_workspaces_for_system_admin(
        &self,
        user_id: &str,
    ) -> Result<Vec<SharedWorkspace>> {
        let rows = sqlx::query_as::<_, SharedWorkspaceRow>(
            r#"
            SELECT w.id, w.name, w.created_at, w.updated_at, w.settings_json, wo.organization_id,
                   CASE wm.role WHEN 'owner' THEN 'owner' ELSE 'admin' END AS role
            FROM workspaces w
            INNER JOIN workspace_organizations wo ON wo.workspace_id = w.id
            LEFT JOIN workspace_memberships wm ON wm.workspace_id = w.id AND wm.user_id = ?1
            ORDER BY w.updated_at DESC, w.name ASC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(SharedWorkspace::try_from).collect()
    }

    pub async fn list_system_users(&self) -> Result<Vec<SystemUser>> {
        let now = Utc::now().to_rfc3339();
        let rows = sqlx::query(
            r#"
            SELECT u.id, u.email, u.display_name, u.created_at, u.last_connected_at, u.is_system_admin, u.is_app_admin,
              (SELECT COUNT(*) FROM organization_memberships m WHERE m.user_id = u.id) AS organization_count,
              (SELECT COUNT(*) FROM workspace_memberships m WHERE m.user_id = u.id) AS workspace_count,
              (SELECT COUNT(*) FROM refresh_tokens t WHERE t.user_id = u.id AND t.revoked_at IS NULL AND t.expires_at > ?1) AS active_sessions
            FROM users u
            ORDER BY u.is_system_admin DESC, u.is_app_admin DESC, u.display_name COLLATE NOCASE ASC
            "#,
        )
        .bind(&now)
        .fetch_all(&self.pool)
        .await?;
        let memberships = sqlx::query(
            "SELECT m.user_id, m.workspace_id, w.name, m.role, o.id AS organization_id, o.name AS organization_name FROM workspace_memberships m JOIN workspaces w ON w.id = m.workspace_id LEFT JOIN workspace_organizations wo ON wo.workspace_id = w.id LEFT JOIN organizations o ON o.id = wo.organization_id ORDER BY w.name COLLATE NOCASE ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut workspaces_by_user: BTreeMap<String, Vec<SystemUserWorkspace>> = BTreeMap::new();
        for row in memberships {
            workspaces_by_user
                .entry(row.get("user_id"))
                .or_default()
                .push(SystemUserWorkspace {
                    workspace_id: row.get("workspace_id"),
                    name: row.get("name"),
                    role: row.get("role"),
                    organization_id: row.get("organization_id"),
                    organization_name: row.get("organization_name"),
                });
        }
        let organization_memberships = sqlx::query(
            "SELECT m.user_id, o.id AS organization_id, o.name, m.role FROM organization_memberships m JOIN organizations o ON o.id = m.organization_id ORDER BY o.name COLLATE NOCASE ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut organizations_by_user: BTreeMap<String, Vec<SystemUserOrganization>> = BTreeMap::new();
        for row in organization_memberships {
            organizations_by_user
                .entry(row.get("user_id"))
                .or_default()
                .push(SystemUserOrganization {
                    organization_id: row.get("organization_id"),
                    name: row.get("name"),
                    role: row.get("role"),
                });
        }
        Ok(rows
            .into_iter()
            .map(|row| SystemUser {
                id: row.get("id"),
                email: row.get("email"),
                display_name: row.get("display_name"),
                created_at: row.get("created_at"),
                last_connected_at: row.get("last_connected_at"),
                is_system_admin: row.get::<i64, _>("is_system_admin") != 0,
                is_app_admin: row.get::<i64, _>("is_app_admin") != 0,
                organization_count: row.get("organization_count"),
                workspace_count: row.get("workspace_count"),
                active_sessions: row.get("active_sessions"),
                workspaces: workspaces_by_user
                    .remove(row.get::<String, _>("id").as_str())
                    .unwrap_or_default(),
                organizations: organizations_by_user
                    .remove(row.get::<String, _>("id").as_str())
                    .unwrap_or_default(),
            })
            .collect())
    }

    pub async fn system_statistics(&self) -> Result<SystemStatistics> {
        let now = Utc::now();
        let since = |days: i64| (now - chrono::Duration::days(days)).to_rfc3339();
        let count = |sql: &'static str| async move {
            sqlx::query_scalar::<_, i64>(sql).fetch_one(&self.pool).await
        };
        let active_since = |cutoff: String| async move {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE last_connected_at >= ?1")
                .bind(cutoff)
                .fetch_one(&self.pool)
                .await
        };
        let jobs_by_status = sqlx::query_as::<_, (String, i64)>(
            "SELECT status, COUNT(*) FROM indexing_jobs GROUP BY status ORDER BY status",
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|(label, count)| CountByLabel { label, count })
        .collect();
        Ok(SystemStatistics {
            user_count: count("SELECT COUNT(*) FROM users").await?,
            system_admin_count: count("SELECT COUNT(*) FROM users WHERE is_system_admin = 1").await?,
            users_active_last_7_days: active_since(since(7)).await?,
            users_active_last_30_days: active_since(since(30)).await?,
            active_session_count: sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM refresh_tokens WHERE revoked_at IS NULL AND expires_at > ?1",
            )
            .bind(now.to_rfc3339())
            .fetch_one(&self.pool)
            .await?,
            organization_count: count("SELECT COUNT(*) FROM organizations").await?,
            workspace_count: count("SELECT COUNT(*) FROM workspaces").await?,
            artifact_count: count("SELECT COUNT(*) FROM artifacts").await?,
            artifact_bytes: count("SELECT COALESCE(SUM(size_bytes), 0) FROM artifacts").await?,
            indexed_artifact_count: count("SELECT COUNT(*) FROM artifacts WHERE indexed_at IS NOT NULL").await?,
            blob_count: count("SELECT COUNT(*) FROM blobs").await?,
            blob_bytes: count("SELECT COALESCE(SUM(size_bytes), 0) FROM blobs").await?,
            chunk_count: count("SELECT COUNT(*) FROM chunks").await?,
            embedding_count: count("SELECT COUNT(*) FROM chunk_embeddings").await?,
            memory_card_count: count("SELECT COUNT(*) FROM memory_cards").await?,
            task_count: count("SELECT COUNT(*) FROM workspace_tasks").await?,
            comment_count: count("SELECT COUNT(*) FROM artifact_comments").await?,
            repository_count: count("SELECT COUNT(*) FROM sources WHERE type = 'git_repo'").await?,
            enabled_provider_count: count("SELECT COUNT(*) FROM provider_settings WHERE enabled = 1").await?,
            cloud_provider_count: count(
                "SELECT COUNT(*) FROM provider_settings WHERE enabled = 1 AND provider_type = 'openrouter'",
            )
            .await?,
            jobs_by_status,
        })
    }

    /// Activity per day across every workspace since `since` (RFC 3339).
    pub async fn system_activity_by_day(&self, since: &str) -> Result<Vec<CountByLabel>> {
        Ok(sqlx::query_as::<_, (String, i64)>(
            "SELECT substr(created_at, 1, 10) AS day, COUNT(*) FROM workspace_activity WHERE created_at >= ?1 GROUP BY day ORDER BY day",
        )
        .bind(since)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|(label, count)| CountByLabel { label, count })
        .collect())
    }

    /// Activity per action across every workspace since `since`, most
    /// frequent first.
    pub async fn system_activity_by_action(&self, since: &str) -> Result<Vec<CountByLabel>> {
        Ok(sqlx::query_as::<_, (String, i64)>(
            "SELECT action, COUNT(*) AS total FROM workspace_activity WHERE created_at >= ?1 GROUP BY action ORDER BY total DESC, action",
        )
        .bind(since)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|(label, count)| CountByLabel { label, count })
        .collect())
    }

    /// The most active people since `since`, by recorded activity.
    pub async fn system_top_users(&self, since: &str, limit: i64) -> Result<Vec<CountByLabel>> {
        Ok(sqlx::query_as::<_, (String, i64)>(
            r#"
            SELECT u.display_name || ' <' || u.email || '>' AS label, COUNT(*) AS total
            FROM workspace_activity a INNER JOIN users u ON u.id = a.actor_user_id
            WHERE a.created_at >= ?1
            GROUP BY u.id ORDER BY total DESC LIMIT ?2
            "#,
        )
        .bind(since)
        .bind(limit.clamp(1, 100))
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|(label, count)| CountByLabel { label, count })
        .collect())
    }

    /// Each workspace's footprint and recent use, largest first.
    pub async fn system_workspace_usage(&self, since: &str) -> Result<Vec<WorkspaceUsage>> {
        let rows = sqlx::query(
            r#"
            SELECT w.id, w.name, o.id AS organization_id, o.name AS organization_name,
              (SELECT COUNT(*) FROM workspace_memberships m WHERE m.workspace_id = w.id) AS member_count,
              (SELECT COUNT(*) FROM artifacts a WHERE a.workspace_id = w.id) AS artifact_count,
              (SELECT COALESCE(SUM(a.size_bytes), 0) FROM artifacts a WHERE a.workspace_id = w.id) AS artifact_bytes,
              (SELECT COUNT(*) FROM chunks c WHERE c.workspace_id = w.id) AS chunk_count,
              (SELECT COUNT(*) FROM workspace_activity e WHERE e.workspace_id = w.id AND e.created_at >= ?1) AS activity_count,
              (SELECT COUNT(*) FROM workspace_activity e WHERE e.workspace_id = w.id AND e.created_at >= ?1 AND e.action IN ('ai_question_answered', 'ai_overview_generated', 'repository_summarized', 'assistant_answered')) AS ai_count,
              (SELECT MAX(e.created_at) FROM workspace_activity e WHERE e.workspace_id = w.id) AS last_activity_at
            FROM workspaces w
            LEFT JOIN workspace_organizations wo ON wo.workspace_id = w.id
            LEFT JOIN organizations o ON o.id = wo.organization_id
            ORDER BY artifact_bytes DESC, w.name ASC
            "#,
        )
        .bind(since)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| WorkspaceUsage {
                workspace_id: row.get("id"),
                workspace_name: row.get("name"),
                organization_id: row.get("organization_id"),
                organization_name: row.get("organization_name"),
                member_count: row.get("member_count"),
                artifact_count: row.get("artifact_count"),
                artifact_bytes: row.get("artifact_bytes"),
                chunk_count: row.get("chunk_count"),
                activity_last_30_days: row.get("activity_count"),
                ai_requests_last_30_days: row.get("ai_count"),
                last_activity_at: row.get("last_activity_at"),
            })
            .collect())
    }

    /// Recent jobs across every workspace, newest first.
    pub async fn list_all_jobs(&self, status: Option<&str>, limit: i64) -> Result<Vec<IndexingJobStatus>> {
        let mut builder = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            "SELECT id, workspace_id, source_id, kind, status, stage, progress_current, progress_total, error_message, cancel_requested, created_at, updated_at FROM indexing_jobs",
        );
        if let Some(status) = status {
            builder.push(" WHERE status = ").push_bind(status.to_owned());
        }
        builder.push(" ORDER BY created_at DESC, id DESC LIMIT ");
        builder.push_bind(limit.clamp(1, 500));
        Ok(builder
            .build_query_as::<IndexingJobRow>()
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(IndexingJobStatus::from)
            .collect())
    }

    /// Workspace names by id, to label jobs and usage rows.
    pub async fn workspace_names(&self) -> Result<BTreeMap<String, String>> {
        Ok(sqlx::query_as::<_, (String, String)>("SELECT id, name FROM workspaces")
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .collect())
    }

    pub async fn list_system_settings(&self) -> Result<Vec<StoredSystemSetting>> {
        let rows = sqlx::query(
            r#"
            SELECT s.key, s.value_json, s.updated_at, u.id AS user_id, u.email, u.display_name
            FROM system_settings s LEFT JOIN users u ON u.id = s.updated_by_user_id
            ORDER BY s.key
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| StoredSystemSetting {
                key: row.get("key"),
                value: serde_json::from_str(&row.get::<String, _>("value_json")).unwrap_or(Value::Null),
                updated_at: row.get("updated_at"),
                updated_by: row.get::<Option<String>, _>("user_id").map(|id| SharedUser {
                    id,
                    email: row.get("email"),
                    display_name: row.get::<Option<String>, _>("display_name").unwrap_or_default(),
                }),
            })
            .collect())
    }

    pub async fn set_system_setting(&self, key: &str, value: &Value, actor_user_id: Option<&str>) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO system_settings (key, value_json, updated_at, updated_by_user_id)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json,
              updated_at = excluded.updated_at, updated_by_user_id = excluded.updated_by_user_id
            "#,
        )
        .bind(key)
        .bind(serde_json::to_string(value)?)
        .bind(Utc::now().to_rfc3339())
        .bind(actor_user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Removes a run-time setting so the environment default applies again.
    pub async fn delete_system_setting(&self, key: &str) -> Result<bool> {
        Ok(sqlx::query("DELETE FROM system_settings WHERE key = ?1")
            .bind(key)
            .execute(&self.pool)
            .await?
            .rows_affected()
            > 0)
    }

    pub async fn record_system_event(&self, actor_user_id: Option<&str>, action: &str, detail: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO system_audit_events (id, created_at, actor_user_id, action, detail) VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(Utc::now().to_rfc3339())
        .bind(actor_user_id)
        .bind(action)
        .bind(detail)
        .execute(&self.pool)
        .await
        .context("the system audit event could not be recorded")?;
        Ok(())
    }

    pub async fn list_system_events(&self, limit: i64) -> Result<Vec<SystemAuditEvent>> {
        let rows = sqlx::query(
            r#"
            SELECT e.id, e.created_at, e.action, e.detail, u.id AS user_id, u.email, u.display_name
            FROM system_audit_events e LEFT JOIN users u ON u.id = e.actor_user_id
            ORDER BY e.created_at DESC LIMIT ?1
            "#,
        )
        .bind(limit.clamp(1, 1000))
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| SystemAuditEvent {
                id: row.get("id"),
                created_at: row.get("created_at"),
                action: row.get("action"),
                detail: row.get("detail"),
                actor: row.get::<Option<String>, _>("user_id").map(|id| SharedUser {
                    id,
                    email: row.get("email"),
                    display_name: row.get::<Option<String>, _>("display_name").unwrap_or_default(),
                }),
            })
            .collect())
    }

    /// Deletes system audit events recorded before `cutoff`.
    pub async fn prune_system_events(&self, cutoff: &str) -> Result<u64> {
        Ok(sqlx::query("DELETE FROM system_audit_events WHERE created_at < ?1")
            .bind(cutoff)
            .execute(&self.pool)
            .await?
            .rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use crate::{StorageConfig, StorageEngine};
    use serde_json::json;

    #[tokio::test]
    async fn system_admins_see_everything_and_the_last_one_stays() {
        let data_dir = std::env::temp_dir().join(format!("repomemo-system-{}", uuid::Uuid::new_v4()));
        let storage = StorageEngine::open(StorageConfig::new(data_dir.clone())).await.unwrap();
        let owner = storage.create_user("owner@example.com", "Owner", "hash").await.unwrap();
        let admin = storage.create_user("Admin@Example.com", "Admin", "hash").await.unwrap();
        let organization = storage.create_organization(&owner.id, "Team").await.unwrap();
        storage.create_shared_workspace(&owner.id, &organization.id, "Ledger").await.unwrap();

        assert!(storage.list_organizations_for_user(&admin.id).await.unwrap().is_empty());
        assert_eq!(storage.promote_system_admins(&["admin@example.com".to_owned(), "nobody@example.com".to_owned()]).await.unwrap(), 1);
        let access = storage.user_access(&admin.id).await.unwrap().unwrap();
        assert!(access.is_system_admin);

        let organizations = storage.list_organizations_for_system_admin(&admin.id).await.unwrap();
        assert_eq!(organizations.len(), 1);
        assert_eq!(organizations[0].role, repomemo_domain::OrganizationRole::Admin);
        let workspaces = storage.list_shared_workspaces_for_system_admin(&admin.id).await.unwrap();
        assert_eq!(workspaces.len(), 1);
        assert!(matches!(workspaces[0].role, repomemo_domain::WorkspaceRole::Admin));
        let owned = storage.list_shared_workspaces_for_system_admin(&owner.id).await.unwrap();
        assert!(matches!(owned[0].role, repomemo_domain::WorkspaceRole::Owner), "owners stay owners");

        assert!(storage.set_system_admin(&admin.id, false).await.is_err(), "the last one stays");
        storage.set_system_admin(&owner.id, true).await.unwrap();
        storage.set_system_admin(&admin.id, false).await.unwrap();
        assert_eq!(storage.system_admin_count().await.unwrap(), 1);

        let users = storage.list_system_users().await.unwrap();
        assert_eq!(users.len(), 2);
        assert!(users[0].is_system_admin);
        let statistics = storage.system_statistics().await.unwrap();
        assert_eq!((statistics.user_count, statistics.organization_count, statistics.workspace_count), (2, 1, 1));
        let usage = storage.system_workspace_usage("1970-01-01T00:00:00Z").await.unwrap();
        assert_eq!(usage[0].member_count, 1);
        assert_eq!(usage[0].activity_last_30_days, 1, "workspace creation is recorded");

        storage.set_system_setting("allow_registration", &json!(false), Some(&owner.id)).await.unwrap();
        let settings = storage.list_system_settings().await.unwrap();
        assert_eq!(settings[0].value, json!(false));
        assert_eq!(settings[0].updated_by.as_ref().unwrap().id, owner.id);
        assert!(storage.delete_system_setting("allow_registration").await.unwrap());
        assert!(storage.list_system_settings().await.unwrap().is_empty());

        storage.record_system_event(Some(&owner.id), "test", "Something happened.").await.unwrap();
        let events = storage.list_system_events(10).await.unwrap();
        assert_eq!(events[0].actor.as_ref().unwrap().display_name, "Owner");

        storage.pool.close().await;
        let _ = std::fs::remove_dir_all(data_dir);
    }
}
