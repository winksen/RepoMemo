//! Bulk work (V0.7.14): one change applied to many tasks (assignee, labels, milestone), previewed first, with an outcome for every
//! task. Authorization and the version check are made for each task, so one refusal never blocks or hides the others, and a task
//! that is refused is left exactly as it was.

use crate::*;
use repomemo_domain::{BulkItem, BulkResult, TaskAttempt};
use repomemo_storage::NewCollaborationTask;
use std::collections::BTreeSet;

/// The most tasks one bulk request names.
const MAX_BULK_TASKS: usize = 100;

/// What stays the same: a save of the task exactly as it is, to which a change is added.
pub(crate) fn save_as_is(current: &CollaborationTask) -> NewCollaborationTask {
    NewCollaborationTask {
        title: current.title.clone(),
        description: current.description.clone(),
        status: current.status.clone(),
        priority: current.priority.clone(),
        assignee_user_id: current.assignee.as_ref().map(|member| member.id.clone()),
        artifact_id: current.artifact_id.clone(),
        due_at: current.due_at.clone(),
        ..Default::default()
    }
}

fn attempt_of(current: &CollaborationTask, saved: &NewCollaborationTask) -> TaskAttempt {
    TaskAttempt {
        title: saved.title.clone(),
        description: saved.description.clone(),
        status: saved.status.clone(),
        priority: saved.priority.clone(),
        assignee_user_id: saved.assignee_user_id.clone(),
        artifact_id: saved.artifact_id.clone(),
        project_id: current.project_id.clone(),
        milestone_id: match &saved.milestone_id {
            Some(milestone) => milestone.clone(),
            None => current.milestone_id.clone(),
        },
        parent_task_id: current.parent_task_id.clone(),
        due_at: saved.due_at.clone(),
    }
}

fn item(
    task: &CollaborationTask,
    outcome: &str,
    reason: Option<String>,
    task_after: Option<CollaborationTask>,
) -> BulkItem {
    BulkItem {
        task_id: task.id.clone(),
        key: task.key.clone(),
        title: task.title.clone(),
        outcome: outcome.to_owned(),
        reason,
        task: task_after,
    }
}

