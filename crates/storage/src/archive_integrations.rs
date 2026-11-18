//! Connector provenance and publication metadata in a workspace archive (V0.8.19).
//!
//! What comes back is a **record of where things came from**, never a live integration: a tracker connection comes back as an
//! upload that keeps its name and says what it was imported from (provider, address, item states), with no credential, no
//! webhook secret, no sync and no link to the tracker. Outbound endpoints and chat destinations are not carried at all, so no
//! event or digest is sent after an import. A publication comes back **withdrawn**, with its text kept as history: nothing is
//! public until an administrator reviews and publishes it again.

use super::*;
use repomemo_domain::{ArchiveConnectorItem, ArchiveConnectorProvenance, ArchiveIntegrations, ArchivePublication};

impl StorageEngine {
    pub(crate) async fn export_integrations(&self, workspace_id: &str) -> Result<ArchiveIntegrations> {
        let mut connectors = Vec::new();
        let sources = sqlx::query_as::<_, (String, String, Option<String>, String)>(
            "SELECT id, name, root_uri, metadata_json FROM sources WHERE workspace_id = ?1 AND type = 'connector' ORDER BY created_at ASC",
        )
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?;
        for (id, name, location, metadata) in sources {
            // A source whose access was lost is held back from an export like the rest of its evidence.
            if self.connector_quarantine(&id).await?.is_some() {
                continue;
            }
            let provider = serde_json::from_str::<Value>(&metadata)
                .ok()
                .and_then(|value| value.get("provider").and_then(Value::as_str).map(str::to_owned))
                .unwrap_or_default();
            let items = sqlx::query_as::<_, (String, String, String, String, String)>(
                "SELECT artifact_id, external_id, kind, state, remote_updated_at FROM connector_items WHERE source_id = ?1 ORDER BY external_id",
            )
            .bind(&id)
            .fetch_all(self.reader())
            .await?
            .into_iter()
            .map(
                |(artifact_id, external_id, kind, state, remote_updated_at)| ArchiveConnectorItem {
                    artifact_id,
                    external_id,
                    kind,
                    state,
                    remote_updated_at,
                },
            )
            .collect();
            connectors.push(ArchiveConnectorProvenance {
                source_id: id,
                name,
                provider,
                location: location.unwrap_or_default(),
                items,
            });
        }
        let publications = sqlx::query_as::<_, (Option<String>, String, String, String, i64, String)>(
            "SELECT source_artifact_id, title, body_markdown, status, version, published_at FROM publications WHERE workspace_id = ?1 ORDER BY published_at ASC",
        )
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?
        .into_iter()
        .map(
            |(artifact_id, title, body_markdown, status, version, published_at)| ArchivePublication {
                artifact_id,
                title,
                body_markdown,
                status_at_export: status,
                version,
                published_at,
            },
        )
        .collect();
        Ok(ArchiveIntegrations {
            connectors,
            publications,
        })
    }

    /// Writes the provenance on the imported sources and brings publications back withdrawn.
    pub(crate) async fn import_integrations(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        workspace_id: &str,
        data: &ArchiveIntegrations,
        source_ids: &HashMap<&str, String>,
        artifact_ids: &HashMap<String, String>,
        importer_user_id: &str,
    ) -> Result<()> {
        for connector in &data.connectors {
            let Some(new_id) = source_ids.get(connector.source_id.as_str()) else {
                continue;
            };
            let imported_from = serde_json::json!({
                "imported_from": {
                    "provider": connector.provider,
                    "location": connector.location,
                    "items": connector.items.len(),
                    "note": "Imported from an archive. Nothing is connected: no credential, no webhook and no sync came with it.",
                }
            });
            sqlx::query("UPDATE sources SET metadata_json = ?2 WHERE id = ?1 AND workspace_id = ?3")
                .bind(new_id)
                .bind(imported_from.to_string())
                .bind(workspace_id)
                .execute(&mut **tx)
                .await?;
        }
        let now = Utc::now().to_rfc3339();
        for publication in &data.publications {
            let artifact = publication
                .artifact_id
                .as_ref()
                .and_then(|old| artifact_ids.get(old))
                .cloned();
            // Withdrawn with a fresh address: nothing is public, and the old address is not reused.
            sqlx::query(
                r#"INSERT INTO publications (id, workspace_id, slug, title, body_markdown, snapshot_hash, status, version,
                     source_artifact_id, published_by_user_id, published_at, updated_at)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'withdrawn', ?7, ?8, ?9, ?10, ?11)"#,
            )
            .bind(Uuid::new_v4().to_string())
            .bind(workspace_id)
            .bind(Uuid::new_v4().simple().to_string())
            .bind(&publication.title)
            .bind(&publication.body_markdown)
            .bind(StorageEngine::content_hash(publication.body_markdown.as_bytes()))
            .bind(publication.version.max(1) + 1)
            .bind(artifact)
            .bind(importer_user_id)
            .bind(&publication.published_at)
            .bind(&now)
            .execute(&mut **tx)
            .await?;
        }
        Ok(())
    }
}
