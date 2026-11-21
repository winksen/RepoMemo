use crate::*;

#[derive(Debug, Deserialize)]
pub(crate) struct RegisterRequest {
    pub(crate) email: String,
    pub(crate) display_name: String,
    pub(crate) password: String,
    /// An invitation for this email: lets the account be created even when
    /// registration is closed. Joining happens when the invitation is accepted.
    #[serde(default)]
    pub(crate) invitation_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LoginRequest {
    pub(crate) email: String,
    pub(crate) password: String,
    /// A refresh token this browser still holds, if any: it shows the
    /// sign-in comes from a device the account already uses (review L10).
    #[serde(default)]
    pub(crate) refresh_token: Option<String>,
    /// The current code of the account's authenticator app, or an unused recovery code, when two-step sign-in is on.
    #[serde(default)]
    pub(crate) mfa_code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateProfileRequest {
    pub(crate) display_name: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChangePasswordRequest {
    pub(crate) current_password: String,
    pub(crate) new_password: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateOrganizationRequest {
    pub(crate) name: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateOrganizationRequest {
    pub(crate) name: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpsertOrganizationMemberRequest {
    pub(crate) email: String,
    pub(crate) role: OrganizationRole,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateWorkspaceRequest {
    pub(crate) organization_id: String,
    pub(crate) name: String,
    /// The template the workspace starts with (V0.2.168); none means `general`, with no folders and no choice saved.
    #[serde(default)]
    pub(crate) template_id: Option<String>,
    /// `shared` (the default) adds everyone in the organization; `restricted` starts with the creator alone.
    #[serde(default)]
    pub(crate) access_policy: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateWorkspaceRequest {
    pub(crate) name: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateWorkspaceColorRequest {
    /// The colour's key, or none to go back to the default colour.
    #[serde(default)]
    pub(crate) color: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateWorkspaceIconRequest {
    /// The icon's key, or none to go back to the default icon.
    #[serde(default)]
    pub(crate) icon: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateTextArtifactRequest {
    pub(crate) title: String,
    pub(crate) content: String,
    pub(crate) language: Option<String>,
    pub(crate) folder_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RenameFolderRequest {
    pub(crate) name: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MoveFolderRequest {
    /// The new parent; `null` for the top level.
    pub(crate) parent_id: Option<String>,
}

/// One action over many files at once.
#[derive(Debug, Deserialize)]
pub(crate) struct BulkArtifactsRequest {
    /// `move`, `delete` or `lifecycle`.
    pub(crate) action: String,
    pub(crate) artifact_ids: Vec<String>,
    /// For `move`: the folder, or `null` for the top level.
    #[serde(default)]
    pub(crate) folder_id: Option<String>,
    /// For `lifecycle`: `active`, `needs_review`, `verified` or `outdated`.
    #[serde(default)]
    pub(crate) status: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct BulkFailure {
    pub(crate) artifact_id: String,
    pub(crate) message: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct BulkArtifactsResponse {
    pub(crate) done: Vec<String>,
    pub(crate) failed: Vec<BulkFailure>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MoveArtifactRequest {
    pub(crate) folder_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateFolderRequest {
    pub(crate) name: String,
    pub(crate) parent_id: Option<String>,
}

/// An artifact summary plus the folder it sits in, for the shared client.
#[derive(Debug, Serialize)]
pub(crate) struct SharedArtifactSummary {
    #[serde(flatten)]
    pub(crate) summary: ArtifactSummary,
    pub(crate) folder_id: Option<String>,
    /// The repository source the file is synced from, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) repository_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateArtifactRequest {
    pub(crate) title: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SaveArtifactLifecycleRequest {
    pub(crate) status: String,
    pub(crate) owner_user_id: Option<String>,
    #[serde(default)]
    pub(crate) review_note: String,
    pub(crate) superseded_by_artifact_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SaveSearchRequest {
    pub(crate) name: String,
    pub(crate) query: String,
    #[serde(default)]
    pub(crate) artifact_types: Vec<ArtifactType>,
    #[serde(default)]
    pub(crate) languages: Vec<String>,
    #[serde(default)]
    pub(crate) source_ids: Vec<String>,
    pub(crate) result_limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ShareSavedSearchRequest {
    pub(crate) shared: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateChecklistItemRequest {
    pub(crate) body: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ToggleChecklistItemRequest {
    pub(crate) completed: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct QueryArtifactsRequest {
    #[serde(default)]
    pub(crate) query: String,
    #[serde(default)]
    pub(crate) artifact_types: Vec<repomemo_domain::ArtifactType>,
    #[serde(default)]
    pub(crate) languages: Vec<String>,
    #[serde(default)]
    pub(crate) source_ids: Vec<String>,
    pub(crate) indexed: Option<bool>,
    /// Only the files in this folder.
    #[serde(default)]
    pub(crate) folder_id: Option<String>,
    /// Only the files that are in no folder.
    #[serde(default)]
    pub(crate) top_level: bool,
    /// Leave out the files a repository tracks.
    #[serde(default)]
    pub(crate) ledger_only: bool,
    /// Kinds of file, any one of which a file may be (a view of the evidence section).
    #[serde(default)]
    pub(crate) kinds: Vec<KindRequest>,
}

/// One kind of file: all of these hold together.
#[derive(Debug, Default, Deserialize)]
pub(crate) struct KindRequest {
    #[serde(default)]
    pub(crate) artifact_types: Vec<repomemo_domain::ArtifactType>,
    #[serde(default)]
    pub(crate) not_artifact_types: Vec<repomemo_domain::ArtifactType>,
    #[serde(default)]
    pub(crate) extensions: Vec<String>,
    #[serde(default)]
    pub(crate) not_extensions: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SearchWorkspaceRequest {
    pub(crate) query: String,
    #[serde(default)]
    pub(crate) artifact_types: Vec<repomemo_domain::ArtifactType>,
    #[serde(default)]
    pub(crate) languages: Vec<String>,
    #[serde(default)]
    pub(crate) source_ids: Vec<String>,
    pub(crate) limit: Option<i64>,
    /// Lifecycle states to search; empty hides outdated and superseded evidence.
    #[serde(default)]
    pub(crate) lifecycle_statuses: Vec<String>,
    #[serde(default)]
    pub(crate) folder_id: Option<String>,
    /// `YYYY-MM-DD`.
    #[serde(default)]
    pub(crate) modified_after: Option<String>,
    #[serde(default)]
    pub(crate) modified_before: Option<String>,
}

/// A search over every workspace the caller can read (or those named).
#[derive(Debug, Deserialize)]
pub(crate) struct SearchAllRequest {
    pub(crate) query: String,
    /// Only these workspaces; empty searches them all. Ones the caller
    /// cannot read are left out without saying so.
    #[serde(default)]
    pub(crate) workspace_ids: Vec<String>,
    #[serde(default)]
    pub(crate) lifecycle_statuses: Vec<String>,
    /// Only these kinds of file, in every workspace searched (V0.3.12).
    #[serde(default)]
    pub(crate) artifact_types: Vec<ArtifactType>,
    /// Only files in these languages or formats, as the file lists name them.
    #[serde(default)]
    pub(crate) languages: Vec<String>,
    /// Only evidence updated on or after this day (`YYYY-MM-DD`) …
    #[serde(default)]
    pub(crate) modified_after: Option<String>,
    /// … and on or before this one.
    #[serde(default)]
    pub(crate) modified_before: Option<String>,
    /// Also look in memory cards (the default); ignored when a type or language is asked for.
    #[serde(default)]
    pub(crate) memory_cards: Option<bool>,
    /// Results in the merged list (20 by default, at most 50).
    pub(crate) limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub(crate) struct RetrievalSourceFacet {
    pub(crate) id: String,
    pub(crate) name: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct RetrievalFacetsResponse {
    pub(crate) artifact_types: Vec<ArtifactType>,
    pub(crate) languages: Vec<String>,
    pub(crate) sources: Vec<RetrievalSourceFacet>,
}

#[derive(Debug, Serialize)]
pub(crate) struct WorkspaceMetricBreakdown {
    pub(crate) label: String,
    pub(crate) value: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct UserProfileResponse {
    pub(crate) user: SharedUser,
    /// The address was confirmed (or came with an invitation).
    pub(crate) email_verified: bool,
    /// Unverified accounts are held back on this server (registration is open and mail is set up).
    pub(crate) verification_required: bool,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
    pub(crate) last_connected_at: Option<String>,
    pub(crate) workspace_count: i64,
    pub(crate) recent_activity_count: i64,
    pub(crate) activity_by_day: Vec<WorkspaceMetricBreakdown>,
}

#[derive(Debug, Serialize)]
pub(crate) struct WorkspaceActivityCalendarResponse {
    pub(crate) total_activity_count: i64,
    pub(crate) activity_by_day: Vec<WorkspaceMetricBreakdown>,
}

/// The figures of an organization's Overview tab, for the workspaces of it the caller can open.
#[derive(Debug, Serialize)]
pub(crate) struct OrganizationOverviewResponse {
    pub(crate) organization_id: String,
    pub(crate) member_count: i64,
    pub(crate) workspace_count: i64,
    pub(crate) artifact_count: i64,
    pub(crate) memory_card_count: i64,
    /// Tasks that are open or in progress.
    pub(crate) active_task_count: i64,
    /// Events a day for the last 30 days, oldest first, summed over the workspaces the caller administers (empty
    /// when the caller administers none: activity is for administrators, as the workspace's own calendar is).
    pub(crate) activity_by_day: Vec<WorkspaceMetricBreakdown>,
    pub(crate) workspaces: Vec<OrganizationOverviewWorkspace>,
}

#[derive(Debug, Serialize)]
pub(crate) struct OrganizationOverviewWorkspace {
    pub(crate) workspace_id: String,
    pub(crate) name: String,
    pub(crate) artifact_count: i64,
    pub(crate) memory_card_count: i64,
    pub(crate) active_task_count: i64,
    /// Events in the last 30 days; `null` when the caller does not administer the workspace.
    pub(crate) activity_count: Option<i64>,
}

/// The dashboard's figures, one entry per workspace the caller can open.
#[derive(Debug, Serialize)]
pub(crate) struct DashboardResponse {
    pub(crate) workspaces: Vec<DashboardWorkspace>,
}

#[derive(Debug, Serialize)]
pub(crate) struct DashboardWorkspace {
    pub(crate) workspace_id: String,
    pub(crate) artifact_count: i64,
    /// Tasks that are open or in progress.
    pub(crate) active_task_count: i64,
    pub(crate) memory_card_count: i64,
    /// The workspace's template, or `null` for `general` (V0.2.187).
    pub(crate) template: Option<DashboardTemplate>,
}

/// What names a workspace's template on its card.
#[derive(Debug, Serialize)]
pub(crate) struct DashboardTemplate {
    pub(crate) id: String,
    pub(crate) name: String,
    /// The one figure the template puts on the card, when it names one.
    pub(crate) headline: Option<DashboardHeadline>,
}

/// A pulse card and its figure for one workspace.
#[derive(Debug, Serialize)]
pub(crate) struct DashboardHeadline {
    pub(crate) card: String,
    pub(crate) value: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct WorkspaceMetricsResponse {
    pub(crate) workspace_id: String,
    pub(crate) generated_at: String,
    pub(crate) source_count: i64,
    pub(crate) member_count: i64,
    pub(crate) artifact_count: i64,
    pub(crate) indexed_artifact_count: i64,
    pub(crate) pending_artifact_count: i64,
    pub(crate) total_artifact_bytes: i64,
    pub(crate) indexed_artifact_bytes: i64,
    pub(crate) pending_artifact_bytes: i64,
    pub(crate) chunk_count: i64,
    /// Chunks with a vector from the current embedding model; `None` when the
    /// workspace has no embedding provider.
    pub(crate) embedded_chunk_count: Option<i64>,
    pub(crate) symbol_count: i64,
    pub(crate) memory_card_count: i64,
    pub(crate) open_task_count: i64,
    pub(crate) in_progress_task_count: i64,
    pub(crate) blocked_task_count: i64,
    pub(crate) completed_task_count: i64,
    pub(crate) overdue_task_count: i64,
    pub(crate) comment_count: i64,
    pub(crate) recent_activity_count: i64,
    pub(crate) artifacts_created_last_7_days: i64,
    pub(crate) artifacts_updated_last_7_days: i64,
    /// Linked git repositories, and how many finished their last sync (V0.3.18).
    pub(crate) repository_count: i64,
    pub(crate) repositories_in_sync: i64,
    /// Files whose lifecycle was reviewed in the last 90 days.
    pub(crate) reviewed_last_90_days: i64,
    /// Files marked *needs review*, and memory cards gone stale: what the Review queue holds for evidence and memory.
    pub(crate) needing_review_files: i64,
    pub(crate) needing_review_cards: i64,
    /// Files that repeat the exact content of another file.
    pub(crate) duplicate_file_count: i64,
    pub(crate) activity_actions: Vec<WorkspaceMetricBreakdown>,
    pub(crate) activity_by_day: Vec<WorkspaceMetricBreakdown>,
    pub(crate) member_roles: Vec<WorkspaceMetricBreakdown>,
    pub(crate) artifact_types: Vec<WorkspaceMetricBreakdown>,
    pub(crate) artifact_bytes_by_type: Vec<WorkspaceMetricBreakdown>,
    pub(crate) languages: Vec<WorkspaceMetricBreakdown>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AskWorkspaceRequest {
    pub(crate) question: String,
    pub(crate) limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SearchMemoryCardsRequest {
    pub(crate) query: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateMemoryCardBody {
    pub(crate) title: String,
    pub(crate) body_markdown: String,
    pub(crate) source: String,
    pub(crate) confidence: Option<f64>,
    #[serde(default)]
    pub(crate) citations: Vec<Citation>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TransferOwnershipRequest {
    pub(crate) user_id: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateMemoryCardBody {
    pub(crate) title: String,
    pub(crate) body_markdown: String,
    pub(crate) source: String,
    pub(crate) confidence: Option<f64>,
    /// Replaces the cited evidence when given.
    #[serde(default)]
    pub(crate) citations: Option<Vec<repomemo_domain::Citation>>,
    /// The revision the editor started from; a newer one makes the write fail with 409 (V0.6.4).
    #[serde(default)]
    pub(crate) expected_revision: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SaveAiProviderRequest {
    pub(crate) id: Option<String>,
    pub(crate) provider_type: String,
    pub(crate) name: String,
    pub(crate) base_url: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) api_key: Option<String>,
    pub(crate) enabled: bool,
    #[serde(default)]
    pub(crate) cloud_content_acknowledged: bool,
    /// `text` or `vision`; keeps the saved purpose when omitted.
    pub(crate) purpose: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpsertWorkspaceMemberRequest {
    pub(crate) email: String,
    pub(crate) role: WorkspaceRole,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SaveCollaborationTaskRequest {
    pub(crate) title: String,
    #[serde(default)]
    pub(crate) description: String,
    /// One of the four categories. Optional when `state_id` names a state (its category is used); when both are sent they must
    /// agree.
    #[serde(default)]
    pub(crate) status: String,
    /// A workflow state: a built-in one (open, in_progress, blocked, done) or one the workspace named.
    #[serde(default)]
    pub(crate) state_id: Option<String>,
    pub(crate) priority: String,
    pub(crate) assignee_user_id: Option<String>,
    pub(crate) artifact_id: Option<String>,
    /// The project to put the task in. Left out, a save keeps the project the task has; an empty value removes it.
    #[serde(default)]
    pub(crate) project_id: Option<String>,
    /// The milestone of the task's project it counts toward. Left out, a save keeps it (and drops it when the project changes);
    /// an empty value removes it.
    #[serde(default)]
    pub(crate) milestone_id: Option<String>,
    /// The task this one is a sub-task of. Left out, a save keeps it; an empty value removes it. It must be a task of the same
    /// workspace, not the task itself or anything below it.
    #[serde(default)]
    pub(crate) parent_task_id: Option<String>,
    /// The `version` of the task this save started from (updating only). A save from an older version is refused with the
    /// task as it is now and the attempted change; left out, the save is not checked and overwrites.
    #[serde(default)]
    pub(crate) expected_version: Option<i64>,
    /// Evidence and memory cards the new task cites, for example the decision that created it (creating only).
    #[serde(default)]
    pub(crate) references: Vec<TaskReferenceInput>,
    pub(crate) due_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TaskReferenceInput {
    /// `evidence` or `memory_card`.
    pub(crate) kind: String,
    pub(crate) target_id: String,
    #[serde(default)]
    pub(crate) note: String,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub(crate) struct ImportTasksRequest {
    /// The CSV text, at most about 2 MB and 1,000 data rows, with a header row.
    pub(crate) csv: String,
    /// Which column feeds which field: `{"title": "Summary", "assignee": "Owner email"}`. Fields left out are found by their own
    /// name or a common one (summary, state, assignee_email, due, tags, id...). Fields: external_id, title, description, status,
    /// priority, assignee, due_at, project, labels.
    #[serde(default)]
    pub(crate) mapping: std::collections::BTreeMap<String, String>,
    /// Preview only (the default): every row is explained and nothing is written. Send `false` to import.
    #[serde(default = "default_true")]
    pub(crate) dry_run: bool,
    /// Import the valid rows even when some are invalid; by default one invalid row refuses the whole import.
    #[serde(default)]
    pub(crate) skip_invalid: bool,
    /// Names the import; the same key and file is the same import. Left out, the file's own content names it.
    #[serde(default)]
    pub(crate) import_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SaveProjectUpdateRequest {
    /// `on_track`, `at_risk` or `blocked`: the author's choice, always sent.
    pub(crate) health: String,
    /// What changed.
    pub(crate) summary: String,
    /// Why this health.
    pub(crate) reason: String,
    /// Milestones of the project the update points at.
    #[serde(default)]
    pub(crate) milestone_ids: Vec<String>,
    /// Tasks the update points at, such as the blockers.
    #[serde(default)]
    pub(crate) task_ids: Vec<String>,
    #[serde(default)]
    pub(crate) evidence_ids: Vec<String>,
    #[serde(default)]
    pub(crate) memory_card_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SaveRecurringTaskRequest {
    pub(crate) title: String,
    #[serde(default)]
    pub(crate) description: String,
    /// low, medium (the default), high or urgent.
    #[serde(default)]
    pub(crate) priority: Option<String>,
    #[serde(default)]
    pub(crate) assignee_user_id: Option<String>,
    #[serde(default)]
    pub(crate) project_id: Option<String>,
    /// Each task is due this many days after its occurrence date (0 to 365); left out, it has no due date.
    #[serde(default)]
    pub(crate) due_after_days: Option<i64>,
    pub(crate) schedule: repomemo_domain::RecurringSchedule,
    /// `latest` (the default) or `skip`.
    #[serde(default)]
    pub(crate) missed_policy: Option<String>,
    /// Make it paused: nothing is made until it is resumed.
    #[serde(default)]
    pub(crate) paused: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct BulkTaskChanges {
    /// The new assignee for every task; an empty string unassigns. Left out, assignees are untouched.
    pub(crate) assignee_user_id: Option<String>,
    pub(crate) add_label_ids: Vec<String>,
    pub(crate) remove_label_ids: Vec<String>,
    /// The milestone for every task (each must be in the milestone's project); an empty string removes it.
    pub(crate) milestone_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BulkTasksRequest {
    /// Up to 100 task ids.
    pub(crate) task_ids: Vec<String>,
    pub(crate) changes: BulkTaskChanges,
    /// The `version` of each task as it was shown, by task id. A task that has moved on is refused, not overwritten.
    #[serde(default)]
    pub(crate) expected_versions: std::collections::BTreeMap<String, i64>,
    /// Report what would happen and write nothing.
    #[serde(default)]
    pub(crate) dry_run: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MoveTaskRequest {
    /// The column to move the task to: a built-in category name or the id of a named state.
    pub(crate) state_id: String,
    /// The task's `version` as the board showed it; a move from an older one is refused with the task as it is now.
    #[serde(default)]
    pub(crate) expected_version: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SaveTaskViewRequest {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) query: repomemo_domain::TaskQuery,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SetViewSharingRequest {
    pub(crate) shared: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SaveWorkflowStateRequest {
    pub(crate) name: String,
    /// `open`, `in_progress`, `blocked` or `done`.
    pub(crate) category: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SaveTaskLabelRequest {
    pub(crate) name: String,
    /// One of gray (the default), red, orange, yellow, green, teal, blue, purple, pink.
    #[serde(default)]
    pub(crate) color: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MergeTaskLabelRequest {
    pub(crate) into_label_id: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SetTaskLabelsRequest {
    pub(crate) label_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AddTaskDependencyRequest {
    /// The task in the path is blocked by this task. Name this or `blocked_task_id`, not both.
    #[serde(default)]
    pub(crate) blocker_task_id: Option<String>,
    /// The task in the path blocks this task.
    #[serde(default)]
    pub(crate) blocked_task_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SaveMilestoneRequest {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) description: String,
    /// A calendar date, YYYY-MM-DD.
    #[serde(default)]
    pub(crate) due_date: Option<String>,
    /// `all_tasks` (the default) or `manual`.
    #[serde(default)]
    pub(crate) completion_rule: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SaveProjectRequest {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) goal: String,
    /// `active` (the default), `on_hold` or `done`.
    #[serde(default)]
    pub(crate) status: Option<String>,
    #[serde(default)]
    pub(crate) owner_user_id: Option<String>,
    /// A calendar date, YYYY-MM-DD.
    #[serde(default)]
    pub(crate) target_date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AddProjectReferenceRequest {
    /// `memory_card` or `evidence`.
    pub(crate) kind: String,
    pub(crate) target_id: String,
    #[serde(default)]
    pub(crate) note: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SaveTaskCommentRequest {
    pub(crate) body: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SetTaskWatchRequest {
    /// `watching`, `muted` or `default`.
    pub(crate) mode: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SaveArtifactCommentRequest {
    pub(crate) body: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct TokenResponse {
    pub(crate) access_token: String,
    pub(crate) refresh_token: String,
    pub(crate) token_type: &'static str,
    pub(crate) expires_in: u64,
    pub(crate) user: SharedUser,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RefreshRequest {
    /// Empty for the web app, whose token travels in a cookie (see `web_session`).
    #[serde(default)]
    pub(crate) refresh_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct JwtClaims {
    pub(crate) sub: String,
    pub(crate) email: String,
    pub(crate) iss: String,
    pub(crate) iat: usize,
    pub(crate) exp: usize,
    /// The user's session version when the token was issued; tokens from
    /// before a password change or "sign out everywhere" no longer match.
    #[serde(default)]
    pub(crate) ver: i64,
}

#[derive(Debug, Clone)]
pub(crate) struct AuthenticatedSubject {
    pub(crate) user_id: String,
    /// When the access token expires (seconds since the epoch). Long-lived
    /// streams end then, like any other use of the token.
    pub(crate) expires_at: u64,
    pub(crate) session_version: i64,
    /// Administers the whole server: acts as an administrator in every
    /// organization and workspace, and may use the system API.
    pub(crate) is_system_admin: bool,
    /// May use the system API and pages, without the wildcard access.
    pub(crate) is_app_admin: bool,
    /// Set when the request came with a personal access token instead of a
    /// browser session.
    pub(crate) token: Option<TokenGrant>,
}

/// What a personal access token used for a request may do.
#[derive(Debug, Clone)]
pub(crate) struct TokenGrant {
    /// The token itself, so a stream it opened can end when it is revoked.
    pub(crate) token_id: String,
    pub(crate) read_only: bool,
    /// The `admin` scope: also the routes that grant access, delete, repoint, link or export.
    pub(crate) admin: bool,
    /// The `system` scope: also changes in the System area.
    pub(crate) system: bool,
    /// The only workspaces the token reaches; `None` when it is not limited to chosen ones.
    pub(crate) workspaces: Option<Vec<String>>,
}

impl AuthenticatedSubject {
    /// Whether a request made with this subject may touch the workspace at all: always for a session or an unlimited
    /// token, and only for the chosen workspaces of a limited one.
    pub(crate) fn token_reaches(&self, workspace_id: &str) -> bool {
        self.token
            .as_ref()
            .and_then(|grant| grant.workspaces.as_ref())
            .is_none_or(|allowed| allowed.iter().any(|id| id == workspace_id))
    }
}

/// Optional `limit`/`offset` paging for list endpoints. The file list cuts the page in SQL
/// (`StorageEngine::list_artifacts_page`) and caps a call without `limit`.
#[derive(Debug, Default, Deserialize)]
pub(crate) struct PageQuery {
    pub(crate) limit: Option<usize>,
    pub(crate) offset: Option<usize>,
    pub(crate) cursor: Option<String>,
}

pub(crate) const MAX_PAGE_SIZE: usize = 500;

pub(crate) const FILE_LINK_ISSUER: &str = "repomemo-file-link";

/// A file link is valid for one minute and for one client (review L1).
pub(crate) const FILE_LINK_TTL_SECONDS: u64 = 60;

/// Office apps open a link with several requests in a row (a probe, then the
/// download, sometimes in ranges). The same client may repeat a request this
/// long after its first; anyone else is refused once the link was used.
pub(crate) const FILE_LINK_GRACE_SECONDS: u64 = 30;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct FileLinkClaims {
    pub(crate) sub: String,
    pub(crate) iss: String,
    pub(crate) exp: u64,
    /// Identifies the link, so its use can be remembered.
    #[serde(default)]
    pub(crate) jti: String,
    /// Who asked for the link. A link outlives its person for no longer than
    /// its minute: it stops working when the account is deactivated.
    #[serde(default)]
    pub(crate) uid: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct FileLinkResponse {
    pub(crate) token: String,
    pub(crate) filename: String,
    pub(crate) expires_in_seconds: u64,
}

#[derive(Debug, Serialize)]
pub(crate) struct RenderStatusResponse {
    /// `ready`, `converting`, `failed`, `disabled` or `unsupported`.
    pub(crate) state: &'static str,
    pub(crate) message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct JobsQuery {
    pub(crate) kind: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<i64>,
    pub(crate) cursor: Option<String>,
}