pub(crate) async fn bulk_update_tasks(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<BulkTasksRequest>,
) -> Result<Json<BulkResult>, ApiError> {
    require_workspace_write(&state, &subject, &workspace_id).await?;
    let mut ids: Vec<String> = Vec::new();
    for id in &request.task_ids {
        let id = id.trim().to_owned();
        if !id.is_empty() && !ids.contains(&id) {
            ids.push(id);
        }
    }
    if ids.is_empty() || ids.len() > MAX_BULK_TASKS {
        return Err(ApiError::bad_request(format!(
            "Name between 1 and {MAX_BULK_TASKS} tasks."
        )));
    }
    let changes = &request.changes;
    let assignee = changes.assignee_user_id.as_deref().map(str::trim);
    let milestone = changes.milestone_id.as_deref().map(str::trim);
    let add: BTreeSet<&str> = changes.add_label_ids.iter().map(String::as_str).collect();
    let remove: BTreeSet<&str> = changes.remove_label_ids.iter().map(String::as_str).collect();
    if assignee.is_none() && milestone.is_none() && add.is_empty() && remove.is_empty() {
        return Err(ApiError::bad_request(
            "Name at least one change: an assignee, labels to add or remove, or a milestone.",
        ));
    }
    if add.intersection(&remove).next().is_some() {
        return Err(ApiError::bad_request("A label cannot be both added and removed."));
    }
    // What is the same for every task is checked once: a bad assignee, milestone or label is a bad request, not a refusal.
    if let Some(user_id) = assignee.filter(|value| !value.is_empty()) {
        let member = state
            .storage
            .workspace_role_for_user(user_id, &workspace_id)
            .await
            .map_err(map_storage_error)?;
        if member.is_none() {
            return Err(ApiError::bad_request("Task assignee must be a workspace member."));
        }
    }
    let milestone_project = match milestone.filter(|value| !value.is_empty()) {
        Some(id) => {
            let found = state
                .storage
                .get_milestone(id)
                .await
                .map_err(|_| ApiError::bad_request("Task milestone must belong to this workspace."))?;
            if found.workspace_id != workspace_id {
                return Err(ApiError::bad_request("Task milestone must belong to this workspace."));
            }
            Some(found.project_id)
        }
        None => None,
    };
    for label_id in add.iter().chain(remove.iter()) {
        let label = state
            .storage
            .get_task_label(label_id)
            .await
            .map_err(|_| ApiError::bad_request("Every label must belong to this workspace."))?;
        if label.workspace_id != workspace_id {
            return Err(ApiError::bad_request("Every label must belong to this workspace."));
        }
    }

    let mut items = Vec::with_capacity(ids.len());
    let mut applied_keys = Vec::new();
    for id in &ids {
        let Some(current) = state
            .storage
            .get_collaboration_task(id)
            .await
            .ok()
            .filter(|task| task.workspace_id == workspace_id)
        else {
            items.push(BulkItem {
                task_id: id.clone(),
                key: String::new(),
                title: String::new(),
                outcome: "refused".to_owned(),
                reason: Some("Task was not found in this workspace.".to_owned()),
                task: None,
            });
            continue;
        };
        // The version the person saw, when they sent one: a task that has moved on is left alone and says so.
        if let Some(seen) = request.expected_versions.get(id) {
            if *seen != current.version {
                items.push(item(
                    &current,
                    "refused",
                    Some(format!(
                        "Changed since you looked (you saw version {seen}, it is at {}).",
                        current.version
                    )),
                    None,
                ));
                continue;
            }
        }
        // What this task would become.
        let mut save = save_as_is(&current);
        let mut needs_save = false;
        if let Some(wanted) = assignee {
            let wanted = (!wanted.is_empty()).then(|| wanted.to_owned());
            let now = current.assignee.as_ref().map(|member| member.id.clone());
            if wanted != now {
                save.assignee_user_id = wanted;
                needs_save = true;
            }
        }
        if let Some(wanted) = milestone {
            match (wanted.is_empty(), &milestone_project) {
                (true, _) => {
                    if current.milestone_id.is_some() {
                        save.milestone_id = Some(None);
                        needs_save = true;
                    }
                }
                (false, Some(project)) => {
                    if current.project_id.as_deref() != Some(project.as_str()) {
                        items.push(item(
                            &current,
                            "refused",
                            Some("The task is not in the milestone's project.".to_owned()),
                            None,
                        ));
                        continue;
                    }
                    if current.milestone_id.as_deref() != Some(wanted) {
                        save.milestone_id = Some(Some(wanted.to_owned()));
                        needs_save = true;
                    }
                }
                (false, None) => {}
            }
        }
        let mut label_ids: Vec<String> = current.labels.iter().map(|label| label.id.clone()).collect();
        let before = label_ids.clone();
        label_ids.retain(|label| !remove.contains(label.as_str()));
        for label in &add {
            if !label_ids.iter().any(|held| held == label) {
                label_ids.push((*label).to_owned());
            }
        }
        let labels_change = {
            let (mut a, mut b) = (before.clone(), label_ids.clone());
            a.sort();
            b.sort();
            a != b
        };
        if labels_change && label_ids.len() > repomemo_storage::MAX_LABELS_PER_TASK {
            items.push(item(
                &current,
                "refused",
                Some(format!(
                    "It would have more than {} labels.",
                    repomemo_storage::MAX_LABELS_PER_TASK
                )),
                None,
            ));
            continue;
        }
        if !needs_save && !labels_change {
            items.push(item(&current, "unchanged", Some("Nothing to change.".to_owned()), None));
            continue;
        }
        if request.dry_run {
            items.push(item(&current, "ok", Some("Would be changed.".to_owned()), None));
            continue;
        }
        // The checked save first (it carries the version); labels follow once it has gone through.
        let mut latest = current.clone();
        if needs_save {
            save.expected_version = Some(current.version);
            let attempted = attempt_of(&current, &save);
            match crate::collaboration::save_task(&state, id, save, attempted).await {
                Ok(task) => {
                    if current.assignee.as_ref().map(|member| &member.id)
                        != task.assignee.as_ref().map(|member| &member.id)
                    {
                        notify_task_assignee(&state, &subject.user_id, &task).await;
                    }
                    latest = task;
                }
                Err(crate::collaboration::TaskSaveError::Stale(conflict)) => {
                    items.push(item(&current, "refused", Some(conflict.error.message), None));
                    continue;
                }
                Err(crate::collaboration::TaskSaveError::Api(error)) => {
                    items.push(item(&current, "refused", Some(error.message), None));
                    continue;
                }
            }
        }
        if labels_change {
            match state.storage.set_task_labels(&latest, &label_ids).await {
                Ok(labels) => latest.labels = labels,
                Err(error) => {
                    items.push(item(
                        &latest,
                        "refused",
                        Some(map_storage_error(error).message),
                        Some(latest.clone()),
                    ));
                    continue;
                }
            }
        }
        applied_keys.push(latest.key.clone());
        items.push(item(&latest, "ok", None, Some(latest.clone())));
    }
    let count = |outcome: &str| items.iter().filter(|entry| entry.outcome == outcome).count() as i64;
    let result = BulkResult {
        dry_run: request.dry_run,
        requested: ids.len() as i64,
        succeeded: count("ok"),
        refused: count("refused"),
        unchanged: count("unchanged"),
        items,
    };
    if !request.dry_run && !applied_keys.is_empty() {
        record_workspace_activity(
            &state,
            &workspace_id,
            &subject.user_id,
            "tasks_bulk_updated",
            "task",
            None,
            format!(
                "Changed {} task(s) together: {}.",
                applied_keys.len(),
                applied_keys.iter().take(10).cloned().collect::<Vec<_>>().join(", ")
            ),
        )
        .await;
    }
    Ok(Json(result))
}
