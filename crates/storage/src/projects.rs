//! Projects (V0.7.1): a named piece of work inside one workspace. A project is a label with an owner, a goal, a lifecycle and
//! a target date; it grants no access, and archiving it keeps the project and its tasks readable.

use super::*;

/// What a person can set on a project. `status` is the lifecycle: `active`, `on_hold`, `done` (or `archived`, which
/// `set_project_archived` sets and unsets).
#[derive(Debug, Clone, Default)]
pub struct NewProject {
    pub name: String,
    pub goal: String,
    pub status: String,
    pub owner_user_id: Option<String>,
    pub target_date: Option<String>,
}

const SELECT_PROJECT: &str = r#"
    SELECT project.id, project.workspace_id, project.name, project.goal, project.status, project.target_date,
           project.archived_at, project.created_at, project.updated_at,
           owner.id AS owner_id, owner.display_name AS owner_name, owner.email AS owner_email,
           creator.id AS creator_id, creator.display_name AS creator_name, creator.email AS creator_email,
           (SELECT COUNT(*) FROM workspace_tasks AS task WHERE task.project_id = project.id) AS task_count,
           (SELECT COUNT(*) FROM workspace_tasks AS task WHERE task.project_id = project.id AND task.status != 'done') AS open_task_count
    FROM workspace_projects AS project
    LEFT JOIN users AS owner ON owner.id = project.owner_user_id
    LEFT JOIN users AS creator ON creator.id = project.created_by_user_id"#;

fn project_person(row: &SqliteRow, prefix: &str) -> Option<SharedUser> {
    let id: Option<String> = row.get(format!("{prefix}_id").as_str());
    id.map(|id| SharedUser {
        id,
        display_name: row.get(format!("{prefix}_name").as_str()),
        email: row.get(format!("{prefix}_email").as_str()),
    })
}

fn project_from(row: &SqliteRow) -> Project {
    Project {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        name: row.get("name"),
        goal: row.get("goal"),
        status: row.get("status"),
        owner: project_person(row, "owner"),
        created_by: project_person(row, "creator"),
        target_date: row.get("target_date"),
        task_count: row.get("task_count"),
        open_task_count: row.get("open_task_count"),
        archived_at: row.get("archived_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

impl StorageEngine {
    /// A workspace's projects, newest change first. `status` narrows to one lifecycle state; without it the archived ones are
    /// left out.
    pub async fn list_projects(&self, workspace_id: &str, status: Option<&str>) -> Result<Vec<Project>> {
        let filter = match status {
            Some(_) => "AND project.status = ?2",
            None => "AND project.status != 'archived' AND ?2 IS NULL",
        };
        let rows = sqlx::query(&format!(
            "{SELECT_PROJECT} WHERE project.workspace_id = ?1 {filter} ORDER BY project.updated_at DESC, project.name ASC"
        ))
        .bind(workspace_id)
        .bind(status)
        .fetch_all(self.reader())
        .await?;
        Ok(rows.iter().map(project_from).collect())
    }

    pub async fn get_project(&self, project_id: &str) -> Result<Project> {
        sqlx::query(&format!("{SELECT_PROJECT} WHERE project.id = ?1"))
            .bind(project_id)
            .fetch_optional(self.reader())
            .await?
            .map(|row| project_from(&row))
            .ok_or_else(|| repomemo_domain::not_found!("Project was not found."))
    }

    pub async fn create_project(&self, workspace_id: &str, created_by: &str, project: NewProject) -> Result<Project> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO workspace_projects (id, workspace_id, name, goal, status, owner_user_id, created_by_user_id, target_date, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
        )
        .bind(&id)
        .bind(workspace_id)
        .bind(project.name)
        .bind(project.goal)
        .bind(project.status)
        .bind(project.owner_user_id)
        .bind(created_by)
        .bind(project.target_date)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.get_project(&id).await
    }

    /// Replaces the project's name, goal, lifecycle, owner and target date. An archived project must be restored first.
    pub async fn update_project(&self, project_id: &str, project: NewProject) -> Result<Project> {
        let changed = sqlx::query(
            "UPDATE workspace_projects SET name = ?2, goal = ?3, status = ?4, owner_user_id = ?5, target_date = ?6, updated_at = ?7
             WHERE id = ?1 AND status != 'archived'",
        )
        .bind(project_id)
        .bind(project.name)
        .bind(project.goal)
        .bind(project.status)
        .bind(project.owner_user_id)
        .bind(project.target_date)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        if changed.rows_affected() == 0 {
            // Either it does not exist (not found) or it is archived (a conflict the caller words).
            self.get_project(project_id).await?;
            repomemo_domain::bail_conflict!("An archived project is restored before it is changed.");
        }
        self.get_project(project_id).await
    }

    /// Archives a project (it and its tasks stay readable), or restores it to `active`.
    pub async fn set_project_archived(&self, project_id: &str, archived: bool) -> Result<Project> {
        let now = Utc::now().to_rfc3339();
        let changed = if archived {
            sqlx::query(
                "UPDATE workspace_projects SET status = 'archived', archived_at = ?2, updated_at = ?2 WHERE id = ?1",
            )
        } else {
            sqlx::query(
                "UPDATE workspace_projects SET status = 'active', archived_at = NULL, updated_at = ?2 WHERE id = ?1",
            )
        }
        .bind(project_id)
        .bind(now)
        .execute(&self.pool)
        .await?;
        if changed.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("Project was not found.");
        }
        self.get_project(project_id).await
    }
}
