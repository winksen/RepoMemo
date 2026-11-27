use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

mod archive;
mod archive_import;
pub mod console;
mod error;

pub use archive::*;
pub use archive_import::*;
pub use error::{classify, DomainError, ErrorKind};

/// Temporary server-side identity shape. Production authentication replaces
/// the dummy session issuer, not this client-facing session contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedUser {
    pub id: String,
    pub display_name: String,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceRole {
    Owner,
    Admin,
    Member,
    Viewer,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OrganizationRole {
    Owner,
    Admin,
    Member,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationMember {
    pub user: SharedUser,
    pub role: OrganizationRole,
    pub joined_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceMembership {
    pub workspace_id: String,
    pub role: WorkspaceRole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceMember {
    pub user: SharedUser,
    pub role: WorkspaceRole,
    pub joined_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceActivityEvent {
    pub id: String,
    pub workspace_id: String,
    pub actor: Option<SharedUser>,
    pub action: String,
    pub subject_type: String,
    pub subject_id: Option<String>,
    pub summary: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollaborationTask {
    pub id: String,
    pub workspace_id: String,
    /// The task's number in its workspace (V0.7.3): unique, never reused and unchanged when the title changes.
    pub number: i64,
    /// The human-readable identifier built from it, such as `T-12`.
    pub key: String,
    /// Starts at 1 and grows by one with every save (V0.7.6); send it back as `expected_version` to save safely.
    pub version: i64,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: String,
    pub assignee: Option<SharedUser>,
    pub created_by: SharedUser,
    pub artifact_id: Option<String>,
    /// The project the task belongs to (V0.7.1); a task with none keeps working in the workspace list.
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    /// The milestone of the task's project it counts toward (V0.7.7).
    pub milestone_id: Option<String>,
    pub milestone_name: Option<String>,
    /// The task this one is a sub-task of, and that task's key (V0.7.8).
    pub parent_task_id: Option<String>,
    pub parent_key: Option<String>,
    /// How many sub-tasks it has, and how many of them are not done. Completing the task changes neither.
    pub child_count: i64,
    pub open_child_count: i64,
    /// How many tasks block this one and are not done (V0.7.9), and how many tasks this one blocks.
    pub open_blocker_count: i64,
    pub blocks_count: i64,
    /// The task's labels (V0.7.10), by name. They organize and filter; they never decide who can see the task.
    pub labels: Vec<TaskLabelTag>,
    /// The workflow state the task is in (V0.7.11): a built-in one (its id is the status) or one the workspace named. The
    /// `status` is always that state's category.
    pub state_id: String,
    pub state_name: String,
    pub due_at: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// What a refused save of a task said (V0.7.6): kept and returned with the task as it is now, so the change is not lost.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAttempt {
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: String,
    pub assignee_user_id: Option<String>,
    pub artifact_id: Option<String>,
    pub project_id: Option<String>,
    pub milestone_id: Option<String>,
    pub parent_task_id: Option<String>,
    pub due_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskConflictError {
    pub code: String,
    pub message: String,
}

/// The answer to a save of a task that started from an older version: the usual error, the task as it is now and the change
/// that was attempted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskConflict {
    pub error: TaskConflictError,
    pub current: CollaborationTask,
    pub attempted: TaskAttempt,
}

/// A task with what its own page shows (V0.7.3). `path` is the address of the task in the web app and keeps naming the same
/// task when its title changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDetail {
    pub task: CollaborationTask,
    pub checklist: Vec<TaskChecklistItem>,
    pub path: String,
    /// How the reader follows the task: `watching`, `muted` or `default` (V0.7.4).
    pub watch: String,
    pub comment_count: i64,
    /// The evidence and memory cards the task cites (V0.7.5), including its older single file link.
    pub references: Vec<TaskReference>,
    /// The task's direct sub-tasks (V0.7.8), open ones first.
    pub children: Vec<CollaborationTask>,
    /// What blocks this task and what it blocks (V0.7.9).
    pub dependencies: TaskDependencies,
}

/// The filters of the task list (V0.7.12), and what a saved view stores. Only filters: a view never holds task ids or
/// rights, so it is evaluated against the reader's current access every time. `assignee` is a user id, `me` or `none`;
/// `project_id` and `milestone_id` may be `none`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TaskQuery {
    pub project_id: Option<String>,
    pub milestone_id: Option<String>,
    pub assignee: Option<String>,
    pub priority: Vec<String>,
    pub status: Vec<String>,
    pub label_id: Option<String>,
    /// Calendar dates (YYYY-MM-DD): due on or before / on or after.
    pub due_before: Option<String>,
    pub due_after: Option<String>,
    /// Not done and past its own due date.
    pub overdue: bool,
    /// Only tasks in one workflow state (V0.7.13): a built-in category name or the id of a named state.
    pub state_id: Option<String>,
    /// Words in the title.
    pub q: Option<String>,
    /// `priority` (the usual order), `due`, `updated` or `number`.
    pub sort: Option<String>,
}

/// A page of tasks (V0.7.12). `total` counts every task that matches; `next_offset` is where the next page starts, none on the
/// last.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPage {
    pub tasks: Vec<CollaborationTask>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
    pub next_offset: Option<i64>,
}

/// When a recurring task is made (V0.7.16): a kind (`daily`, `weekly` on a weekday, `monthly` on a day 1 to 28), an interval
/// and a start date. Every occurrence is a calendar date in UTC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecurringSchedule {
    pub kind: String,
    /// Every this many days, weeks or months (1 to 60).
    pub interval: i64,
    /// Weekly: 0 (Monday) to 6 (Sunday).
    #[serde(default)]
    pub weekday: Option<i64>,
    /// Monthly: 1 to 28, so every month has the day.
    #[serde(default)]
    pub day_of_month: Option<i64>,
    /// YYYY-MM-DD; the first occurrence is on or after it.
    pub start_date: String,
}

/// A task that is made again on a schedule (V0.7.16). `missed_policy` says what happens when occurrences were due while nothing
/// ran: `latest` makes only the newest missed one, `skip` makes one only on its own day. Nothing is made while it is paused, and
/// a stopped one keeps every task it made.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecurringTask {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    pub description: String,
    pub priority: String,
    pub assignee: Option<SharedUser>,
    pub project_id: Option<String>,
    /// Each task made is due this many days after its occurrence date; none means no due date.
    pub due_after_days: Option<i64>,
    pub schedule: RecurringSchedule,
    pub missed_policy: String,
    /// `active`, `paused` or `stopped`.
    pub status: String,
    /// Every occurrence up to and including this date has been made or skipped.
    pub processed_through: String,
    /// The next occurrence dates from today, so the schedule can be checked before it runs.
    pub next_occurrences: Vec<String>,
    pub occurrences_made: i64,
    pub created_by: Option<SharedUser>,
    pub created_at: String,
    pub updated_at: String,
    pub stopped_at: Option<String>,
}

/// One occurrence date that was looked at: made (with its task) or skipped.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecurringOccurrence {
    pub occurrence_date: String,
    /// `created` or `skipped`.
    pub outcome: String,
    pub task_id: Option<String>,
    pub task_key: Option<String>,
    pub task_title: Option<String>,
    pub task_status: Option<String>,
}

/// A recurring task with the occurrences it has looked at, newest first.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecurringDetail {
    pub recurring: RecurringTask,
    pub occurrences: Vec<RecurringOccurrence>,
}

/// What happened to one task in a bulk change (V0.7.14). `outcome` is `ok`, `refused` or `unchanged`; a refused task is left
/// exactly as it was and `reason` says why. In a preview `ok` means "would be changed".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkItem {
    pub task_id: String,
    pub key: String,
    pub title: String,
    pub outcome: String,
    pub reason: Option<String>,
    /// The task after the change (applied changes only).
    pub task: Option<CollaborationTask>,
}

/// The result of a bulk change, preview or applied: an outcome for every task named.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkResult {
    pub dry_run: bool,
    pub requested: i64,
    pub succeeded: i64,
    pub refused: i64,
    pub unchanged: i64,
    pub items: Vec<BulkItem>,
}

/// One column of the board (V0.7.13): a workflow state, how many tasks match the board's filters in it, and the first of them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardColumn {
    pub state: WorkflowState,
    pub tasks: Vec<CollaborationTask>,
    pub total: i64,
    /// Where the column's next page starts in the task list (`state` and the same filters), none when all are shown.
    pub next_offset: Option<i64>,
}

/// The board: the task list's filtered work grouped by workflow state. The same query gives the same tasks in the list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskBoard {
    pub columns: Vec<BoardColumn>,
    pub total: i64,
}

/// A named set of filters (V0.7.12), private to its owner until shared with the workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskView {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub query: TaskQuery,
    pub shared: bool,
    pub owner: SharedUser,
    pub created_at: String,
    pub updated_at: String,
}

/// The four stable categories every workflow state maps to, and the built-in state of each.
pub const BUILTIN_STATE_CATEGORIES: [&str; 4] = ["open", "in_progress", "blocked", "done"];

/// The name of the built-in state of a category.
pub fn builtin_state_name(category: &str) -> &'static str {
    match category {
        "in_progress" => "In progress",
        "blocked" => "Blocked",
        "done" => "Done",
        _ => "Open",
    }
}

/// A state a task can be in (V0.7.11), mapped to one stable category. The four built-in states always exist (their `id` is
/// the category); a workspace can add named ones. Reports and integrations read `category`, never the name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowState {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    /// `open`, `in_progress`, `blocked` or `done`.
    pub category: String,
    pub builtin: bool,
    pub position: i64,
    /// How many tasks are in the state.
    pub task_count: i64,
}

/// The colours a label can have: a name, a background and the text colour on it. Every pair reaches a contrast of at least
/// 4.5 to 1 (a test checks it), and a label is always shown with its name, so colour is never the only signal.
pub const LABEL_COLORS: [(&str, &str, &str); 9] = [
    ("gray", "#E5E7EB", "#111827"),
    ("red", "#FECACA", "#7F1D1D"),
    ("orange", "#FED7AA", "#7C2D12"),
    ("yellow", "#FEF08A", "#713F12"),
    ("green", "#BBF7D0", "#14532D"),
    ("teal", "#99F6E4", "#134E4A"),
    ("blue", "#BFDBFE", "#1E3A8A"),
    ("purple", "#E9D5FF", "#581C87"),
    ("pink", "#FBCFE8", "#831843"),
];

/// The names in `LABEL_COLORS`.
pub const LABEL_COLOR_NAMES: [&str; 9] = [
    "gray", "red", "orange", "yellow", "green", "teal", "blue", "purple", "pink",
];

/// The background and text colour of a label colour; an unknown name gets gray.
pub fn label_colors(name: &str) -> (&'static str, &'static str) {
    LABEL_COLORS
        .iter()
        .find(|(known, _, _)| *known == name)
        .map_or((LABEL_COLORS[0].1, LABEL_COLORS[0].2), |(_, background, text)| {
            (*background, *text)
        })
}

/// A label as it appears on a task (V0.7.10).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskLabelTag {
    pub id: String,
    pub name: String,
    pub color: String,
    pub background_color: String,
    pub text_color: String,
}

/// A workspace's label with how many tasks carry it (V0.7.10).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskLabel {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub color: String,
    pub background_color: String,
    pub text_color: String,
    pub task_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// A task named briefly, as the other end of a dependency.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSummary {
    pub id: String,
    pub key: String,
    pub title: String,
    pub status: String,
}

/// "`blocker` blocks `blocked`" (V0.7.9). It informs and never acts: closing the blocker completes nothing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDependency {
    pub id: String,
    pub blocker: TaskSummary,
    pub blocked: TaskSummary,
    pub created_by: Option<SharedUser>,
    pub created_at: String,
}

/// What blocks a task, what it blocks, and in words why it is blocked (none when nothing blocks it).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDependencies {
    pub blocked_by: Vec<TaskDependency>,
    pub blocks: Vec<TaskDependency>,
    pub explanation: Option<String>,
}

/// One memory card or piece of evidence a task cites (V0.7.5). Only the id is stored; the title is read from the target, and
/// `available` is false when the target is gone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskReference {
    pub id: String,
    pub task_id: String,
    /// `evidence` or `memory_card`.
    pub kind: String,
    pub target_id: String,
    pub title: Option<String>,
    pub available: bool,
    pub note: String,
    pub added_by: Option<SharedUser>,
    /// When the evidence is an issue or pull request mirrored from a tracker, what the tracker says about it (V0.8.8). It is the
    /// tracker's state, never the task's.
    #[serde(default)]
    pub remote: Option<RemoteItemInfo>,
    pub created_at: String,
}

