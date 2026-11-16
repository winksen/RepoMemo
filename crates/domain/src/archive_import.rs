//! Reading a workspace archive back: what it is checked against and what an
//! import answers (roadmap O2b). The archive itself is described in
//! [`crate::archive`].

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{
    ArchiveCounts, ArchiveManifest, ArchivePerson, ArchiveReference, SharedWorkspace, WorkspaceArchiveData,
    ARCHIVE_FORMAT, ARCHIVE_FORMAT_VERSION, MAX_FOLDER_DEPTH,
};

/// What checking an archive found.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ArchiveCheck {
    /// Things that make the archive unusable; any one stops an import.
    pub problems: Vec<String>,
    /// Things that import, but not quite as they were.
    pub warnings: Vec<String>,
}

impl ArchiveCheck {
    pub fn is_ok(&self) -> bool {
        self.problems.is_empty()
    }
}

pub const LIFECYCLE_STATUSES: [&str; 5] = ["active", "needs_review", "verified", "outdated", "superseded"];
pub const TASK_STATUSES: [&str; 4] = ["open", "in_progress", "blocked", "done"];
pub const TASK_PRIORITIES: [&str; 4] = ["low", "medium", "high", "urgent"];
/// How many problems of one kind are written out before the rest are counted.
const PROBLEMS_SHOWN: usize = 5;

impl ArchiveManifest {
    /// Whether this server can read an archive with this manifest.
    pub fn check(&self) -> ArchiveCheck {
        let mut check = ArchiveCheck::default();
        if self.format != ARCHIVE_FORMAT {
            check
                .problems
                .push("This file is not a RepoMemo workspace archive.".to_owned());
        } else if self.format_version > ARCHIVE_FORMAT_VERSION {
            check.problems.push(format!(
                "This archive was written by a newer RepoMemo (archive format {}; this server reads up to {}). Update the server first.",
                self.format_version, ARCHIVE_FORMAT_VERSION
            ));
        } else if self.format_version == 0 {
            check.problems.push("This archive has no format version.".to_owned());
        }
        check
    }

