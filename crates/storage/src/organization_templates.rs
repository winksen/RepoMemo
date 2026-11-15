//! Templates saved by the administrators of an organization (V0.6.23). The text is validated by the caller against the
//! same rules as every template file; storage keeps it, versioned and append-only, and says who owns an id.

use super::*;

/// One saved version of an organization's template.
#[derive(Debug, Clone)]
pub struct StoredOrganizationTemplate {
    pub organization_id: String,
    pub template_id: String,
    pub version: i64,
    pub name: String,
    pub summary: String,
    /// The template file as saved.
    pub definition: String,
    pub note: String,
    pub created_by: Option<SharedUser>,
    pub created_at: String,
}

const SELECT_TEMPLATE: &str = r#"
    SELECT saved.organization_id, saved.template_id, saved.version, saved.name, saved.summary, saved.definition_json,
           saved.note, saved.created_at, users.id AS user_id, users.display_name AS user_name, users.email AS user_email
    FROM organization_templates AS saved
    LEFT JOIN users ON users.id = saved.created_by_user_id"#;

fn stored(row: &SqliteRow) -> StoredOrganizationTemplate {
    let user_id: Option<String> = row.get("user_id");
    StoredOrganizationTemplate {
        organization_id: row.get("organization_id"),
        template_id: row.get("template_id"),
        version: row.get("version"),
        name: row.get("name"),
        summary: row.get("summary"),
        definition: row.get("definition_json"),
        note: row.get("note"),
        created_at: row.get("created_at"),
        created_by: user_id.map(|id| SharedUser {
            id,
            display_name: row.get("user_name"),
            email: row.get("user_email"),
        }),
    }
}

impl StorageEngine {
    /// The organization that owns a template id, when one saved it.
    pub async fn organization_template_owner(&self, template_id: &str) -> Result<Option<String>> {
        Ok(sqlx::query_scalar::<_, String>(
            "SELECT organization_id FROM organization_templates WHERE template_id = ?1 LIMIT 1",
        )
        .bind(template_id)
        .fetch_optional(self.reader())
        .await?)
    }

    /// The newest version of each template, of one organization or (with none) of all of them, by id.
    pub async fn latest_organization_templates(
        &self,
        organization_id: Option<&str>,
    ) -> Result<Vec<StoredOrganizationTemplate>> {
        let rows = sqlx::query(&format!(
            r#"{SELECT_TEMPLATE}
               WHERE (?1 IS NULL OR saved.organization_id = ?1)
                 AND saved.version = (SELECT MAX(version) FROM organization_templates AS later
                                      WHERE later.organization_id = saved.organization_id AND later.template_id = saved.template_id)
               ORDER BY saved.template_id"#
        ))
        .bind(organization_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows.iter().map(stored).collect())
    }

    /// Every version of one template of an organization, newest first.
    pub async fn organization_template_versions(
        &self,
        organization_id: &str,
        template_id: &str,
    ) -> Result<Vec<StoredOrganizationTemplate>> {
        let rows = sqlx::query(&format!(
            "{SELECT_TEMPLATE} WHERE saved.organization_id = ?1 AND saved.template_id = ?2 ORDER BY saved.version DESC"
        ))
        .bind(organization_id)
        .bind(template_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows.iter().map(stored).collect())
    }

    /// Saves the next version of a template. `version` must be exactly one more than the newest saved (1 for a new
    /// template), and the id must be new or already this organization's, both checked in the transaction that writes.
    #[allow(clippy::too_many_arguments)]
    pub async fn save_organization_template(
        &self,
        organization_id: &str,
        template_id: &str,
        version: i64,
        name: &str,
        summary: &str,
        definition: &str,
        note: &str,
        saved_by: &str,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let owner = sqlx::query_scalar::<_, String>(
            "SELECT organization_id FROM organization_templates WHERE template_id = ?1 LIMIT 1",
        )
        .bind(template_id)
        .fetch_optional(&mut *tx)
        .await?;
        if owner.as_deref().is_some_and(|owner| owner != organization_id) {
            repomemo_domain::bail_conflict!("That template id belongs to another organization. Choose another id.");
        }
        let newest = sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(MAX(version), 0) FROM organization_templates WHERE organization_id = ?1 AND template_id = ?2",
        )
        .bind(organization_id)
        .bind(template_id)
        .fetch_one(&mut *tx)
        .await?;
        if version != newest + 1 {
            repomemo_domain::bail_conflict!(
                "The newest saved version of this template is {newest}, so the next one must be version {}; the file says {version}.",
                newest + 1
            );
        }
        sqlx::query(
            r#"INSERT INTO organization_templates
               (id, organization_id, template_id, version, name, summary, definition_json, note, created_by_user_id, created_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(organization_id)
        .bind(template_id)
        .bind(version)
        .bind(name)
        .bind(summary)
        .bind(definition)
        .bind(note)
        .bind(saved_by)
        .bind(Utc::now().to_rfc3339())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// The organization a workspace belongs to.
    pub async fn workspace_organization(&self, workspace_id: &str) -> Result<Option<String>> {
        Ok(sqlx::query_scalar::<_, String>(
            "SELECT organization_id FROM workspace_organizations WHERE workspace_id = ?1",
        )
        .bind(workspace_id)
        .fetch_optional(self.reader())
        .await?)
    }
}