/// What a tracker reports about a mirrored issue or pull request. The tracker owns it: nothing here is edited in RepoMemo,
/// and a task that follows the item has its own status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteItemInfo {
    pub provider: String,
    pub source_id: String,
    pub source_name: String,
    /// `open`, `closed` or `merged`: the tracker's word at `synced_at`.
    pub state: String,
    pub url: Option<String>,
    /// The tracker's own update time of the item.
    pub synced_at: String,
    /// Always `remote`: who owns `state`.
    pub owner: String,
}

/// A local task that cites a mirrored item (V0.8.8).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FollowUpTask {
    pub task_id: String,
    pub key: String,
    pub title: String,
    /// The task's own status, which a person sets here; it never changes the tracker's state.
    pub status: String,
    pub assignee_name: Option<String>,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub created_at: String,
}

/// A local project that cites a mirrored item in its brief.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FollowUpProject {
    pub project_id: String,
    pub name: String,
    pub status: String,
}

/// Both directions of a mirrored item's links: the tracker's state, and the local work that follows it up.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemFollowUps {
    pub artifact_id: String,
    pub remote: RemoteItemInfo,
    pub tasks: Vec<FollowUpTask>,
    pub projects: Vec<FollowUpProject>,
}

/// A comment on a task (V0.7.4). It stays with its task and goes with it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskComment {
    pub id: String,
    pub task_id: String,
    pub workspace_id: String,
    pub author: Option<SharedUser>,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
}

/// How a person follows a task: `watching`, `muted` or `default`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskWatch {
    pub mode: String,
}

/// A named piece of work inside one workspace (V0.7.1). It grants no access: whoever can read the workspace can read it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub goal: String,
    /// `active`, `on_hold`, `done` or `archived`.
    pub status: String,
    pub owner: Option<SharedUser>,
    pub created_by: Option<SharedUser>,
    /// A calendar date (YYYY-MM-DD) stating the intent; no task follows it automatically.
    pub target_date: Option<String>,
    /// Every task of the project, and how many of them are not done.
    pub task_count: i64,
    pub open_task_count: i64,
    /// The health in the newest update that was not retracted (V0.7.17), chosen by the person who published it; none until then.
    pub health: Option<String>,
    pub health_updated_at: Option<String>,
    pub archived_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// A named, dated group of a project's tasks (V0.7.7). The counts say what they count: `task_count` is the denominator,
/// `explanation` says it in words, and a date never moves a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestone {
    pub id: String,
    pub project_id: String,
    pub workspace_id: String,
    pub name: String,
    pub description: String,
    /// A calendar date (YYYY-MM-DD, UTC) or none.
    pub due_date: Option<String>,
    /// `all_tasks` (complete when it has tasks and all are done) or `manual` (a person marks it complete).
    pub completion_rule: String,
    pub completed: bool,
    pub completed_at: Option<String>,
    /// Every task counted toward the milestone, those done, those not done, and those not done past their own due date.
    pub task_count: i64,
    pub done_count: i64,
    pub remaining_count: i64,
    pub overdue_task_count: i64,
    /// Not complete and past its date.
    pub overdue: bool,
    pub explanation: String,
    pub created_at: String,
    pub updated_at: String,
}

/// What a CSV import found in one row (V0.7.18). `outcome` is `valid` (would be made), `invalid`, `duplicate` (already
/// imported), or `created` once it was made. `row` is the row as a spreadsheet numbers it (the header is row 1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvRowReport {
    pub row: i64,
    pub outcome: String,
    pub title: String,
    pub problems: Vec<String>,
    pub task_key: Option<String>,
}

/// The report of a CSV import, preview or committed (V0.7.18): how the columns were mapped, every row explained, the assignees
/// that are not members and what was made.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvImportReport {
    pub dry_run: bool,
    pub committed: bool,
    pub rows: i64,
    pub valid: i64,
    pub invalid: i64,
    pub duplicates: i64,
    pub created: i64,
    /// Field to the column of the file that feeds it.
    pub mapping: BTreeMap<String, String>,
    pub unknown_assignees: Vec<String>,
    pub items: Vec<CsvRowReport>,
    pub note: String,
}

/// A milestone as an update recorded it (V0.7.17).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MilestoneBrief {
    pub id: String,
    pub name: String,
    pub due_date: Option<String>,
    pub completed: bool,
    pub overdue: bool,
    pub done_count: i64,
    pub task_count: i64,
}

/// A project's counts and milestones at one moment: what an update records as it was, and what the draft offers now.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectSnapshot {
    pub task_count: i64,
    pub done_count: i64,
    pub open_count: i64,
    pub blocked_count: i64,
    pub overdue_count: i64,
    pub milestones: Vec<MilestoneBrief>,
}

/// What a project update points at (V0.7.17): a milestone or task of the project, or evidence or a memory card. `label` is the
/// name when it was published; `title` is the name now and `available` is false when the target is gone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectUpdateLink {
    /// `milestone`, `task`, `evidence` or `memory_card`.
    pub kind: String,
    pub target_id: String,
    pub label: String,
    pub title: Option<String>,
    pub available: bool,
}

/// A person's statement of a project's health (V0.7.17). The health, the summary and the reason are the author's; the snapshot is
/// the project as it was when they published, and `changes` says what differs from the update before. Never edited, only
/// retracted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectUpdate {
    pub id: String,
    pub project_id: String,
    pub workspace_id: String,
    pub author: Option<SharedUser>,
    /// `on_track`, `at_risk` or `blocked`.
    pub health: String,
    /// What changed.
    pub summary: String,
    /// Why the author chose this health.
    pub reason: String,
    pub snapshot: ProjectSnapshot,
    pub changes: Vec<String>,
    pub links: Vec<ProjectUpdateLink>,
    pub created_at: String,
    pub retracted_at: Option<String>,
    pub retracted_by: Option<SharedUser>,
}

/// What the product offers someone about to write an update. It carries no health: that is the author's to choose.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectUpdateDraft {
    pub snapshot: ProjectSnapshot,
    pub open_blockers: Vec<TaskSummary>,
    pub previous: Option<ProjectUpdate>,
    pub note: String,
}

/// One memory card or piece of evidence a project's brief cites (V0.7.2). Only the id is stored; the title is read from the
/// target when the brief is read, and `available` is false when the target is gone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectReference {
    pub id: String,
    pub project_id: String,
    /// `memory_card` or `evidence`.
    pub kind: String,
    pub target_id: String,
    pub title: Option<String>,
    pub available: bool,
    pub note: String,
    pub added_by: Option<SharedUser>,
    pub created_at: String,
}

/// A project with the decisions and evidence it cites.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectBrief {
    pub project: Project,
    pub references: Vec<ProjectReference>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedSearch {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub query: String,
    pub artifact_types: Vec<ArtifactType>,
    pub languages: Vec<String>,
    pub source_ids: Vec<String>,
    pub result_limit: i64,
    pub shared: bool,
    pub created_by: SharedUser,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskChecklistItem {
    pub id: String,
    pub task_id: String,
    pub workspace_id: String,
    pub body: String,
    pub position: i64,
    pub completed_at: Option<String>,
    pub completed_by: Option<SharedUser>,
    pub created_by: SharedUser,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactComment {
    pub id: String,
    pub workspace_id: String,
    pub artifact_id: String,
    pub author: SharedUser,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactLifecycle {
    pub artifact_id: String,
    pub workspace_id: String,
    pub status: String,
    pub owner: Option<SharedUser>,
    pub review_note: String,
    pub reviewed_by: Option<SharedUser>,
    pub reviewed_at: Option<String>,
    pub superseded_by_artifact_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactLifecycleEvent {
    pub id: String,
    pub artifact_id: String,
    pub actor: Option<SharedUser>,
    pub action: String,
    pub detail: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedNotification {
    pub id: String,
    pub workspace_id: Option<String>,
    pub notification_type: String,
    pub title: String,
    pub body: String,
    pub href: String,
    pub read_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedSession {
    pub user: SharedUser,
    pub authentication: String,
    pub memberships: Vec<WorkspaceMembership>,
    /// The user administers the whole server: every organization and
    /// workspace, and the system settings.
    #[serde(default)]
    pub is_system_admin: bool,
    /// The user may use the System pages without being an administrator of
    /// every organization and workspace.
    #[serde(default)]
    pub is_app_admin: bool,
    /// The user administers CAIRN (models, limits and settings), by the grant of a system administrator or by being one.
    #[serde(default)]
    pub is_cairn_admin: bool,
    /// The server lets this person create an organization (see the system setting `organization_creators`).
    #[serde(default)]
    pub can_create_organizations: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Organization {
    pub id: String,
    pub name: String,
    pub role: OrganizationRole,
    pub created_at: String,
    pub updated_at: String,
    /// A short description (up to 500 characters), shown on the organization's card and page.
    #[serde(default)]
    pub description: Option<String>,
    /// The key of the icon that stands for the organization; the client owns the set of icons.
    #[serde(default)]
    pub icon: Option<String>,
    /// Listed only because the user belongs to some of its workspaces: they
    /// are not a member of the organization, `role` is then `member` for
    /// display, and nothing flows to its other workspaces.
    #[serde(default)]
    pub guest: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedWorkspace {
    pub workspace: Workspace,
    pub organization_id: String,
    pub role: WorkspaceRole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
    pub settings: Value,
}

/// The template a workspace was given, as kept in its settings under `template`: the template's id, the
/// version it was chosen at and the few adjustments an administrator saved on top of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceTemplateRef {
    pub id: String,
    pub version: u32,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub overrides: Value,
}

/// The most the saved adjustments of a template may take, serialized.
pub const MAX_TEMPLATE_OVERRIDES_BYTES: usize = 16 * 1024;

impl WorkspaceTemplateRef {
    /// Reads the `template` setting of a workspace, or of an archive that came from another server. A value that is
    /// not shaped like a reference (an id that is not a short slug, a version of 0, adjustments that are not an
    /// object or are too large) is `None`, so the workspace is simply on `general`.
    pub fn from_setting(value: &Value) -> Option<Self> {
        let reference: Self = serde_json::from_value(value.clone()).ok()?;
        let slug = !reference.id.is_empty()
            && reference.id.len() <= 40
            && reference
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        let overrides_fit = match &reference.overrides {
            Value::Null => true,
            Value::Object(_) => reference.overrides.to_string().len() <= MAX_TEMPLATE_OVERRIDES_BYTES,
            _ => false,
        };
        (slug && reference.version >= 1 && overrides_fit).then_some(reference)
    }
}

/// A template as the picker lists it: what identifies it, none of its blocks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateSummary {
    pub id: String,
    pub version: u32,
    pub name: String,
    pub summary: String,
    pub icon: String,
    /// True for a template that came from the operator's folder rather than from the server's own files.
    pub custom: bool,
}

/// The template a workspace uses and where it came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceTemplateView {
    /// The template in force: the workspace's choice, or `general`.
    pub template: TemplateSummary,
    /// `default` (no choice saved), `workspace` (its own choice) or `fallback` (the choice cannot be used on this server).
    pub source: String,
    /// Why `general` is in force when `source` is `fallback`.
    pub reason: Option<String>,
    /// The id the workspace chose; not the one in force when `source` is `fallback`.
    pub chosen_id: Option<String>,
    /// The adjustments saved on top of the template (none can be saved yet).
    pub overrides: Value,
    /// The tabs and what is kept under *More*.
    pub sections: TemplateSections,
    /// The names the template gives to sections and kinds of evidence.
    pub labels: TemplateLabels,
    /// The pulse cards of the Overview.
    pub overview: TemplateOverview,
    /// How the evidence ledger opens and which file types it lists first.
    pub evidence: TemplateEvidence,
    /// The assistant's order and suggestions.
    pub assistant: TemplateAssistant,
    /// What the Retrieval page suggests.
    pub retrieval: TemplateRetrieval,
    /// How search weighs what it finds.
    pub ranking: TemplateRanking,
    /// The health checks that run.
    pub health: TemplateHealth,
    /// What the review queue lists first.
    pub review: TemplateReview,
    /// What the knowledge map puts first.
    pub map: TemplateMap,
    /// The include and exclude patterns the repository link form is filled in with.
    pub repositories: TemplateRepositories,
}

/// The tabs of a workspace and the sections kept under *More*, as its template arranges them. Every section is in
/// one of the two lists; for `general` that is the usual twelve in the usual order and none under *More*.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateSections {
    pub primary: Vec<String>,
    pub more: Vec<String>,
    /// The section a workspace opens on when its address names none.
    pub landing: String,
}

/// The names a template gives to sections and kinds of evidence. Only the ones it renames are here; every other
/// keeps its usual name. Display text only: ids and addresses do not change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateLabels {
    pub sections: BTreeMap<String, String>,
    pub artifact_types: BTreeMap<String, String>,
}

/// The pulse cards the Overview shows, in order, as the template chooses; for `general` the usual nine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateOverview {
    pub cards: Vec<String>,
    /// The figure the dashboard shows on the workspace's card, if the template names one.
    pub headline: Option<String>,
}

/// How the evidence ledger opens (`grid` or `list`, which a person's own choice overrides) and the file types its
/// filter lists first. For `general`: `grid` and no preference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateEvidence {
    pub default_view: String,
    pub types_first: Vec<String>,
}

/// What the assistant shows first and the questions it suggests. Both empty for `general`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateAssistant {
    /// Capabilities listed before the others, in this order; the rest follow in the server's order.
    pub order: Vec<String>,
    pub suggestions: Vec<String>,
    /// Phrases the keyword router recognises after its own rules.
    pub phrases: Vec<TemplatePhrase>,
    /// One line about what the workspace is for, given to Ask as context.
    pub focus: Option<String>,
}

