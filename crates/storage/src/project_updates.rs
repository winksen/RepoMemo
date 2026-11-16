//! Project updates (V0.7.17): a person's dated statement of a project's health, with the reason and the links it rests on. The
//! health is always chosen by the person; the counts beside it are the project as it was when they published, and what changed
//! since the update before is written once, at that moment.

use super::*;
use repomemo_domain::{MilestoneBrief, ProjectSnapshot, ProjectUpdate, ProjectUpdateLink};

/// The most links one update can carry.
pub const MAX_UPDATE_LINKS: usize = 30;

/// What a person publishes.
#[derive(Debug, Clone, Default)]
pub struct NewProjectUpdate {
    /// `on_track`, `at_risk` or `blocked`.
    pub health: String,
    pub summary: String,
    pub reason: String,
    pub milestone_ids: Vec<String>,
    pub task_ids: Vec<String>,
    pub evidence_ids: Vec<String>,
    pub memory_card_ids: Vec<String>,
}

fn update_from(row: &SqliteRow, links: Vec<ProjectUpdateLink>) -> ProjectUpdate {
    let person = |prefix: &str| -> Option<SharedUser> {
        let id: Option<String> = row.get(format!("{prefix}_id").as_str());
        id.map(|id| SharedUser {
            id,
            display_name: row.get(format!("{prefix}_name").as_str()),
            email: row.get(format!("{prefix}_email").as_str()),
        })
    };
    let snapshot: String = row.get("snapshot_json");
    let changes: String = row.get("changes_json");
    ProjectUpdate {
        id: row.get("id"),
        project_id: row.get("project_id"),
        workspace_id: row.get("workspace_id"),
        author: person("author"),
        health: row.get("health"),
        summary: row.get("summary"),
        reason: row.get("reason"),
        snapshot: serde_json::from_str(&snapshot).unwrap_or_default(),
        changes: serde_json::from_str(&changes).unwrap_or_default(),
        links,
        created_at: row.get("created_at"),
        retracted_at: row.get("retracted_at"),
        retracted_by: person("retractor"),
    }
}

const SELECT_UPDATE: &str = r#"
    SELECT update_row.id, update_row.project_id, update_row.workspace_id, update_row.health, update_row.summary,
           update_row.reason, update_row.snapshot_json, update_row.changes_json, update_row.created_at, update_row.retracted_at,
           author.id AS author_id, author.display_name AS author_name, author.email AS author_email,
           retractor.id AS retractor_id, retractor.display_name AS retractor_name, retractor.email AS retractor_email
    FROM project_updates AS update_row
    LEFT JOIN users AS author ON author.id = update_row.author_user_id
    LEFT JOIN users AS retractor ON retractor.id = update_row.retracted_by_user_id"#;

/// What changed between two snapshots, in words a reader can check.
fn describe_changes(
    before: &ProjectSnapshot,
    after: &ProjectSnapshot,
    health_before: &str,
    health_after: &str,
) -> Vec<String> {
    let mut changes = Vec::new();
    if health_before != health_after {
        changes.push(format!(
            "Health: {} to {}.",
            health_before.replace('_', " "),
            health_after.replace('_', " ")
        ));
    }
    for (label, was, now) in [
        ("Tasks", before.task_count, after.task_count),
        ("Done", before.done_count, after.done_count),
        ("Blocked", before.blocked_count, after.blocked_count),
        ("Overdue", before.overdue_count, after.overdue_count),
    ] {
        if was != now {
            changes.push(format!("{label}: {was} to {now}."));
        }
    }
    for milestone in &after.milestones {
        match before.milestones.iter().find(|earlier| earlier.id == milestone.id) {
            None => changes.push(format!("Milestone {} was added.", milestone.name)),
            Some(earlier) => {
                if !earlier.completed && milestone.completed {
                    changes.push(format!("Milestone {} was completed.", milestone.name));
                }
                if earlier.due_date != milestone.due_date {
                    changes.push(format!(
                        "Milestone {} moved from {} to {}.",
                        milestone.name,
                        earlier.due_date.as_deref().unwrap_or("no date"),
                        milestone.due_date.as_deref().unwrap_or("no date")
                    ));
                }
            }
        }
    }
    for earlier in &before.milestones {
        if !after.milestones.iter().any(|milestone| milestone.id == earlier.id) {
            changes.push(format!("Milestone {} was removed.", earlier.name));
        }
    }
    changes
}

