//! The work of the 0.7 line in a workspace archive (V0.7.19): projects, milestones, labels, named workflow states, task relations,
//! recurring schedules and project updates, out as plain records named by the archive's own ids and back in as new records.
//!
//! Every relation is carried through the archive's ids only, so a record can never end up pointing at something of another
//! workspace (the check refuses such an archive before this runs). Nobody is credited by account: the importer is the author of
//! what comes back, with the name the archive gives kept in the text, as for the rest of an archive. **A recurring task always
//! comes back paused** (or stopped, if it was), so nothing is made until someone resumes it.

use super::*;
use repomemo_domain::{
    ArchiveMilestone, ArchiveOccurrence, ArchivePerson, ArchiveProject, ArchiveProjectUpdate, ArchiveRecurring,
    ArchiveReference, ArchiveTaskComment, ArchiveTaskExtra, ArchiveUpdateLink, ArchiveWork, ArchiveWorkflowState,
    BUILTIN_STATE_CATEGORIES, LABEL_COLOR_NAMES,
};

fn archive_person(user: SharedUser) -> ArchivePerson {
    ArchivePerson {
        name: user.display_name,
        email: user.email,
    }
}

fn credit(person: &ArchivePerson) -> String {
    match person.email.as_deref().filter(|email| !email.is_empty()) {
        Some(email) => format!("{} <{email}>", person.name),
        None => person.name.clone(),
    }
}

fn with_note(text: &str, note: &str) -> String {
    if text.trim().is_empty() {
        note.to_owned()
    } else {
        format!("{}\n\n{note}", text.trim_end())
    }
}