/// A phrase the assistant's router recognises and the job it maps onto.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplatePhrase {
    pub phrase: String,
    pub capability: String,
}

/// The kinds of evidence the Retrieval page suggests as chips. Empty for `general`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateRetrieval {
    pub type_chips: Vec<String>,
}

/// A factor for the passages of one kind of evidence, one kind of document or one folder: exactly one of the three
/// selectors is given.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateBoost {
    pub artifact_type: Option<String>,
    pub document_kind: Option<String>,
    pub path: Option<String>,
    pub factor: f64,
}

/// A preference for recent evidence: a file changed just now is favoured by `max_factor`, and the nudge halves every
/// `half_life_days`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateRecency {
    pub half_life_days: u32,
    pub max_factor: f64,
}

/// How search weighs what it finds. For `general`: verified evidence ranks a tenth higher and nothing else.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateRanking {
    /// The factor for passages of verified files.
    pub verified: f64,
    pub boosts: Vec<TemplateBoost>,
    /// Absent when the template does not prefer recent evidence.
    pub recency: Option<TemplateRecency>,
}

/// The health checks that run in a workspace, in the order their findings are listed. For `general`: every check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateHealth {
    pub detectors: Vec<String>,
    /// For `stale_evidence`: after how many days without a change or a verification evidence is stale.
    pub stale_after_days: Option<u32>,
}

/// The groups the review queue lists, in order, each with the oldest first. Empty for `general`: the items are mixed,
/// oldest first, with the health findings after what has waited.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateReview {
    pub order: Vec<String>,
}

/// What the knowledge map puts first: the file types whose coverage rows come first and the kinds of link listed first
/// and drawn on top. Both empty for `general`. Presentation only: nothing is left out of the map.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateMap {
    pub coverage_first: Vec<String>,
    pub relations_first: Vec<String>,
}

/// The patterns the repository link form is filled in with: an offer the person may keep, edit or clear. Both empty for
/// `general`, and for a repository that is already linked nothing changes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateRepositories {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

/// A template as a person looks at it before choosing it: what it arranges and names, without any workspace's choice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateDetail {
    pub template: TemplateSummary,
    pub sections: TemplateSections,
    pub labels: TemplateLabels,
    pub overview: TemplateOverview,
    pub evidence: TemplateEvidence,
    pub assistant: TemplateAssistant,
    pub retrieval: TemplateRetrieval,
    pub ranking: TemplateRanking,
    pub health: TemplateHealth,
    pub review: TemplateReview,
    pub map: TemplateMap,
    pub repositories: TemplateRepositories,
}

/// Chooses a workspace's template; `null` returns the workspace to the default. `overrides` are the adjustments
/// saved on it (`landing`, `more`, `suggestions`): left out they stay when the same template is chosen again, an
/// empty object clears them, and choosing another template drops them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateWorkspaceTemplateRequest {
    pub template_id: Option<String>,
    #[serde(default)]
    pub overrides: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceOverview {
    pub workspace_id: String,
    pub source_count: i64,
    pub artifact_count: i64,
    pub chunk_count: i64,
    pub symbol_count: i64,
    pub memory_card_count: i64,
}

/// Server-authoritative permissions for the currently authenticated member.
/// Clients may use this shape to decide which controls to present, but every
/// mutation remains enforced by the shared API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceCapabilities {
    pub role: WorkspaceRole,
    pub can_read: bool,
    pub can_write_content: bool,
    pub can_delete_content: bool,
    pub can_manage_members: bool,
    pub can_assign_admin: bool,
    pub can_manage_workspace: bool,
    pub can_generate_ai_overview: bool,
    pub can_create_tasks: bool,
    pub can_comment: bool,
    pub can_moderate_comments: bool,
    /// Whether the member may inspect the stored index chunks of an artifact.
    pub can_inspect_index: bool,
    /// Whether the member may use AI features (answers, overviews, summaries,
    /// the assistant's AI actions), per the workspace's AI policy.
    #[serde(default)]
    pub can_use_ai: bool,
}

/// A citation-backed workspace briefing generated through an enabled provider.
/// When no provider is configured, the API returns this shape with no summary
/// and an actionable warning instead of fabricating an AI result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceAiOverview {
    pub provider_configured: bool,
    pub provider_name: Option<String>,
    pub summary_markdown: Option<String>,
    pub citations: Vec<Citation>,
    pub warnings: Vec<String>,
}

/// Safe-to-return provider metadata for the shared web client. Credentials
/// remain server-side and are never serialized in this response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedAiProviderSettings {
    pub id: String,
    pub provider_type: String,
    pub name: String,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub enabled: bool,
    /// `text` (answers, summaries, embeddings) or `vision` (image to text).
    pub purpose: String,
    /// How the model takes tools: `native`, `json` or `none`. Filled by *Test connection*, or set by
    /// an administrator; empty until one of them has.
    #[serde(default)]
    pub tool_mode: Option<String>,
    /// The model's context window in tokens, from the same two sources.
    #[serde(default)]
    pub context_tokens: Option<u32>,
    /// How CAIRN answers in this workspace: `jobs` (one job per message, the default) or `agent` (a loop of tool calls, only
    /// for a model that takes tools; V0.9.40).
    #[serde(default)]
    pub cairn_mode: Option<String>,
}

/// A rendered-for-reading view of an uploaded business document.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DocumentPreview {
    /// Extracted text (Word, legacy PowerPoint, OneNote). `approximate` is set
    /// when the text was recovered heuristically and may be incomplete.
    Text {
        text: String,
        truncated: bool,
        approximate: bool,
    },
    Sheets {
        sheets: Vec<SheetPreview>,
        total_sheets: usize,
    },
    Slides {
        slides: Vec<SlidePreview>,
        total_slides: usize,
    },
    Email {
        subject: Option<String>,
        from: Option<String>,
        to: Vec<String>,
        cc: Vec<String>,
        date: Option<String>,
        body: String,
        truncated: bool,
        attachments: Vec<EmailAttachmentInfo>,
    },
    /// The browser renders the PDF itself; only the page count is needed.
    Pdf {
        page_count: Option<usize>,
    },
    Unavailable {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetPreview {
    pub name: String,
    pub rows: Vec<Vec<String>>,
    pub total_rows: usize,
    pub total_columns: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlidePreview {
    pub number: usize,
    pub title: Option<String>,
    pub text: Vec<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailAttachmentInfo {
    pub name: String,
    pub size_bytes: usize,
}

/// Folders may be nested this many levels deep (a top-level folder is level 1).
pub const MAX_FOLDER_DEPTH: usize = 5;

/// A request that a question or a result be improved (V0.6.25), as the person who asked and the workspace's administrators see
/// it. The query text is kept until `expires_at`, then removed with the request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchFeedback {
    pub id: String,
    /// `no_answer` or `unhelpful_result`.
    pub kind: String,
    pub query: String,
    /// For an unhelpful result: the file that did not help.
    pub artifact_id: Option<String>,
    pub artifact_title: Option<String>,
    pub note: String,
    /// `open`, `resolved` or `declined`.
    pub status: String,
    pub asked_by: Option<SharedUser>,
    pub created_at: String,
    /// What answered it: `evidence` or `memory_card`, with its id and title.
    pub resolution_kind: Option<String>,
    pub resolution_id: Option<String>,
    pub resolution_title: Option<String>,
    pub resolved_by: Option<SharedUser>,
    pub resolved_note: String,
    pub resolved_at: Option<String>,
    /// When the request and its query text are removed: 180 days after it was made while open, 30 days after it was closed.
    pub expires_at: String,
}

/// What moving a folder would do, before it is done (V0.6.24): what is inside, what stays attached, where it would end up, and
/// the reason a move is refused when it is.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderMovePreview {
    pub folder_name: String,
    /// Where the folder is now and where it would be, as folder names from the top level.
    pub from_path: String,
    pub to_path: String,
    /// Folders inside it, at any depth.
    pub subfolders: i64,
    /// Files in it and in those folders.
    pub files: i64,
    /// Memory cards that cite one of those files, and how many citations they hold. A move changes none of them.
    pub citing_cards: i64,
    pub citations: i64,
    /// Comments on those files; they stay on their files.
    pub comments: i64,
    /// True when the folder is already where it was asked to go.
    pub unchanged: bool,
    /// False when the move would be refused (a folder into itself or into one inside it, nesting too deep, a name already
    /// used there).
    pub allowed: bool,
    pub refusal: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: String,
    pub workspace_id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub created_at: String,
    /// Files sitting directly in this folder (not in its subfolders); filled by the folder list.
    #[serde(default)]
    pub file_count: i64,
    /// A workspace administrator closed this folder, or one above it, to CAIRN (V0.9.93); filled by the folder list.
    #[serde(default)]
    pub closed_to_cairn: bool,
}

/// Why an artifact could not be indexed, after every retry was used.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactIndexFailure {
    pub artifact_id: String,
    pub message: String,
    pub attempts: i64,
    pub failed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    Upload,
    Folder,
    GitRepo,
    Manual,
    Connector,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub id: String,
    pub workspace_id: String,
    pub source_type: SourceType,
    pub name: String,
    pub root_uri: Option<String>,
    pub last_indexed_at: Option<String>,
    pub status: String,
    pub metadata: Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactType {
    File,
    MarkdownDoc,
    CodeFile,
    Image,
    Issue,
    Pr,
    Decision,
    Incident,
    Runbook,
    ApiSpec,
    Note,
    /// A whole git repository: its generated overview, kept current by syncs.
    Repository,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: String,
    pub workspace_id: String,
    pub source_id: String,
    pub artifact_type: ArtifactType,
    pub title: String,
    pub path: String,
    pub content_hash: String,
    pub mime_type: Option<String>,
    pub language: Option<String>,
    pub size_bytes: i64,
    pub created_at: String,
    pub updated_at: String,
    pub indexed_at: Option<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactSummary {
    pub id: String,
    pub workspace_id: String,
    pub source_id: String,
    pub source_name: String,
    pub artifact_type: ArtifactType,
    pub title: String,
    pub path: String,
    pub content_hash: String,
    pub mime_type: Option<String>,
    pub language: Option<String>,
    pub size_bytes: i64,
    pub created_at: String,
    pub updated_at: String,
    pub indexed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactDetail {
    pub summary: ArtifactSummary,
    pub index_version: i64,
    pub source_synced_at: Option<String>,
    pub metadata: Value,
    pub content_preview: Option<String>,
    pub content_truncated: bool,
    pub chunks: Vec<Chunk>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRequest {
    pub workspace_id: String,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportSkippedItem {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportReport {
    pub workspace_id: String,
    pub scanned: usize,
    pub imported: usize,
    pub duplicates: usize,
    pub skipped: usize,
    pub failed: usize,
    pub imported_artifacts: Vec<ArtifactSummary>,
    pub skipped_items: Vec<ImportSkippedItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id: String,
    pub artifact_id: String,
    pub workspace_id: String,
    pub chunk_index: i64,
    pub text: String,
    pub token_count: Option<i64>,
    pub start_line: Option<i64>,
    pub end_line: Option<i64>,
    pub heading_path: Option<String>,
    pub content_hash: String,
    pub embedding_status: String,
    pub metadata: Value,
}

/// Job kind of a repository sync in the jobs table.
pub const REPO_SYNC_JOB_KIND: &str = "repo_sync";

/// What a repository source indexes. Stored in the source's metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoSettings {
    /// Local branch to index; `None` follows whatever HEAD points at.
    #[serde(default)]
    pub branch: Option<String>,
    /// When not empty, only paths matching one of these patterns are indexed.
    #[serde(default)]
    pub include: Vec<String>,
    /// Paths matching these patterns are skipped, on top of the defaults.
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoCommit {
    pub sha: String,
    pub summary: String,
    pub author_name: String,
    pub committed_at: String,
    /// The branch the commit was read from, when HEAD was on one.
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoSkipCount {
    pub reason: String,
    pub count: usize,
    /// A few of the skipped paths, to explain the reason.
    pub examples: Vec<String>,
}

/// What one sync changed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoSyncReport {
    pub commit_sha: String,
    /// Regular files in the commit's tree, before any rule is applied.
    pub files_in_tree: usize,
    pub added: usize,
    pub updated: usize,
    pub renamed: usize,
    pub removed: usize,
    pub restored: usize,
    pub unchanged: usize,
    pub skipped: usize,
    pub skipped_by_reason: Vec<RepoSkipCount>,
    pub indexed: usize,
    pub index_failed: usize,
    pub cancelled: bool,
    /// The sync stopped after resolving the commit: it, the settings and the
    /// indexer were the same as at the last complete sync, and nothing was left
    /// to index.
    #[serde(default)]
    pub nothing_changed: bool,
}

/// A git repository connected to a workspace. Its files are kept in step with
/// the repository by syncs: one artifact per tracked path, updated in place.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoSource {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    /// Repository root on the server's disk, or the address of a remote
    /// repository.
    pub root_path: String,
    /// The repository is fetched from a host over `https`: `root_path` is its
    /// address, and the server keeps a partial clone of it.
    #[serde(default)]
    pub remote: bool,
    /// A remote repository has a token stored (never shown).
    #[serde(default)]
    pub has_credential: bool,
    /// Push webhooks are set up for this repository.
    #[serde(default)]
    pub has_webhook: bool,
    /// When a correctly signed webhook call last arrived.
    #[serde(default)]
    pub webhook_last_received_at: Option<String>,
    pub settings: RepoSettings,
    /// `pending` (never synced), `syncing`, `ready` or `error`.
    pub status: String,
    pub last_synced_commit: Option<RepoCommit>,
    pub last_synced_at: Option<String>,
    pub last_error: Option<String>,
    pub last_report: Option<RepoSyncReport>,
    /// Files currently tracked and indexed from the repository.
    pub file_count: i64,
    pub indexed_file_count: i64,
    pub active_job: Option<IndexingJobStatus>,
    /// The evidence item that stands for the whole repository.
    pub overview_artifact_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoLanguageShare {
    pub language: String,
    pub files: usize,
    pub bytes: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoFolderShare {
    /// A top-level folder, or "" for files at the repository root.
    pub path: String,
    pub files: usize,
}

/// A file worth opening first: a README, a manifest or an entry point.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoKeyFile {
    pub path: String,
    pub artifact_id: String,
    /// `readme`, `manifest`, `entry_point` or `docs`.
    pub role: String,
}

/// One framework, tool or language detected in a repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoTechnology {
    pub name: String,
    /// `frontend`, `backend`, `meta-framework`, `desktop`, `mobile`, `database`,
    /// `testing`, `tooling`, `language`, ...
    pub category: String,
}

/// What kind of project a repository is, read from its manifests and layout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoStack {
    /// `frontend`, `backend`, `web_app`, `desktop_app`, `mobile_app`, `cli` or `project`.
    pub kind: String,
    /// For example "React single-page app (TypeScript)" or "Laravel PHP backend".
    pub summary: String,
    pub technologies: Vec<RepoTechnology>,
}

/// What a repository holds, computed from its indexed files at each sync.
/// Deterministic: no AI is involved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoOverview {
    pub commit: RepoCommit,
    pub generated_at: String,
    pub file_count: usize,
    pub total_bytes: i64,
    pub languages: Vec<RepoLanguageShare>,
    pub folders: Vec<RepoFolderShare>,
    pub key_files: Vec<RepoKeyFile>,
    /// The opening of the root README, when there is one.
    pub readme_excerpt: Option<String>,
    pub recent_commits: Vec<RepoCommit>,
    /// Absent on overviews written before stack detection existed.
    #[serde(default)]
    pub stack: Option<RepoStack>,
}

/// An AI summary of a repository, kept until someone regenerates it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoSummary {
    pub summary_markdown: String,
    pub citations: Vec<Citation>,
    pub warnings: Vec<String>,
    pub provider_name: String,
    /// The commit the summary describes.
    pub commit_sha: String,
    pub generated_at: String,
}

/// Everything the repository page shows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoDetail {
    pub repository: RepoSource,
    pub overview: Option<RepoOverview>,
    pub summary: Option<RepoSummary>,
}

/// What the server found at a repository link, before it is connected.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoAccessCheck {
    /// The repository root the link resolves to.
    pub root_path: String,
    /// The link is a remote repository: nothing was fetched, so the commit
    /// has only its id and the file counts are zero until the first sync.
    #[serde(default)]
    pub remote: bool,
    /// The name the repository gets unless another is chosen.
    pub name: String,
    /// The commit a sync would read.
    pub commit: RepoCommit,
    /// Files tracked in that commit.
    pub tracked_files: usize,
    /// Of those, files the rules would index (before checking content).
    pub indexable_files: usize,
    /// The workspace already has this repository.
    pub already_connected: bool,
}

