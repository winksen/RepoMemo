//! CAIRN's usage and health (V0.9.102): runs, tokens, failures, fallbacks, steps and time, grouped by organization, workspace and model,
//! counted from the stored runs. Aggregates only: a group says how many people are in it, and nothing here reads a message, the name of
//! a chat or who ran what.

use super::*;

/// One group of runs.
#[derive(Debug, Clone, serde::Serialize)]
pub struct UsageGroup {
    pub organization_id: Option<String>,
    pub organization_name: String,
    pub workspace_id: String,
    pub workspace_name: String,
    pub model: String,
    pub runs: i64,
    pub tokens: i64,
    /// Runs that failed or ended partial.
    pub failures: i64,
    /// Runs a fallback model answered (V0.9.80).
    pub fallbacks: i64,
    pub average_steps: f64,
    pub average_seconds: f64,
    /// How many different people made the runs.
    pub people: i64,
}

impl StorageEngine {
    /// The groups of runs started in the last `days` days, in the organizations given (every organization for `None`).
    pub async fn cairn_usage(&self, days: i64, organizations: Option<&[String]>) -> Result<Vec<UsageGroup>> {
        let since = (Utc::now() - chrono::Duration::days(days.max(1))).to_rfc3339();
        let wanted = serde_json::to_string(&organizations.unwrap_or_default())?;
        let rows = sqlx::query(
            r#"
            SELECT wo.organization_id AS organization_id, COALESCE(o.name, '') AS organization_name,
                   r.workspace_id AS workspace_id, w.name AS workspace_name, COALESCE(r.model, '') AS model,
                   COUNT(*) AS runs,
                   COALESCE(SUM(r.input_tokens + r.output_tokens), 0) AS tokens,
                   COALESCE(SUM(CASE WHEN r.state IN ('failed', 'partial') THEN 1 ELSE 0 END), 0) AS failures,
                   COALESCE(SUM(CASE WHEN EXISTS (SELECT 1 FROM agent_steps s WHERE s.run_id = r.id AND s.summary LIKE 'Fallback:%') THEN 1 ELSE 0 END), 0) AS fallbacks,
                   COALESCE(AVG(r.steps), 0) AS average_steps,
                   COALESCE(AVG((julianday(r.ended_at) - julianday(r.started_at)) * 86400.0), 0) AS average_seconds,
                   COUNT(DISTINCT r.user_id) AS people
            FROM agent_runs r
            JOIN workspaces w ON w.id = r.workspace_id
            LEFT JOIN workspace_organizations wo ON wo.workspace_id = r.workspace_id
            LEFT JOIN organizations o ON o.id = wo.organization_id
            WHERE r.started_at >= ?1 AND (?2 = 1 OR wo.organization_id IN (SELECT value FROM json_each(?3)))
            GROUP BY wo.organization_id, r.workspace_id, COALESCE(r.model, '')
            ORDER BY organization_name, workspace_name, model
            "#,
        )
        .bind(since)
        .bind(organizations.is_none())
        .bind(wanted)
        .fetch_all(self.reader())
        .await?;
        Ok(rows
            .iter()
            .map(|row| UsageGroup {
                organization_id: row.get("organization_id"),
                organization_name: row.get("organization_name"),
                workspace_id: row.get("workspace_id"),
                workspace_name: row.get("workspace_name"),
                model: row.get("model"),
                runs: row.get("runs"),
                tokens: row.get("tokens"),
                failures: row.get("failures"),
                fallbacks: row.get("fallbacks"),
                average_steps: row.get("average_steps"),
                average_seconds: row.get("average_seconds"),
                people: row.get("people"),
            })
            .collect())
    }
}
