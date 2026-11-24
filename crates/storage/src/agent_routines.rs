//! CAIRN routines (V0.9.63): saved prompts CAIRN runs on a schedule as their owner. The storage keeps the routine, its next date and
//! how many runs failed in a row; the server runs it.

use chrono::NaiveDate;
use repomemo_domain::{AgentRoutine, RecurringSchedule};

use super::*;
use crate::recurring::{check_schedule, occurrence_dates};

/// The most routines a workspace has, the characters of a title and of a prompt, and the failures in a row that pause a routine.
pub const ROUTINES_MAX: i64 = 10;
pub const ROUTINE_TITLE_CHARS: usize = 80;
pub const ROUTINE_PROMPT_CHARS: usize = 2_000;
pub const ROUTINE_FAILURES_MAX: i64 = 3;

/// A routine to make.
pub struct NewRoutine {
    pub workspace_id: String,
    pub owner_id: String,
    pub title: String,
    pub prompt: String,
    pub schedule: RecurringSchedule,
}

/// The first date on or after `from` that a schedule falls on, as `YYYY-MM-DD`.
pub fn next_run_on_or_after(schedule: &RecurringSchedule, from: NaiveDate) -> Option<String> {
    let before = from.pred_opt()?;
    let until = from.checked_add_signed(chrono::Duration::days(800))?;
    occurrence_dates(schedule, Some(before), until, 1)
        .first()
        .map(|date| date.format("%Y-%m-%d").to_string())
}

fn routine_from(row: &SqliteRow) -> AgentRoutine {
    AgentRoutine {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        owner_id: row.get("owner_id"),
        owner_name: row.try_get("owner_name").unwrap_or_default(),
        title: row.get("title"),
        prompt: row.get("prompt"),
        schedule: serde_json::from_str(&row.get::<String, _>("schedule_json")).unwrap_or(RecurringSchedule {
            kind: "weekly".to_owned(),
            interval: 1,
            weekday: Some(0),
            day_of_month: None,
            start_date: "2026-01-01".to_owned(),
        }),
        next_run_date: row.get("next_run_date"),
        state: row.get("state"),
        pause_reason: row.get("pause_reason"),
        failures: row.get("failures"),
        last_run_at: row.get("last_run_at"),
        conversation_id: row.get("conversation_id"),
    }
}

const SELECT: &str = "SELECT routine.*, owner.display_name AS owner_name FROM agent_routines routine JOIN users owner ON owner.id = routine.owner_id";

