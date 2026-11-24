//! Tasks and projects as read tools (V0.9.58): `list_tasks`, `get_task` and `get_project`. They read what the person may already read
//! in the workspace, change nothing, and give the model text with task keys (`T-12`) that it can quote and that the person can find.
//! A task's description is shown as quoted data, since a task made from an import or a tracker can hold words from outside.

use repomemo_domain::{CollaborationTask, TaskQuery};

use super::atoms::{bounded, quote_untrusted, EvidenceKind, EvidenceLedger, ToolOutput, TOOL_OUTPUT_CHARS};
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