    /// Total bytes of the stored files the archive holds. Sizes are the archive's own claim, so the
    /// sum saturates instead of wrapping around to a small number.
    pub fn blob_bytes(&self) -> u64 {
        self.blobs
            .iter()
            .fold(0_u64, |total, blob| total.saturating_add(blob.size_bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ArchiveBlob, ArchiveCounts};

    fn manifest(sizes: &[u64]) -> ArchiveManifest {
        ArchiveManifest {
            format: ARCHIVE_FORMAT.to_owned(),
            format_version: ARCHIVE_FORMAT_VERSION,
            created_at: String::new(),
            app_version: "test".to_owned(),
            workspace_name: "Test".to_owned(),
            counts: ArchiveCounts::default(),
            data_sha256: String::new(),
            data_bytes: 0,
            blobs: sizes
                .iter()
                .enumerate()
                .map(|(index, size)| ArchiveBlob {
                    hash: format!("{index:064x}"),
                    size_bytes: *size,
                })
                .collect(),
        }
    }

    #[test]
    fn a_record_that_states_another_size_than_its_stored_file_is_corrected() {
        let mut data: WorkspaceArchiveData = serde_json::from_value(serde_json::json!({
            "workspace": {"name": "Test", "created_at": "", "settings": {}},
            "sources": [], "folders": [], "lifecycle": [], "memory_cards": [], "tasks": [], "comments": [],
            "artifacts": [
                {"id": "a1", "source_id": "s", "folder_id": null, "artifact_type": "text", "title": "One", "path": "one.txt",
                 "content_hash": "aaaa", "mime_type": null, "language": null, "size_bytes": 3, "created_at": "", "updated_at": "", "metadata": {}},
                {"id": "a2", "source_id": "s", "folder_id": null, "artifact_type": "text", "title": "Two", "path": "two.txt",
                 "content_hash": "bbbb", "mime_type": null, "language": null, "size_bytes": 10, "created_at": "", "updated_at": "", "metadata": {}},
                {"id": "a3", "source_id": "s", "folder_id": null, "artifact_type": "text", "title": "Three", "path": "three.txt",
                 "content_hash": "cccc", "mime_type": null, "language": null, "size_bytes": 7, "created_at": "", "updated_at": "", "metadata": {}}
            ],
            "versions": [
                {"artifact_id": "a1", "version": 1, "content_hash": "dddd", "size_bytes": 1, "mime_type": null,
                 "became_current_at": "", "replaced_at": "", "replaced_by": null}
            ]
        }))
        .unwrap();
        let sizes: HashMap<&str, u64> = HashMap::from([("aaaa", 3), ("bbbb", 2_000_000), ("dddd", 400)]);
        // "bbbb" and "dddd" were stated wrong; "aaaa" is right; "cccc" is not listed and is left to the checks.
        assert_eq!(data.use_stored_sizes(&sizes), 2);
        assert_eq!(
            data.artifacts
                .iter()
                .map(|artifact| artifact.size_bytes)
                .collect::<Vec<_>>(),
            vec![3, 2_000_000, 7]
        );
        assert_eq!(data.versions[0].size_bytes, 400);
        assert_eq!(data.use_stored_sizes(&sizes), 0, "nothing left to correct");
    }

    #[test]
    fn the_total_of_declared_sizes_saturates_instead_of_wrapping() {
        assert_eq!(manifest(&[10, 20]).blob_bytes(), 30);
        assert_eq!(manifest(&[u64::MAX / 2 + 1, u64::MAX / 2 + 1]).blob_bytes(), u64::MAX);
        assert_eq!(manifest(&[u64::MAX, 5]).blob_bytes(), u64::MAX);
    }
}

/// Whether text is a calendar date, YYYY-MM-DD (this crate has no date library).
fn chrono_free_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    let number = |range: std::ops::Range<usize>| text.get(range).and_then(|part| part.parse::<u32>().ok());
    matches!(
        (number(0..4), number(5..7), number(8..10)),
        (Some(_), Some(1..=12), Some(1..=31))
    )
}

fn limited(check: &mut ArchiveCheck, found: Vec<String>, what: &str) {
    let total = found.len();
    check.problems.extend(found.into_iter().take(PROBLEMS_SHOWN));
    if total > PROBLEMS_SHOWN {
        check
            .problems
            .push(format!("{} more {what} have the same problem.", total - PROBLEMS_SHOWN));
    }
}

impl WorkspaceArchiveData {
    /// Replaces the size each record states with the size of the stored file it names, where `sizes` (the
    /// archive's list of stored files, each checked against its hash while it was read) has that file; returns how
    /// many records said otherwise. The workspace's storage allowance adds these figures up, and an archive could
    /// state any (V0.3.26).
    pub fn use_stored_sizes(&mut self, sizes: &HashMap<&str, u64>) -> usize {
        let mut changed = 0;
        let mut set = |hash: &str, stated: &mut i64| {
            if let Some(size) = sizes.get(hash) {
                let size = i64::try_from(*size).unwrap_or(i64::MAX);
                if *stated != size {
                    *stated = size;
                    changed += 1;
                }
            }
        };
        for artifact in &mut self.artifacts {
            set(&artifact.content_hash, &mut artifact.size_bytes);
        }
        for version in &mut self.versions {
            set(&version.content_hash, &mut version.size_bytes);
        }
        changed
    }

