//! CAIRN runs and their traces (V0.9.39). A trace is personal like its conversation: it goes with the chat and with the
//! account's anonymization (both through foreign keys), is not part of a workspace archive and is pruned after the retention
//! period. Steps hold one-line summaries and the files they named, never passage text.

use super::*;

/// What is stored when a message has been answered.
#[derive(Debug, Clone)]
pub struct NewAgentRun {
    pub turn_id: Option<String>,
    pub conversation_id: String,
    pub user_id: String,
    pub workspace_id: String,
    /// `chat`, `routine` or `inbox`.
    pub origin: String,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub tool_mode: Option<String>,
    pub state: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub steps: Vec<AgentStep>,
}

fn milliseconds_between(start: &str, end: Option<&str>) -> i64 {
    let parse = |text: &str| chrono::DateTime::parse_from_rfc3339(text).ok();
    match (parse(start), end.and_then(parse)) {
        (Some(start), Some(end)) => (end - start).num_milliseconds().max(0),
        _ => 0,
    }
}

impl StorageEngine {
    /// Stores a run and its steps in one transaction and returns the run's id.
    pub async fn record_agent_run(&self, run: NewAgentRun) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO agent_runs (id, turn_id, conversation_id, user_id, workspace_id, origin, provider_id, model, tool_mode, state, steps, input_tokens, output_tokens, started_at, ended_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        )
        .bind(&id)
        .bind(&run.turn_id)
        .bind(&run.conversation_id)
        .bind(&run.user_id)
        .bind(&run.workspace_id)
        .bind(&run.origin)
        .bind(&run.provider_id)
        .bind(&run.model)
        .bind(&run.tool_mode)
        .bind(&run.state)
        .bind(run.steps.len() as i64)
        .bind(run.input_tokens)
        .bind(run.output_tokens)
        .bind(&run.started_at)
        .bind(&run.ended_at)
        .execute(&mut *transaction)
        .await?;
        for step in &run.steps {
            sqlx::query(
                "INSERT INTO agent_steps (run_id, position, kind, tool, summary, evidence_json, duration_ms, error) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )
            .bind(&id)
            .bind(step.position)
            .bind(&step.kind)
            .bind(&step.tool)
            .bind(&step.summary)
            .bind(serde_json::to_string(&step.evidence)?)
            .bind(step.duration_ms)
            .bind(&step.error)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        Ok(id)
    }

    /// The run that answered a turn, with its steps; none for a turn from before runs were recorded, or one whose trace expired.
    pub async fn get_agent_run_for_turn(&self, turn_id: &str) -> Result<Option<AgentRun>> {
        let row = sqlx::query(
            "SELECT id, turn_id, conversation_id, workspace_id, origin, model, tool_mode, state, steps, input_tokens, output_tokens, started_at, ended_at FROM agent_runs WHERE turn_id = ?1 ORDER BY started_at DESC LIMIT 1",
        )
        .bind(turn_id)
        .fetch_optional(self.reader())
        .await?;
        let Some(row) = row else { return Ok(None) };
        let id: String = row.get("id");
        let step_rows = sqlx::query(
            "SELECT position, kind, tool, summary, evidence_json, duration_ms, error FROM agent_steps WHERE run_id = ?1 ORDER BY position",
        )
        .bind(&id)
        .fetch_all(self.reader())
        .await?;
        let steps = step_rows
            .into_iter()
            .map(|step| AgentStep {
                position: step.get("position"),
                kind: step.get("kind"),
                tool: step.get("tool"),
                summary: step.get("summary"),
                evidence: serde_json::from_str::<Vec<AgentStepEvidence>>(&step.get::<String, _>("evidence_json"))
                    .unwrap_or_default(),
                duration_ms: step.get("duration_ms"),
                error: step.get("error"),
            })
            .collect();
        let started_at: String = row.get("started_at");
        let ended_at: Option<String> = row.get("ended_at");
        Ok(Some(AgentRun {
            id,
            turn_id: row.get("turn_id"),
            conversation_id: row.get("conversation_id"),
            workspace_id: row.get("workspace_id"),
            origin: row.get("origin"),
            model: row.get("model"),
            tool_mode: row.get("tool_mode"),
            state: row.get("state"),
            step_count: row.get("steps"),
            input_tokens: row.get("input_tokens"),
            output_tokens: row.get("output_tokens"),
            duration_ms: milliseconds_between(&started_at, ended_at.as_deref()),
            started_at,
            ended_at,
            steps,
        }))
    }

    /// Deletes the traces that started before `cutoff` (RFC 3339), steps with them, and says how many runs went.
    pub async fn prune_agent_runs(&self, cutoff: &str) -> Result<u64> {
        let done = sqlx::query("DELETE FROM agent_runs WHERE started_at < ?1")
            .bind(cutoff)
            .execute(&self.pool)
            .await?;
        Ok(done.rows_affected())
    }

    /// How many runs are stored for a conversation.
    pub async fn count_agent_runs(&self, conversation_id: &str) -> Result<i64> {
        Ok(
            sqlx::query_scalar("SELECT COUNT(*) FROM agent_runs WHERE conversation_id = ?1")
                .bind(conversation_id)
                .fetch_one(self.reader())
                .await?,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::milliseconds_between;

    #[test]
    fn a_runs_duration_is_the_time_between_its_two_stamps() {
        assert_eq!(
            milliseconds_between("2026-11-23T10:00:00.000Z", Some("2026-11-23T10:00:02.250Z")),
            2250
        );
        assert_eq!(milliseconds_between("2026-11-23T10:00:00Z", None), 0);
        assert_eq!(milliseconds_between("bad", Some("worse")), 0);
    }
}