/// One end of a mention: an issue or pull request that names a file, or a
/// file that an item names.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactMention {
    pub artifact_id: String,
    pub title: String,
    pub artifact_type: ArtifactType,
    pub path: String,
    /// `file`, `symbol`, `commit` or `item` (another issue or pull request): what the text named.
    pub kind: String,
    /// The path, symbol or commit id as named.
    pub label: String,
}

/// What names an artifact, and what it names (roadmap F17d).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ArtifactMentions {
    pub mentioned_in: Vec<ArtifactMention>,
    pub mentions: Vec<ArtifactMention>,
}

/// One call a source's webhook received (roadmap F17g, V0.3.22).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebhookDelivery {
    pub id: String,
    pub received_at: String,
    /// `github`, `gitlab`, `other`, or `resend` for a call made from the page.
    pub sender: String,
    pub event: String,
    /// The sender's own id for the call; empty when it gives none.
    pub delivery_id: String,
    /// `queued`, `absorbed` (a sync was already running), `ignored`, `pong` or `failed`.
    pub outcome: String,
    /// Why it was ignored or failed.
    pub reason: String,
    /// When it was sent again from the page, if it was.
    pub resent_at: Option<String>,
}

/// A public snapshot (V0.8.16): the record, never the text. `slug` is the opaque public address.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Publication {
    pub id: String,
    pub workspace_id: String,
    pub slug: String,
    pub title: String,
    pub snapshot_hash: String,
    /// `published`, `withdrawn` or `stale`.
    pub status: String,
    pub version: i64,
    pub source_artifact_id: Option<String>,
    pub published_at: String,
    pub updated_at: String,
}

/// A chat channel an administrator approved to receive digests (V0.8.9). The address is a secret and is never part of this.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatDestination {
    pub id: String,
    pub workspace_id: String,
    /// `slack`.
    pub provider: String,
    pub name: String,
    /// Where it is (the host only).
    pub host: String,
    /// `review` and `projects`.
    pub digests: Vec<String>,
    pub enabled: bool,
    pub created_at: String,
    pub last_sent_at: Option<String>,
    pub last_error: Option<String>,
}

/// A destination that receives signed events from a workspace (V0.8.4). The signing secret is never part of it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundEndpoint {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub url: String,
    /// The events it receives, such as `connector.quarantined`.
    pub events: Vec<String>,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
    pub last_delivered_at: Option<String>,
}

/// One event for one endpoint. `event_id` is the same on every attempt and for every endpoint that gets the event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundDelivery {
    pub id: String,
    pub event_id: String,
    pub event_type: String,
    /// `pending`, `delivered`, `failed` (attempts used up) or `cancelled` (the endpoint was turned off or is no longer approved).
    pub status: String,
    pub attempts: i64,
    pub next_attempt_at: Option<String>,
    pub last_status_code: Option<u16>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub delivered_at: Option<String>,
}

/// The job kind of a connector's sync.
pub const CONNECTOR_SYNC_JOB_KIND: &str = "connector_sync";

/// A tracker connected to a workspace: its issues (and later pull requests)
/// are kept as evidence (roadmap F17).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorSource {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    /// `github`.
    pub provider: String,
    /// What it reads, such as `https://github.com/acme/app`.
    pub location: String,
    /// `pending`, `syncing`, `ready` or `error`.
    pub status: String,
    pub last_synced_at: Option<String>,
    pub last_error: Option<String>,
    pub last_report: Option<ConnectorSyncReport>,
    /// Items kept, by the state the tracker reports (a merged pull request
    /// counts as closed).
    pub open_items: i64,
    pub closed_items: i64,
    /// A token is stored (never shown).
    pub has_credential: bool,
    pub active_job: Option<IndexingJobStatus>,
    /// Webhooks are set up for this connector.
    #[serde(default)]
    pub has_webhook: bool,
    /// When a correctly signed webhook call last arrived.
    #[serde(default)]
    pub webhook_last_received_at: Option<String>,
    /// Why the connector stopped syncing by itself: the tracker's rate limit
    /// (until `paused_until`) or a token it refused (until it is replaced).
    #[serde(default)]
    pub paused_reason: Option<String>,
    #[serde(default)]
    pub paused_until: Option<String>,
    /// The administrator who approved sharing the copy with the whole
    /// workspace, and when (V0.8.1). Empty for a connector made earlier.
    #[serde(default)]
    pub sharing_approved_by: Option<String>,
    #[serde(default)]
    pub sharing_approved_at: Option<String>,
    /// Set while the connector's evidence is held back because access to the
    /// source was lost or cannot be established (V0.8.2).
    #[serde(default)]
    pub quarantine: Option<ConnectorQuarantine>,
    /// What an operator needs to know about the sync (V0.8.3).
    #[serde(default)]
    pub health: ConnectorHealth,
    pub created_at: String,
}

/// How well a connector keeps its copy current, in words an operator can act on.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConnectorHealth {
    /// `complete`, `partial`, `stale`, `paused`, `quarantined`, `failed`, `syncing` or `pending`.
    pub state: String,
    /// One sentence saying what the state means now.
    pub summary: String,
    /// The last sync that finished without an error.
    pub last_success_at: Option<String>,
    /// The last sync that finished with nothing left to read and nothing that failed to index.
    pub last_complete_at: Option<String>,
    /// A connector with no successful sync for this long reads `stale`.
    pub stale_after_seconds: i64,
    /// Items kept from the tracker.
    pub items_kept: i64,
    /// Items the last sync read.
    pub items_seen_last_sync: usize,
    /// The last sync stopped at its limit and the next one carries on.
    pub backlog: bool,
    /// Items stored but not indexed by the last sync.
    pub index_failed: usize,
    /// Where the next sync starts: the tracker's update time of the last item stored.
    pub resume_from: Option<String>,
    /// What is not copied or is cut, so a gap is not mistaken for a failure.
    pub exclusions: Vec<String>,
    /// What went wrong and what to do about it.
    pub problem: Option<ConnectorProblem>,
}

/// An error a person can recover from, with the step that does it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorProblem {
    /// `token`, `rate_limit`, `unreachable`, `not_found`, `forbidden` or `other`.
    pub kind: String,
    pub message: String,
    pub action: String,
}

/// Why a connector's evidence is held back. The detail is the tracker's own
/// words and never holds a secret.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorQuarantine {
    /// `access_revoked` (the token is refused), `access_denied` (the tracker
    /// forbids the partition) or `not_found` (the tracker does not know it).
    pub reason: String,
    pub detail: String,
    /// How many copied items are held back.
    pub items: i64,
    pub since: String,
}

/// What one connector sync did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorSyncReport {
    pub items_seen: usize,
    /// Of those, pull requests.
    #[serde(default)]
    pub pull_requests_seen: usize,
    pub added: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub indexed: usize,
    pub index_failed: usize,
    /// The sync stopped at its page limit; the next one carries on.
    pub more_to_fetch: bool,
    /// Items not copied because the tracker restricts who may see them (V0.8.5).
    #[serde(default)]
    pub restricted_skipped: usize,
    /// Items taken out of the evidence because the tracker no longer lists them for this account: deleted, moved away,
    /// or restricted since (V0.8.6).
    #[serde(default)]
    pub removed: usize,
}

/// What the tracker answered for a link, before it is connected.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorAccessCheck {
    pub provider: String,
    pub name: String,
    pub location: String,
    pub open_issues: i64,
    pub private: bool,
    pub already_connected: bool,
    /// What importing would copy and to whom, shown before anything is read.
    pub preview: ConnectorPreview,
}

/// What an import would do, so an administrator approves it knowingly
/// (V0.8.1). It holds no token and no secret.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorPreview {
    /// The part of the tracker that is copied: a repository or a project.
    pub partition: String,
    /// What is copied into the workspace.
    pub copies: Vec<String>,
    /// What is never copied.
    pub not_copied: Vec<String>,
    /// Who will be able to read the copy.
    pub audience: String,
    pub audience_members: i64,
    /// `uniform` (the partition has one audience) or `refused`.
    pub sharing: String,
    /// Why the partition cannot be connected, when it is refused.
    pub refusal: Option<String>,
}