    /// Checks that the records agree with each other: every reference points
    /// at something in the archive, folders form a tree no deeper than the
    /// product allows, and states are ones this server knows.
    pub fn check(&self) -> ArchiveCheck {
        let mut check = ArchiveCheck::default();

        let mut seen = HashSet::new();
        let duplicates = self
            .artifacts
            .iter()
            .map(|artifact| artifact.id.as_str())
            .chain(self.folders.iter().map(|folder| folder.id.as_str()))
            .chain(self.sources.iter().map(|source| source.id.as_str()))
            .chain(self.memory_cards.iter().map(|card| card.id.as_str()))
            .chain(self.tasks.iter().map(|task| task.id.as_str()))
            .chain(self.comments.iter().map(|comment| comment.id.as_str()))
            .filter(|id| !seen.insert(*id))
            .map(|id| format!("The id {id} is used twice."))
            .collect::<Vec<_>>();
        limited(&mut check, duplicates, "records");

        let sources: HashSet<&str> = self.sources.iter().map(|source| source.id.as_str()).collect();
        let artifacts: HashSet<&str> = self.artifacts.iter().map(|artifact| artifact.id.as_str()).collect();
        if self.workspace.name.trim().is_empty() {
            check.problems.push("The workspace has no name.".to_owned());
        }

        let orphans = self
            .artifacts
            .iter()
            .filter(|artifact| !sources.contains(artifact.source_id.as_str()))
            .map(|artifact| format!("The file {} names a source that is not in the archive.", artifact.path))
            .collect::<Vec<_>>();
        limited(&mut check, orphans, "files");
        let bad_hashes = self
            .artifacts
            .iter()
            .filter(|artifact| {
                artifact.content_hash.len() < 4 || !artifact.content_hash.chars().all(|c| c.is_ascii_hexdigit())
            })
            .map(|artifact| format!("The file {} has no valid content hash.", artifact.path))
            .collect::<Vec<_>>();
        limited(&mut check, bad_hashes, "files");

        // Folders: parents exist, no loops, no deeper than allowed.
        let parents: HashMap<&str, Option<&str>> = self
            .folders
            .iter()
            .map(|folder| (folder.id.as_str(), folder.parent_id.as_deref()))
            .collect();
        let mut folder_problems = Vec::new();
        for folder in &self.folders {
            let mut depth = 1;
            let mut current = folder.parent_id.as_deref();
            let mut visited = HashSet::new();
            while let Some(parent) = current {
                if !visited.insert(parent) {
                    folder_problems.push(format!("The folder {} is inside itself.", folder.name));
                    break;
                }
                match parents.get(parent) {
                    Some(next) => {
                        depth += 1;
                        current = *next;
                    }
                    None => {
                        folder_problems.push(format!(
                            "The folder {} sits in a folder that is not in the archive.",
                            folder.name
                        ));
                        break;
                    }
                }
            }
            if depth > MAX_FOLDER_DEPTH {
                folder_problems.push(format!(
                    "The folder {} is nested {depth} levels deep; {MAX_FOLDER_DEPTH} is the most.",
                    folder.name
                ));
            }
        }
        limited(&mut check, folder_problems, "folders");
        let folder_ids: HashSet<&str> = parents.keys().copied().collect();
        let lost = self
            .artifacts
            .iter()
            .filter(|artifact| artifact.folder_id.as_deref().is_some_and(|id| !folder_ids.contains(id)))
            .count();
        if lost > 0 {
            check.warnings.push(format!(
                "{lost} file(s) were in a folder that is not in the archive and will be at the top."
            ));
        }

        // Earlier versions: of a file in the archive, numbered once, with a valid content hash.
        let stray_versions = self
            .versions
            .iter()
            .filter(|version| !artifacts.contains(version.artifact_id.as_str()))
            .count();
        if stray_versions > 0 {
            check.warnings.push(format!(
                "{stray_versions} earlier version(s) are of a file that is not in the archive and will be left out."
            ));
        }
        let mut numbered = HashSet::new();
        let version_problems = self
            .versions
            .iter()
            .filter(|version| artifacts.contains(version.artifact_id.as_str()))
            .filter_map(|version| {
                if version.version < 1 {
                    return Some(format!(
                        "An earlier version of {} has no valid number.",
                        version.artifact_id
                    ));
                }
                if version.content_hash.len() < 4 || !version.content_hash.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Some(format!(
                        "Version {} of {} has no valid content hash.",
                        version.version, version.artifact_id
                    ));
                }
                (!numbered.insert((version.artifact_id.as_str(), version.version))).then(|| {
                    format!(
                        "Version {} of {} is listed twice.",
                        version.version, version.artifact_id
                    )
                })
            })
            .collect::<Vec<_>>();
        limited(&mut check, version_problems, "earlier versions");
        let lost_sources = self
            .saved_searches
            .iter()
            .filter(|search| search.source_ids.iter().any(|id| !sources.contains(id.as_str())))
            .count();
        if lost_sources > 0 {
            check.warnings.push(format!(
                "{lost_sources} saved search(es) keep to a source that is not in the archive; the source is dropped from them."
            ));
        }

