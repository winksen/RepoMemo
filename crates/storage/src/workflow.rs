//! Workflow states (V0.7.11): named task states mapped to the four stable categories. The built-in states (`open`, `in_progress`,
//! `blocked`, `done`) always exist; a task's `status` is always its category. Changing a state's category, or removing a state,
//! moves the tasks in it explicitly, and removal is refused until it is told where they go.

use super::*;
use repomemo_domain::{builtin_state_name, WorkflowState, BUILTIN_STATE_CATEGORIES};

/// The most named states one workspace can add.
pub const MAX_CUSTOM_STATES: i64 = 20;

fn state_from(row: &SqliteRow) -> WorkflowState {
    WorkflowState {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        name: row.get("name"),
        category: row.get("category"),
        builtin: false,
        position: row.get("position"),
        task_count: row.get("task_count"),
    }
}

const SELECT_STATE: &str = r#"
    SELECT state.id, state.workspace_id, state.name, state.category, state.position,
           (SELECT COUNT(*) FROM workspace_tasks AS task WHERE task.state_id = state.id) AS task_count
    FROM workflow_states AS state"#;

/// Moves the tasks of a state: sets their state and status, keeps the completion time right (set when they enter `done`,
/// cleared when they leave it) and counts the move as a new version of each task.
async fn move_tasks(
    transaction: &mut sqlx::SqliteConnection,
    from_state: &str,
    to_state: Option<&str>,
    category: &str,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE workspace_tasks
         SET state_id = ?2, status = ?3, completed_at = CASE WHEN ?3 = 'done' THEN COALESCE(completed_at, ?4) ELSE NULL END,
             updated_at = ?4, version = version + 1
         WHERE state_id = ?1",
    )
    .bind(from_state)
    .bind(to_state)
    .bind(category)
    .bind(now)
    .execute(transaction)
    .await?;
    Ok(())
}

impl StorageEngine {
    /// The four built-in states, then the workspace's own, each with how many tasks are in it.
    pub async fn list_workflow_states(&self, workspace_id: &str) -> Result<Vec<WorkflowState>> {
        let counts = sqlx::query(
            "SELECT status, COUNT(*) AS held FROM workspace_tasks WHERE workspace_id = ?1 AND state_id IS NULL GROUP BY status",
        )
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?;
        let held = |category: &str| {
            counts
                .iter()
                .find(|row| row.get::<String, _>("status") == category)
                .map_or(0, |row| row.get::<i64, _>("held"))
        };
        let mut states: Vec<WorkflowState> = BUILTIN_STATE_CATEGORIES
            .iter()
            .enumerate()
            .map(|(position, category)| WorkflowState {
                id: (*category).to_owned(),
                workspace_id: workspace_id.to_owned(),
                name: builtin_state_name(category).to_owned(),
                category: (*category).to_owned(),
                builtin: true,
                position: position as i64,
                task_count: held(category),
            })
            .collect();
        let custom = sqlx::query(&format!(
            "{SELECT_STATE} WHERE state.workspace_id = ?1 ORDER BY state.position ASC, state.name_key ASC"
        ))
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?;
        states.extend(custom.iter().map(state_from));
        Ok(states)
    }

    /// A state the workspace added. The built-in ones are not rows and answer "not found".
    pub async fn get_workflow_state(&self, state_id: &str) -> Result<WorkflowState> {
        sqlx::query(&format!("{SELECT_STATE} WHERE state.id = ?1"))
            .bind(state_id)
            .fetch_optional(self.reader())
            .await?
            .map(|row| state_from(&row))
            .ok_or_else(|| repomemo_domain::not_found!("Workflow state was not found."))
    }

