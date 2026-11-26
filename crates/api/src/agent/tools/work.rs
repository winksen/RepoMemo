//! Tasks and projects as read tools (V0.9.58): `list_tasks`, `get_task` and `get_project`. They read what the person may already read
//! in the workspace, change nothing, and give the model text with task keys (`T-12`) that it can quote and that the person can find.
//! A task's description is shown as quoted data, since a task made from an import or a tracker can hold words from outside.

use repomemo_domain::{
    AgentProposal, AgentProposalKind, AgentProposalState, CollaborationTask, Project, SearchRequest, TaskQuery,
};
use serde_json::{json, Value};

use super::atoms::{bounded, quote_untrusted, EvidenceKind, EvidenceLedger, ToolOutput, TOOL_OUTPUT_CHARS};
use crate::agent::run::{arg_list, arg_text};
use crate::agent::*;

/// The most tasks a listing names, and the most of a project's tasks and links shown.
const TASKS_LISTED_MAX: i64 = 25;
const PROJECT_TASKS_MAX: i64 = 15;
const PROJECT_LINKS_MAX: usize = 8;
/// A task's description is cut to this many characters.
const DESCRIPTION_CHARS: usize = 500;

/// The statuses that are not done.
fn open_statuses() -> Vec<String> {
    ["open", "in_progress", "blocked"]
        .iter()
        .map(|status| (*status).to_owned())
        .collect()
}

/// One task as a line the model reads.
pub(in crate::agent) fn task_line(task: &CollaborationTask) -> String {
    let mut parts = vec![format!(
        "{} [{} · {}]",
        task.key,
        task.status.replace('_', " "),
        task.priority
    )];
    parts.push(task.title.clone());
    if let Some(assignee) = &task.assignee {
        parts.push(format!("assigned to {}", assignee.display_name));
    }
    if let Some(due) = &task.due_at {
        parts.push(format!("due {}", due.get(..10).unwrap_or(due)));
    }
    if let Some(project) = &task.project_name {
        parts.push(format!("project {project}"));
    }
    if task.open_blocker_count > 0 {
        parts.push(format!(
            "blocked by {} open {}",
            task.open_blocker_count,
            plural(task.open_blocker_count as usize, "task", "tasks")
        ));
    }
    parts.join(" · ")
}

/// The number in a task key (`T-12`, `t-12` or `12`).
pub(in crate::agent) fn task_number(key: &str) -> Option<i64> {
    key.trim()
        .rsplit(['-', '#'])
        .next()?
        .trim()
        .parse()
        .ok()
        .filter(|number| *number > 0)
}

impl RepoMemoCore {
    /// The workspace's tasks that match a few filters, open ones unless a status is named.
    #[allow(clippy::too_many_arguments)] // the filters of the tool, as the model gives them
    pub(in crate::agent) async fn tool_list_tasks(
        &self,
        ledger: &EvidenceLedger,
        user_id: &str,
        project: &str,
        assignee: &str,
        status: &str,
        overdue: bool,
        words: &str,
    ) -> Result<ToolOutput> {
        let workspace_id = ledger.workspace_id();
        let mut query = TaskQuery {
            overdue,
            sort: Some("priority".to_owned()),
            ..TaskQuery::default()
        };
        let (project, assignee, status, words) = (project.trim(), assignee.trim(), status.trim(), words.trim());
        if !project.is_empty() {
            let projects = self.storage.list_projects(workspace_id, None).await?;
            let lower = project.to_lowercase();
            match projects
                .iter()
                .find(|known| known.name.to_lowercase() == lower)
                .or_else(|| projects.iter().find(|known| known.name.to_lowercase().contains(&lower)))
            {
                Some(found) => query.project_id = Some(found.id.clone()),
                None => return Ok(ToolOutput::text(format!("No project is named like \"{project}\"."))),
            }
        }
        if !assignee.is_empty() {
            query.assignee = Some(
                if assignee.eq_ignore_ascii_case("me") || assignee.eq_ignore_ascii_case("none") {
                    assignee.to_lowercase()
                } else {
                    match self.person_in_workspace(workspace_id, assignee).await {
                        Some(id) => id,
                        None => {
                            return Ok(ToolOutput::text(format!(
                                "Nobody in this workspace is named like \"{assignee}\"."
                            )))
                        }
                    }
                },
            );
        }
        query.status = if status.is_empty() || status.eq_ignore_ascii_case("open") {
            open_statuses()
        } else {
            vec![status.to_lowercase().replace(' ', "_")]
        };
        if !words.is_empty() {
            query.q = Some(words.to_owned());
        }
        let page = self
            .storage
            .query_collaboration_tasks(workspace_id, &query, user_id, TASKS_LISTED_MAX, 0)
            .await?;
        if page.tasks.is_empty() {
            return Ok(ToolOutput::text("No task matches those filters."));
        }
        let mut lines = vec![format!(
            "{} {} match{}:",
            page.total,
            plural(page.total as usize, "task", "tasks"),
            if page.total == 1 { "es" } else { "" }
        )];
        lines.extend(page.tasks.iter().map(task_line));
        Ok(ToolOutput::text(bounded(&lines, TOOL_OUTPUT_CHARS)))
    }