        let lifecycle = self
            .lifecycle
            .iter()
            .filter(|entry| {
                !artifacts.contains(entry.artifact_id.as_str()) || !LIFECYCLE_STATUSES.contains(&entry.status.as_str())
            })
            .map(|entry| format!("The lifecycle of {} is not valid here.", entry.artifact_id))
            .collect::<Vec<_>>();
        limited(&mut check, lifecycle, "lifecycle records");
        let comments = self
            .comments
            .iter()
            .filter(|comment| !artifacts.contains(comment.artifact_id.as_str()))
            .count();
        if comments > 0 {
            check.warnings.push(format!(
                "{comments} comment(s) are on a file that is not in the archive and will be left out."
            ));
        }
        let tasks = self
            .tasks
            .iter()
            .filter(|task| {
                !TASK_STATUSES.contains(&task.status.as_str()) || !TASK_PRIORITIES.contains(&task.priority.as_str())
            })
            .map(|task| format!("The task {} has a state this server does not know.", task.title))
            .collect::<Vec<_>>();
        limited(&mut check, tasks, "tasks");
        let linked = self
            .tasks
            .iter()
            .filter(|task| task.artifact_id.as_deref().is_some_and(|id| !artifacts.contains(id)))
            .count();
        if linked > 0 {
            check.warnings.push(format!(
                "{linked} task(s) name a file that is not in the archive; the link is dropped."
            ));
        }
        let missing_citations = self
            .memory_cards
            .iter()
            .flat_map(|card| card.citations.iter())
            .filter(|citation| !artifacts.contains(citation.artifact_id.as_str()))
            .count();
        if missing_citations > 0 {
            check.warnings.push(format!(
                "{missing_citations} citation(s) name a file that is not in the archive and will be left out."
            ));
        }
        if self.trash_left_out > 0 {
            check.warnings.push(format!(
                "{} deleted item(s) were in the trash of the workspace this came from. A trash is not part of an archive, so they are not here and cannot be restored from it.",
                self.trash_left_out
            ));
        }
        let approvals: usize = self.memory_cards.iter().map(|card| card.approvals.len()).sum();
        if approvals > 0 {
            check.warnings.push(format!(
                "{approvals} approval decision(s) come back as an imported record. They show who decided what and when, but nothing is approved on this server until someone here approves it."
            ));
        }
        let passages = self
            .memory_cards
            .iter()
            .flat_map(|card| card.revisions.iter())
            .flat_map(|revision| revision.citations.iter())
            .filter(|citation| citation.passage)
            .count();
        if passages > 0 {
            check.warnings.push(format!(
                "{passages} citation(s) of earlier revisions cited a passage; they come back as the file they were in."
            ));
        }
        self.check_work(&mut check);
        if self.omitted_citations > 0 {
            check.warnings.push(format!(
                "{} citation(s) were already left out of the archive because what they cited no longer existed.",
                self.omitted_citations
            ));
        }
        check
    }

