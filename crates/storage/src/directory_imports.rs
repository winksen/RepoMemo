//! The ledger of a directory import (V0.8.11).

use super::*;

/// A file of an earlier run of the same import.
#[derive(Debug, Clone)]
pub struct ImportedFile {
    pub artifact_id: String,
    pub source_hash: String,
}

impl StorageEngine {
    /// What earlier runs of an import brought in, by path.
    pub async fn directory_import_files(
        &self,
        workspace_id: &str,
        source_key: &str,
    ) -> Result<std::collections::HashMap<String, ImportedFile>> {
        let rows = sqlx::query_as::<_, (String, String, String)>(
            "SELECT path, artifact_id, source_hash FROM directory_imports WHERE workspace_id = ?1 AND source_key = ?2",
        )
        .bind(workspace_id)
        .bind(source_key)
        .fetch_all(self.reader())
        .await?;
        Ok(rows
            .into_iter()
            .map(|(path, artifact_id, source_hash)| {
                (
                    path,
                    ImportedFile {
                        artifact_id,
                        source_hash,
                    },
                )
            })
            .collect())
    }

    /// Records a file as done, so the next run skips it when it is unchanged.
    pub async fn record_directory_import_file(
        &self,
        workspace_id: &str,
        source_key: &str,
        path: &str,
        artifact_id: &str,
        source_hash: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO directory_imports (workspace_id, source_key, path, artifact_id, source_hash, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6)
               ON CONFLICT(workspace_id, source_key, path) DO UPDATE SET artifact_id = excluded.artifact_id,
                 source_hash = excluded.source_hash, updated_at = excluded.updated_at"#,
        )
        .bind(workspace_id)
        .bind(source_key)
        .bind(path)
        .bind(artifact_id)
        .bind(source_hash)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// The folder of a name under a parent, made when it is not there yet.
    pub async fn ensure_folder(
        &self,
        workspace_id: &str,
        parent_id: Option<&str>,
        name: &str,
        created_by: Option<&str>,
    ) -> Result<String> {
        let found = self
            .list_folders(workspace_id)
            .await?
            .into_iter()
            .find(|folder| folder.parent_id.as_deref() == parent_id && folder.name == name);
        match found {
            Some(folder) => Ok(folder.id),
            None => Ok(self.create_folder(workspace_id, parent_id, name, created_by).await?.id),
        }
    }
}