impl StorageEngine {
    /// The project's task counts and milestones right now: what an update records as it was.
    pub async fn project_snapshot(&self, project_id: &str) -> Result<ProjectSnapshot> {
        let today = Utc::now().format("%Y-%m-%d").to_string();
        let now = Utc::now().to_rfc3339();
        let row = sqlx::query(
            "SELECT COUNT(*) AS total,
                    COALESCE(SUM(CASE WHEN status = 'done' THEN 1 ELSE 0 END), 0) AS done,
                    COALESCE(SUM(CASE WHEN status = 'blocked' THEN 1 ELSE 0 END), 0) AS blocked,
                    COALESCE(SUM(CASE WHEN status != 'done' AND due_at IS NOT NULL
                        AND (CASE WHEN length(due_at) = 10 THEN due_at < ?2 ELSE due_at < ?3 END) THEN 1 ELSE 0 END), 0) AS overdue
             FROM workspace_tasks WHERE project_id = ?1",
        )
        .bind(project_id)
        .bind(&today)
        .bind(&now)
        .fetch_one(self.reader())
        .await?;
        let total: i64 = row.get("total");
        let done: i64 = row.get("done");
        Ok(ProjectSnapshot {
            task_count: total,
            done_count: done,
            open_count: total - done,
            blocked_count: row.get("blocked"),
            overdue_count: row.get("overdue"),
            milestones: self
                .list_milestones(project_id)
                .await?
                .into_iter()
                .map(|milestone| MilestoneBrief {
                    id: milestone.id,
                    name: milestone.name,
                    due_date: milestone.due_date,
                    completed: milestone.completed,
                    overdue: milestone.overdue,
                    done_count: milestone.done_count,
                    task_count: milestone.task_count,
                })
                .collect(),
        })
    }

    async fn update_links(&self, update_id: &str) -> Result<Vec<ProjectUpdateLink>> {
        let rows = sqlx::query(
            r#"SELECT link.kind, link.target_id, link.label,
                      CASE link.kind
                        WHEN 'milestone' THEN (SELECT name FROM project_milestones WHERE id = link.target_id AND workspace_id = update_row.workspace_id)
                        WHEN 'task' THEN (SELECT 'T-' || COALESCE(number, 0) || ' ' || title FROM workspace_tasks WHERE id = link.target_id AND workspace_id = update_row.workspace_id)
                        WHEN 'memory_card' THEN (SELECT title FROM memory_cards WHERE id = link.target_id AND workspace_id = update_row.workspace_id)
                        ELSE (SELECT title FROM artifacts WHERE id = link.target_id AND workspace_id = update_row.workspace_id)
                      END AS title_now
               FROM project_update_links AS link
               JOIN project_updates AS update_row ON update_row.id = link.update_id
               WHERE link.update_id = ?1 ORDER BY link.rowid"#,
        )
        .bind(update_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows
            .iter()
            .map(|row| {
                let title: Option<String> = row.get("title_now");
                ProjectUpdateLink {
                    kind: row.get("kind"),
                    target_id: row.get("target_id"),
                    label: row.get("label"),
                    available: title.is_some(),
                    title,
                }
            })
            .collect())
    }

    /// The updates of a project, newest first. Retracted ones are left out unless asked for.
    pub async fn list_project_updates(
        &self,
        project_id: &str,
        include_retracted: bool,
        limit: i64,
    ) -> Result<Vec<ProjectUpdate>> {
        let rows = sqlx::query(&format!(
            "{SELECT_UPDATE} WHERE update_row.project_id = ?1 AND (?2 = 1 OR update_row.retracted_at IS NULL)
             ORDER BY update_row.created_at DESC, update_row.id DESC LIMIT ?3"
        ))
        .bind(project_id)
        .bind(include_retracted)
        .bind(limit)
        .fetch_all(self.reader())
        .await?;
        let mut updates = Vec::with_capacity(rows.len());
        for row in &rows {
            let id: String = row.get("id");
            updates.push(update_from(row, self.update_links(&id).await?));
        }
        Ok(updates)
    }

    pub async fn get_project_update(&self, update_id: &str) -> Result<ProjectUpdate> {
        let row = sqlx::query(&format!("{SELECT_UPDATE} WHERE update_row.id = ?1"))
            .bind(update_id)
            .fetch_optional(self.reader())
            .await?
            .ok_or_else(|| repomemo_domain::not_found!("Project update was not found."))?;
        Ok(update_from(&row, self.update_links(update_id).await?))
    }