    /// The work of the 0.7 line (format 4): every relation points at a record that is in the archive, so nothing can name a
    /// record of another workspace; the relations are acyclic; and what an import will not do is said.
    fn check_work(&self, check: &mut ArchiveCheck) {
        let work = &self.work;
        let tasks: HashSet<&str> = self.tasks.iter().map(|task| task.id.as_str()).collect();
        let projects: HashSet<&str> = work.projects.iter().map(|project| project.id.as_str()).collect();
        let milestones: HashMap<&str, &str> = work
            .milestones
            .iter()
            .map(|milestone| (milestone.id.as_str(), milestone.project_id.as_str()))
            .collect();
        let labels: HashSet<&str> = work.labels.iter().map(|label| label.id.as_str()).collect();
        let states: HashSet<&str> = work.workflow_states.iter().map(|state| state.id.as_str()).collect();

        let mut seen = HashSet::new();
        let repeated = work
            .projects
            .iter()
            .map(|record| record.id.as_str())
            .chain(work.milestones.iter().map(|record| record.id.as_str()))
            .chain(work.labels.iter().map(|record| record.id.as_str()))
            .chain(work.workflow_states.iter().map(|record| record.id.as_str()))
            .chain(work.recurring.iter().map(|record| record.id.as_str()))
            .chain(work.updates.iter().map(|record| record.id.as_str()))
            .filter(|id| !seen.insert(*id))
            .map(|id| format!("The id {id} is used twice among the projects and what hangs on them."))
            .collect::<Vec<_>>();
        limited(check, repeated, "project records");

        let mut problems = Vec::new();
        for project in &work.projects {
            if !["active", "on_hold", "done", "archived"].contains(&project.status.as_str()) {
                problems.push(format!(
                    "The project {} has a status this server does not know.",
                    project.name
                ));
            }
        }
        for milestone in &work.milestones {
            if !projects.contains(milestone.project_id.as_str()) {
                problems.push(format!(
                    "The milestone {} names a project that is not in the archive.",
                    milestone.name
                ));
            }
            if !["all_tasks", "manual"].contains(&milestone.completion_rule.as_str()) {
                problems.push(format!(
                    "The milestone {} has a completion rule this server does not know.",
                    milestone.name
                ));
            }
        }
        let mut label_names = HashSet::new();
        for label in &work.labels {
            if !label_names.insert(label.name.to_lowercase()) {
                problems.push(format!("The label {} appears twice.", label.name));
            }
        }
        let mut state_names = HashSet::new();
        for state in &work.workflow_states {
            let lower = state.name.to_lowercase();
            if ["open", "in progress", "blocked", "done"].contains(&lower.as_str()) {
                problems.push(format!(
                    "The state {} repeats the name of a built-in state.",
                    state.name
                ));
            }
            if !state_names.insert(lower) {
                problems.push(format!("The state {} appears twice.", state.name));
            }
            if !TASK_STATUSES.contains(&state.category.as_str()) {
                problems.push(format!(
                    "The state {} has a category this server does not know.",
                    state.name
                ));
            }
        }
        let mut numbers = HashSet::new();
        let mut extras_seen = HashSet::new();
        let mut parents: HashMap<&str, &str> = HashMap::new();
        let mut blockers: HashMap<&str, Vec<&str>> = HashMap::new();
        for extra in &work.task_extras {
            if !tasks.contains(extra.task_id.as_str()) {
                problems.push(format!(
                    "A task record names {}, which is not a task of the archive.",
                    extra.task_id
                ));
                continue;
            }
            if !extras_seen.insert(extra.task_id.as_str()) {
                problems.push(format!("The task {} has two records of its relations.", extra.task_id));
            }
            if extra.number < 1 || !numbers.insert(extra.number) {
                problems.push(format!("The task {} has no valid, unique number.", extra.task_id));
            }
            if let Some(project) = extra.project_id.as_deref() {
                if !projects.contains(project) {
                    problems.push(format!(
                        "The task {} is in a project that is not in the archive.",
                        extra.task_id
                    ));
                }
            }
            if let Some(milestone) = extra.milestone_id.as_deref() {
                match milestones.get(milestone) {
                    None => problems.push(format!(
                        "The task {} is in a milestone that is not in the archive.",
                        extra.task_id
                    )),
                    Some(project) if extra.project_id.as_deref() != Some(*project) => problems.push(format!(
                        "The task {} is in a milestone of another project than its own.",
                        extra.task_id
                    )),
                    Some(_) => {}
                }
            }
            if extra.label_ids.iter().any(|id| !labels.contains(id.as_str())) {
                problems.push(format!(
                    "The task {} carries a label that is not in the archive.",
                    extra.task_id
                ));
            }
            if extra.state_id.as_deref().is_some_and(|id| !states.contains(id)) {
                problems.push(format!(
                    "The task {} is in a state that is not in the archive.",
                    extra.task_id
                ));
            }
            if let Some(parent) = extra.parent_task_id.as_deref() {
                if parent == extra.task_id || !tasks.contains(parent) {
                    problems.push(format!(
                        "The task {} has a parent that is itself or not in the archive.",
                        extra.task_id
                    ));
                } else {
                    parents.insert(extra.task_id.as_str(), parent);
                }
            }
            for blocker in &extra.blocked_by {
                if blocker == &extra.task_id || !tasks.contains(blocker.as_str()) {
                    problems.push(format!(
                        "The task {} is blocked by itself or by a task that is not in the archive.",
                        extra.task_id
                    ));
                } else {
                    blockers
                        .entry(extra.task_id.as_str())
                        .or_default()
                        .push(blocker.as_str());
                }
            }
        }
        // Parents and blockers must not loop, however long the loop is.
        for start in parents.keys() {
            let mut at = *start;
            let mut steps = 0;
            while let Some(next) = parents.get(at) {
                steps += 1;
                if steps > tasks.len() {
                    problems.push(format!("The task {start} is part of a loop of parent tasks."));
                    break;
                }
                at = next;
            }
        }
        for start in blockers.keys() {
            let mut stack = vec![*start];
            let mut visited = HashSet::new();
            while let Some(at) = stack.pop() {
                for next in blockers.get(at).into_iter().flatten() {
                    if next == start {
                        problems.push(format!("The task {start} is blocked, through other tasks, by itself."));
                        stack.clear();
                        break;
                    }
                    if visited.insert(*next) {
                        stack.push(next);
                    }
                }
            }
        }
        for recurring in &work.recurring {
            if recurring.project_id.as_deref().is_some_and(|id| !projects.contains(id)) {
                problems.push(format!(
                    "The recurring task {} is in a project that is not in the archive.",
                    recurring.title
                ));
            }
            let schedule = &recurring.schedule;
            let valid = chrono_free_date(&schedule.start_date)
                && (1..=60).contains(&schedule.interval)
                && match schedule.kind.as_str() {
                    "daily" => true,
                    "weekly" => schedule.weekday.is_some_and(|day| (0..=6).contains(&day)),
                    "monthly" => schedule.day_of_month.is_some_and(|day| (1..=28).contains(&day)),
                    _ => false,
                };
            if !valid || !["latest", "skip"].contains(&recurring.missed_policy.as_str()) {
                problems.push(format!(
                    "The schedule of {} is not one this server can run.",
                    recurring.title
                ));
            }
            if !["active", "paused", "stopped"].contains(&recurring.status.as_str()) {
                problems.push(format!(
                    "The recurring task {} has a status this server does not know.",
                    recurring.title
                ));
            }
        }
        for update in &work.updates {
            if !projects.contains(update.project_id.as_str()) {
                problems.push("A project update names a project that is not in the archive.".to_owned());
            }
            if !["on_track", "at_risk", "blocked"].contains(&update.health.as_str()) {
                problems.push("A project update has a health this server does not know.".to_owned());
            }
        }
        limited(check, problems, "projects and tasks");

        // What cannot be resolved here is dropped, and said.
        let artifacts: HashSet<&str> = self.artifacts.iter().map(|artifact| artifact.id.as_str()).collect();
        let cards: HashSet<&str> = self.memory_cards.iter().map(|card| card.id.as_str()).collect();
        let dangling = |references: &[ArchiveReference]| {
            references
                .iter()
                .filter(|reference| match reference.kind.as_str() {
                    "memory_card" => !cards.contains(reference.target_id.as_str()),
                    _ => !artifacts.contains(reference.target_id.as_str()),
                })
                .count()
        };
        let lost: usize = work
            .projects
            .iter()
            .map(|project| dangling(&project.references))
            .sum::<usize>()
            + work
                .task_extras
                .iter()
                .map(|extra| dangling(&extra.references))
                .sum::<usize>();
        if lost > 0 {
            check.warnings.push(format!(
                "{lost} citation(s) of projects and tasks name a file or card that is not in the archive and will be left out."
            ));
        }
        let lost_links = work
            .updates
            .iter()
            .flat_map(|update| update.links.iter())
            .filter(|link| match link.kind.as_str() {
                "milestone" => !milestones.contains_key(link.target_id.as_str()),
                "task" => !tasks.contains(link.target_id.as_str()),
                "memory_card" => !cards.contains(link.target_id.as_str()),
                _ => !artifacts.contains(link.target_id.as_str()),
            })
            .count();
        if lost_links > 0 {
            check.warnings.push(format!(
                "{lost_links} link(s) of project updates name something that is not in the archive and will be left out."
            ));
        }
        let schedules = work
            .recurring
            .iter()
            .filter(|recurring| recurring.status == "active")
            .count();
        if schedules > 0 {
            check.warnings.push(format!(
                "{schedules} recurring task(s) were running. They come back **paused**: nothing is made until someone here resumes each one."
            ));
        }
        if !work.updates.is_empty() {
            check.warnings.push(format!(
                "{} project update(s) come back as a record; the person who wrote each is named in its text.",
                work.updates.len()
            ));
        }
        if !work.task_extras.is_empty() && self.tasks.iter().any(|task| !extras_seen.contains(task.id.as_str())) {
            check.warnings.push(
                "Some tasks have no record of their number or relations; they are numbered after the others."
                    .to_owned(),
            );
        }
    }

