//! The task list (V0.7.12): one query model for a workspace's tasks, read a page at a time, and saved views of it. A view stores
//! only filters; it is evaluated against the reader's current rights each time, and `me` means the reader.

use super::*;
use crate::collaboration::TASK_SELECT;
use repomemo_domain::{TaskPage, TaskQuery, TaskView};

/// The most tasks one page holds.
pub const MAX_TASK_PAGE: i64 = 200;
/// The page size when none is asked for.
pub const DEFAULT_TASK_PAGE: i64 = 50;
/// The most saved views one person can have in a workspace.
pub const MAX_VIEWS_PER_USER: i64 = 50;

fn like_pattern(text: &str) -> String {
    let escaped = text.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    format!("%{}%", escaped.to_lowercase())
}

/// Adds the filters of a query to a builder that already ends in `WHERE <something>`.
fn push_filters(builder: &mut QueryBuilder<'_, Sqlite>, query: &TaskQuery, requester: &str) {
    let now = Utc::now();
    let today = now.format("%Y-%m-%d").to_string();
    if let Some(project) = &query.project_id {
        if project == "none" {
            builder.push(" AND task.project_id IS NULL");
        } else {
            builder.push(" AND task.project_id = ").push_bind(project.clone());
        }
    }
    if let Some(milestone) = &query.milestone_id {
        if milestone == "none" {
            builder.push(" AND task.milestone_id IS NULL");
        } else {
            builder.push(" AND task.milestone_id = ").push_bind(milestone.clone());
        }
    }
    if let Some(assignee) = &query.assignee {
        match assignee.as_str() {
            "none" => {
                builder.push(" AND task.assignee_user_id IS NULL");
            }
            "me" => {
                builder
                    .push(" AND task.assignee_user_id = ")
                    .push_bind(requester.to_owned());
            }
            other => {
                builder
                    .push(" AND task.assignee_user_id = ")
                    .push_bind(other.to_owned());
            }
        }
    }
    for (column, values) in [("priority", &query.priority), ("status", &query.status)] {
        if !values.is_empty() {
            builder.push(format!(" AND task.{column} IN ("));
            let mut separated = builder.separated(", ");
            for value in values {
                separated.push_bind(value.clone());
            }
            separated.push_unseparated(")");
        }
    }
    if let Some(state) = &query.state_id {
        if repomemo_domain::BUILTIN_STATE_CATEGORIES.contains(&state.as_str()) {
            builder
                .push(" AND task.state_id IS NULL AND task.status = ")
                .push_bind(state.clone());
        } else {
            builder.push(" AND task.state_id = ").push_bind(state.clone());
        }
    }
    if let Some(label) = &query.label_id {
        builder
            .push(" AND EXISTS (SELECT 1 FROM task_label_links AS wanted WHERE wanted.task_id = task.id AND wanted.label_id = ")
            .push_bind(label.clone())
            .push(")");
    }
    if let Some(before) = &query.due_before {
        builder
            .push(" AND task.due_at IS NOT NULL AND substr(task.due_at, 1, 10) <= ")
            .push_bind(before.clone());
    }
    if let Some(after) = &query.due_after {
        builder
            .push(" AND task.due_at IS NOT NULL AND substr(task.due_at, 1, 10) >= ")
            .push_bind(after.clone());
    }
    if query.overdue {
        builder
            .push(" AND task.status != 'done' AND task.due_at IS NOT NULL AND (CASE WHEN length(task.due_at) = 10 THEN task.due_at < ")
            .push_bind(today)
            .push(" ELSE task.due_at < ")
            .push_bind(now.to_rfc3339())
            .push(" END)");
    }
    if let Some(text) = query.q.as_deref().filter(|text| !text.is_empty()) {
        builder
            .push(" AND lower(task.title) LIKE ")
            .push_bind(like_pattern(text))
            .push(" ESCAPE '\\'");
    }
}

/// The ORDER BY of a query. Every order ends on the task's number, so a page never shuffles between requests.
fn order_by(sort: Option<&str>) -> &'static str {
    match sort {
        Some("due") => " ORDER BY task.due_at IS NULL, task.due_at ASC, task.number ASC",
        Some("updated") => " ORDER BY task.updated_at DESC, task.number ASC",
        Some("number") => " ORDER BY task.number ASC",
        _ => {
            " ORDER BY CASE task.status WHEN 'in_progress' THEN 0 WHEN 'blocked' THEN 1 WHEN 'open' THEN 2 ELSE 3 END, \
             CASE task.priority WHEN 'urgent' THEN 0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2 ELSE 3 END, \
             task.due_at IS NULL, task.due_at ASC, task.number ASC"
        }
    }
}