    async fn check_state_name(&self, workspace_id: &str, name: &str, except: Option<&str>) -> Result<()> {
        let key = name.to_lowercase();
        if BUILTIN_STATE_CATEGORIES
            .iter()
            .any(|category| builtin_state_name(category).to_lowercase() == key)
        {
            repomemo_domain::bail_conflict!("\"{name}\" is the name of a built-in state.");
        }
        let taken: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM workflow_states WHERE workspace_id = ?1 AND name_key = ?2 AND id != COALESCE(?3, '')",
        )
        .bind(workspace_id)
        .bind(&key)
        .bind(except)
        .fetch_one(&self.pool)
        .await?;
        if taken > 0 {
            repomemo_domain::bail_conflict!("A state named \"{name}\" already exists.");
        }
        Ok(())
    }

    pub async fn create_workflow_state(&self, workspace_id: &str, name: &str, category: &str) -> Result<WorkflowState> {
        self.check_state_name(workspace_id, name, None).await?;
        let held: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workflow_states WHERE workspace_id = ?1")
            .bind(workspace_id)
            .fetch_one(&self.pool)
            .await?;
        if held >= MAX_CUSTOM_STATES {
            repomemo_domain::bail_conflict!("A workspace can add at most {MAX_CUSTOM_STATES} states.");
        }
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO workflow_states (id, workspace_id, name, name_key, category, position, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        )
        .bind(&id)
        .bind(workspace_id)
        .bind(name)
        .bind(name.to_lowercase())
        .bind(category)
        .bind(held)
        .bind(now)
        .execute(&self.pool)
        .await?;
        self.get_workflow_state(&id).await
    }

    /// Renames a state or maps it to another category. Tasks in it follow the new category (their completion time with it).
    pub async fn update_workflow_state(&self, state_id: &str, name: &str, category: &str) -> Result<WorkflowState> {
        let current = self.get_workflow_state(state_id).await?;
        self.check_state_name(&current.workspace_id, name, Some(state_id))
            .await?;
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "UPDATE workflow_states SET name = ?2, name_key = ?3, category = ?4, updated_at = ?5 WHERE id = ?1",
        )
        .bind(state_id)
        .bind(name)
        .bind(name.to_lowercase())
        .bind(category)
        .bind(Utc::now().to_rfc3339())
        .execute(&mut *transaction)
        .await?;
        if current.category != category {
            move_tasks(&mut transaction, state_id, Some(state_id), category).await?;
        }
        transaction.commit().await?;
        self.get_workflow_state(state_id).await
    }

    /// Removes a state. When tasks are in it, `move_to` (a built-in category name or another state's id) says where they go;
    /// without it the removal is refused with how many tasks are in the way. Returns how many tasks were moved.
    pub async fn delete_workflow_state(&self, state_id: &str, move_to: Option<&str>) -> Result<i64> {
        let current = self.get_workflow_state(state_id).await?;
        let target: Option<(Option<String>, String)> = match move_to {
            None => None,
            Some(id) if BUILTIN_STATE_CATEGORIES.contains(&id) => Some((None, id.to_owned())),
            Some(id) if id == state_id => {
                repomemo_domain::bail_invalid!("Tasks cannot be moved to the state being removed.")
            }
            Some(id) => {
                let other = self
                    .get_workflow_state(id)
                    .await
                    .map_err(|_| repomemo_domain::invalid!("The state to move the tasks to was not found."))?;
                if other.workspace_id != current.workspace_id {
                    repomemo_domain::bail_invalid!("The state to move the tasks to must belong to this workspace.");
                }
                Some((Some(other.id), other.category))
            }
        };
        if current.task_count > 0 && target.is_none() {
            repomemo_domain::bail_conflict!(
                "{} task(s) are in \"{}\"; say where to move them before removing it.",
                current.task_count,
                current.name
            );
        }
        let mut transaction = self.pool.begin().await?;
        if let Some((state, category)) = &target {
            move_tasks(&mut transaction, state_id, state.as_deref(), category).await?;
        }
        sqlx::query("DELETE FROM workflow_states WHERE id = ?1")
            .bind(state_id)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(current.task_count)
    }
}
