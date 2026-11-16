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
    ArchiveChunkRef, ArchiveMemoryCard, ArchivePerson, ArchiveRevision, WorkspaceArchiveData, WorkspaceTemplateRef,
    LIFECYCLE_STATUSES, TASK_PRIORITIES, TASK_STATUSES,
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

fn person_label(person: &ArchivePerson) -> String {
    match person.email.as_deref().filter(|email| !email.is_empty()) {
        Some(email) => format!("{} <{email}>", person.name),
        None => person.name.clone(),
    }
}

impl StorageEngine {
    /// A card's saved history, approvals and review assignment from an archive (V0.6.14). Revisions keep their numbers,
    /// text, dates and the content each citation recorded; their authors are names, since an archive cannot vouch for an
    /// account. An archive from before format 3 carries none, so the card's present state becomes its revision 1
    /// (V0.6.12). An approval comes back as a record and approves nothing here. A reviewer is kept only when the same
    /// address belongs to a person who can work in the new workspace; otherwise the name is kept as a reviewer who has
    /// left, and an administrator assigns someone.
    async fn import_card_knowledge(
        &self,
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        workspace_id: &str,
        card_id: &str,
        card: &ArchiveMemoryCard,
        artifact_ids: &HashMap<String, String>,
    ) -> Result<()> {
        let mut revisions: Vec<&ArchiveRevision> = card.revisions.iter().filter(|found| found.revision >= 1).collect();
        revisions.sort_by_key(|found| found.revision);
        revisions.dedup_by_key(|found| found.revision);
        if revisions.is_empty() {
            crate::memory_revisions::snapshot_memory_card(&mut *tx, card_id, None, None, None).await?;
            sqlx::query("UPDATE memory_card_revisions SET import_baseline = 1 WHERE card_id = ?1")
                .bind(card_id)
                .execute(&mut **tx)
                .await?;
        } else {
            for revision in &revisions {
                let citations: Vec<crate::memory_revisions::RevisionCitation> = revision
                    .citations
                    .iter()
                    .map(|citation| {
                        let artifact = citation
                            .artifact_id
                            .as_ref()
                            .and_then(|id| artifact_ids.get(id))
                            .cloned();
                        crate::memory_revisions::RevisionCitation {
                            target_type: "artifact".to_owned(),
                            target_id: artifact.clone().unwrap_or_default(),
                            confidence: citation.confidence,
                            metadata: "{}".to_owned(),
                            title: citation.title.clone(),
                            path: citation.path.clone(),
                            missing_at_save: citation.missing_at_save || artifact.is_none(),
                            artifact_id: artifact.unwrap_or_default(),
                            content_hash: citation.content_hash.clone().unwrap_or_default(),
                        }
                    })
                    .collect();
                sqlx::query(
                    r#"INSERT INTO memory_card_revisions
                       (id, card_id, workspace_id, revision, title, body_markdown, source, confidence, citations_json,
                        created_by_user_id, created_at, restored_from, author_label, import_baseline)
                       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, ?10, ?11, ?12, 0)"#,
                )
                .bind(Uuid::new_v4().to_string())
                .bind(card_id)
                .bind(workspace_id)
                .bind(revision.revision)
                .bind(&revision.title)
                .bind(&revision.body_markdown)
                .bind(&revision.source)
                .bind(revision.confidence)
                .bind(serde_json::to_string(&citations)?)
                .bind(&revision.created_at)
                .bind(revision.restored_from)
                .bind(revision.author.as_ref().map(person_label))
                .execute(&mut **tx)
                .await?;
            }
            // The newest revision has to be the card as it is. An archive whose card was changed without a revision gets
            // that state as one more.
            let newest = revisions.last().expect("revisions are not empty");
            if newest.title.trim() != card.title.trim()
                || newest.body_markdown.trim() != card.body_markdown.trim()
                || newest.source.trim() != card.source.trim()
            {
                crate::memory_revisions::snapshot_memory_card(&mut *tx, card_id, None, None, None).await?;
            }
        }
        let now = Utc::now().to_rfc3339();
        for approval in &card.approvals {
            sqlx::query(
                r#"INSERT INTO memory_card_imported_approvals
                   (card_id, workspace_id, revision, decision, person_label, note, decided_at, imported_at)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
            )
            .bind(card_id)
            .bind(workspace_id)
            .bind(approval.revision)
            .bind(if approval.decision == "withdrawn" {
                "withdrawn"
            } else {
                "approved"
            })
            .bind(
                approval
                    .by
                    .as_ref()
                    .map_or_else(|| "someone who is no longer known".to_owned(), person_label),
            )
            .bind(&approval.note)
            .bind(&approval.created_at)
            .bind(&now)
            .execute(&mut **tx)
            .await?;
        }
        if let Some(review) = &card.review {
            let mut reviewer: Option<(String, String)> = None;
            if let Some(person) = &review.reviewer {
                if let Some(email) = person.email.as_deref().filter(|email| !email.is_empty()) {
                    if let Some(user) = self.find_user_by_email(email).await? {
                        let role = self.workspace_role_for_user(&user.id, workspace_id).await?;
                        if role.is_some_and(|role| !matches!(role, WorkspaceRole::Viewer)) {
                            reviewer = Some((user.id, user.display_name));
                        }
                    }
                }
            }
            let label = reviewer
                .as_ref()
                .map(|(_, name)| name.clone())
                .or_else(|| review.reviewer.as_ref().map(|person| person.name.clone()));
            sqlx::query(
                r#"INSERT OR REPLACE INTO review_assignments
                   (item_kind, item_id, workspace_id, reviewer_user_id, reviewer_label, next_review_at, last_reviewed_at,
                    assigned_by_user_id, updated_at)
                   VALUES ('memory_card', ?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7)"#,
            )
            .bind(card_id)
            .bind(workspace_id)
            .bind(reviewer.as_ref().map(|(id, _)| id.as_str()))
            .bind(label)
            .bind(&review.next_review_at)
            .bind(&review.last_reviewed_at)
            .bind(&now)
            .execute(&mut **tx)
            .await?;
        }
        Ok(())
    }

    /// Refuses a person who may not create workspaces in the organization.
    pub async fn check_can_create_workspace(&self, creator_id: &str, organization_id: &str) -> Result<()> {
        self.require_workspace_creator(creator_id, organization_id)
            .await
            .map(|_| ())
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

        // Earlier versions of the files: their numbers and dates as exported, nobody credited by account.
        for version in &data.versions {
            let Some(artifact_id) = imported.artifact_ids.get(&version.artifact_id) else {
                continue;
            };
            sqlx::query(
                r#"
                INSERT INTO artifact_versions (
                  id, artifact_id, workspace_id, version, content_hash, size_bytes, mime_type,
                  became_current_at, replaced_at, replaced_by_user_id
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL)
                "#,
            )
            .bind(Uuid::new_v4().to_string())
            .bind(artifact_id)
            .bind(workspace_id)
            .bind(version.version)
            .bind(&version.content_hash)
            .bind(version.size_bytes)
            .bind(&version.mime_type)
            .bind(&version.became_current_at)
            .bind(&version.replaced_at)
            .execute(&mut *tx)
            .await?;
        }

        // Saved searches belong to the person who imports, with the author's name in the title of the copy kept as
        // the name it had; the sources they keep to are the new ones.
        for search in &data.saved_searches {
            let kept_sources = search
                .source_ids
                .iter()
                .filter_map(|old| source_ids.get(old.as_str()).cloned())
                .collect::<Vec<_>>();
            let artifact_types = search
                .artifact_types
                .iter()
                .map(|name| artifact_type_to_db(&artifact_type_from_db(name)).to_owned())
                .collect::<Vec<_>>();
            sqlx::query(
                "INSERT INTO workspace_saved_searches (id, workspace_id, created_by_user_id, name, query, artifact_types_json, languages_json, source_ids_json, result_limit, shared, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(workspace_id)
            .bind(importer_user_id)
            .bind(&search.name)
            .bind(&search.query)
            .bind(serde_json::to_string(&artifact_types)?)
            .bind(serde_json::to_string(&search.languages)?)
            .bind(serde_json::to_string(&kept_sources)?)
            .bind(search.result_limit)
            .bind(search.shared)
            .bind(&search.created_at)
            .bind(&search.updated_at)
            .execute(&mut *tx)
            .await?;
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
                note = with_note(
                    &note,
                    &format!("Reviewed in the exported workspace by {}.", credit(reviewer)),
                );
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

        // Tasks and their checklists. A task keeps its number from the archive when it has one (V0.7.19), so T-12 names the same
        // task after a move; the counter starts above the highest, and above any number that was given out and deleted.
        let preserved: HashMap<&str, i64> = data
            .work
            .task_extras
            .iter()
            .filter(|extra| extra.number > 0)
            .map(|extra| (extra.task_id.as_str(), extra.number))
            .collect();
        let counter = preserved
            .values()
            .copied()
            .max()
            .unwrap_or(0)
            .max(data.work.task_counter);
        if counter > 0 {
            sqlx::query(
                "INSERT INTO workspace_task_counters (workspace_id, last_number) VALUES (?1, ?2)
                 ON CONFLICT(workspace_id) DO UPDATE SET last_number = MAX(last_number, excluded.last_number)",
            )
            .bind(workspace_id)
            .bind(counter)
            .execute(&mut *tx)
            .await?;
        }
        let mut task_ids: HashMap<String, String> = HashMap::new();
        for task in &data.tasks {
            if !TASK_STATUSES.contains(&task.status.as_str()) || !TASK_PRIORITIES.contains(&task.priority.as_str()) {
                bail!("the task {} has a state this server does not know", task.title);
            }
            let id = Uuid::new_v4().to_string();
            let number = match preserved.get(task.id.as_str()) {
                Some(number) => *number,
                None => crate::collaboration::take_task_number(&mut tx, workspace_id).await?,
            };
            task_ids.insert(task.id.clone(), id.clone());
            let mut credits = format!("Created by {} in the exported workspace.", credit(&task.created_by));
            if let Some(assignee) = &task.assignee {
                credits.push_str(&format!(" Assigned to {}.", credit(assignee)));
            }
            sqlx::query(
                r#"
                INSERT INTO workspace_tasks (
                  id, workspace_id, title, description, status, priority, assignee_user_id,
                  created_by_user_id, artifact_id, due_at, completed_at, created_at, updated_at, number
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
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
            .bind(number)
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
        let mut card_ids: HashMap<String, String> = HashMap::new();
        for card in &data.memory_cards {
            let id = Uuid::new_v4().to_string();
            card_ids.insert(card.id.clone(), id.clone());
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
                        insert_citation_link_row(
                            &mut tx,
                            workspace_id,
                            &id,
                            artifact_id,
                            "artifact",
                            citation.confidence,
                            &next_time(),
                        )
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
            self.import_card_knowledge(&mut tx, workspace_id, &id, card, &imported.artifact_ids)
                .await?;
        }

        // The work of the 0.7 line: projects, relations between tasks, schedules (paused) and updates (V0.7.19).
        self.import_work(
            &mut tx,
            workspace_id,
            importer_user_id,
            &data.work,
            &imported.artifact_ids,
            &task_ids,
            &card_ids,
        )
        .await?;

        // The settings that make sense on another server.
        if let Some(policy) = data
            .workspace
            .settings
            .get("ai_min_role")
            .and_then(Value::as_str)
            .filter(|value| matches!(*value, "viewer" | "member" | "admin"))
        {
            sqlx::query(
                "UPDATE workspaces SET settings_json = json_set(settings_json, '$.ai_min_role', ?1) WHERE id = ?2",
            )
            .bind(policy)
            .bind(workspace_id)
            .execute(&mut *tx)
            .await?;
        }

        // The rule that a revision cannot be approved by the person who made it travels with the workspace (V0.6.14).
        if let Some(policy) = data
            .workspace
            .settings
            .get("independent_review")
            .and_then(Value::as_bool)
        {
            sqlx::query(
                "UPDATE workspaces SET settings_json = json_set(settings_json, '$.independent_review', json(?1)) WHERE id = ?2",
            )
            .bind(if policy { "true" } else { "false" })
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
        // A passage that is linked once its file is indexed belongs to the import's own revision 1 while that is
        // still the card's only revision and nobody has edited it: the baseline is written again with it, so it
        // does not become a revision of its own. After any edit the card's history is left alone.
        let baseline = sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*) FROM memory_card_revisions AS kept
               WHERE kept.card_id = ?1 AND kept.revision = 1 AND kept.import_baseline = 1
                 AND NOT EXISTS (SELECT 1 FROM memory_card_revisions AS later WHERE later.card_id = ?1 AND later.revision > 1)"#,
        )
        .bind(card_id)
        .fetch_one(&mut *tx)
        .await?
            > 0;
        if baseline {
            sqlx::query("DELETE FROM memory_card_revisions WHERE card_id = ?1")
                .bind(card_id)
                .execute(&mut *tx)
                .await?;
            crate::memory_revisions::snapshot_memory_card(&mut tx, card_id, None, None, None).await?;
        }
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
