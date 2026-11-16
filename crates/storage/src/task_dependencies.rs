//! Dependencies (V0.7.9): which tasks block which. A dependency is information: closing a blocker changes what the blocked task
//! shows and never its status. A self-link, a cycle and a link across workspaces cannot be made.

use super::*;

/// The most blockers one task can have, and the most tasks it can block.
pub const MAX_TASK_DEPENDENCIES: i64 = 50;

const SELECT_DEPENDENCY: &str = r#"
    SELECT dependency.id, dependency.created_at,
           blocker.id AS blocker_id, blocker.number AS blocker_number, blocker.title AS blocker_title, blocker.status AS blocker_status,
           blocked.id AS blocked_id, blocked.number AS blocked_number, blocked.title AS blocked_title, blocked.status AS blocked_status,
           adder.id AS adder_id, adder.display_name AS adder_name, adder.email AS adder_email
    FROM task_dependencies AS dependency
    JOIN workspace_tasks AS blocker ON blocker.id = dependency.blocker_task_id
    JOIN workspace_tasks AS blocked ON blocked.id = dependency.blocked_task_id
    LEFT JOIN users AS adder ON adder.id = dependency.created_by_user_id"#;

fn summary(row: &SqliteRow, prefix: &str) -> TaskSummary {
    let number: Option<i64> = row.get(format!("{prefix}_number").as_str());
    TaskSummary {
        id: row.get(format!("{prefix}_id").as_str()),
        key: format!("T-{}", number.unwrap_or(0)),
        title: row.get(format!("{prefix}_title").as_str()),
        status: row.get(format!("{prefix}_status").as_str()),
    }
}

fn dependency_from(row: &SqliteRow) -> TaskDependency {
    let adder_id: Option<String> = row.get("adder_id");
    TaskDependency {
        id: row.get("id"),
        blocker: summary(row, "blocker"),
        blocked: summary(row, "blocked"),
        created_by: adder_id.map(|id| SharedUser {
            id,
            display_name: row.get("adder_name"),
            email: row.get("adder_email"),
        }),
        created_at: row.get("created_at"),
    }
}

impl StorageEngine {
    /// What blocks a task, and what it blocks.
    pub async fn list_task_dependencies(&self, task_id: &str) -> Result<(Vec<TaskDependency>, Vec<TaskDependency>)> {
        let blocked_by = sqlx::query(&format!(
            "{SELECT_DEPENDENCY} WHERE dependency.blocked_task_id = ?1 ORDER BY blocker.number ASC"
        ))
        .bind(task_id)
        .fetch_all(self.reader())
        .await?;
        let blocks = sqlx::query(&format!(
            "{SELECT_DEPENDENCY} WHERE dependency.blocker_task_id = ?1 ORDER BY blocked.number ASC"
        ))
        .bind(task_id)
        .fetch_all(self.reader())
        .await?;
        Ok((
            blocked_by.iter().map(dependency_from).collect(),
            blocks.iter().map(dependency_from).collect(),
        ))
    }

    /// Whether `from` blocks `to`, directly or through other tasks.
    async fn task_blocks_through(&self, from: &str, to: &str) -> Result<bool> {
        let mut seen = std::collections::HashSet::new();
        let mut queue = VecDeque::from([from.to_owned()]);
        while let Some(at) = queue.pop_front() {
            if at == to {
                return Ok(true);
            }
            if !seen.insert(at.clone()) {
                continue;
            }
            let next: Vec<String> =
                sqlx::query_scalar("SELECT blocked_task_id FROM task_dependencies WHERE blocker_task_id = ?1")
                    .bind(&at)
                    .fetch_all(self.reader())
                    .await?;
            queue.extend(next);
        }
        Ok(false)
    }

    /// Makes `blocker` block `blocked`. Both tasks are of the workspace; a self-link, a repeat and a link that would close a
    /// cycle are refused with the reason.
    pub async fn add_task_dependency(
        &self,
        workspace_id: &str,
        blocker: &CollaborationTask,
        blocked: &CollaborationTask,
        created_by: &str,
    ) -> Result<TaskDependency> {
        if blocker.id == blocked.id {
            repomemo_domain::bail_invalid!("A task cannot block itself.");
        }
        if blocker.workspace_id != workspace_id || blocked.workspace_id != workspace_id {
            repomemo_domain::bail_invalid!("Both tasks must belong to this workspace.");
        }
        let repeated: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM task_dependencies WHERE blocker_task_id = ?1 AND blocked_task_id = ?2",
        )
        .bind(&blocker.id)
        .bind(&blocked.id)
        .fetch_one(&self.pool)
        .await?;
        if repeated > 0 {
            repomemo_domain::bail_conflict!("{} already blocks {}.", blocker.key, blocked.key);
        }
        if self.task_blocks_through(&blocked.id, &blocker.id).await? {
            repomemo_domain::bail_invalid!(
                "{} already blocks {}, directly or through other tasks, so this would make a cycle.",
                blocked.key,
                blocker.key
            );
        }
        for (task, column) in [(&blocked.id, "blocked_task_id"), (&blocker.id, "blocker_task_id")] {
            let held: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM task_dependencies WHERE {column} = ?1"))
                .bind(task)
                .fetch_one(&self.pool)
                .await?;
            if held >= MAX_TASK_DEPENDENCIES {
                repomemo_domain::bail_conflict!("A task can have at most {MAX_TASK_DEPENDENCIES} links of one kind.");
            }
        }
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO task_dependencies (id, workspace_id, blocker_task_id, blocked_task_id, created_by_user_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(&id)
        .bind(workspace_id)
        .bind(&blocker.id)
        .bind(&blocked.id)
        .bind(created_by)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        let row = sqlx::query(&format!("{SELECT_DEPENDENCY} WHERE dependency.id = ?1"))
            .bind(&id)
            .fetch_one(&self.pool)
            .await?;
        Ok(dependency_from(&row))
    }

    /// The workspace of a dependency, so the caller can check access before removing it.
    pub async fn task_dependency_workspace(&self, dependency_id: &str) -> Result<String> {
        sqlx::query_scalar("SELECT workspace_id FROM task_dependencies WHERE id = ?1")
            .bind(dependency_id)
            .fetch_optional(self.reader())
            .await?
            .ok_or_else(|| repomemo_domain::not_found!("The dependency was not found."))
    }

    pub async fn remove_task_dependency(&self, dependency_id: &str) -> Result<()> {
        let removed = sqlx::query("DELETE FROM task_dependencies WHERE id = ?1")
            .bind(dependency_id)
            .execute(&self.pool)
            .await?;
        if removed.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("The dependency was not found.");
        }
        Ok(())
    }
}