fn view_from(row: &SqliteRow) -> TaskView {
    let query_json: String = row.get("query_json");
    TaskView {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        name: row.get("name"),
        query: serde_json::from_str(&query_json).unwrap_or_default(),
        shared: row.get("shared"),
        owner: SharedUser {
            id: row.get("owner_id"),
            display_name: row.get("owner_name"),
            email: row.get("owner_email"),
        },
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

const SELECT_VIEW: &str = r#"
    SELECT view.id, view.workspace_id, view.name, view.query_json, view.shared, view.created_at, view.updated_at,
           owner.id AS owner_id, owner.display_name AS owner_name, owner.email AS owner_email
    FROM saved_task_views AS view
    JOIN users AS owner ON owner.id = view.owner_user_id"#;

impl StorageEngine {
    /// One page of a workspace's tasks that match a query, with the total that match. `requester` is who is reading: it is what
    /// `me` means, and nothing else about them changes what matches (the caller has already checked they may read).
    pub async fn query_collaboration_tasks(
        &self,
        workspace_id: &str,
        query: &TaskQuery,
        requester: &str,
        limit: i64,
        offset: i64,
    ) -> Result<TaskPage> {
        let limit = limit.clamp(1, MAX_TASK_PAGE);
        let offset = offset.max(0);
        let mut count =
            QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM workspace_tasks AS task WHERE task.workspace_id = ");
        count.push_bind(workspace_id.to_owned());
        push_filters(&mut count, query, requester);
        let total: i64 = count.build_query_scalar().fetch_one(self.reader()).await?;
        let mut page = QueryBuilder::<Sqlite>::new(format!("{TASK_SELECT} WHERE task.workspace_id = "));
        page.push_bind(workspace_id.to_owned());
        push_filters(&mut page, query, requester);
        page.push(order_by(query.sort.as_deref()));
        page.push(" LIMIT ").push_bind(limit).push(" OFFSET ").push_bind(offset);
        let rows = page
            .build_query_as::<CollaborationTaskRow>()
            .fetch_all(self.reader())
            .await?;
        let tasks: Vec<CollaborationTask> = rows.into_iter().map(CollaborationTask::from).collect();
        let next = offset + tasks.len() as i64;
        Ok(TaskPage {
            next_offset: (next < total).then_some(next),
            tasks,
            total,
            limit,
            offset,
        })
    }

    /// The views a person can use in a workspace: their own and the ones shared with everyone.
    pub async fn list_task_views(&self, workspace_id: &str, user_id: &str) -> Result<Vec<TaskView>> {
        let rows = sqlx::query(&format!(
            "{SELECT_VIEW} WHERE view.workspace_id = ?1 AND (view.shared = 1 OR view.owner_user_id = ?2)
             ORDER BY view.name COLLATE NOCASE ASC"
        ))
        .bind(workspace_id)
        .bind(user_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows.iter().map(view_from).collect())
    }

    pub async fn get_task_view(&self, view_id: &str) -> Result<TaskView> {
        sqlx::query(&format!("{SELECT_VIEW} WHERE view.id = ?1"))
            .bind(view_id)
            .fetch_optional(self.reader())
            .await?
            .map(|row| view_from(&row))
            .ok_or_else(|| repomemo_domain::not_found!("View was not found."))
    }

    pub async fn create_task_view(
        &self,
        workspace_id: &str,
        owner_user_id: &str,
        name: &str,
        query: &TaskQuery,
    ) -> Result<TaskView> {
        let held: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM saved_task_views WHERE workspace_id = ?1 AND owner_user_id = ?2")
                .bind(workspace_id)
                .bind(owner_user_id)
                .fetch_one(&self.pool)
                .await?;
        if held >= MAX_VIEWS_PER_USER {
            repomemo_domain::bail_conflict!("You can save at most {MAX_VIEWS_PER_USER} views in a workspace.");
        }
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO saved_task_views (id, workspace_id, owner_user_id, name, query_json, shared, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?6)",
        )
        .bind(&id)
        .bind(workspace_id)
        .bind(owner_user_id)
        .bind(name)
        .bind(serde_json::to_string(query)?)
        .bind(now)
        .execute(&self.pool)
        .await?;
        self.get_task_view(&id).await
    }

    pub async fn update_task_view(&self, view_id: &str, name: &str, query: &TaskQuery) -> Result<TaskView> {
        let changed =
            sqlx::query("UPDATE saved_task_views SET name = ?2, query_json = ?3, updated_at = ?4 WHERE id = ?1")
                .bind(view_id)
                .bind(name)
                .bind(serde_json::to_string(query)?)
                .bind(Utc::now().to_rfc3339())
                .execute(&self.pool)
                .await?;
        if changed.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("View was not found.");
        }
        self.get_task_view(view_id).await
    }

    pub async fn set_task_view_sharing(&self, view_id: &str, shared: bool) -> Result<TaskView> {
        let changed = sqlx::query("UPDATE saved_task_views SET shared = ?2, updated_at = ?3 WHERE id = ?1")
            .bind(view_id)
            .bind(shared)
            .bind(Utc::now().to_rfc3339())
            .execute(&self.pool)
            .await?;
        if changed.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("View was not found.");
        }
        self.get_task_view(view_id).await
    }

    pub async fn delete_task_view(&self, view_id: &str) -> Result<()> {
        let removed = sqlx::query("DELETE FROM saved_task_views WHERE id = ?1")
            .bind(view_id)
            .execute(&self.pool)
            .await?;
        if removed.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("View was not found.");
        }
        Ok(())
    }
}
