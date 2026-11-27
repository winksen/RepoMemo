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
