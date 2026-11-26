//! Reading a whole workspace for its archive (roadmap O2a).
//!
//! Everything is read in plain queries, in the shape the archive carries:
//! people as a name and an address, never as an account id, and links between
//! records by the ids the workspace uses today (an importer maps them to new
//! ones). Nothing is written.

use repomemo_domain::{
    ArchiveApproval, ArchiveArtifact, ArchiveCairn, ArchiveChecklistItem, ArchiveChunkRef, ArchiveCitation,
    ArchiveComment, ArchiveFolder, ArchiveLifecycle, ArchiveLifecycleEvent, ArchiveMemoryCard, ArchiveModelRef,
    ArchivePerson, ArchiveReview, ArchiveRevision, ArchiveRevisionCitation, ArchiveSavedSearch, ArchiveSource,
    ArchiveTask, ArchiveVersion, ArchiveWorkspace, SharedUser, WorkspaceArchiveData,
};

use super::*;
use crate::cairn_catalog::CairnModel;

/// The address an anonymized account was given; it is not an address worth carrying.
const ANONYMIZED_ADDRESS_SUFFIX: &str = "@anonymized.invalid";

/// A card's revisions, approvals and review assignment, as they are exported.
type CardKnowledge = (Vec<ArchiveRevision>, Vec<ArchiveApproval>, Option<ArchiveReview>);

fn person(name: String, email: Option<String>) -> ArchivePerson {
    let email = email.filter(|email| !email.trim().is_empty() && !email.ends_with(ANONYMIZED_ADDRESS_SUFFIX));
    ArchivePerson { name, email }
}

fn person_from_user(user: SharedUser) -> ArchivePerson {
    person(user.display_name, user.email)
}

fn optional_person(name: Option<String>, email: Option<String>) -> Option<ArchivePerson> {
    name.map(|name| person(name, email))
}

fn json_or_empty_object(text: &str) -> Value {
    serde_json::from_str::<Value>(text)
        .ok()
        .filter(Value::is_object)
        .unwrap_or_else(|| Value::Object(Default::default()))
}

