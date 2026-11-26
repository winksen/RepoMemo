//! Retention by organization (V0.9.100): how long CAIRN's run history, its chats and its people's personal memory are kept in an
//! organization's workspaces. The maintenance sweep applies the shorter of this and the server's setting, and an administrator sees
//! what a change would delete before saving it.

use super::*;
use crate::organization_cairn::Guardrails;

/// What a retention would delete now.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct RetentionCounts {
    pub run_histories: i64,
    pub chats: i64,
    pub personal_memories: i64,
}

fn cutoff(days: Option<i64>) -> Option<String> {
    days.filter(|days| *days > 0)
        .map(|days| (Utc::now() - chrono::Duration::days(days)).to_rfc3339())
}

const IN_ORGANIZATION: &str =
    "workspace_id IN (SELECT workspace_id FROM workspace_organizations WHERE organization_id = ?1)";

impl StorageEngine {
    /// The ids of every organization.
    pub async fn organization_ids(&self) -> Result<Vec<String>> {
        Ok(sqlx::query_scalar("SELECT id FROM organizations")
            .fetch_all(self.reader())
            .await?)
    }

    /// What deleting with these retentions would remove from an organization's workspaces right now.
    pub async fn retention_counts(&self, organization_id: &str, retention: &Guardrails) -> Result<RetentionCounts> {
        let count = |sql: String, cutoff: Option<String>| async move {
            match cutoff {
                None => Ok::<i64, anyhow::Error>(0),
                Some(cutoff) => Ok(sqlx::query_scalar::<_, i64>(&sql)
                    .bind(organization_id)
                    .bind(cutoff)
                    .fetch_one(self.reader())
                    .await?),
            }
        };
        Ok(RetentionCounts {
            run_histories: count(format!("SELECT COUNT(*) FROM agent_runs WHERE {IN_ORGANIZATION} AND started_at < ?2"), cutoff(retention.retention_trace_days)).await?,
            chats: count(format!("SELECT COUNT(*) FROM assistant_conversations WHERE origin = 'chat' AND {IN_ORGANIZATION} AND updated_at < ?2"), cutoff(retention.retention_chat_days)).await?,
            personal_memories: count(format!("SELECT COUNT(*) FROM agent_personal_memory WHERE {IN_ORGANIZATION} AND updated_at < ?2"), cutoff(retention.retention_memory_days)).await?,
        })
    }

    /// Deletes what an organization's retentions no longer keep, and returns how much.
    pub async fn apply_organization_retention(&self, organization_id: &str) -> Result<RetentionCounts> {
        let guardrails = self
            .get_organization_cairn_settings(organization_id)
            .await?
            .policy
            .guardrails;
        let mut removed = RetentionCounts::default();
        let run = |sql: String, cutoff: Option<String>| {
            let pool = self.pool.clone();
            async move {
                match cutoff {
                    None => Ok::<i64, anyhow::Error>(0),
                    Some(cutoff) => Ok(sqlx::query(&sql)
                        .bind(organization_id)
                        .bind(cutoff)
                        .execute(&pool)
                        .await?
                        .rows_affected() as i64),
                }
            }
        };
        removed.run_histories = run(
            format!("DELETE FROM agent_runs WHERE {IN_ORGANIZATION} AND started_at < ?2"),
            cutoff(guardrails.retention_trace_days),
        )
        .await?;
        removed.chats = run(
            format!(
                "DELETE FROM assistant_conversations WHERE origin = 'chat' AND {IN_ORGANIZATION} AND updated_at < ?2"
            ),
            cutoff(guardrails.retention_chat_days),
        )
        .await?;
        removed.personal_memories = run(
            format!("DELETE FROM agent_personal_memory WHERE {IN_ORGANIZATION} AND updated_at < ?2"),
            cutoff(guardrails.retention_memory_days),
        )
        .await?;
        Ok(removed)
    }
}