/// A repository file's text as it was at a commit, for a citation that names
/// one. It is not the file's current content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoFileAtCommit {
    pub artifact_id: String,
    pub path: String,
    /// The full commit id.
    pub commit: String,
    pub summary: String,
    pub author_name: String,
    pub committed_at: String,
    /// The commit the workspace's current copy is from, when it differs.
    pub current_commit: Option<String>,
    pub text: String,
}

/// A file of a repository source that is currently in its tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoFile {
    pub path: String,
    pub artifact_id: String,
    pub artifact_type: ArtifactType,
    pub language: Option<String>,
    pub size_bytes: i64,
    pub indexed: bool,
    pub index_failure: Option<String>,
    pub commit_sha: String,
    /// Who last changed the file, from the repository's history.
    #[serde(default)]
    pub last_author: Option<String>,
    /// When that change was authored.
    #[serde(default)]
    pub last_changed_at: Option<String>,
    /// The commit of that change.
    #[serde(default)]
    pub last_change_commit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingJobStatus {
    pub id: String,
    pub workspace_id: String,
    pub source_id: Option<String>,
    pub kind: String,
    pub status: String,
    pub stage: String,
    pub progress_current: i64,
    pub progress_total: Option<i64>,
    pub error_message: Option<String>,
    pub cancel_requested: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Function,
    Class,
    Method,
    Interface,
    Enum,
    Route,
    Endpoint,
    Config,
    Test,
    /// A named value fixed at compile time: a Go `const`, a PHP constant (V0.3.17).
    Constant,
    /// A package-level variable (Go `var`).
    Variable,
    /// A field of a class (Java).
    Field,
    /// A property of a class (C#, PHP).
    Property,
    /// A named type that is not a struct or an interface: Go `type ID int` and `type A = B`.
    TypeAlias,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub id: String,
    pub artifact_id: String,
    pub workspace_id: String,
    pub kind: SymbolKind,
    pub name: String,
    pub signature: Option<String>,
    pub start_line: Option<i64>,
    pub end_line: Option<i64>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolSearchResult {
    pub symbol: Symbol,
    pub title: String,
    pub path: String,
    pub language: Option<String>,
    pub source_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCard {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    pub body_markdown: String,
    pub source: String,
    pub confidence: Option<f64>,
    pub created_at: String,
    pub updated_at: String,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCardSummary {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    pub body_excerpt: String,
    pub source: String,
    pub evidence_count: i64,
    pub created_at: String,
    pub updated_at: String,
    /// Evidence the card cites has changed, disappeared or been retired
    /// since the card was last written (see [`MemoryReviewReason`]).
    pub needs_review: bool,
}

/// Why a memory card needs another look.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryReviewReason {
    /// `missing`: the cited passage or file no longer exists. `retired`: the
    /// cited file is marked outdated or superseded. `changed`: a cited file
    /// was updated after the card was last written.
    pub kind: String,
    pub artifact_id: Option<String>,
    pub title: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEvidence {
    pub link_id: String,
    pub target_id: String,
    pub target_type: String,
    pub artifact_id: Option<String>,
    pub chunk_id: Option<String>,
    pub title: Option<String>,
    pub path: Option<String>,
    pub source_id: Option<String>,
    pub heading_path: Option<String>,
    pub review_status: Option<String>,
    pub start_line: Option<i64>,
    pub end_line: Option<i64>,
    pub exists: bool,
    /// For evidence from a file of a git repository: the commit the card cited it at (V0.3.14), when it said.
    #[serde(default)]
    pub commit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCardDetail {
    pub card: MemoryCard,
    pub evidence: Vec<MemoryEvidence>,
    /// Empty when the card is up to date with its evidence.
    pub review: Vec<MemoryReviewReason>,
    /// The newest revision number (V0.6.4); an editor sends it back as `expected_revision`.
    #[serde(default)]
    pub revision: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMemoryCardRequest {
    pub workspace_id: String,
    pub title: String,
    pub body_markdown: String,
    pub source: String,
    pub confidence: Option<f64>,
    pub citations: Vec<Citation>,
    /// Who is saving. Written with the revision in the same transaction (V0.6.11); `None` leaves it unattributed.
    #[serde(default)]
    pub author_user_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMemoryCardRequest {
    pub card_id: String,
    pub title: String,
    pub body_markdown: String,
    pub source: String,
    pub confidence: Option<f64>,
    /// Replaces the cited evidence when given; `None` keeps it.
    #[serde(default)]
    pub citations: Option<Vec<Citation>>,
    /// The revision the writer started from (V0.6.4). When given and no longer the newest, the write is refused so
    /// it cannot silently overwrite a later save; `None` writes without the check.
    #[serde(default)]
    pub expected_revision: Option<i64>,
    /// Who is saving. Written with the revision in the same transaction (V0.6.11); `None` leaves it unattributed.
    #[serde(default)]
    pub author_user_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub data_dir: String,
    pub ai_enabled: bool,
    pub active_provider: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchRequest {
    pub workspace_id: String,
    pub query: String,
    pub artifact_types: Vec<ArtifactType>,
    pub languages: Vec<String>,
    pub source_ids: Vec<String>,
    pub limit: Option<i64>,
    /// Lifecycle states to search (`active`, `needs_review`, `verified`,
    /// `outdated`, `superseded`). Empty means every state but `outdated` and
    /// `superseded`, so retired evidence is not served as current.
    #[serde(default)]
    pub lifecycle_statuses: Vec<String>,
    /// Only evidence in this folder or below it.
    #[serde(default)]
    pub folder_id: Option<String>,
    /// Only evidence updated on or after this day (`YYYY-MM-DD`) …
    #[serde(default)]
    pub modified_after: Option<String>,
    /// … and on or before this one.
    #[serde(default)]
    pub modified_before: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub artifact_id: String,
    pub chunk_id: String,
    #[serde(default)]
    pub status: Option<String>,
    pub title: String,
    pub path: String,
    pub artifact_type: ArtifactType,
    pub language: Option<String>,
    pub snippet: String,
    pub start_line: Option<i64>,
    pub end_line: Option<i64>,
    pub score: f64,
    pub source_name: String,
    /// `keyword`, `semantic`, or `keyword_and_semantic` for this passage.
    pub match_kind: String,
    /// For a file of a git repository: the commit its text is from.
    #[serde(default)]
    pub commit: Option<String>,
    /// For a file of a git repository: who last changed it.
    #[serde(default)]
    pub last_author: Option<String>,
    /// For a file of a git repository: when that change was authored.
    #[serde(default)]
    pub last_changed_at: Option<String>,
}

/// An owner that `CODEOWNERS` names for a file, and the member of the workspace it stands for (V0.3.15). An owner
/// written as an address stands for the member whose account has that address; `@handles` and `@org/teams` belong to
/// the code host, not to RepoMemo, so they stay hints with no member.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeOwnerMatch {
    /// The owner as `CODEOWNERS` writes it.
    pub owner: String,
    pub member: Option<SharedUser>,
}

/// A search result from one of several workspaces searched together.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossSearchResult {
    pub workspace_id: String,
    pub workspace_name: String,
    pub result: SearchResult,
}

/// How one workspace took part in a search across workspaces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossSearchWorkspace {
    pub workspace_id: String,
    pub name: String,
    /// Results this workspace contributed before the merged list was cut.
    pub result_count: i64,
    /// True when the query was also matched by meaning, which sends it to
    /// the workspace's embedding provider.
    pub by_meaning: bool,
    /// Why this workspace was searched by keywords only, or not at all.
    pub note: Option<String>,
}

/// A memory card found by a search across workspaces (V0.3.12).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossMemoryCard {
    pub workspace_id: String,
    pub workspace_name: String,
    pub card: MemoryCardSummary,
}

/// A search over every workspace the caller can read.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossSearchResponse {
    /// Merged by score, best first.
    pub results: Vec<CrossSearchResult>,
    /// Memory cards whose title or text match, workspace by workspace in the order the workspaces were
    /// searched; none when a file type or language was asked for, since cards are not files.
    #[serde(default)]
    pub memory_cards: Vec<CrossMemoryCard>,
    pub workspaces: Vec<CrossSearchWorkspace>,
    /// Workspaces left out because the search is capped.
    pub not_searched: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSettings {
    pub id: String,
    pub workspace_id: Option<String>,
    pub provider_type: String,
    pub name: String,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub embedding_model: Option<String>,
    pub enabled: bool,
    pub metadata: Value,
    #[serde(skip_serializing, default)]
    pub api_key: Option<String>,
}

impl ProviderSettings {
    /// What this provider is used for: `text` (answers and summaries),
    /// `vision` (image descriptions) or `embedding` (semantic search vectors).
    /// Providers saved before the split have no purpose and are text providers.
    pub fn purpose(&self) -> &str {
        match self.metadata.get("purpose").and_then(Value::as_str) {
            Some("vision") => "vision",
            Some("embedding") => "embedding",
            _ => "text",
        }
    }

    /// The model whose vectors this provider produces; stored with each
    /// embedding so vectors from different models are never compared.
    pub fn embedding_model_name(&self) -> &str {
        self.embedding_model
            .as_deref()
            .or(self.model.as_deref())
            .unwrap_or("unknown")
    }

    pub fn has_explicit_purpose(&self) -> bool {
        self.metadata.get("purpose").is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderTestResult {
    pub provider_id: String,
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Citation {
    pub artifact_id: String,
    pub chunk_id: Option<String>,
    pub title: String,
    pub path: String,
    pub start_line: Option<i64>,
    pub end_line: Option<i64>,
    pub confidence: Option<f64>,
    /// For a file of a git repository: the commit the cited text is from.
    #[serde(default)]
    pub commit: Option<String>,
    /// For a file of a git repository: who last changed it (V0.3.16).
    #[serde(default)]
    pub last_author: Option<String>,
    /// For a file of a git repository: when that change was authored.
    #[serde(default)]
    pub last_changed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryResult {
    pub summary_markdown: String,
    pub citations: Vec<Citation>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AskRequest {
    pub workspace_id: String,
    pub question: String,
    pub provider_id: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AskAnswer {
    pub answer_markdown: String,
    pub citations: Vec<Citation>,
    pub retrieved_context: Vec<SearchResult>,
    pub confidence: Option<f64>,
    pub warnings: Vec<String>,
}

/// Everything the knowledge map page draws: how far evidence has moved
/// through indexing and embedding, and how files relate to each other.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeMap {
    pub pipeline: KnowledgePipeline,
    pub coverage: Vec<KnowledgeCoverage>,
    pub nodes: Vec<KnowledgeNode>,
    pub edges: Vec<KnowledgeEdge>,
    /// Files left out of the graph to keep it readable (the smallest ones).
    pub hidden_file_count: i64,
    /// False when there is no embedding provider or nothing is embedded
    /// yet, so the graph can only show memory links.
    pub similarity_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgePipeline {
    pub file_count: i64,
    pub indexed_count: i64,
    pub pending_count: i64,
    pub failed_count: i64,
    pub passage_count: i64,
    /// `None` when the workspace has no embedding provider.
    pub embedded_count: Option<i64>,
    pub embedding_model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeCoverage {
    /// A readable kind such as "PDF", "Markdown" or "Code", from the
    /// artifact type and, for uploaded documents, the file extension.
    pub label: String,
    pub file_count: i64,
    pub indexed_count: i64,
    pub pending_count: i64,
    pub failed_count: i64,
    pub passage_count: i64,
    pub embedded_count: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeNodeKind {
    File,
    Memory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexState {
    Indexed,
    Pending,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeNode {
    /// The artifact id for files, the memory card id for memory.
    pub id: String,
    pub kind: KnowledgeNodeKind,
    pub title: String,
    pub path: Option<String>,
    pub artifact_type: Option<ArtifactType>,
    pub state: Option<IndexState>,
    pub passage_count: i64,
    pub embedded_count: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeEdgeKind {
    /// Two files whose content means similar things (cosine of their
    /// average passage embeddings).
    Similar,
    /// A memory card citing a file or one of its passages.
    Cites,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEdge {
    pub source: String,
    pub target: String,
    pub kind: KnowledgeEdgeKind,
    /// Cosine similarity for `similar`; 1 for `cites`.
    pub weight: f64,
}

/// A deterministic check the Workspace Health page runs. None of them call
/// an AI provider; each finding is backed by stored facts the page shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthDetector {
    /// An earlier upload of the same file is still active next to a newer one.
    OlderVersionActive,
    /// Files with byte-identical content under different names.
    DuplicateContent,
    /// A document mentions a code symbol that the latest version of its
    /// file no longer defines.
    RemovedSymbolMentioned,
    /// A document mentions a file marked outdated or superseded.
    OutdatedEvidenceReferenced,
    /// A file that could not be indexed, so search never sees it.
    IndexFailed,
    /// A memory card cites evidence that changed, disappeared or was retired.
    StaleMemoryCitation,
    /// A file with no close neighbour by meaning and no memory citing it.
    /// A weak signal: shown as information, never as a removal.
    Unconnected,
    /// A file nobody has changed or verified for as long as the workspace's template allows. Only run where the
    /// template asks for it (V0.2.182).
    StaleEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthSeverity {
    Warning,
    Info,
}

/// What an administrator can do with a finding. Every action is reversible
/// from the evidence lifecycle or the task list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthAction {
    /// Mark the finding's other files superseded by the one kept.
    Supersede,
    NeedsReview,
    MarkOutdated,
    CreateTask,
    Dismiss,
}

impl HealthAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Supersede => "supersede",
            Self::NeedsReview => "needs_review",
            Self::MarkOutdated => "mark_outdated",
            Self::CreateTask => "create_task",
            Self::Dismiss => "dismiss",
        }
    }
}

impl HealthDetector {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OlderVersionActive => "older_version_active",
            Self::DuplicateContent => "duplicate_content",
            Self::RemovedSymbolMentioned => "removed_symbol_mentioned",
            Self::OutdatedEvidenceReferenced => "outdated_evidence_referenced",
            Self::IndexFailed => "index_failed",
            Self::StaleMemoryCitation => "stale_memory_citation",
            Self::Unconnected => "unconnected",
            Self::StaleEvidence => "stale_evidence",
        }
    }

    /// The check with this id (`as_str`), if there is one.
    pub fn from_id(id: &str) -> Option<HealthDetector> {
        Self::ALL.into_iter().find(|detector| detector.as_str() == id)
    }

    /// Every check there is, in the order they are listed.
    pub const ALL: [HealthDetector; 8] = [
        Self::OlderVersionActive,
        Self::DuplicateContent,
        Self::RemovedSymbolMentioned,
        Self::OutdatedEvidenceReferenced,
        Self::StaleMemoryCitation,
        Self::IndexFailed,
        Self::Unconnected,
        Self::StaleEvidence,
    ];

    /// The checks a workspace runs when its template does not choose: the seven Health has always had. The check for
    /// evidence nobody has touched (V0.2.182) is only run where a template asks for it.
    pub const DEFAULT: [HealthDetector; 7] = [
        Self::OlderVersionActive,
        Self::DuplicateContent,
        Self::RemovedSymbolMentioned,
        Self::OutdatedEvidenceReferenced,
        Self::StaleMemoryCitation,
        Self::IndexFailed,
        Self::Unconnected,
    ];
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthFile {
    pub artifact_id: String,
    pub title: String,
    pub path: String,
    pub created_at: String,
}

/// A passage that shows why a finding was raised.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthEvidence {
    pub artifact_id: String,
    pub title: String,
    pub start_line: Option<i64>,
    pub end_line: Option<i64>,
    pub excerpt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthFinding {
    /// Stable for as long as the underlying facts are unchanged, so a
    /// handled finding stays hidden until something new happens.
    pub fingerprint: String,
    pub detector: HealthDetector,
    pub severity: HealthSeverity,
    pub title: String,
    pub detail: String,
    /// The files an action applies to.
    pub files: Vec<HealthFile>,
    pub evidence: Vec<HealthEvidence>,
    /// For `supersede`: the file kept by default.
    pub keep_artifact_id: Option<String>,
    pub actions: Vec<HealthAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthDetectorStats {
    pub detector: HealthDetector,
    pub open_count: i64,
    /// Findings an administrator acted on (anything but dismiss).
    pub acted_count: i64,
    pub dismissed_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceHealth {
    pub findings: Vec<HealthFinding>,
    pub detectors: Vec<HealthDetectorStats>,
    /// Files still in use (not outdated or superseded) that were checked.
    pub checked_file_count: i64,
    /// False when there is no embedding provider, so `unconnected` is skipped.
    pub similarity_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthActionRequest {
    pub fingerprint: String,
    pub action: HealthAction,
    /// For `supersede`: which of the finding's files to keep.
    pub keep_artifact_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthActionResult {
    pub action: HealthAction,
    pub updated_artifact_ids: Vec<String>,
    pub task_id: Option<String>,
}

/// One version of an uploaded file. Uploading a new version keeps the one it
/// replaces, so earlier text stays readable and can be compared.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactVersion {
    /// 1 is the first upload; the current version has the highest number.
    pub version: i64,
    pub content_hash: String,
    pub size_bytes: i64,
    /// When this content became the file's current content.
    pub created_at: String,
    /// Who uploaded it; the first upload's author is not recorded.
    pub uploaded_by: Option<String>,
    /// When a newer version replaced it; none for the current one.
    pub replaced_at: Option<String>,
    pub is_current: bool,
}

/// The text of one version, as search would read it: the file's own text, or
/// what is extracted from a business document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactVersionText {
    pub version: i64,
    /// None when the version has no text to show (an image, or a binary file).
    pub text: Option<String>,
    pub truncated: bool,
    /// Why there is no text.
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffKind {
    Same,
    Added,
    Removed,
    /// Unchanged lines left out of the comparison; `text` says how many.
    Gap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionDiffLine {
    pub kind: DiffKind,
    pub text: String,
    /// The line's number in the older version; none for an added line.
    pub old_line: Option<i64>,
    /// The line's number in the newer version; none for a removed line.
    pub new_line: Option<i64>,
}

/// Two versions of a file compared line by line, with a few unchanged lines
/// around each change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionComparison {
    pub from_version: i64,
    pub to_version: i64,
    pub lines: Vec<VersionDiffLine>,
    pub added: i64,
    pub removed: i64,
    /// The files were long: only the first lines of each were compared.
    pub truncated: bool,
    /// Why nothing is shown (a version has no text, or the two are the same).
    pub note: Option<String>,
}

/// What is waiting in a review queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewItemKind {
    /// A file someone marked as needing review.
    Evidence,
    /// A memory card whose cited evidence changed, disappeared or was retired.
    MemoryCard,
    /// An open Workspace Health finding.
    HealthFinding,
    /// A finding of CAIRN's weekly memory upkeep (V0.9.56).
    Upkeep,
}

/// Who can resolve a review item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewResolver {
    /// Anyone who can change the workspace's content.
    Writer,
    /// Workspace owners and administrators.
    Admin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewItem {
    pub kind: ReviewItemKind,
    /// The file's or memory card's id, or the finding's fingerprint.
    pub id: String,
    pub title: String,
    /// Why it is waiting, in a sentence.
    pub reason: String,
    /// Where it lives: a file's path, a card's evidence, a finding's files.
    pub detail: String,
    /// The file to open: the file itself, or the first file of a finding.
    pub artifact_id: Option<String>,
    /// When it started waiting; open findings have no date.
    pub since: Option<String>,
    pub resolver: ReviewResolver,
    /// When a snoozed item comes back to the person's list (V0.3.10); `None` for an item that is waiting now.
    #[serde(default)]
    pub snoozed_until: Option<String>,
    /// Why it is in the queue (V0.6.6): `marked` (set to needs review), `changed_evidence` (a cited source changed),
    /// `overdue` (its review date passed) or `finding` (a health finding).
    #[serde(default)]
    pub cause: String,
    /// The accountable reviewer, when one is assigned.
    #[serde(default)]
    pub reviewer: Option<SharedUser>,
    /// The reviewer's account is gone, deactivated or no longer in the workspace.
    #[serde(default)]
    pub reviewer_departed: bool,
    #[serde(default)]
    pub next_review_at: Option<String>,
    /// The review date has passed, whatever else put the item here.
    #[serde(default)]
    pub overdue: bool,
}

/// Everything in a workspace that needs a person's attention, in one list:
/// evidence marked *needs review*, memory cards that went stale and open
/// health findings. An item leaves the list when it is resolved where it is
/// worked on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewQueue {
    /// The oldest first; items without a date last. Items the person snoozed are not here (V0.3.10).
    pub items: Vec<ReviewItem>,
    pub evidence_count: i64,
    pub memory_card_count: i64,
    pub finding_count: i64,
    /// What the person snoozed and that is still waiting, soonest to come back first. Each has `snoozed_until`.
    #[serde(default)]
    pub snoozed: Vec<ReviewItem>,
    /// Items waiting only because, or also because, their review date passed (V0.6.6).
    #[serde(default)]
    pub overdue_count: i64,
}

/// One step of the first-value checklist. A step is done when the thing it asks for exists in the workspace, not when a
/// button was clicked (V0.6.1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingStep {
    /// `add_evidence`, `searchable`, `cited_memory` or `invite`.
    pub id: String,
    pub title: String,
    pub detail: String,
    pub done: bool,
}

/// The workspace checklist for one person: derived steps plus whether this person hid it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceOnboarding {
    pub steps: Vec<OnboardingStep>,
    pub done_count: i64,
    pub total_count: i64,
    /// This person dismissed the checklist; the steps are still returned so it can be shown again.
    pub dismissed: bool,
    /// The name of the workspace's template, which words the steps.
    pub template_name: String,
}

/// A file waiting for the person's review, in the workspace it is in (V0.6.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyWorkReview {
    /// `evidence` or `memory_card` (V0.6.6).
    #[serde(default)]
    pub kind: String,
    pub workspace_id: String,
    pub workspace_name: String,
    pub artifact_id: String,
    pub title: String,
    pub reason: String,
    pub since: String,
}

/// One decision about a revision of a memory card (V0.6.8).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryApprovalDecision {
    pub revision: i64,
    /// `approved` or `withdrawn`.
    pub decision: String,
    pub user: Option<SharedUser>,
    pub note: String,
    pub created_at: String,
}

/// One deleted memory card or file that can still be put back (V0.6.15).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrashItem {
    pub id: String,
    /// `memory_card` or `artifact`.
    pub kind: String,
    pub title: String,
    pub deleted_by: Option<SharedUser>,
    pub deleted_at: String,
    /// After this the item is removed for good.
    pub expires_at: String,
}

/// What putting an item back brought with it (V0.6.15).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrashRestoreResult {
    pub kind: String,
    /// The restored card's or file's id, the same it had before.
    pub item_id: String,
    pub title: String,
    /// Citations of a card left out because the file or passage they cite no longer exists.
    pub skipped_citations: u64,
    /// Citations of a file that were to a passage and came back as the file, because the passage could not be found.
    pub widened_citations: u64,
}

/// An approval decision that came with an imported workspace archive (V0.6.14): a record of what was decided on another
/// server, kept for reading. It never counts as an approval here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedApproval {
    pub revision: i64,
    /// `approved` or `withdrawn`, as it was decided elsewhere.
    pub decision: String,
    /// Who decided, as a name and an address.
    pub person: String,
    pub note: String,
    pub decided_at: String,
}

/// Whether the newest revision of a card is approved, who may decide, and the decisions so far.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryApprovalStatus {
    /// The newest revision, the one a decision would be about.
    pub revision: i64,
    pub approved: bool,
    pub approved_by: Option<SharedUser>,
    pub approved_at: Option<String>,
    /// The workspace requires that the person who made a revision cannot approve it.
    pub independent_review: bool,
    pub can_approve: bool,
    /// Why the caller cannot approve, in a sentence.
    pub cannot_approve_reason: Option<String>,
    /// Newest first, including decisions about earlier revisions.
    pub history: Vec<MemoryApprovalDecision>,
    /// Decisions that arrived with an imported archive; read-only records, never counted as approval (V0.6.14).
    #[serde(default)]
    pub imported: Vec<ImportedApproval>,
}

/// Who is accountable for reviewing one item and when it is next due (V0.6.6).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewAssignmentView {
    /// `evidence` or `memory_card`.
    pub kind: String,
    pub item_id: String,
    pub reviewer: Option<SharedUser>,
    pub reviewer_departed: bool,
    /// The name the reviewer had when assigned, for a reviewer who has left.
    pub reviewer_label: Option<String>,
    pub next_review_at: Option<String>,
    pub last_reviewed_at: Option<String>,
    pub overdue: bool,
}

/// What waits for one person across the workspaces they can open: assigned tasks that are not done, files they own
/// that need review and unread mentions. Everything is filtered by current access, so work in a workspace the person
/// lost is neither listed nor counted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyWork {
    pub tasks: Vec<CollaborationTask>,
    pub reviews: Vec<MyWorkReview>,
    pub mentions: Vec<SharedNotification>,
    /// Tasks with a due date before now.
    pub overdue_task_count: i64,
    /// Tasks, reviews and mentions together.
    pub total_count: i64,
}

/// A cited passage or file as a memory revision kept it (V0.6.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRevisionCitation {
    /// `chunk` (a passage) or `artifact` (a whole file).
    pub target_type: String,
    pub target_id: String,
    pub title: String,
    pub path: String,
    pub confidence: Option<f64>,
    /// The cited file, when the revision recorded it (V0.6.10).
    #[serde(default)]
    pub artifact_id: Option<String>,
    /// The hash of the file's content when the revision was saved.
    #[serde(default)]
    pub content_hash: Option<String>,
    /// Where the cited file stands now, so a historical citation never reads as current by default: `current` (same
    /// content), `changed` (the file was updated since), `gone` (the file no longer exists) or `unbound` (the revision
    /// predates content binding).
    #[serde(default)]
    pub currency: String,
    /// True when the cited passage or file was already gone when the revision was saved.
    #[serde(default)]
    pub missing_at_save: bool,
}

