//! "My work" (V0.6.2): one permission-filtered list of what waits for the caller across workspaces. Nothing is stored;
//! each part is read from its own place with current access applied, so revoked access removes the work and the count.

use axum::{extract::State, Json};
use chrono::Utc;
use repomemo_domain::{MyWork, MyWorkReview};

use crate::{map_storage_error, ApiError, AppState, AuthenticatedSubject};

pub(crate) async fn my_work(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<MyWork>, ApiError> {
    let mut tasks = state
        .storage
        .list_assigned_collaboration_tasks(&subject.user_id)
        .await
        .map_err(map_storage_error)?;
    tasks.retain(|task| subject.token_reaches(&task.workspace_id));
    let reviews: Vec<MyWorkReview> = state
        .storage
        .evidence_reviews_owned_by(&subject.user_id)
        .await
        .map_err(map_storage_error)?
        .into_iter()
        .filter(|review| subject.token_reaches(&review.workspace_id))
        .map(|review| MyWorkReview {
            kind: "evidence".to_owned(),
            workspace_id: review.workspace_id,
            workspace_name: review.workspace_name,
            artifact_id: review.artifact_id,
            title: review.title,
            reason: review.reason,
            since: review.since,
        })
        .collect();
    // Reviews assigned to the person whose date has passed; a file already listed above is not listed twice.
    let mut reviews = reviews;
    for (due, workspace_name) in state
        .storage
        .due_reviews_for(&subject.user_id)
        .await
        .map_err(map_storage_error)?
    {
        let listed = reviews
            .iter()
            .any(|review| review.kind == due.item_kind && review.artifact_id == due.item_id);
        if listed || !subject.token_reaches(&due.workspace_id) {
            continue;
        }
        reviews.push(MyWorkReview {
            kind: due.item_kind,
            workspace_id: due.workspace_id,
            workspace_name,
            artifact_id: due.item_id,
            title: due.title,
            reason: "Its review date has passed.".to_owned(),
            since: due.next_review_at,
        });
    }
    // A token limited to some workspaces gets no mentions, as with notifications: they may name any workspace.
    let mentions = if subject.token.as_ref().is_some_and(|grant| grant.workspaces.is_some()) {
        Vec::new()
    } else {
        state
            .storage
            .unread_mentions_for(&subject.user_id)
            .await
            .map_err(map_storage_error)?
    };
    let now = Utc::now().to_rfc3339();
    let overdue_task_count = tasks
        .iter()
        .filter(|task| task.due_at.as_deref().is_some_and(|due| due < now.as_str()))
        .count() as i64;
    Ok(Json(MyWork {
        total_count: (tasks.len() + reviews.len() + mentions.len()) as i64,
        overdue_task_count,
        tasks,
        reviews,
        mentions,
    }))
}
