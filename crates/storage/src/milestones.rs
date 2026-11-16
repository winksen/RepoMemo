//! Milestones (V0.7.7): named, dated groups of one project's tasks, with a stated completion rule and a count that says what it
//! is counting. A date never moves a task; dates are compared in UTC, a date without a time is overdue the day after it.

use super::*;

/// What a person can set on a milestone.
#[derive(Debug, Clone, Default)]
pub struct NewMilestone {
    pub name: String,
    pub description: String,
    pub due_date: Option<String>,
    /// `all_tasks` or `manual`.
    pub completion_rule: String,
}

const SELECT_MILESTONE: &str = r#"
    SELECT milestone.id, milestone.project_id, milestone.workspace_id, milestone.name, milestone.description,
           milestone.due_date, milestone.completion_rule, milestone.completed_at, milestone.created_at, milestone.updated_at,
           (SELECT COUNT(*) FROM workspace_tasks AS task WHERE task.milestone_id = milestone.id) AS task_count,
           (SELECT COUNT(*) FROM workspace_tasks AS task WHERE task.milestone_id = milestone.id AND task.status = 'done') AS done_count,
           (SELECT COUNT(*) FROM workspace_tasks AS task WHERE task.milestone_id = milestone.id AND task.status != 'done'
              AND task.due_at IS NOT NULL
              AND (CASE WHEN length(task.due_at) = 10 THEN task.due_at < ?1 ELSE task.due_at < ?2 END)) AS overdue_task_count
    FROM project_milestones AS milestone"#;

fn milestone_from(row: &SqliteRow, today: &str) -> Milestone {
    let rule: String = row.get("completion_rule");
    let completed_at: Option<String> = row.get("completed_at");
    let task_count: i64 = row.get("task_count");
    let done_count: i64 = row.get("done_count");
    let overdue_task_count: i64 = row.get("overdue_task_count");
    let due_date: Option<String> = row.get("due_date");
    let remaining_count = task_count - done_count;
    let completed = if rule == "manual" {
        completed_at.is_some()
    } else {
        task_count > 0 && remaining_count == 0
    };
    let overdue = !completed && due_date.as_deref().is_some_and(|due| due < today);
    let counted = match task_count {
        0 => "It has no tasks yet.".to_owned(),
        _ => format!(
            "{done_count} of {task_count} tasks are done and {remaining_count} remain{}.",
            if overdue_task_count > 0 {
                format!(", {overdue_task_count} of them overdue")
            } else {
                String::new()
            }
        ),
    };
    let rule_text = if rule == "manual" {
        "A person marks it complete."
    } else {
        "It is complete when it has tasks and every one is done."
    };
    Milestone {
        id: row.get("id"),
        project_id: row.get("project_id"),
        workspace_id: row.get("workspace_id"),
        name: row.get("name"),
        description: row.get("description"),
        due_date,
        completion_rule: rule,
        completed,
        completed_at,
        task_count,
        done_count,
        remaining_count,
        overdue_task_count,
        overdue,
        explanation: format!("{counted} {rule_text}"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

/// The date and the moment that "now" means when work is counted overdue.
fn clock() -> (String, String) {
    let now = Utc::now();
    (now.format("%Y-%m-%d").to_string(), now.to_rfc3339())
}

impl StorageEngine {
    pub async fn list_milestones(&self, project_id: &str) -> Result<Vec<Milestone>> {
        let (today, now) = clock();
        let rows = sqlx::query(&format!(
            "{SELECT_MILESTONE} WHERE milestone.project_id = ?3 ORDER BY milestone.due_date IS NULL, milestone.due_date ASC, milestone.created_at ASC"
        ))
        .bind(&today)
        .bind(&now)
        .bind(project_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows.iter().map(|row| milestone_from(row, &today)).collect())
    }

    pub async fn get_milestone(&self, milestone_id: &str) -> Result<Milestone> {
        let (today, now) = clock();
        sqlx::query(&format!("{SELECT_MILESTONE} WHERE milestone.id = ?3"))
            .bind(&today)
            .bind(&now)
            .bind(milestone_id)
            .fetch_optional(self.reader())
            .await?
            .map(|row| milestone_from(&row, &today))
            .ok_or_else(|| repomemo_domain::not_found!("Milestone was not found."))
    }

    pub async fn create_milestone(
        &self,
        project: &Project,
        created_by: &str,
        milestone: NewMilestone,
    ) -> Result<Milestone> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO project_milestones (id, project_id, workspace_id, name, description, due_date, completion_rule, created_by_user_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
        )
        .bind(&id)
        .bind(&project.id)
        .bind(&project.workspace_id)
        .bind(milestone.name)
        .bind(milestone.description)
        .bind(milestone.due_date)
        .bind(milestone.completion_rule)
        .bind(created_by)
        .bind(now)
        .execute(&self.pool)
        .await?;
        self.get_milestone(&id).await
    }

    /// Changes a milestone's name, description, date and rule. No task is touched, and the date moves no task's own date.
    pub async fn update_milestone(&self, milestone_id: &str, milestone: NewMilestone) -> Result<Milestone> {
        let changed = sqlx::query(
            "UPDATE project_milestones SET name = ?2, description = ?3, due_date = ?4, completion_rule = ?5, updated_at = ?6 WHERE id = ?1",
        )
        .bind(milestone_id)
        .bind(milestone.name)
        .bind(milestone.description)
        .bind(milestone.due_date)
        .bind(milestone.completion_rule)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        if changed.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("Milestone was not found.");
        }
        self.get_milestone(milestone_id).await
    }

    /// Marks a `manual` milestone complete or open again. A milestone that completes by rule cannot be set by hand.
    pub async fn set_milestone_completed(&self, milestone_id: &str, completed: bool) -> Result<Milestone> {
        let current = self.get_milestone(milestone_id).await?;
        if current.completion_rule != "manual" {
            repomemo_domain::bail_conflict!(
                "This milestone completes when all its tasks are done; change its rule to complete it by hand."
            );
        }
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE project_milestones SET completed_at = ?2, updated_at = ?3 WHERE id = ?1")
            .bind(milestone_id)
            .bind(completed.then(|| now.clone()))
            .bind(&now)
            .execute(&self.pool)
            .await?;
        self.get_milestone(milestone_id).await
    }

    /// Removes a milestone; its tasks stay in the project.
    pub async fn delete_milestone(&self, milestone_id: &str) -> Result<()> {
        let removed = sqlx::query("DELETE FROM project_milestones WHERE id = ?1")
            .bind(milestone_id)
            .execute(&self.pool)
            .await?;
        if removed.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("Milestone was not found.");
        }
        Ok(())
    }
}