/// One saved version of a memory card: its text and the citations it had, who saved it and when. Revisions are never
/// edited; a restore adds a new one that names the revision it brought back.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRevision {
    pub revision: i64,
    pub title: String,
    pub body_markdown: String,
    pub source: String,
    pub confidence: Option<f64>,
    pub citations: Vec<MemoryRevisionCitation>,
    /// None for a revision that predates attribution or whose author was removed.
    pub author: Option<SharedUser>,
    /// The name an imported revision's author had on the server it came from, when there is no account here (V0.6.14).
    #[serde(default)]
    pub author_label: Option<String>,
    pub created_at: String,
    pub restored_from: Option<i64>,
}

/// Two revisions side by side: the text difference and which citations were added or dropped.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRevisionComparison {
    pub from_revision: i64,
    pub to_revision: i64,
    pub from_title: String,
    pub to_title: String,
    pub body: VersionComparison,
    pub citations_added: Vec<MemoryRevisionCitation>,
    pub citations_removed: Vec<MemoryRevisionCitation>,
}

/// The answer to a restore: the card as it is now, the number of the revision the restore created, and the citations
/// that could not be brought back because the cited passage or file no longer exists.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRestoreResult {
    pub card: MemoryCard,
    pub revision: i64,
    pub skipped_citations: Vec<MemoryRevisionCitation>,
}