    /// The distinct people the records name, for the import report.
    pub fn people(&self) -> Vec<ArchivePerson> {
        let mut people: Vec<ArchivePerson> = Vec::new();
        let mut add = |person: &ArchivePerson| {
            if !people.contains(person) {
                people.push(person.clone());
            }
        };
        for task in &self.tasks {
            add(&task.created_by);
            task.assignee.iter().for_each(&mut add);
            for item in &task.checklist {
                add(&item.created_by);
                item.completed_by.iter().for_each(&mut add);
            }
        }
        self.comments.iter().for_each(|comment| add(&comment.author));
        for project in &self.work.projects {
            project.owner.iter().for_each(&mut add);
            project.created_by.iter().for_each(&mut add);
        }
        for extra in &self.work.task_extras {
            extra.comments.iter().for_each(|comment| add(&comment.author));
        }
        for recurring in &self.work.recurring {
            recurring.assignee.iter().for_each(&mut add);
        }
        for update in &self.work.updates {
            update.author.iter().for_each(&mut add);
        }
        for card in &self.memory_cards {
            card.revisions
                .iter()
                .filter_map(|revision| revision.author.as_ref())
                .for_each(&mut add);
            card.approvals
                .iter()
                .filter_map(|found| found.by.as_ref())
                .for_each(&mut add);
            card.review
                .iter()
                .filter_map(|review| review.reviewer.as_ref())
                .for_each(&mut add);
        }
        for entry in &self.lifecycle {
            entry.owner.iter().for_each(&mut add);
            entry.reviewed_by.iter().for_each(&mut add);
            entry
                .events
                .iter()
                .filter_map(|event| event.actor.as_ref())
                .for_each(&mut add);
        }
        people
    }
}

/// The answer to an import, or to its dry run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveImportReport {
    /// True when nothing was written.
    pub dry_run: bool,
    /// Whether the archive can be imported (always true after an import).
    pub ok: bool,
    pub workspace_name: String,
    pub archive_format_version: u32,
    pub source_version: String,
    pub exported_at: String,
    pub counts: ArchiveCounts,
    pub total_bytes: u64,
    /// People the archive names. Their work is credited to the importer, with
    /// their name in the text, because an archive cannot vouch for who wrote what.
    pub people: Vec<ArchivePerson>,
    pub problems: Vec<String>,
    pub warnings: Vec<String>,
    /// Citations that point at the whole file because the passage could not
    /// be found again after indexing.
    #[serde(default)]
    pub widened_citations: u64,
    /// The new workspace, once imported.
    pub workspace: Option<SharedWorkspace>,
}
