//! Writing a workspace archive into a new workspace (roadmap O2b).
//!
//! Every record gets a new id, and links between records are carried over
//! through the old ids. The records are written in one transaction, so a
//! failed import leaves nothing behind but unreferenced files, which garbage
//! collection removes.
//!
//! **Nobody is credited by account.** An archive can be written by anyone, so
//! it cannot be trusted to say who wrote what: comments, tasks and history
//! belong to the person who imports, and the name the archive gives is kept in
//! the text. The same goes for owners and assignees.

use repomemo_domain::{
    ArchiveChunkRef, ArchivePerson, WorkspaceArchiveData, WorkspaceTemplateRef, LIFECYCLE_STATUSES, TASK_PRIORITIES,
    TASK_STATUSES,
};

use crate::organizations::TEMPLATE_SETTING;

use super::*;

/// A citation of a passage, which can only be placed once its file is indexed.
#[derive(Debug, Clone)]
pub struct PendingPassage {
    pub card_id: String,
    pub artifact_id: String,
    pub chunk: ArchiveChunkRef,
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct ImportedWorkspace {
    /// Artifact id in the archive to its id here.
    pub artifact_ids: HashMap<String, String>,
    pub pending_passages: Vec<PendingPassage>,
}

fn credit(person: &ArchivePerson) -> &str {
    person.name.as_str()
}

fn day(timestamp: &str) -> &str {
    timestamp.get(..10).unwrap_or(timestamp)
}

fn with_note(text: &str, note: &str) -> String {
    if text.trim().is_empty() {
        note.to_owned()
    } else {
        format!("{}\n\n{note}", text.trim_end())
    }
}

impl StorageEngine {
    /// Refuses a person who may not create workspaces in the organization.
    pub async fn check_can_create_workspace(&self, creator_id: &str, organization_id: &str) -> Result<()> {
        self.require_workspace_creator(creator_id, organization_id).await.map(|_| ())
    }