/// The text a revision's citation pointed at, or the exact reason it cannot be shown (V0.6.10).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitedSource {
    /// `current` (the file is as cited), `kept_version` (the file changed; the cited version is kept), `unavailable`
    /// (the file changed and the cited text was not kept), `gone` (the file no longer exists) or `unbound` (the
    /// revision predates content binding).
    pub state: String,
    pub title: String,
    pub content_hash: Option<String>,
    pub text: Option<String>,
    pub truncated: bool,
    pub note: String,
}

/// One reason a card needs review, explained with the evidence change behind it (V0.6.9).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryChange {
    /// `missing`, `retired` or `changed`, as in the review reasons.
    pub kind: String,
    pub artifact_id: Option<String>,
    pub title: String,
    pub summary: String,
    /// The earlier and current text of the cited file, line by line, when an earlier version is kept.
    pub comparison: Option<VersionComparison>,
    /// Why no text comparison is shown, when there is none.
    pub unavailable_reason: Option<String>,
}

/// What changed behind a card's "needs review" flag.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryChanges {
    pub revision: i64,
    pub changes: Vec<MemoryChange>,
}

/// A job the workspace assistant knows how to do. The assistant only ever runs
/// one of these; a message that matches none of them gets a plain reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentCapability {
    FindFiles,
    /// Where a function, class or other code symbol is defined.
    FindSymbol,
    SearchContent,
    SummarizeFile,
    AskQuestion,
    WorkspaceOverview,
    /// Turns an earlier answer, or a sentence the person writes, into a memory card after they confirm (V0.9.10).
    SaveMemory,
    /// Moves a file to a folder, renames it or sends it to the trash, after the person confirms (V0.9.11).
    ManageFiles,
    /// Summarizes the whole conversation (V0.9.13).
    SummarizeChat,
    /// Reads several files together, as a kind, a folder or a selection, and writes one analysis (V0.9.27).
    AnalyzeFiles,
    /// Searches the workspace's memory cards and shows them (V0.9.29).
    RecallMemory,
    /// Changes, adds to, renames or deletes a memory card, after the person confirms (V0.9.30).
    ManageMemory,
    /// Creates a note from a sentence or from the last answer, after the person confirms (V0.9.31).
    CreateNote,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCapabilityInfo {
    pub id: AgentCapability,
    pub label: String,
    pub description: String,
    pub placeholder: String,
    pub requires_ai: bool,
    /// False when the capability needs AI and no text provider is enabled.
    pub available: bool,
    /// The kind of job, to group them (V0.9.32): `Find`, `Understand` or `Change`.
    #[serde(default)]
    pub group: String,
    /// Whether the job changes the workspace once the person confirms it (V0.9.32); a viewer is not offered it.
    #[serde(default)]
    pub writes: bool,
    /// Who turned the job off, `organization` or `workspace` (V0.9.91); none when it is on.
    #[serde(default)]
    pub switched_off_by: Option<String>,
}

/// What CAIRN offers to do for the person, which it does only when they confirm (V0.9.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProposalKind {
    /// Save a memory card, with the evidence it cites.
    SaveMemory,
    /// Move a file to a folder, or to the top level.
    MoveFiles,
    /// Give a file a new title.
    RenameFile,
    /// Move a file to the workspace's trash, which can restore it.
    TrashFiles,
    /// Store a chat's summary as a note, which is indexed like any other (V0.9.14).
    SaveSummary,
    /// Change the evidence state of files: verified, needs review, outdated or active (V0.9.28).
    SetStatus,
    /// Change a memory card's title or text (V0.9.30).
    UpdateMemory,
    /// Delete a memory card (V0.9.30).
    DeleteMemory,
    /// Create a note, indexed like any file (V0.9.31).
    SaveNote,
    /// Remember something about the person, for their own runs only (V0.9.51). Only the person's own memory changes, so a viewer
    /// may confirm it.
    SavePersonalMemory,
    /// Save how a run was done as a note in the workspace's Playbooks folder, unverified until a writer verifies it (V0.9.55).
    SavePlaybook,
    /// Make a task (V0.9.59), applied through the task handlers as the person.
    CreateTask,
    /// Comment on a task (V0.9.59), refused when the task changed since CAIRN read it.
    CommentTask,
    /// Change a task's status (V0.9.59), refused when the task changed since CAIRN read it.
    SetTaskStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProposalState {
    Pending,
    Applied,
    Dismissed,
}

/// An action CAIRN proposes in a reply. Nothing happens until the person confirms it; the action then runs as that person,
/// through the same checks as when they do it themselves.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProposal {
    /// Names the proposal within its reply.
    pub id: String,
    pub kind: AgentProposalKind,
    pub state: AgentProposalState,
    /// What confirming does, as a short title.
    pub title: String,
    /// What exactly would change, in Markdown, so the person sees it before confirming.
    pub detail: String,
    /// What the action runs with (for a memory card: its title, text and citations).
    pub payload: Value,
    /// What happened once it was confirmed.
    #[serde(default)]
    pub outcome: Option<String>,
    /// Content CAIRN read before making this proposal that it does not trust (an issue, a pull request, a mail), by title (V0.9.42).
    #[serde(default)]
    pub untrusted_sources: Vec<String>,
}

/// How a message was matched to a capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRouting {
    /// The user picked the capability.
    Explicit,
    /// Keyword rules recognised the request; no AI was involved.
    Rules,
    /// The enabled provider classified the request.
    Model,
    /// CAIRN worked it out with its tools, in a loop (V0.9.40).
    Agent,
    Unmatched,
}

/// What the user sent: typed text, optionally pinned to a capability or file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentMessage {
    #[serde(default)]
    pub message: String,
    pub capability: Option<AgentCapability>,
    pub artifact_id: Option<String>,
    /// Files the person selected in an earlier reply and the message is about (V0.9.26).
    #[serde(default)]
    pub artifact_ids: Vec<String>,
}

/// A saved assistant chat. Only the user who started it can see it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConversation {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    pub turn_count: i64,
    pub created_at: String,
    pub updated_at: String,
    /// Whether its owner shares it with the workspace (V0.9.57).
    #[serde(default)]
    pub shared: bool,
    /// Whether it looks across all the person's workspaces rather than the one it was started in (V0.9.60).
    #[serde(default)]
    pub all_workspaces: bool,
    /// `chat`, or `inbox` for the chat where an outside agent's proposals wait (V0.9.67).
    #[serde(default)]
    pub origin: String,
    /// The catalog model the person chose for this chat (V0.9.86); none uses what the workspace resolves.
    #[serde(default)]
    pub model_id: Option<String>,
}

/// A chat shared with the workspace (V0.9.57), as the others see it in the list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedConversation {
    pub id: String,
    pub title: String,
    pub shared_by_id: String,
    pub shared_by_name: String,
    pub shared_at: String,
    pub turn_count: i64,
}

/// One exchange: the request as the user saw it and the assistant's reply.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTurn {
    pub id: String,
    pub conversation_id: String,
    pub position: i64,
    pub label: String,
    pub request: AgentMessage,
    pub reply: AgentReply,
    pub created_at: String,
}

/// A file a step of a run named or used (V0.9.39). The trace keeps its title, never its text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStepEvidence {
    pub artifact_id: String,
    pub title: String,
}

/// One step of a CAIRN run: a tool that ran, a change it proposed, or the answer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStep {
    /// 1 is the first step.
    pub position: i64,
    /// `model`, `tool`, `proposal`, `answer` or `note`.
    pub kind: String,
    /// The registered tool, for a `tool` step.
    #[serde(default)]
    pub tool: Option<String>,
    /// One line saying what the step did; never a passage of a file.
    pub summary: String,
    #[serde(default)]
    pub evidence: Vec<AgentStepEvidence>,
    pub duration_ms: i64,
    #[serde(default)]
    pub error: Option<String>,
}

/// One message CAIRN answered, with how (V0.9.39). Personal to the person whose chat it belongs to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRun {
    pub id: String,
    pub turn_id: Option<String>,
    pub conversation_id: String,
    pub workspace_id: String,
    /// `chat`, `routine` or `inbox`.
    pub origin: String,
    #[serde(default)]
    pub model: Option<String>,
    /// `native`, `json` or `none`, for a run that used the model's tools.
    #[serde(default)]
    pub tool_mode: Option<String>,
    /// `running`, `answered`, `awaiting_confirmation`, `partial`, `failed` or `cancelled`.
    pub state: String,
    pub step_count: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_ms: i64,
    pub steps: Vec<AgentStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConversationDetail {
    pub conversation: AgentConversation,
    pub turns: Vec<AgentTurn>,
}

/// A prompt CAIRN runs on a schedule as its owner, with read tools only (V0.9.63).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRoutine {
    pub id: String,
    pub workspace_id: String,
    pub owner_id: String,
    pub owner_name: String,
    pub title: String,
    pub prompt: String,
    pub schedule: RecurringSchedule,
    /// The date (YYYY-MM-DD, UTC) of the next run.
    pub next_run_date: String,
    /// `active` or `paused`.
    pub state: String,
    pub pause_reason: Option<String>,
    /// Failed runs in a row; the third pauses the routine.
    pub failures: i64,
    pub last_run_at: Option<String>,
    /// The chat the routine posts into, once it has run.
    pub conversation_id: Option<String>,
    /// For a routine that runs on an event (V0.9.64): `files_indexed` or `file_outdated`; none for one that runs on a date.
    #[serde(default)]
    pub event_kind: Option<String>,
    /// Only files in this folder count for `files_indexed`.
    #[serde(default)]
    pub event_folder_id: Option<String>,
}

/// Whether a workspace's weekly memory upkeep is on, and when it last ran (V0.9.56).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpkeepSettings {
    pub enabled: bool,
    pub last_run_at: Option<String>,
}

/// What the weekly memory upkeep found and left for a person to look at (V0.9.56).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpkeepFinding {
    pub id: String,
    pub workspace_id: String,
    /// `possible_contradiction`, `unanswered_question` or `unused_playbook`.
    pub kind: String,
    pub title: String,
    pub detail: String,
    /// What the finding rests on, as lines of text.
    pub evidence: String,
    pub artifact_id: Option<String>,
    pub created_at: String,
}

/// One thing CAIRN remembers about a person (V0.9.51).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalMemoryEntry {
    pub id: String,
    /// The workspace it applies in; none means everywhere.
    #[serde(default)]
    pub workspace_id: Option<String>,
    pub text: String,
    pub created_at: String,
    pub updated_at: String,
}

/// A person's memory with CAIRN: their entries and whether it is on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalMemory {
    pub enabled: bool,
    pub entries: Vec<PersonalMemoryEntry>,
}