    /// The project a name means: the exact name, else the one that holds it.
    async fn project_named(&self, workspace_id: &str, name: &str) -> Result<Option<Project>> {
        let projects = self.storage.list_projects(workspace_id, None).await?;
        let lower = name.trim().to_lowercase();
        Ok(projects
            .iter()
            .find(|known| known.name.to_lowercase() == lower)
            .or_else(|| projects.iter().find(|known| known.name.to_lowercase().contains(&lower)))
            .cloned())
    }

    /// A task to make, as a proposal (V0.9.59): the project and the assignee are looked up by name now, so a wrong one is
    /// the model's to correct, and the files it should cite are the ones this run read.
    pub(in crate::agent) async fn proposal_for_task(
        &self,
        ledger: &EvidenceLedger,
        arguments: &Value,
    ) -> Result<AgentProposal> {
        let workspace_id = ledger.workspace_id();
        let title = shorten(arg_text(arguments, "title").trim(), 180);
        if title.is_empty() {
            anyhow::bail!("Give the task a title.");
        }
        let priority = Some(arg_text(arguments, "priority"))
            .map(|priority| priority.to_lowercase())
            .filter(|priority| ["low", "medium", "high", "urgent"].contains(&priority.as_str()))
            .unwrap_or_else(|| "medium".to_owned());
        let mut details = Vec::new();
        let project = arg_text(arguments, "project");
        let project_id = if project.trim().is_empty() {
            None
        } else {
            let Some(found) = self.project_named(workspace_id, &project).await? else {
                anyhow::bail!("No project is named like \"{project}\". Use get_project or leave the project out.");
            };
            details.push(format!("Project: {}", found.name));
            Some(found.id)
        };
        let assignee = arg_text(arguments, "assignee");
        let assignee_id = if assignee.trim().is_empty() {
            None
        } else {
            let Some(id) = self.person_in_workspace(workspace_id, &assignee).await else {
                anyhow::bail!("Nobody in this workspace is named like \"{assignee}\".");
            };
            details.push(format!("Assigned to: {assignee}"));
            Some(id)
        };
        let due = arg_text(arguments, "due");
        let due_at = (!due.trim().is_empty()).then(|| due.trim().to_owned());
        if let Some(date) = &due_at {
            if chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
                anyhow::bail!("The due date is a calendar date such as 2026-12-31.");
            }
            details.push(format!("Due: {date}"));
        }
        let mut references = Vec::new();
        for handle in arg_list(arguments, "files") {
            let Some(item) = ledger.get(&handle) else {
                anyhow::bail!("{handle} is not a handle you were given.");
            };
            references.push(json!({ "kind": "evidence", "target_id": item.artifact_id, "note": "" }));
            details.push(format!("Cites: {}", item.title));
        }
        let description = shorten(arg_text(arguments, "description").trim(), 2_000);
        let detail = format!(
            "**{title}**\n\n{}\n\nPriority: {priority}{}",
            if description.is_empty() {
                "No description."
            } else {
                &description
            },
            details.iter().map(|line| format!("\n{line}")).collect::<String>()
        );
        Ok(AgentProposal {
            id: "p1".to_owned(),
            kind: AgentProposalKind::CreateTask,
            state: AgentProposalState::Pending,
            title: format!("Create the task “{}”", shorten(&title, 80)),
            detail,
            payload: json!({
                "title": title, "description": description, "status": "open", "priority": priority,
                "assignee_user_id": assignee_id, "project_id": project_id, "due_at": due_at, "references": references
            }),
            outcome: None,
            untrusted_sources: Vec::new(),
        })
    }

    /// A comment or a status change on a task, as a proposal that remembers the version of the task it was written against.
    pub(in crate::agent) async fn proposal_for_task_change(
        &self,
        ledger: &EvidenceLedger,
        arguments: &Value,
        comment: bool,
    ) -> Result<AgentProposal> {
        let key = arg_text(arguments, "key");
        let found = match task_number(&key) {
            Some(number) => {
                self.storage
                    .get_collaboration_task_by_number(ledger.workspace_id(), number)
                    .await?
            }
            None => None,
        };
        let Some(task) = found else {
            anyhow::bail!("This workspace has no task {key}.");
        };
        if comment {
            let body = shorten(arg_text(arguments, "comment").trim(), 2_000);
            if body.is_empty() {
                anyhow::bail!("Write the comment.");
            }
            return Ok(AgentProposal {
                id: "p1".to_owned(),
                kind: AgentProposalKind::CommentTask,
                state: AgentProposalState::Pending,
                title: format!("Comment on {}", task.key),
                detail: format!("**{} {}**\n\n> {body}", task.key, task.title),
                payload: json!({ "task_id": task.id, "key": task.key, "expected_version": task.version, "body": body }),
                outcome: None,
                untrusted_sources: Vec::new(),
            });
        }
        let status = arg_text(arguments, "status").to_lowercase().replace(' ', "_");
        if !["open", "in_progress", "blocked", "done"].contains(&status.as_str()) {
            anyhow::bail!("The status is open, in_progress, blocked or done.");
        }
        Ok(AgentProposal {
            id: "p1".to_owned(),
            kind: AgentProposalKind::SetTaskStatus,
            state: AgentProposalState::Pending,
            title: format!("Set {} to {}", task.key, status.replace('_', " ")),
            detail: format!(
                "**{} {}**\n\nStatus: {} → {}",
                task.key,
                task.title,
                task.status.replace('_', " "),
                status.replace('_', " ")
            ),
            payload: json!({ "task_id": task.id, "key": task.key, "expected_version": task.version, "status": status }),
            outcome: None,
            untrusted_sources: Vec::new(),
        })
    }

    /// A workspace member whose name or email matches, by id.
    async fn person_in_workspace(&self, workspace_id: &str, name: &str) -> Option<String> {
        let lower = name.to_lowercase();
        self.storage
            .list_workspace_members(workspace_id)
            .await
            .ok()?
            .into_iter()
            .find(|member| {
                member.user.display_name.to_lowercase().contains(&lower)
                    || member
                        .user
                        .email
                        .as_deref()
                        .is_some_and(|email| email.to_lowercase() == lower)
            })
            .map(|member| member.user.id)
    }

    /// One task by its key, with its blockers and what it blocks and the files it links.
    pub(in crate::agent) async fn tool_get_task(&self, ledger: &mut EvidenceLedger, key: &str) -> Result<ToolOutput> {
        let Some(number) = task_number(key) else {
            return Ok(ToolOutput::text("Give the task's key, such as T-12."));
        };
        let Some(task) = self
            .storage
            .get_collaboration_task_by_number(ledger.workspace_id(), number)
            .await?
        else {
            return Ok(ToolOutput::text(format!("This workspace has no task {key}.")));
        };
        let mut lines = vec![task_line(&task)];
        if !task.description.trim().is_empty() {
            lines.push(quote_untrusted(
                "a task description",
                &shorten(
                    &task.description.split_whitespace().collect::<Vec<_>>().join(" "),
                    DESCRIPTION_CHARS,
                ),
            ));
        }
        if let Some(parent) = &task.parent_key {
            lines.push(format!("Sub-task of {parent}."));
        }
        if let Some(milestone) = &task.milestone_name {
            lines.push(format!("Counts toward the milestone {milestone}."));
        }
        let (blocked_by, blocks) = self.storage.list_task_dependencies(&task.id).await?;
        for dependency in &blocked_by {
            lines.push(format!(
                "Blocked by {} {} ({}).",
                dependency.blocker.key, dependency.blocker.title, dependency.blocker.status
            ));
        }
        for dependency in &blocks {
            lines.push(format!(
                "Blocks {} {}.",
                dependency.blocked.key, dependency.blocked.title
            ));
        }
        let mut handles = Vec::new();
        let references = self.storage.list_task_references(&task.id).await?;
        for reference in references
            .iter()
            .filter(|reference| reference.available)
            .take(PROJECT_LINKS_MAX)
        {
            lines.push(
                self.linked_line(
                    ledger,
                    &reference.kind,
                    &reference.target_id,
                    reference.title.as_deref(),
                    &mut handles,
                )
                .await,
            );
        }
        Ok(ToolOutput {
            for_model: bounded(&lines, TOOL_OUTPUT_CHARS),
            handles,
        })
    }

    /// One project by name: its goal, owner and milestones, the open tasks that are blocked and what blocks them, and what it links.
    pub(in crate::agent) async fn tool_get_project(
        &self,
        ledger: &mut EvidenceLedger,
        name: &str,
    ) -> Result<ToolOutput> {
        let workspace_id = ledger.workspace_id().to_owned();
        let name = name.trim();
        let projects = self.storage.list_projects(&workspace_id, None).await?;
        let lower = name.to_lowercase();
        let Some(project) = projects
            .iter()
            .find(|known| known.name.to_lowercase() == lower)
            .or_else(|| {
                projects
                    .iter()
                    .find(|known| known.name.to_lowercase().contains(&lower) && !lower.is_empty())
            })
        else {
            return Ok(ToolOutput::text(format!(
                "No project is named like \"{name}\". The projects are: {}.",
                projects
                    .iter()
                    .map(|known| known.name.as_str())
                    .take(20)
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        };
        let mut lines = vec![format!(
            "Project {} [{}]: {} open of {} tasks{}{}{}.",
            project.name,
            project.status.replace('_', " "),
            project.open_task_count,
            project.task_count,
            project
                .owner
                .as_ref()
                .map(|owner| format!(" · owner {}", owner.display_name))
                .unwrap_or_default(),
            project
                .target_date
                .as_ref()
                .map(|date| format!(" · target {date}"))
                .unwrap_or_default(),
            project
                .health
                .as_ref()
                .map(|health| format!(" · health {health}"))
                .unwrap_or_default(),
        )];
        if !project.goal.trim().is_empty() {
            lines.push(format!("Goal: {}", shorten(&project.goal, 300)));
        }
        for milestone in self.storage.list_milestones(&project.id).await?.iter().take(10) {
            lines.push(format!(
                "Milestone {}{}: {} of {} tasks done{}.",
                milestone.name,
                milestone
                    .due_date
                    .as_ref()
                    .map(|date| format!(" (due {date})"))
                    .unwrap_or_default(),
                milestone.done_count,
                milestone.task_count,
                if milestone.completed { ", complete" } else { "" }
            ));
        }
        let query = TaskQuery {
            project_id: Some(project.id.clone()),
            status: open_statuses(),
            sort: Some("priority".to_owned()),
            ..TaskQuery::default()
        };
        let page = self
            .storage
            .query_collaboration_tasks(&workspace_id, &query, "", PROJECT_TASKS_MAX, 0)
            .await?;
        for task in &page.tasks {
            lines.push(task_line(task));
            if task.open_blocker_count > 0 {
                let (blocked_by, _) = self.storage.list_task_dependencies(&task.id).await?;
                for dependency in blocked_by
                    .iter()
                    .filter(|dependency| dependency.blocker.status != "done")
                {
                    lines.push(format!(
                        "  {} waits for {} {} ({})",
                        task.key, dependency.blocker.key, dependency.blocker.title, dependency.blocker.status
                    ));
                }
            }
        }
        if page.total > page.tasks.len() as i64 {
            lines.push(format!(
                "… and {} more open tasks.",
                page.total - page.tasks.len() as i64
            ));
        }
        let mut handles = Vec::new();
        for reference in self
            .storage
            .list_project_references(&project.id)
            .await?
            .iter()
            .filter(|reference| reference.available)
            .take(PROJECT_LINKS_MAX)
        {
            lines.push(
                self.linked_line(
                    ledger,
                    &reference.kind,
                    &reference.target_id,
                    reference.title.as_deref(),
                    &mut handles,
                )
                .await,
            );
        }
        Ok(ToolOutput {
            for_model: bounded(&lines, TOOL_OUTPUT_CHARS),
            handles,
        })
    }

    /// A line for something a task or project links: a file with its handle, or a memory card by title.
    async fn linked_line(
        &self,
        ledger: &mut EvidenceLedger,
        kind: &str,
        target_id: &str,
        title: Option<&str>,
        handles: &mut Vec<String>,
    ) -> String {
        if kind == "evidence" {
            if let Ok(file) = self.storage.get_artifact_summary(target_id).await {
                if file.workspace_id == ledger.workspace_id() {
                    let handle = ledger.handle_for(EvidenceKind::File, &file.id, None, &file.title, &file.path, None);
                    handles.push(handle.clone());
                    return format!("Links [{handle}] {}", file.path);
                }
            }
        }
        format!("Links {} {}", kind.replace('_', " "), title.unwrap_or("(untitled)"))
    }
}

impl RepoMemoCore {
    /// What changed around a file since it was last verified (V0.9.65), for CAIRN to draft a review from: its state and who last
    /// reviewed it and when, its versions since, newer files that mention its name, and open corrections that do. Facts only: the
    /// model weighs them and proposes a state with the status job, which the person confirms.
    pub(in crate::agent) async fn tool_review_file(
        &self,
        ledger: &mut EvidenceLedger,
        user_id: &str,
        file: &str,
    ) -> Result<ToolOutput> {
        let summary = match self.tool_file(ledger, file).await? {
            Ok(summary) => summary,
            Err(problem) => return Ok(ToolOutput::text(problem)),
        };
        let lifecycle = self.storage.get_artifact_lifecycle(&summary.id).await.ok();
        let state = lifecycle
            .as_ref()
            .map_or("active", |found| found.status.as_str())
            .to_owned();
        let handle = ledger.handle_for(
            EvidenceKind::File,
            &summary.id,
            None,
            &summary.title,
            &summary.path,
            Some(&state),
        );
        let mut lines = vec![format!("[{handle}] {} is {state}.", summary.path)];
        let reviewed_at = lifecycle.as_ref().and_then(|found| found.reviewed_at.clone());
        match (
            &reviewed_at,
            lifecycle.as_ref().and_then(|found| found.reviewed_by.as_ref()),
        ) {
            (Some(at), Some(by)) => lines.push(format!(
                "Last verified by {} on {}.",
                by.display_name,
                at.get(..10).unwrap_or(at)
            )),
            (Some(at), None) => lines.push(format!("Last verified on {}.", at.get(..10).unwrap_or(at))),
            _ => lines.push("It has never been verified.".to_owned()),
        }
        if let Some(note) = lifecycle
            .as_ref()
            .map(|found| found.review_note.trim())
            .filter(|note| !note.is_empty())
        {
            lines.push(format!(
                "Review note: {}",
                quote_untrusted("a review note", &shorten(note, 300))
            ));
        }
        if let Some(successor) = lifecycle
            .as_ref()
            .and_then(|found| found.superseded_by_artifact_id.clone())
        {
            lines.push(format!("It was superseded by the file {successor}."));
        }
        let since = reviewed_at.clone().unwrap_or_default();
        lines.push(format!(
            "It was last changed on {}.",
            summary.updated_at.get(..10).unwrap_or(&summary.updated_at)
        ));
        let versions = self.storage.list_artifact_versions(&summary.id).await?;
        let newer = versions.iter().filter(|version| version.created_at > since).count();
        lines.push(format!(
            "{} {} in all; {newer} since it was verified.",
            versions.len(),
            plural(versions.len(), "version", "versions")
        ));
        // Newer files that mention it by name.
        let name = summary
            .title
            .trim_end_matches(|c: char| c != '.' && !c.is_alphanumeric())
            .to_owned();
        if name.chars().count() >= 4 {
            let mut found = self
                .search_workspace(SearchRequest {
                    workspace_id: ledger.workspace_id().to_owned(),
                    query: format!("\"{}\"", name.replace('"', " ")),
                    artifact_types: Vec::new(),
                    languages: Vec::new(),
                    source_ids: Vec::new(),
                    limit: Some(12),
                    lifecycle_statuses: Vec::new(),
                    folder_id: None,
                    modified_after: reviewed_at.as_deref().map(|at| at.get(..10).unwrap_or(at).to_owned()),
                    modified_before: None,
                })
                .await
                .unwrap_or_default();
            self.drop_closed_hits(ledger.workspace_id(), &mut found).await;
            let mut seen = Vec::new();
            for hit in found.iter().filter(|hit| hit.artifact_id != summary.id) {
                if seen.contains(&hit.artifact_id) || seen.len() >= 5 {
                    continue;
                }
                seen.push(hit.artifact_id.clone());
                let mention = ledger.handle_for(
                    EvidenceKind::Passage,
                    &hit.artifact_id,
                    Some(&hit.chunk_id),
                    &hit.title,
                    &hit.path,
                    hit.status.as_deref(),
                );
                lines.push(format!("Mentioned since in [{mention}] {}.", hit.path));
            }
            if seen.is_empty() {
                lines.push("No newer file in the workspace mentions it by name.".to_owned());
            }
        }
        // What was said was wrong, in the person's own words, about this file's name.
        let corrections = self
            .storage
            .open_agent_corrections(ledger.workspace_id(), user_id, 20)
            .await
            .unwrap_or_default();
        for correction in corrections
            .iter()
            .filter(|text| text.to_lowercase().contains(&summary.title.to_lowercase()))
            .take(3)
        {
            lines.push(format!("Your correction (not reviewed): {}", shorten(correction, 200)));
        }
        lines.push("Weigh these, say what you could not check, and propose verified, needs review or outdated with the reasons; the person confirms.".to_owned());
        Ok(ToolOutput {
            for_model: bounded(&lines, TOOL_OUTPUT_CHARS),
            handles: vec![handle],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_task_key_is_read_in_the_ways_people_write_it() {
        assert_eq!(task_number("T-12"), Some(12));
        assert_eq!(task_number(" t-7 "), Some(7));
        assert_eq!(task_number("#30"), Some(30));
        assert_eq!(task_number("12"), Some(12));
        assert_eq!(task_number("T-0"), None);
        assert_eq!(task_number("the migration"), None);
    }
}