impl StorageEngine {
    pub async fn create_agent_routine(&self, routine: NewRoutine) -> Result<AgentRoutine> {
        let title = routine.title.split_whitespace().collect::<Vec<_>>().join(" ");
        let prompt = routine.prompt.trim().to_owned();
        if title.is_empty() || title.chars().count() > ROUTINE_TITLE_CHARS {
            repomemo_domain::bail_invalid!("A routine's title is 1 to {ROUTINE_TITLE_CHARS} characters.");
        }
        if prompt.is_empty() || prompt.chars().count() > ROUTINE_PROMPT_CHARS {
            repomemo_domain::bail_invalid!("A routine's prompt is 1 to {ROUTINE_PROMPT_CHARS} characters.");
        }
        if let Err(problem) = check_schedule(&routine.schedule) {
            repomemo_domain::bail_invalid!("{problem}");
        }
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_routines WHERE workspace_id = ?1")
            .bind(&routine.workspace_id)
            .fetch_one(self.reader())
            .await?;
        if count >= ROUTINES_MAX {
            repomemo_domain::bail_conflict!("A workspace has at most {ROUTINES_MAX} routines. Delete one first.");
        }
        let Some(next) = next_run_on_or_after(&routine.schedule, Utc::now().date_naive()) else {
            repomemo_domain::bail_invalid!("That schedule never runs.");
        };
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO agent_routines (id, workspace_id, owner_id, title, prompt, schedule_json, next_run_date, state, failures, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'active', 0, ?8)",
        )
        .bind(&id)
        .bind(&routine.workspace_id)
        .bind(&routine.owner_id)
        .bind(&title)
        .bind(&prompt)
        .bind(serde_json::to_string(&routine.schedule)?)
        .bind(&next)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        self.get_agent_routine(&id).await
    }

    pub async fn get_agent_routine(&self, routine_id: &str) -> Result<AgentRoutine> {
        let row = sqlx::query(&format!("{SELECT} WHERE routine.id = ?1"))
            .bind(routine_id)
            .fetch_optional(self.reader())
            .await?
            .ok_or_else(|| repomemo_domain::not_found!("That routine was not found."))?;
        Ok(routine_from(&row))
    }

    pub async fn list_agent_routines(&self, workspace_id: &str) -> Result<Vec<AgentRoutine>> {
        let rows = sqlx::query(&format!(
            "{SELECT} WHERE routine.workspace_id = ?1 ORDER BY routine.created_at"
        ))
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows.iter().map(routine_from).collect())
    }

    pub async fn delete_agent_routine(&self, routine_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM agent_routines WHERE id = ?1")
            .bind(routine_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The active routines whose date has come, oldest first.
    pub async fn due_agent_routines(&self, today: &str) -> Result<Vec<AgentRoutine>> {
        let rows = sqlx::query(&format!(
            "{SELECT} WHERE routine.state = 'active' AND routine.next_run_date <= ?1 ORDER BY routine.next_run_date, routine.created_at LIMIT 50"
        ))
        .bind(today)
        .fetch_all(self.reader())
        .await?;
        Ok(rows.iter().map(routine_from).collect())
    }

    /// Pauses a routine and says why; a person resumes it.
    pub async fn pause_agent_routine(&self, routine_id: &str, reason: &str) -> Result<()> {
        sqlx::query("UPDATE agent_routines SET state = 'paused', pause_reason = ?2 WHERE id = ?1")
            .bind(routine_id)
            .bind(reason)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Resumes a routine from its next date, with its failures forgotten.
    pub async fn resume_agent_routine(&self, routine_id: &str) -> Result<AgentRoutine> {
        let routine = self.get_agent_routine(routine_id).await?;
        let next = next_run_on_or_after(&routine.schedule, Utc::now().date_naive()).unwrap_or(routine.next_run_date);
        sqlx::query(
            "UPDATE agent_routines SET state = 'active', pause_reason = NULL, failures = 0, next_run_date = ?2 WHERE id = ?1",
        )
        .bind(routine_id)
        .bind(next)
        .execute(&self.pool)
        .await?;
        self.get_agent_routine(routine_id).await
    }

    /// Records how a run went: a success clears the failures and moves to the next date, and the third failure in a row pauses the
    /// routine. A failed run is not tried again before its next date.
    pub async fn record_agent_routine_run(
        &self,
        routine_id: &str,
        succeeded: bool,
        conversation_id: Option<&str>,
        failure: &str,
    ) -> Result<AgentRoutine> {
        let routine = self.get_agent_routine(routine_id).await?;
        let today = Utc::now().date_naive();
        let tomorrow = today.succ_opt().unwrap_or(today);
        let next = next_run_on_or_after(&routine.schedule, tomorrow).unwrap_or(routine.next_run_date.clone());
        let failures = if succeeded { 0 } else { routine.failures + 1 };
        let pause = !succeeded && failures >= ROUTINE_FAILURES_MAX;
        sqlx::query(
            "UPDATE agent_routines SET failures = ?2, next_run_date = ?3, last_run_at = ?4, conversation_id = COALESCE(?5, conversation_id), state = CASE WHEN ?6 THEN 'paused' ELSE state END, pause_reason = CASE WHEN ?6 THEN ?7 ELSE pause_reason END WHERE id = ?1",
        )
        .bind(routine_id)
        .bind(failures)
        .bind(next)
        .bind(Utc::now().to_rfc3339())
        .bind(conversation_id)
        .bind(pause)
        .bind(format!("It failed {ROUTINE_FAILURES_MAX} times in a row: {failure}"))
        .execute(&self.pool)
        .await?;
        self.get_agent_routine(routine_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn weekly_monday() -> RecurringSchedule {
        RecurringSchedule {
            kind: "weekly".to_owned(),
            interval: 1,
            weekday: Some(0),
            day_of_month: None,
            start_date: "2026-01-01".to_owned(),
        }
    }

    #[test]
    fn the_next_run_is_the_first_matching_day_on_or_after_a_date() {
        // 2026-11-24 is a Tuesday; the next Monday is 2026-11-30, and a Monday itself counts.
        let tuesday = NaiveDate::from_ymd_opt(2026, 11, 24).unwrap();
        assert_eq!(
            next_run_on_or_after(&weekly_monday(), tuesday).as_deref(),
            Some("2026-11-30")
        );
        let monday = NaiveDate::from_ymd_opt(2026, 11, 30).unwrap();
        assert_eq!(
            next_run_on_or_after(&weekly_monday(), monday).as_deref(),
            Some("2026-11-30")
        );
    }
}