    /// Writes the archive's records into `workspace_id`, which must be empty.
    /// The stored files the artifacts name must already be stored.
    pub async fn import_workspace_data(
        &self,
        workspace_id: &str,
        importer_user_id: &str,
        data: &WorkspaceArchiveData,
    ) -> Result<ImportedWorkspace> {
        let mut imported = ImportedWorkspace::default();
        let mut tx = self.pool.begin().await?;
        let started = Utc::now();
        let mut tick = 0_i64;
        let mut next_time = || {
            tick += 1;
            (started + chrono::Duration::microseconds(tick)).to_rfc3339()
        };

        // Sources. Links to a folder, a repository or a tracker are not
        // carried, so those sources become uploads that keep their name.
        let mut source_ids = HashMap::new();
        for source in &data.sources {
            let id = Uuid::new_v4().to_string();
            let (kind, root_uri) = match source.source_type.as_str() {
                "manual" => ("manual", None),
                "upload" => ("upload", None),
                _ => ("upload", Some(format!("imported:{}", source.name))),
            };
            sqlx::query(
                r#"
                INSERT INTO sources (id, workspace_id, type, name, root_uri, status, metadata_json, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, 'ready', '{}', ?6, ?6)
                "#,
            )
            .bind(&id)
            .bind(workspace_id)
            .bind(kind)
            .bind(&source.name)
            .bind(root_uri)
            .bind(&source.created_at)
            .execute(&mut *tx)
            .await?;
            source_ids.insert(source.id.as_str(), id);
        }

        // Folders, parents first.
        let mut folder_ids: HashMap<&str, String> = HashMap::new();
        let mut waiting: Vec<_> = data.folders.iter().collect();
        while !waiting.is_empty() {
            let before = waiting.len();
            let mut still = Vec::new();
            for folder in waiting {
                let parent = match folder.parent_id.as_deref() {
                    None => None,
                    Some(parent) => match folder_ids.get(parent) {
                        Some(id) => Some(id.clone()),
                        None => {
                            still.push(folder);
                            continue;
                        }
                    },
                };
                let id = Uuid::new_v4().to_string();
                sqlx::query(
                    "INSERT INTO folders (id, workspace_id, parent_id, name, created_by, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                )
                .bind(&id)
                .bind(workspace_id)
                .bind(parent)
                .bind(&folder.name)
                .bind(importer_user_id)
                .bind(&folder.created_at)
                .execute(&mut *tx)
                .await?;
                folder_ids.insert(folder.id.as_str(), id);
            }
            if still.len() == before {
                bail!("the archive's folders do not form a tree");
            }
            waiting = still;
        }

        // Evidence.
        for artifact in &data.artifacts {
            let source_id = source_ids
                .get(artifact.source_id.as_str())
                .with_context(|| format!("{} names a source that is not in the archive", artifact.path))?;
            let id = Uuid::new_v4().to_string();
            let folder_id = artifact
                .folder_id
                .as_deref()
                .and_then(|folder| folder_ids.get(folder).cloned());
            // The index is rebuilt in the background (`index_version` 0 asks
            // for it). Marking a file indexed as of its last change keeps its
            // dates: a file's first indexing would otherwise move its
            // `updated_at`, and every card citing it would look out of date.
            // Images stay unindexed so their analysis is not skipped.
            let indexed_at = (artifact_type_from_db(&artifact.artifact_type) != ArtifactType::Image)
                .then(|| artifact.updated_at.clone());
            sqlx::query(
                r#"
                INSERT INTO artifacts (
                  id, workspace_id, source_id, type, title, path, content_hash, mime_type, language,
                  size_bytes, created_at, updated_at, metadata_json, folder_id, indexed_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
                "#,
            )
            .bind(&id)
            .bind(workspace_id)
            .bind(source_id)
            .bind(artifact_type_to_db(&artifact_type_from_db(&artifact.artifact_type)))
            .bind(&artifact.title)
            .bind(&artifact.path)
            .bind(&artifact.content_hash)
            .bind(&artifact.mime_type)
            .bind(&artifact.language)
            .bind(artifact.size_bytes)
            .bind(&artifact.created_at)
            .bind(&artifact.updated_at)
            .bind(serde_json::to_string(&artifact.metadata)?)
            .bind(folder_id)
            .bind(indexed_at)
            .execute(&mut *tx)
            .await?;
            imported.artifact_ids.insert(artifact.id.clone(), id);
        }

        // Lifecycle and its history.
        for entry in &data.lifecycle {
            let Some(artifact_id) = imported.artifact_ids.get(&entry.artifact_id) else {
                continue;
            };
            if !LIFECYCLE_STATUSES.contains(&entry.status.as_str()) {
                bail!("the lifecycle of {} has an unknown state", entry.artifact_id);
            }
            let mut note = entry.review_note.clone();
            if let Some(owner) = &entry.owner {
                note = with_note(&note, &format!("Owner in the exported workspace: {}.", credit(owner)));
            }
            if let Some(reviewer) = &entry.reviewed_by {
                note = with_note(&note, &format!("Reviewed in the exported workspace by {}.", credit(reviewer)));
            }
            let superseded_by = entry
                .superseded_by_artifact_id
                .as_deref()
                .and_then(|old| imported.artifact_ids.get(old));
            sqlx::query(
                r#"
                INSERT INTO artifact_lifecycle (
                  artifact_id, workspace_id, status, owner_user_id, review_note,
                  reviewed_by_user_id, reviewed_at, superseded_by_artifact_id, created_at, updated_at
                ) VALUES (?1, ?2, ?3, NULL, ?4, NULL, ?5, ?6, ?7, ?8)
                "#,
            )
            .bind(artifact_id)
            .bind(workspace_id)
            .bind(&entry.status)
            .bind(note)
            .bind(&entry.reviewed_at)
            .bind(superseded_by)
            .bind(&entry.created_at)
            .bind(&entry.updated_at)
            .execute(&mut *tx)
            .await?;
            for event in &entry.events {
                let detail = match &event.actor {
                    Some(actor) => format!("{}: {}", credit(actor), event.detail),
                    None => event.detail.clone(),
                };
                sqlx::query(
                    "INSERT INTO artifact_lifecycle_events (id, artifact_id, actor_user_id, action, detail, created_at) VALUES (?1, ?2, NULL, ?3, ?4, ?5)",
                )
                .bind(Uuid::new_v4().to_string())
                .bind(artifact_id)
                .bind(&event.action)
                .bind(detail)
                .bind(&event.created_at)
                .execute(&mut *tx)
                .await?;
            }
        }

        // Comments.
        for comment in &data.comments {
            let Some(artifact_id) = imported.artifact_ids.get(&comment.artifact_id) else {
                continue;
            };
            let body = format!(
                "Originally written by {} on {}:\n\n{}",
                credit(&comment.author),
                day(&comment.created_at),
                comment.body
            );
            sqlx::query(
                "INSERT INTO artifact_comments (id, workspace_id, artifact_id, author_user_id, body, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(workspace_id)
            .bind(artifact_id)
            .bind(importer_user_id)
            .bind(body)
            .bind(&comment.created_at)
            .bind(&comment.updated_at)
            .execute(&mut *tx)
            .await?;
        }

        // Tasks and their checklists.
        for task in &data.tasks {
            if !TASK_STATUSES.contains(&task.status.as_str()) || !TASK_PRIORITIES.contains(&task.priority.as_str()) {
                bail!("the task {} has a state this server does not know", task.title);
            }
            let id = Uuid::new_v4().to_string();
            let mut credits = format!("Created by {} in the exported workspace.", credit(&task.created_by));
            if let Some(assignee) = &task.assignee {
                credits.push_str(&format!(" Assigned to {}.", credit(assignee)));
            }
            sqlx::query(
                r#"
                INSERT INTO workspace_tasks (
                  id, workspace_id, title, description, status, priority, assignee_user_id,
                  created_by_user_id, artifact_id, due_at, completed_at, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?8, ?9, ?10, ?11, ?12)
                "#,
            )
            .bind(&id)
            .bind(workspace_id)
            .bind(&task.title)
            .bind(with_note(&task.description, &credits))
            .bind(&task.status)
            .bind(&task.priority)
            .bind(importer_user_id)
            .bind(
                task.artifact_id
                    .as_deref()
                    .and_then(|old| imported.artifact_ids.get(old)),
            )
            .bind(&task.due_at)
            .bind(&task.completed_at)
            .bind(&task.created_at)
            .bind(&task.updated_at)
            .execute(&mut *tx)
            .await?;
            for item in &task.checklist {
                sqlx::query(
                    r#"
                    INSERT INTO workspace_task_checklist_items (
                      id, task_id, workspace_id, created_by_user_id, body, position,
                      completed_at, completed_by_user_id, created_at, updated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8, ?9)
                    "#,
                )
                .bind(Uuid::new_v4().to_string())
                .bind(&id)
                .bind(workspace_id)
                .bind(importer_user_id)
                .bind(&item.body)
                .bind(item.position)
                .bind(&item.completed_at)
                .bind(&item.created_at)
                .bind(&item.updated_at)
                .execute(&mut *tx)
                .await?;
            }
        }

        // Memory cards. A file citation is linked now; a passage waits for
        // the file's index.
        for card in &data.memory_cards {
            let id = Uuid::new_v4().to_string();
            sqlx::query(
                r#"
                INSERT INTO memory_cards (
                  id, workspace_id, title, body_markdown, source, confidence, created_at, updated_at, metadata_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                "#,
            )
            .bind(&id)
            .bind(workspace_id)
            .bind(&card.title)
            .bind(&card.body_markdown)
            .bind(&card.source)
            .bind(card.confidence)
            .bind(&card.created_at)
            .bind(&card.updated_at)
            .bind(serde_json::to_string(&card.metadata)?)
            .execute(&mut *tx)
            .await?;
            for citation in &card.citations {
                let Some(artifact_id) = imported.artifact_ids.get(&citation.artifact_id) else {
                    continue;
                };
                match &citation.chunk {
                    None => {
                        insert_citation_link_row(&mut tx, workspace_id, &id, artifact_id, "artifact", citation.confidence, &next_time())
                            .await?
                    }
                    Some(chunk) => imported.pending_passages.push(PendingPassage {
                        card_id: id.clone(),
                        artifact_id: artifact_id.clone(),
                        chunk: chunk.clone(),
                        confidence: citation.confidence,
                    }),
                }
            }
        }

        // The settings that make sense on another server.
        if let Some(policy) = data
            .workspace
            .settings
            .get("ai_min_role")
            .and_then(Value::as_str)
            .filter(|value| matches!(*value, "viewer" | "member" | "admin"))
        {
            sqlx::query("UPDATE workspaces SET settings_json = json_set(settings_json, '$.ai_min_role', ?1) WHERE id = ?2")
                .bind(policy)
                .bind(workspace_id)
                .execute(&mut *tx)
                .await?;
        }

        // The template, by its reference: a server that lacks the template resolves it to `general` (V0.2.166).
        if let Some(reference) = data
            .workspace
            .settings
            .get(TEMPLATE_SETTING)
            .and_then(WorkspaceTemplateRef::from_setting)
        {
            sqlx::query(&format!(
                "UPDATE workspaces SET settings_json = json_set(settings_json, '$.{TEMPLATE_SETTING}', json(?1)) WHERE id = ?2"
            ))
            .bind(serde_json::to_string(&reference)?)
            .bind(workspace_id)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(imported)
    }

    /// Links a card to a passage (or, when the passage was not found, to its
    /// whole file) after an import.
    pub async fn link_imported_citation(
        &self,
        workspace_id: &str,
        card_id: &str,
        artifact_id: &str,
        chunk_id: Option<&str>,
        confidence: Option<f64>,
    ) -> Result<()> {
        let (target, kind) = match chunk_id {
            Some(chunk) => (chunk, "chunk"),
            None => (artifact_id, "artifact"),
        };
        let mut tx = self.pool.begin().await?;
        insert_citation_link_row(
            &mut tx,
            workspace_id,
            card_id,
            target,
            kind,
            confidence,
            &Utc::now().to_rfc3339(),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
}

async fn insert_citation_link_row(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    workspace_id: &str,
    card_id: &str,
    target_id: &str,
    target_type: &str,
    confidence: Option<f64>,
    at: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO links (
          id, workspace_id, from_id, from_type, to_id, to_type,
          relation_type, confidence, created_by, metadata_json, created_at
        ) VALUES (?1, ?2, ?3, 'memory_card', ?4, ?5, 'cites', ?6, 'user', '{}', ?7)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(workspace_id)
    .bind(card_id)
    .bind(target_id)
    .bind(target_type)
    .bind(confidence)
    .bind(at)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