impl StorageEngine {
    /// Everything `workspace_id` holds that an archive carries, except the
    /// files' bytes (the caller reads those by content hash).
    pub async fn export_workspace_data(&self, workspace_id: &str) -> Result<WorkspaceArchiveData> {
        let workspace_row = sqlx::query("SELECT name, created_at, settings_json FROM workspaces WHERE id = ?1")
            .bind(workspace_id)
            .fetch_optional(self.reader())
            .await?
            .ok_or_else(|| repomemo_domain::not_found!("Workspace was not found."))?;
        let workspace = ArchiveWorkspace {
            name: workspace_row.get("name"),
            created_at: workspace_row.get("created_at"),
            settings: json_or_empty_object(&workspace_row.get::<String, _>("settings_json")),
        };

        let sources = sqlx::query(
            "SELECT id, name, type, created_at FROM sources WHERE workspace_id = ?1 ORDER BY created_at ASC, id ASC",
        )
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?
        .into_iter()
        .map(|row| ArchiveSource {
            id: row.get("id"),
            name: row.get("name"),
            source_type: row.get("type"),
            created_at: row.get("created_at"),
        })
        .collect();

        let folders = self
            .list_folders(workspace_id)
            .await?
            .into_iter()
            .map(|folder| ArchiveFolder {
                id: folder.id,
                parent_id: folder.parent_id,
                name: folder.name,
                created_at: folder.created_at,
            })
            .collect();

        let artifacts = sqlx::query(
            r#"
            SELECT id, source_id, folder_id, type, title, path, content_hash, mime_type, language,
                   size_bytes, created_at, updated_at, metadata_json
            FROM artifacts WHERE workspace_id = ?1
              AND NOT EXISTS (SELECT 1 FROM connector_quarantine WHERE connector_quarantine.source_id = artifacts.source_id)
            ORDER BY created_at ASC, id ASC
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?
        .into_iter()
        .map(|row| ArchiveArtifact {
            id: row.get("id"),
            source_id: row.get("source_id"),
            folder_id: row.get("folder_id"),
            artifact_type: row.get("type"),
            title: row.get("title"),
            path: row.get("path"),
            content_hash: row.get("content_hash"),
            mime_type: row.get("mime_type"),
            language: row.get("language"),
            size_bytes: row.get("size_bytes"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            metadata: json_or_empty_object(&row.get::<String, _>("metadata_json")),
        })
        .collect::<Vec<ArchiveArtifact>>();

        let lifecycle = self.export_lifecycle(workspace_id).await?;
        let (memory_cards, omitted_citations) = self.export_memory_cards(workspace_id).await?;
        // A trash is not carried, and the archive says how much was left out instead of staying silent (V0.6.28).
        let trash_left_out: u64 = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM trash_items WHERE workspace_id = ?1 AND expires_at >= ?2",
        )
        .bind(workspace_id)
        .bind(Utc::now().to_rfc3339())
        .fetch_one(self.reader())
        .await?
        .max(0) as u64;
        let tasks = self.export_tasks(workspace_id).await?;
        let comments = self.export_comments(workspace_id).await?;
        let versions = self.export_versions(workspace_id).await?;
        let saved_searches = self.export_saved_searches(workspace_id).await?;
        let work = self.export_work(workspace_id).await?;
        let integrations = self.export_integrations(workspace_id).await?;
        let cairn = self.export_cairn_settings(workspace_id).await?;

        Ok(WorkspaceArchiveData {
            workspace,
            sources,
            folders,
            artifacts,
            lifecycle,
            memory_cards,
            tasks,
            comments,
            omitted_citations,
            trash_left_out,
            work,
            integrations,
            versions,
            saved_searches,
            cairn,
        })
    }

    /// The models a workspace offers and its default, named as their servers name them (V0.9.85). None for a workspace that never
    /// saved settings, so an older archive and a workspace that never chose read the same.
    async fn export_cairn_settings(&self, workspace_id: &str) -> Result<Option<ArchiveCairn>> {
        let settings = self.get_workspace_cairn_settings(workspace_id).await?;
        if settings.version == 0 {
            return Ok(None);
        }
        let mut reference = |model: CairnModel| ArchiveModelRef {
            model: model.model_id,
            purpose: model.purpose,
        };
        let mut offered = Vec::new();
        for id in &settings.policy.offered_model_ids {
            if let Ok(model) = self.get_cairn_model(id).await {
                offered.push(reference(model));
            }
        }
        let default = match &settings.policy.default_model_id {
            Some(id) => self.get_cairn_model(id).await.ok().map(&mut reference),
            None => None,
        };
        Ok(Some(ArchiveCairn {
            offered_models: offered,
            default_model: default,
        }))
    }

    /// The earlier versions of the workspace's files, each file's oldest first (V0.3.19).
    async fn export_versions(&self, workspace_id: &str) -> Result<Vec<ArchiveVersion>> {
        Ok(sqlx::query(
            r#"
            SELECT version.artifact_id, version.version, version.content_hash, version.size_bytes, version.mime_type,
                   version.became_current_at, version.replaced_at,
                   uploader.display_name AS uploader_name, uploader.email AS uploader_email
            FROM artifact_versions AS version
            LEFT JOIN users AS uploader ON uploader.id = version.replaced_by_user_id
            WHERE version.workspace_id = ?1
            ORDER BY version.artifact_id ASC, version.version ASC
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?
        .into_iter()
        .map(|row| ArchiveVersion {
            artifact_id: row.get("artifact_id"),
            version: row.get("version"),
            content_hash: row.get("content_hash"),
            size_bytes: row.get("size_bytes"),
            mime_type: row.get("mime_type"),
            became_current_at: row.get("became_current_at"),
            replaced_at: row.get("replaced_at"),
            replaced_by: optional_person(row.get("uploader_name"), row.get("uploader_email")),
        })
        .collect())
    }

    /// The workspace's saved searches, oldest first (V0.3.19).
    async fn export_saved_searches(&self, workspace_id: &str) -> Result<Vec<ArchiveSavedSearch>> {
        let mut searches = self.list_saved_searches(workspace_id).await?;
        searches.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.name.cmp(&right.name))
        });
        Ok(searches
            .into_iter()
            .map(|search| ArchiveSavedSearch {
                name: search.name,
                query: search.query,
                artifact_types: search
                    .artifact_types
                    .iter()
                    .map(|artifact_type| artifact_type_to_db(artifact_type).to_owned())
                    .collect(),
                languages: search.languages,
                source_ids: search.source_ids,
                result_limit: search.result_limit,
                shared: search.shared,
                created_by: person_from_user(search.created_by),
                created_at: search.created_at,
                updated_at: search.updated_at,
            })
            .collect())
    }

    async fn export_lifecycle(&self, workspace_id: &str) -> Result<Vec<ArchiveLifecycle>> {
        let rows = sqlx::query(
            r#"
            SELECT lifecycle.artifact_id, lifecycle.status, lifecycle.review_note, lifecycle.reviewed_at,
                   lifecycle.superseded_by_artifact_id, lifecycle.created_at, lifecycle.updated_at,
                   owner.display_name AS owner_name, owner.email AS owner_email,
                   reviewer.display_name AS reviewer_name, reviewer.email AS reviewer_email
            FROM artifact_lifecycle AS lifecycle
            JOIN artifacts ON artifacts.id = lifecycle.artifact_id
            LEFT JOIN users AS owner ON owner.id = lifecycle.owner_user_id
            LEFT JOIN users AS reviewer ON reviewer.id = lifecycle.reviewed_by_user_id
            WHERE artifacts.workspace_id = ?1
            ORDER BY lifecycle.created_at ASC, lifecycle.artifact_id ASC
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?;
        let event_rows = sqlx::query(
            r#"
            SELECT event.artifact_id, event.action, event.detail, event.created_at,
                   actor.display_name AS actor_name, actor.email AS actor_email
            FROM artifact_lifecycle_events AS event
            JOIN artifacts ON artifacts.id = event.artifact_id
            LEFT JOIN users AS actor ON actor.id = event.actor_user_id
            WHERE artifacts.workspace_id = ?1
            ORDER BY event.created_at ASC, event.id ASC
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?;

        let mut events: HashMap<String, Vec<ArchiveLifecycleEvent>> = HashMap::new();
        for row in event_rows {
            events
                .entry(row.get("artifact_id"))
                .or_default()
                .push(ArchiveLifecycleEvent {
                    action: row.get("action"),
                    detail: row.get("detail"),
                    actor: optional_person(row.get("actor_name"), row.get("actor_email")),
                    created_at: row.get("created_at"),
                });
        }

        let mut lifecycle = Vec::with_capacity(rows.len());
        let mut with_row = HashSet::new();
        for row in rows {
            let artifact_id: String = row.get("artifact_id");
            with_row.insert(artifact_id.clone());
            lifecycle.push(ArchiveLifecycle {
                events: events.remove(&artifact_id).unwrap_or_default(),
                artifact_id,
                status: row.get("status"),
                owner: optional_person(row.get("owner_name"), row.get("owner_email")),
                review_note: row.get("review_note"),
                reviewed_by: optional_person(row.get("reviewer_name"), row.get("reviewer_email")),
                reviewed_at: row.get("reviewed_at"),
                superseded_by_artifact_id: row.get("superseded_by_artifact_id"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            });
        }
        // History is kept with its file even if the state row was never written.
        let mut orphans = events.into_iter().collect::<Vec<_>>();
        orphans.sort_by(|left, right| left.0.cmp(&right.0));
        for (artifact_id, events) in orphans {
            let at = events.first().map(|event| event.created_at.clone()).unwrap_or_default();
            lifecycle.push(ArchiveLifecycle {
                artifact_id,
                status: "active".to_owned(),
                owner: None,
                review_note: String::new(),
                reviewed_by: None,
                reviewed_at: None,
                superseded_by_artifact_id: None,
                created_at: at.clone(),
                updated_at: at,
                events,
            });
        }
        Ok(lifecycle)
    }

    /// The memory cards with what they cite, and how many citations were left
    /// out because the passage or file they named is gone.
    async fn export_memory_cards(&self, workspace_id: &str) -> Result<(Vec<ArchiveMemoryCard>, u64)> {
        let cards = sqlx::query(
            r#"
            SELECT id, title, body_markdown, source, confidence, created_at, updated_at, metadata_json
            FROM memory_cards WHERE workspace_id = ?1
            ORDER BY created_at ASC, id ASC
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?;
        let links = sqlx::query(
            r#"
            SELECT links.from_id AS card_id, links.to_type AS to_type, links.confidence AS confidence,
                   chunks.artifact_id AS chunk_artifact_id, chunks.chunk_index AS chunk_index,
                   chunks.content_hash AS chunk_hash, chunks.start_line AS start_line,
                   chunks.end_line AS end_line, chunks.heading_path AS heading_path,
                   target.id AS artifact_id
            FROM links
            LEFT JOIN chunks ON links.to_type = 'chunk' AND chunks.id = links.to_id
            LEFT JOIN artifacts AS target ON target.id = CASE
              WHEN links.to_type = 'artifact' THEN links.to_id
              ELSE chunks.artifact_id
            END
            WHERE links.workspace_id = ?1 AND links.from_type = 'memory_card' AND links.relation_type = 'cites'
            ORDER BY links.created_at ASC, links.id ASC
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?;

        let mut citations: HashMap<String, Vec<ArchiveCitation>> = HashMap::new();
        let mut omitted = 0_u64;
        for row in links {
            let Some(artifact_id) = row.get::<Option<String>, _>("artifact_id") else {
                omitted += 1;
                continue;
            };
            let chunk = if row.get::<String, _>("to_type") == "chunk" {
                match (
                    row.get::<Option<i64>, _>("chunk_index"),
                    row.get::<Option<String>, _>("chunk_hash"),
                ) {
                    (Some(chunk_index), Some(content_hash)) => Some(ArchiveChunkRef {
                        chunk_index,
                        content_hash,
                        start_line: row.get("start_line"),
                        end_line: row.get("end_line"),
                        heading_path: row.get("heading_path"),
                    }),
                    _ => {
                        omitted += 1;
                        continue;
                    }
                }
            } else {
                None
            };
            citations.entry(row.get("card_id")).or_default().push(ArchiveCitation {
                artifact_id,
                chunk,
                confidence: row.get("confidence"),
            });
        }

        let artifact_ids: HashSet<String> = sqlx::query_scalar(
            "SELECT id FROM artifacts WHERE workspace_id = ?1 AND NOT EXISTS (SELECT 1 FROM connector_quarantine WHERE connector_quarantine.source_id = artifacts.source_id)",
        )
            .bind(workspace_id)
            .fetch_all(self.reader())
            .await?
            .into_iter()
            .collect();
        let mut knowledge: HashMap<String, CardKnowledge> = HashMap::new();
        for row in &cards {
            let id: String = row.get("id");
            let revisions = self
                .memory_revisions(&id)
                .await?
                .into_iter()
                .rev()
                .map(|stored| ArchiveRevision {
                    revision: stored.revision,
                    title: stored.title,
                    body_markdown: stored.body_markdown,
                    source: stored.source,
                    confidence: stored.confidence,
                    citations: stored
                        .citations
                        .into_iter()
                        .map(|citation| ArchiveRevisionCitation {
                            artifact_id: Some(citation.artifact_id)
                                .filter(|artifact| !artifact.is_empty() && artifact_ids.contains(artifact)),
                            passage: citation.target_type == "chunk",
                            title: citation.title,
                            path: citation.path,
                            confidence: citation.confidence,
                            content_hash: Some(citation.content_hash).filter(|hash| !hash.is_empty()),
                            missing_at_save: citation.missing_at_save,
                        })
                        .collect(),
                    author: stored.author.map(person_from_user).or_else(|| {
                        stored.author_label.map(|label| ArchivePerson {
                            name: label,
                            email: None,
                        })
                    }),
                    created_at: stored.created_at,
                    restored_from: stored.restored_from,
                })
                .collect();
            // Decisions imported from another server come first (they are older), then this server's own.
            let carried: Vec<ArchiveApproval> = self
                .imported_approvals(&id)
                .await?
                .into_iter()
                .rev()
                .map(|record| ArchiveApproval {
                    imported: true,
                    revision: record.revision,
                    decision: record.decision,
                    by: Some(ArchivePerson {
                        name: record.person,
                        email: None,
                    }),
                    note: record.note,
                    created_at: record.decided_at,
                })
                .collect();
            let own = self
                .memory_approval_history(&id)
                .await?
                .into_iter()
                .rev()
                .map(|decision| ArchiveApproval {
                    imported: false,
                    revision: decision.revision,
                    decision: decision.decision,
                    by: decision.user.map(person_from_user),
                    note: decision.note,
                    created_at: decision.created_at,
                });
            let approvals: Vec<ArchiveApproval> = carried.into_iter().chain(own).collect();
            let review = self
                .review_assignment("memory_card", &id)
                .await?
                .filter(|found| found.reviewer.is_some() || found.next_review_at.is_some())
                .map(|found| ArchiveReview {
                    reviewer: found.reviewer.map(person_from_user).or_else(|| {
                        found.reviewer_label.map(|label| ArchivePerson {
                            name: label,
                            email: None,
                        })
                    }),
                    next_review_at: found.next_review_at,
                    last_reviewed_at: found.last_reviewed_at,
                });
            knowledge.insert(id, (revisions, approvals, review));
        }

        let cards = cards
            .into_iter()
            .map(|row| {
                let id: String = row.get("id");
                let (revisions, approvals, review) = knowledge.remove(&id).unwrap_or_default();
                ArchiveMemoryCard {
                    citations: citations.remove(&id).unwrap_or_default(),
                    revisions,
                    approvals,
                    review,
                    id,
                    title: row.get("title"),
                    body_markdown: row.get("body_markdown"),
                    source: row.get("source"),
                    confidence: row.get("confidence"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                    metadata: json_or_empty_object(&row.get::<String, _>("metadata_json")),
                }
            })
            .collect();
        Ok((cards, omitted))
    }

    async fn export_tasks(&self, workspace_id: &str) -> Result<Vec<ArchiveTask>> {
        let tasks = self.list_collaboration_tasks(workspace_id).await?;
        let mut exported = Vec::with_capacity(tasks.len());
        for task in tasks {
            let checklist = self
                .list_task_checklist_items(&task.id)
                .await?
                .into_iter()
                .map(|item| ArchiveChecklistItem {
                    body: item.body,
                    position: item.position,
                    completed_at: item.completed_at,
                    completed_by: item.completed_by.map(person_from_user),
                    created_by: person_from_user(item.created_by),
                    created_at: item.created_at,
                    updated_at: item.updated_at,
                })
                .collect();
            exported.push(ArchiveTask {
                id: task.id,
                title: task.title,
                description: task.description,
                status: task.status,
                priority: task.priority,
                assignee: task.assignee.map(person_from_user),
                created_by: person_from_user(task.created_by),
                artifact_id: task.artifact_id,
                due_at: task.due_at,
                completed_at: task.completed_at,
                created_at: task.created_at,
                updated_at: task.updated_at,
                checklist,
            });
        }
        Ok(exported)
    }

    async fn export_comments(&self, workspace_id: &str) -> Result<Vec<ArchiveComment>> {
        let rows = sqlx::query(
            r#"
            SELECT comment.id, comment.artifact_id, comment.body, comment.created_at, comment.updated_at,
                   author.display_name AS author_name, author.email AS author_email
            FROM artifact_comments AS comment
            JOIN users AS author ON author.id = comment.author_user_id
            WHERE comment.workspace_id = ?1
            ORDER BY comment.created_at ASC, comment.id ASC
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| ArchiveComment {
                id: row.get("id"),
                artifact_id: row.get("artifact_id"),
                author: person(row.get("author_name"), row.get("author_email")),
                body: row.get("body"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect())
    }
}
