//! A project's brief (V0.7.2): the decision memories and the evidence it cites, kept as references. Nothing cited is copied;
//! a reference holds the target's id and a note, and the title is read from the target when the brief is read, so a deleted
//! target shows as unavailable and never as stale text.

use super::*;

/// The most references one project can hold.
pub const MAX_PROJECT_REFERENCES: i64 = 100;

const SELECT_REFERENCE: &str = r#"
    SELECT reference.id, reference.project_id, reference.kind, reference.target_id, reference.note, reference.created_at,
           adder.id AS adder_id, adder.display_name AS adder_name, adder.email AS adder_email,
           CASE reference.kind
             WHEN 'memory_card' THEN (SELECT title FROM memory_cards WHERE memory_cards.id = reference.target_id AND memory_cards.workspace_id = reference.workspace_id)
             ELSE (SELECT title FROM artifacts WHERE artifacts.id = reference.target_id AND artifacts.workspace_id = reference.workspace_id)
           END AS title
    FROM project_references AS reference
    LEFT JOIN users AS adder ON adder.id = reference.added_by_user_id"#;

fn reference_from(row: &SqliteRow) -> ProjectReference {
    let adder_id: Option<String> = row.get("adder_id");
    let title: Option<String> = row.get("title");
    ProjectReference {
        id: row.get("id"),
        project_id: row.get("project_id"),
        kind: row.get("kind"),
        target_id: row.get("target_id"),
        available: title.is_some(),
        title,
        note: row.get("note"),
        added_by: adder_id.map(|id| SharedUser {
            id,
            display_name: row.get("adder_name"),
            email: row.get("adder_email"),
        }),
        created_at: row.get("created_at"),
    }
}

impl StorageEngine {
    pub async fn list_project_references(&self, project_id: &str) -> Result<Vec<ProjectReference>> {
        let rows = sqlx::query(&format!(
            "{SELECT_REFERENCE} WHERE reference.project_id = ?1 ORDER BY reference.created_at ASC, reference.id ASC"
        ))
        .bind(project_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows.iter().map(reference_from).collect())
    }

    /// Cites a memory card (`memory_card`) or a piece of evidence (`evidence`) of the project's own workspace.
    pub async fn add_project_reference(
        &self,
        project: &Project,
        added_by: &str,
        kind: &str,
        target_id: &str,
        note: &str,
    ) -> Result<ProjectReference> {
        let exists: i64 = if kind == "memory_card" {
            sqlx::query_scalar("SELECT COUNT(*) FROM memory_cards WHERE id = ?1 AND workspace_id = ?2")
        } else {
            sqlx::query_scalar("SELECT COUNT(*) FROM artifacts WHERE id = ?1 AND workspace_id = ?2")
        }
        .bind(target_id)
        .bind(&project.workspace_id)
        .fetch_one(&self.pool)
        .await?;
        if exists == 0 {
            repomemo_domain::bail_not_found!("The memory card or evidence was not found in this workspace.");
        }
        let held: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM project_references WHERE project_id = ?1")
            .bind(&project.id)
            .fetch_one(&self.pool)
            .await?;
        if held >= MAX_PROJECT_REFERENCES {
            repomemo_domain::bail_conflict!("A project can cite at most {MAX_PROJECT_REFERENCES} items.");
        }
        let already: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM project_references WHERE project_id = ?1 AND kind = ?2 AND target_id = ?3",
        )
        .bind(&project.id)
        .bind(kind)
        .bind(target_id)
        .fetch_one(&self.pool)
        .await?;
        if already > 0 {
            repomemo_domain::bail_conflict!("The project already cites this item.");
        }
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO project_references (id, project_id, workspace_id, kind, target_id, note, added_by_user_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(&id)
        .bind(&project.id)
        .bind(&project.workspace_id)
        .bind(kind)
        .bind(target_id)
        .bind(note)
        .bind(added_by)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        let row = sqlx::query(&format!("{SELECT_REFERENCE} WHERE reference.id = ?1"))
            .bind(&id)
            .fetch_one(&self.pool)
            .await?;
        Ok(reference_from(&row))
    }

    pub async fn remove_project_reference(&self, project_id: &str, reference_id: &str) -> Result<()> {
        let removed = sqlx::query("DELETE FROM project_references WHERE id = ?1 AND project_id = ?2")
            .bind(reference_id)
            .bind(project_id)
            .execute(&self.pool)
            .await?;
        if removed.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("The reference was not found.");
        }
        Ok(())
    }

    /// The projects of a workspace whose brief cites an item: the backlinks shown on a card or a file.
    pub async fn list_projects_citing(&self, workspace_id: &str, kind: &str, target_id: &str) -> Result<Vec<Project>> {
        let ids: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT project_id FROM project_references WHERE workspace_id = ?1 AND kind = ?2 AND target_id = ?3",
        )
        .bind(workspace_id)
        .bind(kind)
        .bind(target_id)
        .fetch_all(self.reader())
        .await?;
        let mut projects = Vec::with_capacity(ids.len());
        for id in ids {
            let project = self.get_project(&id).await?;
            if project.status != "archived" {
                projects.push(project);
            }
        }
        projects.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(projects)
    }
}