    /// The health of the newest update that was not retracted.
    pub async fn latest_project_update(&self, project_id: &str) -> Result<Option<ProjectUpdate>> {
        Ok(self
            .list_project_updates(project_id, false, 1)
            .await?
            .into_iter()
            .next())
    }

    /// Publishes an update. Every link is checked against the project's workspace (and milestones against the project) before
    /// anything is written; the counts and what changed since the previous update are recorded now.
    pub async fn publish_project_update(
        &self,
        project: &Project,
        author: &str,
        update: NewProjectUpdate,
    ) -> Result<ProjectUpdate> {
        let mut links: Vec<(&str, String, String)> = Vec::new();
        for id in &update.milestone_ids {
            let milestone = self.get_milestone(id).await.map_err(|_| {
                repomemo_domain::invalid!("A milestone linked to the update must belong to this project.")
            })?;
            if milestone.project_id != project.id {
                repomemo_domain::bail_invalid!("A milestone linked to the update must belong to this project.");
            }
            links.push(("milestone", id.clone(), milestone.name));
        }
        for id in &update.task_ids {
            let task = self
                .get_collaboration_task(id)
                .await
                .map_err(|_| repomemo_domain::invalid!("A task linked to the update must belong to this workspace."))?;
            if task.workspace_id != project.workspace_id {
                repomemo_domain::bail_invalid!("A task linked to the update must belong to this workspace.");
            }
            links.push(("task", id.clone(), format!("{} {}", task.key, task.title)));
        }
        for (kind, ids) in [
            ("evidence", &update.evidence_ids),
            ("memory_card", &update.memory_card_ids),
        ] {
            for id in ids {
                self.check_reference_target(&project.workspace_id, kind, id).await?;
                let title: Option<String> = if kind == "memory_card" {
                    sqlx::query_scalar("SELECT title FROM memory_cards WHERE id = ?1")
                } else {
                    sqlx::query_scalar("SELECT title FROM artifacts WHERE id = ?1")
                }
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;
                links.push((kind, id.clone(), title.unwrap_or_default()));
            }
        }
        if links.len() > MAX_UPDATE_LINKS {
            repomemo_domain::bail_invalid!("An update can link at most {MAX_UPDATE_LINKS} items.");
        }
        let snapshot = self.project_snapshot(&project.id).await?;
        let previous = self.latest_project_update(&project.id).await?;
        let changes = match &previous {
            Some(previous) => describe_changes(&previous.snapshot, &snapshot, &previous.health, &update.health),
            None => vec!["This is the project's first update.".to_owned()],
        };
        let id = Uuid::new_v4().to_string();
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO project_updates (id, project_id, workspace_id, author_user_id, health, summary, reason, snapshot_json, changes_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )
        .bind(&id)
        .bind(&project.id)
        .bind(&project.workspace_id)
        .bind(author)
        .bind(&update.health)
        .bind(&update.summary)
        .bind(&update.reason)
        .bind(serde_json::to_string(&snapshot)?)
        .bind(serde_json::to_string(&changes)?)
        .bind(Utc::now().to_rfc3339())
        .execute(&mut *transaction)
        .await?;
        for (kind, target, label) in &links {
            sqlx::query("INSERT OR IGNORE INTO project_update_links (update_id, kind, target_id, label) VALUES (?1, ?2, ?3, ?4)")
                .bind(&id)
                .bind(kind)
                .bind(target)
                .bind(label)
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;
        self.get_project_update(&id).await
    }

    /// Hides an update from the default list and keeps the record, with who retracted it and when.
    pub async fn retract_project_update(&self, update_id: &str, by: &str) -> Result<ProjectUpdate> {
        let changed = sqlx::query(
            "UPDATE project_updates SET retracted_at = ?2, retracted_by_user_id = ?3 WHERE id = ?1 AND retracted_at IS NULL",
        )
        .bind(update_id)
        .bind(Utc::now().to_rfc3339())
        .bind(by)
        .execute(&self.pool)
        .await?;
        if changed.rows_affected() == 0 {
            self.get_project_update(update_id).await?;
            repomemo_domain::bail_conflict!("This update was already retracted.");
        }
        self.get_project_update(update_id).await
    }
}