impl StorageEngine {
    /// Everything the workspace has from the 0.7 line, for an archive.
    pub(crate) async fn export_work(&self, workspace_id: &str) -> Result<ArchiveWork> {
        let task_counter: i64 = sqlx::query_scalar(
            "SELECT COALESCE((SELECT last_number FROM workspace_task_counters WHERE workspace_id = ?1), 0)",
        )
        .bind(workspace_id)
        .fetch_one(self.reader())
        .await?;
        let mut all_projects = Vec::new();
        for status in ["active", "on_hold", "done", "archived"] {
            all_projects.extend(self.list_projects(workspace_id, Some(status)).await?);
        }
        all_projects.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));
        let reference = |reference: repomemo_domain::ProjectReference| ArchiveReference {
            kind: reference.kind,
            target_id: reference.target_id,
            note: reference.note,
            added_by: reference.added_by.map(archive_person),
            created_at: reference.created_at,
        };
        let mut projects = Vec::new();
        let mut milestones = Vec::new();
        let mut updates = Vec::new();
        for project in all_projects {
            let references = self
                .list_project_references(&project.id)
                .await?
                .into_iter()
                .map(reference)
                .collect();
            for milestone in self.list_milestones(&project.id).await? {
                milestones.push(ArchiveMilestone {
                    id: milestone.id,
                    project_id: milestone.project_id,
                    name: milestone.name,
                    description: milestone.description,
                    due_date: milestone.due_date,
                    completion_rule: milestone.completion_rule,
                    completed_at: milestone.completed_at,
                    created_at: milestone.created_at,
                    updated_at: milestone.updated_at,
                });
            }
            for update in self
                .list_project_updates(&project.id, true, 1_000)
                .await?
                .into_iter()
                .rev()
            {
                updates.push(ArchiveProjectUpdate {
                    id: update.id,
                    project_id: update.project_id,
                    author: update.author.map(archive_person),
                    health: update.health,
                    summary: update.summary,
                    reason: update.reason,
                    snapshot: update.snapshot,
                    changes: update.changes,
                    links: update
                        .links
                        .into_iter()
                        .map(|link| ArchiveUpdateLink {
                            kind: link.kind,
                            target_id: link.target_id,
                            label: link.label,
                        })
                        .collect(),
                    created_at: update.created_at,
                    retracted_at: update.retracted_at,
                });
            }
            projects.push(ArchiveProject {
                id: project.id,
                name: project.name,
                goal: project.goal,
                status: project.status,
                owner: project.owner.map(archive_person),
                created_by: project.created_by.map(archive_person),
                target_date: project.target_date,
                archived_at: project.archived_at,
                created_at: project.created_at,
                updated_at: project.updated_at,
                references,
            });
        }
        let labels = self
            .list_task_labels(workspace_id)
            .await?
            .into_iter()
            .map(|label| repomemo_domain::ArchiveLabel {
                id: label.id,
                name: label.name,
                color: label.color,
            })
            .collect();
        let workflow_states = self
            .list_workflow_states(workspace_id)
            .await?
            .into_iter()
            .filter(|state| !state.builtin)
            .map(|state| ArchiveWorkflowState {
                id: state.id,
                name: state.name,
                category: state.category,
                position: state.position,
            })
            .collect();
        let mut task_extras = Vec::new();
        for task in self.list_collaboration_tasks(workspace_id).await? {
            let references = self
                .list_task_references(&task.id)
                .await?
                .into_iter()
                .map(|found| ArchiveReference {
                    kind: found.kind,
                    target_id: found.target_id,
                    note: found.note,
                    added_by: found.added_by.map(archive_person),
                    created_at: found.created_at,
                })
                .collect();
            let comments = self
                .list_task_comments(&task.id)
                .await?
                .into_iter()
                .map(|comment| ArchiveTaskComment {
                    author: comment.author.map_or_else(
                        || ArchivePerson {
                            name: "Former member".to_owned(),
                            email: None,
                        },
                        archive_person,
                    ),
                    body: comment.body,
                    created_at: comment.created_at,
                    updated_at: comment.updated_at,
                })
                .collect();
            let (blocked_by, _) = self.list_task_dependencies(&task.id).await?;
            task_extras.push(ArchiveTaskExtra {
                number: task.number,
                project_id: task.project_id,
                milestone_id: task.milestone_id,
                parent_task_id: task.parent_task_id,
                label_ids: task.labels.into_iter().map(|label| label.id).collect(),
                state_id: (!BUILTIN_STATE_CATEGORIES.contains(&task.state_id.as_str())).then_some(task.state_id),
                references,
                comments,
                blocked_by: blocked_by.into_iter().map(|link| link.blocker.id).collect(),
                task_id: task.id,
            });
        }
        let mut recurring = Vec::new();
        for schedule in self.list_recurring_tasks(workspace_id).await? {
            let occurrences = self
                .list_recurring_occurrences(&schedule.id, 100_000)
                .await?
                .into_iter()
                .rev()
                .map(|occurrence| ArchiveOccurrence {
                    occurrence_date: occurrence.occurrence_date,
                    outcome: occurrence.outcome,
                    task_id: occurrence.task_id,
                    created_at: schedule.created_at.clone(),
                })
                .collect();
            recurring.push(ArchiveRecurring {
                id: schedule.id,
                title: schedule.title,
                description: schedule.description,
                priority: schedule.priority,
                assignee: schedule.assignee.map(archive_person),
                project_id: schedule.project_id,
                due_after_days: schedule.due_after_days,
                schedule: schedule.schedule,
                missed_policy: schedule.missed_policy,
                status: schedule.status,
                created_at: schedule.created_at,
                updated_at: schedule.updated_at,
                stopped_at: schedule.stopped_at,
                occurrences,
            });
        }
        Ok(ArchiveWork {
            task_counter,
            projects,
            milestones,
            labels,
            workflow_states,
            task_extras,
            recurring,
            updates,
        })
    }

    /// A person of the archive as a member of the new workspace, by address; none when nobody here has it.
    async fn member_by_email(&self, workspace_id: &str, person: &ArchivePerson) -> Result<Option<String>> {
        let Some(email) = person.email.as_deref().filter(|email| !email.is_empty()) else {
            return Ok(None);
        };
        let Some(user) = self.find_user_by_email(email).await? else {
            return Ok(None);
        };
        Ok(self
            .workspace_role_for_user(&user.id, workspace_id)
            .await?
            .map(|_| user.id))
    }

    /// Writes the work section into the new workspace, inside the import's transaction. `task_ids`, `artifact_ids` and `card_ids`
    /// take the archive's ids to the new ones; a citation whose target is not among them is left out.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn import_work(
        &self,
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        workspace_id: &str,
        importer: &str,
        work: &ArchiveWork,
        artifact_ids: &HashMap<String, String>,
        task_ids: &HashMap<String, String>,
        card_ids: &HashMap<String, String>,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let mut label_map: HashMap<&str, String> = HashMap::new();
        for label in &work.labels {
            let id = Uuid::new_v4().to_string();
            let color = if LABEL_COLOR_NAMES.contains(&label.color.as_str()) {
                label.color.as_str()
            } else {
                "gray"
            };
            sqlx::query(
                "INSERT INTO task_labels (id, workspace_id, name, name_key, color, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
            )
            .bind(&id)
            .bind(workspace_id)
            .bind(&label.name)
            .bind(label.name.to_lowercase())
            .bind(color)
            .bind(&now)
            .execute(&mut **tx)
            .await?;
            label_map.insert(label.id.as_str(), id);
        }
        let mut state_map: HashMap<&str, String> = HashMap::new();
        for state in &work.workflow_states {
            let id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO workflow_states (id, workspace_id, name, name_key, category, position, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
            )
            .bind(&id)
            .bind(workspace_id)
            .bind(&state.name)
            .bind(state.name.to_lowercase())
            .bind(&state.category)
            .bind(state.position)
            .bind(&now)
            .execute(&mut **tx)
            .await?;
            state_map.insert(state.id.as_str(), id);
        }
        let target = |reference: &ArchiveReference| -> Option<String> {
            match reference.kind.as_str() {
                "memory_card" => card_ids.get(&reference.target_id).cloned(),
                _ => artifact_ids.get(&reference.target_id).cloned(),
            }
        };
        let kind_of = |reference: &ArchiveReference| {
            if reference.kind == "memory_card" {
                "memory_card"
            } else {
                "evidence"
            }
        };

        let mut project_map: HashMap<&str, String> = HashMap::new();
        for project in &work.projects {
            let id = Uuid::new_v4().to_string();
            let mut owner = None;
            let mut goal = project.goal.clone();
            if let Some(person) = &project.owner {
                owner = self.member_by_email(workspace_id, person).await?;
                if owner.is_none() {
                    goal = with_note(&goal, &format!("Owner in the exported workspace: {}.", credit(person)));
                }
            }
            sqlx::query(
                "INSERT INTO workspace_projects (id, workspace_id, name, goal, status, owner_user_id, created_by_user_id, target_date, archived_at, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            )
            .bind(&id)
            .bind(workspace_id)
            .bind(&project.name)
            .bind(goal)
            .bind(&project.status)
            .bind(owner)
            .bind(importer)
            .bind(&project.target_date)
            .bind(&project.archived_at)
            .bind(&project.created_at)
            .bind(&project.updated_at)
            .execute(&mut **tx)
            .await?;
            for reference in &project.references {
                let Some(target_id) = target(reference) else { continue };
                sqlx::query(
                    "INSERT OR IGNORE INTO project_references (id, project_id, workspace_id, kind, target_id, note, added_by_user_id, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                )
                .bind(Uuid::new_v4().to_string())
                .bind(&id)
                .bind(workspace_id)
                .bind(kind_of(reference))
                .bind(target_id)
                .bind(&reference.note)
                .bind(importer)
                .bind(&reference.created_at)
                .execute(&mut **tx)
                .await?;
            }
            project_map.insert(project.id.as_str(), id);
        }
        let mut milestone_map: HashMap<&str, String> = HashMap::new();
        for milestone in &work.milestones {
            let Some(project_id) = project_map.get(milestone.project_id.as_str()) else {
                continue;
            };
            let id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO project_milestones (id, project_id, workspace_id, name, description, due_date, completion_rule, completed_at, created_by_user_id, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            )
            .bind(&id)
            .bind(project_id)
            .bind(workspace_id)
            .bind(&milestone.name)
            .bind(&milestone.description)
            .bind(&milestone.due_date)
            .bind(&milestone.completion_rule)
            .bind(&milestone.completed_at)
            .bind(importer)
            .bind(&milestone.created_at)
            .bind(&milestone.updated_at)
            .execute(&mut **tx)
            .await?;
            milestone_map.insert(milestone.id.as_str(), id);
        }

        // Tasks: where each belongs, what it carries, what it cites, what was said about it and what blocks it.
        for extra in &work.task_extras {
            let Some(task_id) = task_ids.get(&extra.task_id) else {
                continue;
            };
            sqlx::query("UPDATE workspace_tasks SET project_id = ?2, milestone_id = ?3, state_id = ?4, parent_task_id = ?5 WHERE id = ?1")
                .bind(task_id)
                .bind(extra.project_id.as_deref().and_then(|id| project_map.get(id)))
                .bind(extra.milestone_id.as_deref().and_then(|id| milestone_map.get(id)))
                .bind(extra.state_id.as_deref().and_then(|id| state_map.get(id)))
                .bind(extra.parent_task_id.as_deref().and_then(|id| task_ids.get(id)))
                .execute(&mut **tx)
                .await?;
            for label in &extra.label_ids {
                if let Some(label_id) = label_map.get(label.as_str()) {
                    sqlx::query(
                        "INSERT OR IGNORE INTO task_label_links (task_id, label_id, created_at) VALUES (?1, ?2, ?3)",
                    )
                    .bind(task_id)
                    .bind(label_id)
                    .bind(&now)
                    .execute(&mut **tx)
                    .await?;
                }
            }
            for reference in &extra.references {
                let Some(target_id) = target(reference) else { continue };
                sqlx::query(
                    "INSERT OR IGNORE INTO task_references (id, task_id, workspace_id, kind, target_id, note, added_by_user_id, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                )
                .bind(Uuid::new_v4().to_string())
                .bind(task_id)
                .bind(workspace_id)
                .bind(kind_of(reference))
                .bind(target_id)
                .bind(&reference.note)
                .bind(importer)
                .bind(&reference.created_at)
                .execute(&mut **tx)
                .await?;
            }
            for comment in &extra.comments {
                sqlx::query(
                    "INSERT INTO task_comments (id, task_id, workspace_id, author_user_id, body, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                )
                .bind(Uuid::new_v4().to_string())
                .bind(task_id)
                .bind(workspace_id)
                .bind(importer)
                .bind(with_note(
                    &comment.body,
                    &format!("(Written by {} in the exported workspace.)", credit(&comment.author)),
                ))
                .bind(&comment.created_at)
                .bind(&comment.updated_at)
                .execute(&mut **tx)
                .await?;
            }
            for blocker in &extra.blocked_by {
                let Some(blocker_id) = task_ids.get(blocker) else {
                    continue;
                };
                if blocker_id == task_id {
                    continue;
                }
                sqlx::query(
                    "INSERT OR IGNORE INTO task_dependencies (id, workspace_id, blocker_task_id, blocked_task_id, created_by_user_id, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                )
                .bind(Uuid::new_v4().to_string())
                .bind(workspace_id)
                .bind(blocker_id)
                .bind(task_id)
                .bind(importer)
                .bind(&now)
                .execute(&mut **tx)
                .await?;
            }
        }
        // The single file link of an older task is one of its references too.
        sqlx::query(
            "INSERT OR IGNORE INTO task_references (id, task_id, workspace_id, kind, target_id, note, added_by_user_id, created_at)
             SELECT lower(hex(randomblob(16))), id, workspace_id, 'evidence', artifact_id, '', created_by_user_id, created_at
             FROM workspace_tasks WHERE workspace_id = ?1 AND artifact_id IS NOT NULL",
        )
        .bind(workspace_id)
        .execute(&mut **tx)
        .await?;

        // Schedules always come back paused (or stopped); what they looked at before is kept as history.
        let yesterday = (Utc::now() - chrono::Duration::days(1)).format("%Y-%m-%d").to_string();
        for recurring in &work.recurring {
            let id = Uuid::new_v4().to_string();
            let mut description = recurring.description.clone();
            let assignee = match &recurring.assignee {
                Some(person) => {
                    let found = self.member_by_email(workspace_id, person).await?;
                    if found.is_none() {
                        description = with_note(
                            &description,
                            &format!("Assigned in the exported workspace to {}.", credit(person)),
                        );
                    }
                    found
                }
                None => None,
            };
            let status = if recurring.status == "stopped" {
                "stopped"
            } else {
                "paused"
            };
            sqlx::query(
                "INSERT INTO recurring_tasks (id, workspace_id, title, description, priority, assignee_user_id, project_id, due_after_days,
                    schedule_kind, interval_count, weekday, day_of_month, start_date, missed_policy, status, processed_through,
                    created_by_user_id, created_at, updated_at, stopped_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
            )
            .bind(&id)
            .bind(workspace_id)
            .bind(&recurring.title)
            .bind(description)
            .bind(&recurring.priority)
            .bind(assignee)
            .bind(recurring.project_id.as_deref().and_then(|project| project_map.get(project)))
            .bind(recurring.due_after_days)
            .bind(&recurring.schedule.kind)
            .bind(recurring.schedule.interval)
            .bind(recurring.schedule.weekday)
            .bind(recurring.schedule.day_of_month)
            .bind(&recurring.schedule.start_date)
            .bind(&recurring.missed_policy)
            .bind(status)
            .bind(&yesterday)
            .bind(importer)
            .bind(&recurring.created_at)
            .bind(&recurring.updated_at)
            .bind(&recurring.stopped_at)
            .execute(&mut **tx)
            .await?;
            for occurrence in &recurring.occurrences {
                sqlx::query(
                    "INSERT OR IGNORE INTO recurring_occurrences (recurring_id, occurrence_date, outcome, task_id, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                )
                .bind(&id)
                .bind(&occurrence.occurrence_date)
                .bind(if occurrence.outcome == "created" { "created" } else { "skipped" })
                .bind(occurrence.task_id.as_deref().and_then(|task| task_ids.get(task)))
                .bind(&occurrence.created_at)
                .execute(&mut **tx)
                .await?;
            }
        }

        // Updates come back as records: the person who wrote each is named in its text, and the health stays the one they chose.
        for update in &work.updates {
            let Some(project_id) = project_map.get(update.project_id.as_str()) else {
                continue;
            };
            let id = Uuid::new_v4().to_string();
            let mut snapshot = update.snapshot.clone();
            for milestone in &mut snapshot.milestones {
                if let Some(new_id) = milestone_map.get(milestone.id.as_str()) {
                    milestone.id = new_id.clone();
                }
            }
            let reason = match &update.author {
                Some(person) => with_note(
                    &update.reason,
                    &format!("(Published by {} in the exported workspace.)", credit(person)),
                ),
                None => update.reason.clone(),
            };
            sqlx::query(
                "INSERT INTO project_updates (id, project_id, workspace_id, author_user_id, health, summary, reason, snapshot_json, changes_json, created_at, retracted_at)
                 VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )
            .bind(&id)
            .bind(project_id)
            .bind(workspace_id)
            .bind(&update.health)
            .bind(&update.summary)
            .bind(reason)
            .bind(serde_json::to_string(&snapshot)?)
            .bind(serde_json::to_string(&update.changes)?)
            .bind(&update.created_at)
            .bind(&update.retracted_at)
            .execute(&mut **tx)
            .await?;
            for link in &update.links {
                let new_target = match link.kind.as_str() {
                    "milestone" => milestone_map.get(link.target_id.as_str()).cloned(),
                    "task" => task_ids.get(&link.target_id).cloned(),
                    "memory_card" => card_ids.get(&link.target_id).cloned(),
                    _ => artifact_ids.get(&link.target_id).cloned(),
                };
                let Some(new_target) = new_target else { continue };
                sqlx::query("INSERT OR IGNORE INTO project_update_links (update_id, kind, target_id, label) VALUES (?1, ?2, ?3, ?4)")
                    .bind(&id)
                    .bind(match link.kind.as_str() {
                        "milestone" | "task" | "memory_card" => link.kind.as_str(),
                        _ => "evidence",
                    })
                    .bind(new_target)
                    .bind(&link.label)
                    .execute(&mut **tx)
                    .await?;
            }
        }
        Ok(())
    }
}
