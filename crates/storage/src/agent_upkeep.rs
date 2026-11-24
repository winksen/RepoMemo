//! Memory upkeep (V0.9.56): the switch, the findings of the weekly pass and the few facts the pass reads that only the database holds.

use repomemo_domain::{UpkeepFinding, UpkeepSettings};

use super::*;

/// The most open findings a workspace keeps; a pass adds none past it.
pub const UPKEEP_OPEN_MAX: i64 = 100;
/// The most findings one pass adds.
pub const UPKEEP_PER_PASS_MAX: usize = 20;

/// A finding the pass adds.
#[derive(Debug, Clone)]
pub struct NewUpkeepFinding {
    /// `possible_contradiction`, `unanswered_question` or `unused_playbook`.
    pub kind: &'static str,
    /// Stable for the same facts, so a resolved finding is not raised again.
    pub fingerprint: String,
    pub title: String,
    pub detail: String,
    pub evidence: String,
    pub artifact_id: Option<String>,
}

/// One question that was asked, what was answered and by whom, as the pass reads it.
#[derive(Debug, Clone)]
pub struct AskedQuestion {
    pub label: String,
    pub answer: String,
    pub user_id: String,
}

fn finding_from(row: &SqliteRow) -> UpkeepFinding {
    UpkeepFinding {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        kind: row.get("kind"),
        title: row.get("title"),
        detail: row.get("detail"),
        evidence: row.get("evidence"),
        artifact_id: row.get("artifact_id"),
        created_at: row.get("created_at"),
    }
}

impl StorageEngine {
    pub async fn upkeep_settings(&self, workspace_id: &str) -> Result<UpkeepSettings> {
        let row = sqlx::query("SELECT enabled, last_run_at FROM agent_upkeep_settings WHERE workspace_id = ?1")
            .bind(workspace_id)
            .fetch_optional(self.reader())
            .await?;
        Ok(match row {
            Some(row) => UpkeepSettings {
                enabled: row.get::<i64, _>("enabled") != 0,
                last_run_at: row.get("last_run_at"),
            },
            None => UpkeepSettings {
                enabled: false,
                last_run_at: None,
            },
        })
    }

    pub async fn set_upkeep_enabled(&self, workspace_id: &str, enabled: bool) -> Result<()> {
        sqlx::query(
            "INSERT INTO agent_upkeep_settings (workspace_id, enabled) VALUES (?1, ?2) ON CONFLICT(workspace_id) DO UPDATE SET enabled = excluded.enabled",
        )
        .bind(workspace_id)
        .bind(i64::from(enabled))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// The workspaces whose upkeep is on and that were not run since `cutoff`.
    pub async fn workspaces_due_for_upkeep(&self, cutoff: &str) -> Result<Vec<String>> {
        Ok(sqlx::query_scalar(
            "SELECT workspace_id FROM agent_upkeep_settings WHERE enabled = 1 AND (last_run_at IS NULL OR last_run_at < ?1)",
        )
        .bind(cutoff)
        .fetch_all(self.reader())
        .await?)
    }

    pub async fn mark_upkeep_run(&self, workspace_id: &str) -> Result<()> {
        sqlx::query("UPDATE agent_upkeep_settings SET last_run_at = ?2 WHERE workspace_id = ?1")
            .bind(workspace_id)
            .bind(Utc::now().to_rfc3339())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Adds a finding unless the same facts were found before (open or resolved) or the workspace already holds the most it keeps.
    /// Returns whether one was added.
    pub async fn add_upkeep_finding(&self, workspace_id: &str, finding: &NewUpkeepFinding) -> Result<bool> {
        let open: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM agent_upkeep_findings WHERE workspace_id = ?1 AND state = 'open'")
                .bind(workspace_id)
                .fetch_one(self.reader())
                .await?;
        if open >= UPKEEP_OPEN_MAX {
            return Ok(false);
        }
        let done = sqlx::query(
            "INSERT OR IGNORE INTO agent_upkeep_findings (id, workspace_id, kind, fingerprint, title, detail, evidence, artifact_id, state, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'open', ?9)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(workspace_id)
        .bind(finding.kind)
        .bind(&finding.fingerprint)
        .bind(&finding.title)
        .bind(&finding.detail)
        .bind(&finding.evidence)
        .bind(&finding.artifact_id)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(done.rows_affected() > 0)
    }

    pub async fn open_upkeep_findings(&self, workspace_id: &str) -> Result<Vec<UpkeepFinding>> {
        let rows = sqlx::query(
            "SELECT id, workspace_id, kind, title, detail, evidence, artifact_id, created_at FROM agent_upkeep_findings WHERE workspace_id = ?1 AND state = 'open' ORDER BY created_at, id",
        )
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows.iter().map(finding_from).collect())
    }

    pub async fn resolve_upkeep_finding(&self, workspace_id: &str, finding_id: &str) -> Result<()> {
        let done = sqlx::query(
            "UPDATE agent_upkeep_findings SET state = 'resolved' WHERE id = ?1 AND workspace_id = ?2 AND state = 'open'",
        )
        .bind(finding_id)
        .bind(workspace_id)
        .execute(&self.pool)
        .await?;
        if done.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("That finding was not found.");
        }
        Ok(())
    }

    /// The questions asked in a workspace's chats since a date and how each was answered, newest first, at most `limit`. Only the
    /// pass reads them, and only to count how many people met the same unanswered question.
    pub async fn asked_questions_since(
        &self,
        workspace_id: &str,
        since: &str,
        limit: i64,
    ) -> Result<Vec<AskedQuestion>> {
        let rows = sqlx::query(
            "SELECT turn.label AS label, COALESCE(json_extract(turn.reply_json, '$.reply_markdown'), '') AS answer, conversation.user_id AS user_id \
             FROM assistant_turns turn JOIN assistant_conversations conversation ON conversation.id = turn.conversation_id \
             WHERE conversation.workspace_id = ?1 AND turn.created_at >= ?2 ORDER BY turn.created_at DESC LIMIT ?3",
        )
        .bind(workspace_id)
        .bind(since)
        .bind(limit)
        .fetch_all(self.reader())
        .await?;
        Ok(rows
            .iter()
            .map(|row| AskedQuestion {
                label: row.get("label"),
                answer: row.get("answer"),
                user_id: row.get("user_id"),
            })
            .collect())
    }

    /// Whether a run in the workspace read the playbook of that name since a date.
    pub async fn playbook_read_since(&self, workspace_id: &str, name: &str, since: &str) -> Result<bool> {
        let found: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM agent_steps step JOIN agent_runs run ON run.id = step.run_id \
             WHERE run.workspace_id = ?1 AND step.tool = 'load_playbook' AND run.started_at >= ?3 AND lower(step.summary) LIKE '%' || lower(?2) || '%' LIMIT 1",
        )
        .bind(workspace_id)
        .bind(name)
        .bind(since)
        .fetch_optional(self.reader())
        .await?;
        Ok(found.is_some())
    }
}
