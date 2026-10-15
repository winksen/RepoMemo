//! Figures computed by the database instead of by loading rows: the artifact
//! query (roadmap P2), the workspace figures (P3), the dashboard (P4) and the
//! content version that map and health caches are keyed by (P5).

use super::*;

/// What the artifact query asks for. Text and languages are compared without
/// regard to case.
#[derive(Debug, Clone, Default)]
pub struct ArtifactQuery {
    pub workspace_id: String,
    /// Part of the title or path.
    pub text: String,
    pub artifact_types: Vec<ArtifactType>,
    pub languages: Vec<String>,
    pub source_ids: Vec<String>,
    /// Only indexed (`true`) or only not yet indexed (`false`) files.
    pub indexed: Option<bool>,
    /// A page of this many files; `None` returns every match.
    pub limit: Option<i64>,
    pub offset: i64,
}

/// Counts and sizes of a workspace's artifacts.
#[derive(Debug, Clone, Default)]
pub struct ArtifactStats {
    pub indexed: i64,
    pub total_bytes: i64,
    pub indexed_bytes: i64,
    pub created_since: i64,
    pub updated_since: i64,
    /// `(type, files, bytes)`
    pub by_type: Vec<(ArtifactType, i64, i64)>,
    /// `(language, files)` for files that have one.
    pub by_language: Vec<(String, i64)>,
}

/// The figures of one workspace shown on the dashboard.
#[derive(Debug, Clone, Default)]
pub struct DashboardCounts {
    pub workspace_id: String,
    pub artifacts: i64,
    /// Tasks that are open or in progress.
    pub active_tasks: i64,
    pub memory_cards: i64,
}

fn push_in<'a, T>(query: &mut QueryBuilder<'a, Sqlite>, column: &str, values: Vec<T>)
where
    T: 'a + sqlx::Encode<'a, Sqlite> + sqlx::Type<Sqlite> + Send,
{
    query.push(" AND ");
    query.push(column);
    query.push(" IN (");
    let mut separated = query.separated(", ");
    for value in values {
        separated.push_bind(value);
    }
    separated.push_unseparated(")");
}

fn push_artifact_filters<'a>(query: &mut QueryBuilder<'a, Sqlite>, request: &'a ArtifactQuery) {
    query.push(" WHERE artifacts.workspace_id = ");
    query.push_bind(&request.workspace_id);
    if !request.text.is_empty() {
        let text = request.text.to_lowercase();
        query.push(" AND (instr(lower(artifacts.title), ");
        query.push_bind(text.clone());
        query.push(") > 0 OR instr(lower(artifacts.path), ");
        query.push_bind(text);
        query.push(") > 0)");
    }
    if !request.artifact_types.is_empty() {
        push_in(
            query,
            "artifacts.type",
            request.artifact_types.iter().map(artifact_type_to_db).collect(),
        );
    }
    if !request.languages.is_empty() {
        push_in(
            query,
            "lower(artifacts.language)",
            request
                .languages
                .iter()
                .map(|language| language.to_lowercase())
                .collect(),
        );
    }
    if !request.source_ids.is_empty() {
        push_in(query, "artifacts.source_id", request.source_ids.clone());
    }
    match request.indexed {
        Some(true) => {
            query.push(" AND artifacts.indexed_at IS NOT NULL");
        }
        Some(false) => {
            query.push(" AND artifacts.indexed_at IS NULL");
        }
        None => {}
    }
}

impl StorageEngine {
    /// The files that match, newest change first, and how many match in all
    /// (the page is cut after counting).
    pub async fn query_artifacts(&self, request: &ArtifactQuery) -> Result<(Vec<ArtifactSummary>, i64)> {
        let mut count = QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM artifacts");
        push_artifact_filters(&mut count, request);
        let total = count.build_query_scalar::<i64>().fetch_one(self.reader()).await?;

        let mut rows = QueryBuilder::<Sqlite>::new(
            r#"SELECT artifacts.id, artifacts.workspace_id, artifacts.source_id, sources.name AS source_name,
                 artifacts.type AS artifact_type, artifacts.title, artifacts.path, artifacts.content_hash,
                 artifacts.mime_type, artifacts.language, artifacts.size_bytes, artifacts.created_at,
                 artifacts.updated_at, artifacts.indexed_at
               FROM artifacts JOIN sources ON sources.id = artifacts.source_id"#,
        );
        push_artifact_filters(&mut rows, request);
        rows.push(" ORDER BY artifacts.updated_at DESC, artifacts.path ASC");
        if let Some(limit) = request.limit {
            rows.push(" LIMIT ");
            rows.push_bind(limit.max(1));
            rows.push(" OFFSET ");
            rows.push_bind(request.offset.max(0));
        }
        let page = rows
            .build_query_as::<ArtifactSummaryRow>()
            .fetch_all(self.reader())
            .await?;
        Ok((page.into_iter().map(ArtifactSummary::from).collect(), total))
    }

