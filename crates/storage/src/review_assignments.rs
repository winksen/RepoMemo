//! Review ownership (V0.6.6): the accountable reviewer and next-review date of a file or memory card.

use super::*;

/// A reviewer and a due date for one item.
#[derive(Debug, Clone)]
pub struct ReviewAssignment {
    pub item_kind: String,
    pub item_id: String,
    pub workspace_id: String,
    pub reviewer: Option<SharedUser>,
    /// The reviewer's account is gone, deactivated or no longer in the workspace.
    pub reviewer_departed: bool,
    /// The name the reviewer had when assigned; shown when the account is gone.
    pub reviewer_label: Option<String>,
    pub next_review_at: Option<String>,
    pub last_reviewed_at: Option<String>,
    pub updated_at: String,
}

/// An item whose review date has passed, with what is needed to list it.
#[derive(Debug, Clone)]
pub struct DueReview {
    pub item_kind: String,
    pub item_id: String,
    pub workspace_id: String,
    pub title: String,
    pub detail: String,
    pub artifact_id: Option<String>,
    pub next_review_at: String,
}

const ASSIGNMENT_SELECT: &str = r#"
    SELECT a.item_kind, a.item_id, a.workspace_id, a.reviewer_user_id, a.reviewer_label, a.next_review_at,
           a.last_reviewed_at, a.updated_at,
           u.display_name AS reviewer_name, u.email AS reviewer_email, u.deactivated_at AS reviewer_deactivated,
           (SELECT COUNT(*) FROM workspace_memberships m
             WHERE m.workspace_id = a.workspace_id AND m.user_id = a.reviewer_user_id
               AND (m.expires_at IS NULL OR m.expires_at > ?1)) AS reviewer_member
    FROM review_assignments AS a
    LEFT JOIN users AS u ON u.id = a.reviewer_user_id
"#;

fn assignment(row: &SqliteRow) -> ReviewAssignment {
    let reviewer_id: Option<String> = row.get("reviewer_user_id");
    let deactivated: Option<String> = row.get("reviewer_deactivated");
    let member: i64 = row.get("reviewer_member");
    let departed = reviewer_id.is_none() && row.get::<Option<String>, _>("reviewer_label").is_some()
        || deactivated.is_some()
        || (reviewer_id.is_some() && member == 0);
    ReviewAssignment {
        item_kind: row.get("item_kind"),
        item_id: row.get("item_id"),
        workspace_id: row.get("workspace_id"),
        reviewer: reviewer_id.map(|id| SharedUser {
            id,
            display_name: row.get("reviewer_name"),
            email: row.get("reviewer_email"),
        }),
        reviewer_departed: departed,
        reviewer_label: row.get("reviewer_label"),
        next_review_at: row.get("next_review_at"),
        last_reviewed_at: row.get("last_reviewed_at"),
        updated_at: row.get("updated_at"),
    }
}

impl StorageEngine {
    /// Every assignment in a workspace, by `(kind, id)`.
    pub async fn review_assignments(&self, workspace_id: &str) -> Result<HashMap<(String, String), ReviewAssignment>> {
        let rows = sqlx::query(&format!("{ASSIGNMENT_SELECT} WHERE a.workspace_id = ?2"))
            .bind(Utc::now().to_rfc3339())
            .bind(workspace_id)
            .fetch_all(self.reader())
            .await?;
        Ok(rows
            .iter()
            .map(assignment)
            .map(|found| ((found.item_kind.clone(), found.item_id.clone()), found))
            .collect())
    }

    pub async fn review_assignment(&self, kind: &str, item_id: &str) -> Result<Option<ReviewAssignment>> {
        let row = sqlx::query(&format!(
            "{ASSIGNMENT_SELECT} WHERE a.item_kind = ?2 AND a.item_id = ?3"
        ))
        .bind(Utc::now().to_rfc3339())
        .bind(kind)
        .bind(item_id)
        .fetch_optional(self.reader())
        .await?;
        Ok(row.as_ref().map(assignment))
    }