/// What a CAIRN chat remembers of its older turns (V0.9.48): a short summary, how far it reaches and the files those turns were
/// about, so "that file" still means the file of turn 2 at turn 30.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentSummary {
    pub text: String,
    /// The position of the last turn the summary covers.
    pub covers_up_to: i64,
    /// The files the covered turns were about, oldest first.
    #[serde(default)]
    pub files: Vec<AgentHistoryFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRequest {
    pub workspace_id: String,
    pub message: String,
    pub capability: Option<AgentCapability>,
    /// Target file for capabilities that work on one artifact.
    pub artifact_id: Option<String>,
    pub provider_id: Option<String>,
    /// Why AI may not be used for this request even though a provider may be
    /// enabled (for example the workspace keeps AI to some roles). Shown in
    /// place of the "no provider" reply when an AI capability is asked for.
    #[serde(default)]
    pub ai_unavailable_reason: Option<String>,
    /// Earlier turns of the same conversation, oldest first, so a follow-up
    /// such as "and what about its retries?" can be understood. The server
    /// fills this from the stored conversation, never from the client; the
    /// core keeps the newest ones that fit its token budget.
    #[serde(default)]
    pub history: Vec<AgentHistoryTurn>,
    /// The files the message is about, as the person selected them (V0.9.26). Like everything the client sends, they are
    /// checked against the workspace before they are used.
    #[serde(default)]
    pub artifact_ids: Vec<String>,
    /// What the chat remembers of turns older than `history` (V0.9.48). The server fills it from the stored conversation.
    #[serde(default)]
    pub summary: Option<AgentSummary>,
    /// What CAIRN remembers about the person asking, newest first; the server fills it for their own runs only (V0.9.51).
    #[serde(default)]
    pub personal_memory: Vec<String>,
    /// What the person told CAIRN was wrong earlier in this workspace and nobody has reviewed, newest first; the server fills it
    /// for their own runs only (V0.9.53).
    #[serde(default)]
    pub corrections: Vec<String>,
    /// Whether the person dismissed the offer to save a decision in this chat, so none is made again (V0.9.53).
    #[serde(default)]
    pub decision_offers_off: bool,
    /// How many estimated tokens of `history` the model's window has room for; the router's own small budget when none (V0.9.48).
    #[serde(default)]
    pub history_tokens: Option<usize>,
    /// Every earlier exchange of the conversation in full, oldest first (V0.9.10), for the jobs that act on an earlier
    /// answer, such as saving it as a memory card. Like `history`, the server fills it from the stored conversation.
    #[serde(default)]
    pub exchanges: Vec<AgentExchange>,
}

/// One earlier exchange of a conversation as it was: the request, the whole answer and the evidence it cited.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentExchange {
    pub question: String,
    pub answer: String,
    #[serde(default)]
    pub citations: Vec<Citation>,
    /// The files that answer listed (V0.9.26), so "move them to Runbooks" can mean them.
    #[serde(default)]
    pub file_ids: Vec<String>,
}

/// One earlier exchange as the assistant is reminded of it: what was asked,
/// what was answered and which files the answer was about.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHistoryTurn {
    pub question: String,
    pub answer: String,
    /// The files the answer was about: those it listed or summarized, else
    /// those it cited.
    #[serde(default)]
    pub files: Vec<AgentHistoryFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHistoryFile {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentReply {
    pub capability: Option<AgentCapability>,
    pub routing: AgentRouting,
    /// What the capability ran on after routing stripped the instruction.
    pub subject: String,
    pub reply_markdown: String,
    /// True when `reply_markdown` was written by an AI provider rather than
    /// assembled from stored facts.
    pub generated: bool,
    pub files: Vec<ArtifactSummary>,
    pub matches: Vec<SearchResult>,
    pub citations: Vec<Citation>,
    pub warnings: Vec<String>,
    /// How many earlier turns went with this question (0 for the first one,
    /// and for lookups that never read them).
    #[serde(default)]
    pub context_turns: i64,
    /// Actions CAIRN proposes and that wait for the person's confirmation (V0.9.10).
    #[serde(default)]
    pub proposals: Vec<AgentProposal>,
    /// The whole set of files a reply found, when it found some (V0.9.26): `files` shows the first of them, and the set is what
    /// the person selects from and what the next job can work on.
    #[serde(default)]
    pub file_set: Option<AgentFileSet>,
    /// The memory cards a reply found (V0.9.29), shown in the chat with what each cites.
    #[serde(default)]
    pub memory_cards: Vec<MemoryCardSummary>,
    /// The reply's proposals taken as one set to apply together (V0.9.44); none for a reply of the router.
    #[serde(default)]
    pub changeset: Option<AgentChangeset>,
    /// How far the sources an answer cites can be trusted (V0.9.45); only a run of the loop has it.
    #[serde(default)]
    pub trust: Option<AgentTrust>,
    /// Why the reply was made without AI when no model could be used for it (V0.9.142): none enabled, the workspace's AI policy or
    /// the person's allowance. Empty for a reply a model wrote, and for one made by a job while a model was available.
    #[serde(default)]
    pub ai_unavailable_reason: Option<String>,
}

/// What the sources an answer rests on are like: their review state and where they came from.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTrust {
    /// One line for under the answer, such as `3 verified · 1 outdated · 1 from an issue (synced 9 days ago)`.
    pub summary: String,
    pub sources: Vec<AgentSourceState>,
}

/// One cited file's state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSourceState {
    pub artifact_id: String,
    /// `verified`, `needs_review`, `outdated`, `superseded` or `active` (never reviewed).
    pub state: String,
    /// What kind of untrusted content it is ("an issue"), when it is.
    #[serde(default)]
    pub untrusted: Option<String>,
    /// For content from a connector: how many days ago its source was last synced.
    #[serde(default)]
    pub synced_days_ago: Option<i64>,
}

/// A run's proposals as one card of ticked actions, applied in order as the person when they confirm it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentChangeset {
    /// `pending`, `applied` (every ticked action was applied), `partial` (it stopped at a failure) or `dismissed`.
    pub state: String,
    /// What happened, once it was confirmed: which actions were applied and which was not.
    #[serde(default)]
    pub outcome: Option<String>,
}

/// The files a reply found, said in words and by id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentFileSet {
    /// What they are, as the filter reads ("Word documents in the folder Finance").
    pub description: String,
    /// How many there are, which may be more than `ids` holds.
    pub total: i64,
    pub ids: Vec<String>,
}

/// An account as system administrators see it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemUser {
    pub id: String,
    pub email: String,
    pub display_name: String,
    pub created_at: String,
    pub last_connected_at: Option<String>,
    pub is_system_admin: bool,
    #[serde(default)]
    pub is_app_admin: bool,
    #[serde(default)]
    pub is_cairn_admin: bool,
    /// When the account was deactivated, if it is. It cannot sign in or use
    /// tokens until it is reactivated.
    #[serde(default)]
    pub deactivated_at: Option<String>,
    /// When the account's personal data was removed, if it was. The account
    /// then shows as "Former member" and cannot be reactivated.
    #[serde(default)]
    pub anonymized_at: Option<String>,
    pub organization_count: i64,
    pub workspace_count: i64,
    /// Refresh tokens that are neither revoked nor expired.
    pub active_sessions: i64,
    /// Whether the account has confirmed two-step sign-in.
    #[serde(default)]
    pub two_step: bool,
    /// The workspaces the account is a direct member of, with its role in each.
    /// System administrators additionally act as administrators everywhere.
    #[serde(default)]
    pub workspaces: Vec<SystemUserWorkspace>,
    /// The organizations the account is a direct member of, with its role.
    #[serde(default)]
    pub organizations: Vec<SystemUserOrganization>,
}

/// One organization membership in the system user list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemUserOrganization {
    pub organization_id: String,
    pub name: String,
    /// `owner`, `admin` or `member`.
    pub role: String,
}

/// One workspace membership in the system user list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemUserWorkspace {
    pub workspace_id: String,
    pub name: String,
    /// `owner`, `admin`, `member` or `viewer`.
    pub role: String,
    /// The organization the workspace belongs to, if any.
    #[serde(default)]
    pub organization_id: Option<String>,
    #[serde(default)]
    pub organization_name: Option<String>,
}

/// Totals across the whole server.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemStatistics {
    pub user_count: i64,
    pub system_admin_count: i64,
    pub users_active_last_7_days: i64,
    pub users_active_last_30_days: i64,
    pub active_session_count: i64,
    pub organization_count: i64,
    pub workspace_count: i64,
    pub artifact_count: i64,
    pub artifact_bytes: i64,
    pub indexed_artifact_count: i64,
    pub blob_count: i64,
    pub blob_bytes: i64,
    pub chunk_count: i64,
    pub embedding_count: i64,
    pub memory_card_count: i64,
    pub task_count: i64,
    pub comment_count: i64,
    pub repository_count: i64,
    pub enabled_provider_count: i64,
    pub cloud_provider_count: i64,
    /// Jobs per status (`running`, `completed`, `failed`, `cancelled`).
    pub jobs_by_status: Vec<CountByLabel>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CountByLabel {
    pub label: String,
    pub count: i64,
}

/// One workspace's footprint, for the system usage page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceUsage {
    pub workspace_id: String,
    pub workspace_name: String,
    pub organization_id: Option<String>,
    pub organization_name: Option<String>,
    pub member_count: i64,
    pub artifact_count: i64,
    pub artifact_bytes: i64,
    pub chunk_count: i64,
    pub activity_last_30_days: i64,
    pub ai_requests_last_30_days: i64,
    pub last_activity_at: Option<String>,
}

/// A durable record of something a system administrator (or the system
/// itself) did at the system level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemAuditEvent {
    pub id: String,
    pub created_at: String,
    pub actor: Option<SharedUser>,
    pub action: String,
    pub detail: String,
}

/// A pending invitation to an organization or a workspace. The link token
/// itself is never stored or listed; it is returned once, when created.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invitation {
    pub id: String,
    pub email: String,
    /// `organization` or `workspace`.
    pub target_type: String,
    pub target_id: String,
    /// An organization role (`admin`, `member`) or a workspace role (`admin`,
    /// `member`, `viewer`), depending on `target_type`.
    pub role: String,
    pub invited_by: Option<SharedUser>,
    pub created_at: String,
    pub expires_at: String,
}

/// A personal access token as its owner sees it: never the secret itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalAccessToken {
    pub id: String,
    pub name: String,
    /// The start of the token (`rmp_` and a few characters), to tell tokens apart.
    pub prefix: String,
    /// `read` (look and search) or `write` (everything the owner may do).
    pub scope: String,
    pub created_at: String,
    pub expires_at: String,
    pub last_used_at: Option<String>,
    /// The workspaces the token is limited to; none means every workspace its owner can open (tokens made before
    /// V0.5.15 and any token created without a choice).
    #[serde(default)]
    pub workspaces: Option<Vec<String>>,
}

/// How the server reaches its outgoing mail server, as administrators see it.
/// The password is never part of it: only whether one is saved.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailSettingsView {
    /// A server and a sender are saved (whether or not sending is switched on).
    pub configured: bool,
    pub enabled: bool,
    pub host: String,
    pub port: i64,
    /// `tls` (TLS from the first byte) or `starttls`.
    pub security: String,
    pub username: String,
    pub password_set: bool,
    pub from_address: String,
    pub from_name: String,
    pub updated_at: Option<String>,
}

/// One message in the outbox, without its body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailOutboxEntry {
    pub id: String,
    pub to_address: String,
    pub subject: String,
    /// What the mail is for: `test`, and later `invitation`, `password_reset`, …
    pub kind: String,
    /// `queued`, `sending`, `sent` or `failed`.
    pub status: String,
    pub attempts: i64,
    pub last_error: Option<String>,
    pub created_at: String,
    pub next_attempt_at: Option<String>,
    pub sent_at: Option<String>,
}

/// The mail page of System › Settings: the settings, what the outbox holds and
/// the latest messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailOverview {
    pub settings: MailSettingsView,
    pub queued_count: i64,
    pub sent_last_day: i64,
    pub failed_last_day: i64,
    pub entries: Vec<MailOutboxEntry>,
}

#[cfg(test)]
mod label_color_tests {
    use super::{label_colors, LABEL_COLORS, LABEL_COLOR_NAMES};

    fn channel(hex: &str, at: usize) -> f64 {
        let value = u8::from_str_radix(&hex[at..at + 2], 16).unwrap() as f64 / 255.0;
        if value <= 0.03928 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    }

    fn luminance(hex: &str) -> f64 {
        0.2126 * channel(hex, 1) + 0.7152 * channel(hex, 3) + 0.0722 * channel(hex, 5)
    }

    #[test]
    fn every_label_colour_reaches_the_contrast_a_reader_needs() {
        for (name, background, text) in LABEL_COLORS {
            let (light, dark) = (luminance(background), luminance(text));
            let ratio = (light.max(dark) + 0.05) / (light.min(dark) + 0.05);
            assert!(ratio >= 4.5, "{name}: {ratio:.2} to 1");
        }
        let names: Vec<&str> = LABEL_COLORS.iter().map(|(name, _, _)| *name).collect();
        assert_eq!(names, LABEL_COLOR_NAMES);
        assert_eq!(label_colors("unknown"), label_colors("gray"));
    }
}