    /// Counts and sizes by type and language, and how many files were created
    /// and updated since `since` (an RFC 3339 time).
    pub async fn workspace_artifact_stats(&self, workspace_id: &str, since: &str) -> Result<ArtifactStats> {
        let rows = sqlx::query(
            r#"SELECT type,
                 COUNT(*) AS files,
                 COALESCE(SUM(size_bytes), 0) AS bytes,
                 COALESCE(SUM(indexed_at IS NOT NULL), 0) AS indexed,
                 COALESCE(SUM(CASE WHEN indexed_at IS NOT NULL THEN size_bytes ELSE 0 END), 0) AS indexed_bytes,
                 COALESCE(SUM(created_at >= ?2), 0) AS created_since,
                 COALESCE(SUM(updated_at >= ?2), 0) AS updated_since
               FROM artifacts WHERE workspace_id = ?1 GROUP BY type"#,
        )
        .bind(workspace_id)
        .bind(since)
        .fetch_all(self.reader())
        .await?;
        let mut stats = ArtifactStats::default();
        for row in rows {
            stats.indexed += row.get::<i64, _>("indexed");
            stats.total_bytes += row.get::<i64, _>("bytes");
            stats.indexed_bytes += row.get::<i64, _>("indexed_bytes");
            stats.created_since += row.get::<i64, _>("created_since");
            stats.updated_since += row.get::<i64, _>("updated_since");
            stats.by_type.push((
                artifact_type_from_db(&row.get::<String, _>("type")),
                row.get("files"),
                row.get("bytes"),
            ));
        }
        stats.by_language = sqlx::query_as::<_, (String, i64)>(
            r#"SELECT language, COUNT(*) FROM artifacts
               WHERE workspace_id = ?1 AND language IS NOT NULL AND trim(language) != ''
               GROUP BY language"#,
        )
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?;
        Ok(stats)
    }

    /// The dashboard figures of several workspaces in three queries.
    pub async fn dashboard_counts(&self, workspace_ids: &[String]) -> Result<Vec<DashboardCounts>> {
        if workspace_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut by_workspace: HashMap<String, DashboardCounts> = workspace_ids
            .iter()
            .map(|id| {
                (
                    id.clone(),
                    DashboardCounts {
                        workspace_id: id.clone(),
                        ..Default::default()
                    },
                )
            })
            .collect();
        for (table, condition) in [
            ("artifacts", ""),
            ("memory_cards", ""),
            ("workspace_tasks", " AND status IN ('open', 'in_progress')"),
        ] {
            let mut query = QueryBuilder::<Sqlite>::new(format!(
                "SELECT workspace_id, COUNT(*) AS total FROM {table} WHERE workspace_id IN ("
            ));
            let mut separated = query.separated(", ");
            for id in workspace_ids {
                separated.push_bind(id.clone());
            }
            separated.push_unseparated(")");
            query.push(condition);
            query.push(" GROUP BY workspace_id");
            for row in query.build().fetch_all(self.reader()).await? {
                let Some(entry) = by_workspace.get_mut(&row.get::<String, _>("workspace_id")) else {
                    continue;
                };
                let total = row.get::<i64, _>("total");
                match table {
                    "artifacts" => entry.artifacts = total,
                    "memory_cards" => entry.memory_cards = total,
                    _ => entry.active_tasks = total,
                }
            }
        }
        Ok(workspace_ids.iter().filter_map(|id| by_workspace.remove(id)).collect())
    }

