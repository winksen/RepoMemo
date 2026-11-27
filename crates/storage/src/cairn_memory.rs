//! CAIRN's memory across workspaces (V0.9.112), in figures: for each workspace of an organization, how many memory cards it holds, how
//! many of them need review, and when one was last written. Nothing here reads a card's text.

use super::*;

/// The memory of one workspace.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MemoryFigures {
    pub organization_id: String,
    pub organization_name: String,
    pub workspace_id: String,
    pub workspace_name: String,
    pub cards: i64,
    pub needing_review: i64,
    pub last_written: Option<String>,
}

impl StorageEngine {
    /// The figures of the workspaces of the organizations given (every organization for `None`), by organization and workspace.
    pub async fn cairn_memory_figures(&self, organizations: Option<&[String]>) -> Result<Vec<MemoryFigures>> {
        let wanted = serde_json::to_string(&organizations.unwrap_or_default())?;
        let rows = sqlx::query(
            r#"
            SELECT wo.organization_id AS organization_id, o.name AS organization_name, w.id AS workspace_id, w.name AS workspace_name,
                   (SELECT COUNT(*) FROM memory_cards c WHERE c.workspace_id = w.id) AS cards,
                   (SELECT MAX(c.updated_at) FROM memory_cards c WHERE c.workspace_id = w.id) AS last_written
            FROM workspaces w
            JOIN workspace_organizations wo ON wo.workspace_id = w.id
            JOIN organizations o ON o.id = wo.organization_id
            WHERE ?1 = 1 OR wo.organization_id IN (SELECT value FROM json_each(?2))
            ORDER BY o.name, w.name
            "#,
        )
        .bind(organizations.is_none())
        .bind(wanted)
        .fetch_all(self.reader())
        .await?;
        let mut figures = Vec::with_capacity(rows.len());
        for row in &rows {
            let workspace_id: String = row.get("workspace_id");
            let needing_review = self.memory_cards_needing_review(&workspace_id).await?.len() as i64;
            figures.push(MemoryFigures {
                organization_id: row.get("organization_id"),
                organization_name: row.get("organization_name"),
                workspace_name: row.get("workspace_name"),
                cards: row.get("cards"),
                needing_review,
                last_written: row.get("last_written"),
                workspace_id,
            });
        }
        Ok(figures)
    }
}

/// One routine as the administration page lists it: no prompt, only what an administrator needs to see how it is going.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RoutineFigures {
    pub id: String,
    pub organization_id: String,
    pub organization_name: String,
    pub workspace_id: String,
    pub workspace_name: String,
    pub title: String,
    pub owner_name: String,
    /// `daily`, `weekly`, `monthly`, or the event that starts it.
    pub runs: String,
    /// `active` or `paused`.
    pub state: String,
    pub pause_reason: Option<String>,
    pub failures: i64,
    pub last_run_at: Option<String>,
    pub next_run_date: String,
}

impl StorageEngine {
    /// The routines of the workspaces of the organizations given (every organization for `None`), by organization, workspace and title.
    pub async fn cairn_routines(&self, organizations: Option<&[String]>) -> Result<Vec<RoutineFigures>> {
        let wanted = serde_json::to_string(&organizations.unwrap_or_default())?;
        let workspaces = sqlx::query(
            r#"
            SELECT wo.organization_id AS organization_id, o.name AS organization_name, w.id AS workspace_id, w.name AS workspace_name
            FROM workspaces w
            JOIN workspace_organizations wo ON wo.workspace_id = w.id
            JOIN organizations o ON o.id = wo.organization_id
            WHERE ?1 = 1 OR wo.organization_id IN (SELECT value FROM json_each(?2))
            ORDER BY o.name, w.name
            "#,
        )
        .bind(organizations.is_none())
        .bind(wanted)
        .fetch_all(self.reader())
        .await?;
        let mut figures = Vec::new();
        for workspace in &workspaces {
            let workspace_id: String = workspace.get("workspace_id");
            for routine in self.list_agent_routines(&workspace_id).await? {
                figures.push(RoutineFigures {
                    id: routine.id,
                    organization_id: workspace.get("organization_id"),
                    organization_name: workspace.get("organization_name"),
                    workspace_id: workspace_id.clone(),
                    workspace_name: workspace.get("workspace_name"),
                    title: routine.title,
                    owner_name: routine.owner_name,
                    runs: routine.event_kind.unwrap_or(routine.schedule.kind),
                    state: routine.state,
                    pause_reason: routine.pause_reason,
                    failures: routine.failures,
                    last_run_at: routine.last_run_at,
                    next_run_date: routine.next_run_date,
                });
            }
        }
        Ok(figures)
    }
}