    /// Sets the reviewer and the next review date together (either may be cleared). The reviewer must already have been
    /// checked against the workspace by the caller.
    pub async fn set_review_assignment(
        &self,
        workspace_id: &str,
        kind: &str,
        item_id: &str,
        reviewer_user_id: Option<&str>,
        next_review_at: Option<&str>,
        assigned_by: &str,
    ) -> Result<()> {
        let label = match reviewer_user_id {
            Some(id) => {
                sqlx::query_scalar::<_, String>("SELECT display_name FROM users WHERE id = ?1")
                    .bind(id)
                    .fetch_optional(self.reader())
                    .await?
            }
            None => None,
        };
        sqlx::query(
            r#"INSERT INTO review_assignments
                 (item_kind, item_id, workspace_id, reviewer_user_id, reviewer_label, next_review_at, assigned_by_user_id, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
               ON CONFLICT(item_kind, item_id) DO UPDATE SET
                 reviewer_user_id = excluded.reviewer_user_id, reviewer_label = excluded.reviewer_label,
                 next_review_at = excluded.next_review_at, assigned_by_user_id = excluded.assigned_by_user_id,
                 updated_at = excluded.updated_at"#,
        )
        .bind(kind)
        .bind(item_id)
        .bind(workspace_id)
        .bind(reviewer_user_id)
        .bind(label)
        .bind(next_review_at)
        .bind(assigned_by)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Records that the item was reviewed now and sets (or clears) the next date. The assignment must exist.
    pub async fn complete_review(&self, kind: &str, item_id: &str, next_review_at: Option<&str>) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let changed = sqlx::query(
            "UPDATE review_assignments SET last_reviewed_at = ?3, next_review_at = ?4, updated_at = ?3 WHERE item_kind = ?1 AND item_id = ?2",
        )
        .bind(kind)
        .bind(item_id)
        .bind(&now)
        .bind(next_review_at)
        .execute(&self.pool)
        .await?;
        if changed.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("Nobody is assigned to review that yet.");
        }
        Ok(())
    }

    /// Items of a workspace whose review date has passed, oldest due first. Items that no longer exist are not listed.
    pub async fn due_reviews(&self, workspace_id: &str) -> Result<Vec<DueReview>> {
        let now = Utc::now().to_rfc3339();
        let rows = sqlx::query(
            r#"
            SELECT a.item_kind, a.item_id, a.workspace_id, a.next_review_at,
                   COALESCE(artifact.title, card.title) AS title,
                   COALESCE(artifact.path, '') AS detail,
                   artifact.id AS artifact_id
            FROM review_assignments AS a
            LEFT JOIN artifacts AS artifact ON a.item_kind = 'evidence' AND artifact.id = a.item_id
            LEFT JOIN memory_cards AS card ON a.item_kind = 'memory_card' AND card.id = a.item_id
            WHERE a.workspace_id = ?1 AND a.next_review_at IS NOT NULL AND a.next_review_at <= ?2
              AND COALESCE(artifact.id, card.id) IS NOT NULL
            ORDER BY a.next_review_at ASC, a.item_id ASC
            "#,
        )
        .bind(workspace_id)
        .bind(&now)
        .fetch_all(self.reader())
        .await?;
        Ok(rows.iter().map(due_review).collect())
    }

    /// Items whose review date has passed and whose reviewer is this person, across the workspaces they can still open.
    pub async fn due_reviews_for(&self, user_id: &str) -> Result<Vec<(DueReview, String)>> {
        let now = Utc::now().to_rfc3339();
        let rows = sqlx::query(
            r#"
            SELECT a.item_kind, a.item_id, a.workspace_id, a.next_review_at,
                   COALESCE(artifact.title, card.title) AS title,
                   COALESCE(artifact.path, '') AS detail,
                   artifact.id AS artifact_id,
                   workspaces.name AS workspace_name
            FROM review_assignments AS a
            JOIN workspaces ON workspaces.id = a.workspace_id
            JOIN workspace_memberships AS membership
              ON membership.workspace_id = a.workspace_id AND membership.user_id = ?1
             AND (membership.expires_at IS NULL OR membership.expires_at > ?2)
            LEFT JOIN artifacts AS artifact ON a.item_kind = 'evidence' AND artifact.id = a.item_id
            LEFT JOIN memory_cards AS card ON a.item_kind = 'memory_card' AND card.id = a.item_id
            WHERE a.reviewer_user_id = ?1 AND a.next_review_at IS NOT NULL AND a.next_review_at <= ?2
              AND COALESCE(artifact.id, card.id) IS NOT NULL
            ORDER BY a.next_review_at ASC, a.item_id ASC
            LIMIT 200
            "#,
        )
        .bind(user_id)
        .bind(&now)
        .fetch_all(self.reader())
        .await?;
        Ok(rows
            .iter()
            .map(|row| (due_review(row), row.get("workspace_name")))
            .collect())
    }
}

fn due_review(row: &SqliteRow) -> DueReview {
    DueReview {
        item_kind: row.get("item_kind"),
        item_id: row.get("item_id"),
        workspace_id: row.get("workspace_id"),
        title: row.get("title"),
        detail: row.get("detail"),
        artifact_id: row.get("artifact_id"),
        next_review_at: row.get("next_review_at"),
    }
}