    /// Bytes of evidence a workspace holds: its files' sizes, and the earlier
    /// versions kept of files that were uploaded again.
    pub async fn workspace_stored_bytes(&self, workspace_id: &str) -> Result<i64> {
        Ok(sqlx::query_scalar::<_, i64>(
            r#"
            SELECT (SELECT COALESCE(SUM(size_bytes), 0) FROM artifacts WHERE workspace_id = ?1)
                 + (SELECT COALESCE(SUM(size_bytes), 0) FROM artifact_versions WHERE workspace_id = ?1)
            "#,
        )
        .bind(workspace_id)
        .fetch_one(self.reader())
        .await?)
    }

    /// Workspaces the person owns.
    pub async fn count_owned_workspaces(&self, user_id: &str) -> Result<i64> {
        Ok(sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM workspace_memberships WHERE user_id = ?1 AND role = 'owner'",
        )
        .bind(user_id)
        .fetch_one(self.reader())
        .await?)
    }

    /// Writes a consistent copy of the database to `target`, which must not
    /// exist. Reads and writes carry on while it is made.
    pub async fn snapshot_database(&self, target: &Path) -> Result<()> {
        let target = target.to_str().context("the backup path is not valid text")?.to_owned();
        sqlx::query("VACUUM INTO ?1")
            .bind(target)
            .execute(self.reader())
            .await
            .context("the database snapshot could not be written")?;
        Ok(())
    }

    /// A short text that changes whenever something the knowledge map or
    /// Workspace Health is computed from changes: files and their index,
    /// passages, vectors, memory cards and their links, evidence states,
    /// health decisions and the providers.
    pub async fn workspace_content_version(&self, workspace_id: &str) -> Result<String> {
        let row = sqlx::query(
            r#"SELECT
                 (SELECT COUNT(*) || ':' || COALESCE(MAX(updated_at), '') || ':' || COALESCE(MAX(indexed_at), '')
                    || ':' || COALESCE(SUM(index_version), 0)
                    FROM artifacts WHERE workspace_id = ?1) AS files,
                 (SELECT COUNT(*) FROM chunks WHERE workspace_id = ?1) AS passages,
                 (SELECT COUNT(*) FROM chunk_embeddings WHERE workspace_id = ?1) AS vectors,
                 (SELECT COUNT(*) || ':' || COALESCE(MAX(updated_at), '') FROM memory_cards WHERE workspace_id = ?1) AS cards,
                 (SELECT COUNT(*) FROM links WHERE workspace_id = ?1) AS links,
                 (SELECT COUNT(*) || ':' || COALESCE(MAX(updated_at), '') FROM artifact_lifecycle WHERE workspace_id = ?1) AS states,
                 (SELECT COUNT(*) FROM workspace_health_actions WHERE workspace_id = ?1) AS decisions,
                 (SELECT COUNT(*) || ':' || COALESCE(MAX(updated_at), '') FROM provider_settings WHERE workspace_id = ?1) AS providers,
                 (SELECT COUNT(*) FROM repo_files WHERE removed_at IS NULL AND artifact_id IN
                    (SELECT id FROM artifacts WHERE workspace_id = ?1)) AS repo_files"#,
        )
        .bind(workspace_id)
        .fetch_one(self.reader())
        .await?;
        let mut parts = Vec::new();
        for column in [
            "files",
            "passages",
            "vectors",
            "cards",
            "links",
            "states",
            "decisions",
            "providers",
            "repo_files",
        ] {
            let value = row
                .try_get::<String, _>(column)
                .or_else(|_| row.try_get::<i64, _>(column).map(|number| number.to_string()))?;
            parts.push(value);
        }
        Ok(parts.join("|"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_are_written_as_bound_parameters() {
        let request = ArtifactQuery {
            workspace_id: "w".to_owned(),
            text: "Read".to_owned(),
            languages: vec!["Markdown".to_owned()],
            indexed: Some(true),
            ..Default::default()
        };
        let mut query = QueryBuilder::<Sqlite>::new("SELECT 1 FROM artifacts");
        push_artifact_filters(&mut query, &request);
        let sql = query.sql().to_owned();
        assert!(sql.contains("instr(lower(artifacts.title), ?"));
        assert!(sql.contains("lower(artifacts.language) IN (?"));
        assert!(sql.contains("artifacts.indexed_at IS NOT NULL"));
        assert!(!sql.contains("Markdown"), "values never enter the statement text");
    }
}
