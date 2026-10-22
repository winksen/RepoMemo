//! Git repository sources.
//!
//! A repository is connected once and then kept in step with what it has
//! committed. Each sync lists the tree of the chosen branch, compares it with
//! the files stored for the source (by git blob id, so unchanged files cost
//! nothing), and applies the difference: new files become artifacts, edited
//! and renamed files update their artifact in place, and files that left the
//! tree are kept but dropped from search. Changed files are then indexed in
//! the same job, so a repository's progress is one job instead of one per file.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use repomemo_domain::{
    IndexingJobStatus, RepoAccessCheck, RepoCommit, RepoDetail, RepoFile, RepoOverview, RepoSettings, RepoSkipCount,
    RepoSource, RepoSummary, RepoSyncReport, Source, SourceType, REPO_SYNC_JOB_KIND,
};
use repomemo_git::{GitRepo, LastChange, TreeFile};
use repomemo_indexer::INDEXER_VERSION;
use repomemo_ingestion::repo::{classify_repo_file, is_indexable_text, RepoFileKind, RepoFileRules};
use repomemo_storage::{RepoFileRecord, RepoFileWrite, RepoLastChange, StorageEngine};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::codeowners::{CodeOwners, CODEOWNERS_PATHS, MAX_CODEOWNERS_BYTES};
use crate::repo_overview::{compute_overview, key_file_role, overview_markdown, readme_excerpt, RECENT_COMMITS};
use crate::repo_stack::{detect_stack, MAX_MANIFEST_BYTES, MAX_STACK_MANIFESTS, STACK_MANIFESTS};
use crate::RepoMemoCore;

/// Skipped paths kept per reason in a sync report.
const SKIP_EXAMPLES: usize = 5;
/// How many of the latest commits are read to find who last changed each
/// file. Files untouched for longer than that show no author.
const LAST_CHANGE_COMMITS: usize = 20_000;
/// Job progress is written every this many files, not after each one.
const PROGRESS_EVERY: usize = 20;
const MAX_PATTERNS: usize = 50;

/// The address of a remote repository (V0.2.20). Its token, if it has one, is
/// in the credential store, never here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RemoteLink {
    pub(crate) url: String,
    /// For an `ssh` address: the host's public key the connection is pinned
    /// to, as pasted. Public, so it is kept here and not with the secret.
    #[serde(default)]
    pub(crate) host_key: Option<String>,
}

/// What a repository source keeps in `sources.metadata_json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct RepoSourceMetadata {
    #[serde(default)]
    pub(crate) settings: RepoSettings,
    #[serde(default)]
    pub(crate) last_synced_commit: Option<RepoCommit>,
    #[serde(default)]
    pub(crate) last_synced_at: Option<String>,
    #[serde(default)]
    pub(crate) last_error: Option<String>,
    #[serde(default)]
    pub(crate) last_report: Option<RepoSyncReport>,
    /// What the repository held at the last complete sync.
    #[serde(default)]
    pub(crate) overview: Option<RepoOverview>,
    /// The artifact that stands for the whole repository.
    #[serde(default)]
    pub(crate) overview_artifact_id: Option<String>,
    /// The settings and the indexer version of the last complete sync. A sync whose commit, settings
    /// and indexer version are all the same has nothing to do (`nothing_to_sync`).
    #[serde(default)]
    pub(crate) last_synced_settings: Option<RepoSettings>,
    #[serde(default)]
    pub(crate) last_synced_indexer_version: Option<i64>,
    /// The last AI summary, kept until it is regenerated.
    #[serde(default)]
    pub(crate) summary: Option<RepoSummary>,
    /// Set for a repository fetched from a host.
    #[serde(default)]
    pub(crate) remote: Option<RemoteLink>,
}

impl RepoSourceMetadata {
    pub(crate) fn of(source: &Source) -> Self {
        serde_json::from_value(source.metadata.clone()).unwrap_or_default()
    }

    pub(crate) fn to_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_else(|_| json!({}))
    }
}

/// Where a repository's branch is now, and where it was at the last sync.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoHead {
    pub current: String,
    /// `None` until a sync has completed.
    pub last_synced: Option<String>,
}

impl RepoHead {
    /// The branch points at another commit than the last synced one.
    pub fn moved(&self) -> bool {
        self.last_synced.as_deref() != Some(self.current.as_str())
    }
}

enum Change<'a> {
    Add,
    Update { record: &'a RepoFileRecord },
    Rename { record: &'a RepoFileRecord },
}

impl RepoMemoCore {
    /// Looks at a repository link before it is connected: whether the server
    /// can reach and read it, which repository and commit it resolves to, and
    /// how many of its files the settings would index. Nothing is stored.
    pub async fn check_repo_link(
        &self,
        workspace_id: &str,
        link: &str,
        settings: RepoSettings,
    ) -> Result<RepoAccessCheck> {
        if !self.storage.workspace_exists(workspace_id).await? {
            repomemo_domain::bail_not_found!("Workspace was not found.");
        }
        let settings = normalize_settings(settings)?;
        let repo = self.open_repo_link(link).await?;
        let commit = repo.resolve_commit(settings.branch.as_deref()).await?;
        let info = repo.commit_info(&commit).await?;
        let branch = match settings.branch.clone() {
            Some(branch) => Some(branch),
            None => repo.current_branch().await?,
        };
        let tree = repo.list_files(&commit).await?;
        let rules = RepoFileRules {
            include: settings.include.clone(),
            exclude: settings.exclude.clone(),
        };
        let indexable_files = tree
            .iter()
            .filter(|file| classify_repo_file(&file.path, file.size_bytes, &rules).is_ok())
            .count();
        let root_path = repo.root().to_string_lossy().to_string();
        let already_connected = self
            .storage
            .list_sources_of_type(Some(workspace_id), SourceType::GitRepo)
            .await?
            .iter()
            .any(|source| source.root_uri.as_deref() == Some(root_path.as_str()));
        Ok(RepoAccessCheck {
            name: repo_name(repo.root()),
            root_path,
            remote: false,
            commit: RepoCommit {
                sha: info.sha,
                summary: info.summary,
                author_name: info.author_name,
                committed_at: info.committed_at,
                branch,
            },
            tracked_files: tree.len(),
            indexable_files,
            already_connected,
        })
    }

    /// Connects the repository a link points at to a workspace. The link is a
    /// folder on the server, anywhere inside the repository. Nothing is read
    /// yet; start a sync to bring its files in.
    pub async fn connect_local_repo(
        &self,
        workspace_id: &str,
        link: &str,
        name: Option<String>,
        settings: RepoSettings,
    ) -> Result<RepoSource> {
        if !self.storage.workspace_exists(workspace_id).await? {
            repomemo_domain::bail_not_found!("Workspace was not found.");
        }
        let settings = normalize_settings(settings)?;
        let repo = self.open_repo_link(link).await?;
        // Fail now, not on the first sync, when the branch does not exist.
        repo.resolve_commit(settings.branch.as_deref()).await?;
        let root = repo.root().to_string_lossy().to_string();
        let name = name
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| repo_name(repo.root()));
        let metadata = RepoSourceMetadata {
            settings,
            ..Default::default()
        };
        let source = self
            .storage
            .create_repo_source(workspace_id, &name, &root, &metadata.to_value())
            .await?;
        self.repo_source(&source.id).await
    }

    pub async fn repo_source(&self, source_id: &str) -> Result<RepoSource> {
        let source = self.repo_source_row(source_id).await?;
        let counts = self.storage.repo_source_file_counts(&source.workspace_id).await?;
        self.build_repo_source(source, &counts).await
    }

    pub async fn list_repo_sources(&self, workspace_id: &str) -> Result<Vec<RepoSource>> {
        let counts = self.storage.repo_source_file_counts(workspace_id).await?;
        let mut sources = Vec::new();
        for source in self
            .storage
            .list_sources_of_type(Some(workspace_id), SourceType::GitRepo)
            .await?
        {
            sources.push(self.build_repo_source(source, &counts).await?);
        }
        Ok(sources)
    }

    /// Every repository source of every workspace, for resuming after a
    /// restart.
    pub async fn list_all_repo_source_ids(&self) -> Result<Vec<String>> {
        Ok(self
            .storage
            .list_sources_of_type(None, SourceType::GitRepo)
            .await?
            .into_iter()
            .map(|source| source.id)
            .collect())
    }

    /// Changes the name and what is indexed. The new settings apply from the
    /// next sync.
    pub async fn update_repo_source(
        &self,
        source_id: &str,
        name: Option<String>,
        settings: RepoSettings,
    ) -> Result<RepoSource> {
        let source = self.repo_source_row(source_id).await?;
        let settings = normalize_settings(settings)?;
        let current = RepoSourceMetadata::of(&source);
        if settings.branch != current.settings.branch {
            match &current.remote {
                Some(remote) => {
                    let target = self.remote_target(source_id, remote).await?;
                    repomemo_git::remote::remote_head(&target, settings.branch.as_deref()).await?;
                }
                None => {
                    let repo = self
                        .open_linked_repo(Path::new(source.root_uri.as_deref().unwrap_or_default()))
                        .await?;
                    repo.resolve_commit(settings.branch.as_deref()).await?;
                }
            }
        }
        let name = name
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or(source.name.clone());
        let mut metadata = RepoSourceMetadata::of(&source);
        metadata.settings = settings;
        self.storage
            .update_source(source_id, &name, &source.status, &metadata.to_value(), false)
            .await?;
        if let Some(artifact_id) = &metadata.overview_artifact_id {
            let _ = self.storage.update_artifact_title(artifact_id, &name).await;
        }
        self.repo_source(source_id).await
    }

    /// The repository with its overview and last summary, for its page.
    pub async fn repo_detail(&self, source_id: &str) -> Result<RepoDetail> {
        let source = self.repo_source_row(source_id).await?;
        let metadata = RepoSourceMetadata::of(&source);
        Ok(RepoDetail {
            repository: self.repo_source(source_id).await?,
            overview: metadata.overview,
            summary: metadata.summary,
        })
    }

    pub(crate) async fn store_repo_summary(&self, source_id: &str, summary: RepoSummary) -> Result<()> {
        let source = self.repo_source_row(source_id).await?;
        let mut metadata = RepoSourceMetadata::of(&source);
        metadata.summary = Some(summary);
        self.storage
            .update_source(source_id, &source.name, &source.status, &metadata.to_value(), false)
            .await?;
        Ok(())
    }

    /// The summary as stored now, so a sync that read the metadata earlier
    /// does not drop a summary generated while it ran.
    async fn latest_repo_summary(&self, source_id: &str) -> Result<Option<RepoSummary>> {
        Ok(RepoSourceMetadata::of(&self.storage.get_source(source_id).await?).summary)
    }

    /// Disconnects a repository and deletes every file stored from it.
    pub async fn remove_repo_source(&self, source_id: &str) -> Result<()> {
        self.repo_source_row(source_id).await?;
        self.storage.delete_source(source_id).await?;
        // The server's copy of a remote repository goes with its link.
        let _ = tokio::fs::remove_dir_all(self.remote_clone_dir(source_id)).await;
        Ok(())
    }

    pub async fn list_repo_files(&self, source_id: &str) -> Result<Vec<RepoFile>> {
        self.repo_source_row(source_id).await?;
        self.storage.list_repo_file_entries(source_id).await
    }

    /// The commit the repository's branch points at now, next to the one last
    /// synced. Cheap (one `git rev-parse`), so it can be polled to sync
    /// automatically when the branch moves.
    pub async fn repo_head(&self, source_id: &str) -> Result<RepoHead> {
        let source = self.repo_source_row(source_id).await?;
        let metadata = RepoSourceMetadata::of(&source);
        if let Some(remote) = &metadata.remote {
            // One question to the host, no fetch: where the branch is now.
            let target = self.remote_target(source_id, remote).await?;
            let head = repomemo_git::remote::remote_head(&target, metadata.settings.branch.as_deref()).await?;
            return Ok(RepoHead {
                current: head.commit,
                last_synced: metadata.last_synced_commit.map(|commit| commit.sha),
            });
        }
        let repo = self
            .open_linked_repo(Path::new(source.root_uri.as_deref().unwrap_or_default()))
            .await?;
        let current = repo.resolve_commit(metadata.settings.branch.as_deref()).await?;
        Ok(RepoHead {
            current,
            last_synced: metadata.last_synced_commit.map(|commit| commit.sha),
        })
    }

    /// Opens the repository behind a new link, explaining in plain words why
    /// the server cannot read it when it cannot.
    async fn open_repo_link(&self, link: &str) -> Result<GitRepo> {
        // "Copy as path" in Windows Explorer wraps the path in quotes.
        let link = link.trim().trim_matches('"').trim();
        if link.is_empty() {
            repomemo_domain::bail_invalid!("Enter the folder of a git repository on the server.");
        }
        let lower = link.to_ascii_lowercase();
        let is_url = ["http://", "https://", "ssh://", "git://", "git@"]
            .iter()
            .any(|prefix| lower.starts_with(prefix));
        if is_url {
            repomemo_domain::bail_invalid!(
                "This is the address of a remote repository. Enter the folder of a checkout on the server, such as C:\\code\\my-repo or /srv/code/my-repo."
            );
        }
        let path = PathBuf::from(link.strip_prefix("file://").unwrap_or(link));
        // Before anything is read, so folders outside the allowed roots and
        // network shares are never even probed.
        self.repo_policy.check(&path)?;
        if !path.is_dir() {
            repomemo_domain::bail_invalid!(
                "The folder {} does not exist on the server, or the server cannot see it.",
                path.display()
            );
        }
        if let Err(error) = std::fs::read_dir(&path) {
            if error.kind() == std::io::ErrorKind::PermissionDenied {
                repomemo_domain::bail_invalid!("The server has no read access to {}.", path.display());
            }
            repomemo_domain::bail_invalid!("The folder {} cannot be read: {error}", path.display());
        }
        let repo = GitRepo::open(&path).await?;
        // The repository may start above the linked folder.
        self.repo_policy.check(repo.root())?;
        Ok(repo)
    }

    /// Opens a repository that is already linked, re-checking the link policy
    /// so a narrowed policy also stops syncs of older links.
    pub(crate) async fn open_linked_repo(&self, root: &Path) -> Result<GitRepo> {
        self.repo_policy.check(root)?;
        let repo = GitRepo::open(root).await?;
        self.repo_policy.check(repo.root())?;
        Ok(repo)
    }

    /// Records a queued sync. Run it with [`RepoMemoCore::run_repo_sync`].
    pub async fn begin_repo_sync(&self, source_id: &str) -> Result<IndexingJobStatus> {
        let source = self.repo_source_row(source_id).await?;
        let job = self
            .storage
            .create_job(
                &source.workspace_id,
                Some(source_id),
                REPO_SYNC_JOB_KIND,
                "queued",
                None,
            )
            .await?;
        self.storage.set_source_status(source_id, "syncing").await?;
        Ok(job)
    }

    /// Brings the source in step with its repository and indexes what
    /// changed, reporting progress on `job_id`. The outcome is also stored on
    /// the source, so it can be shown later.
    pub async fn run_repo_sync(&self, source_id: &str, job_id: &str) -> Result<RepoSyncReport> {
        match self.sync_repo(source_id, job_id).await {
            Ok(report) => {
                let (status, stage) = if report.cancelled {
                    ("cancelled", "cancelled")
                } else {
                    ("completed", "synced")
                };
                let job = self.storage.get_job(job_id).await?;
                let progress = job.map(|job| job.progress_current).unwrap_or_default();
                self.storage
                    .update_indexing_job(job_id, status, stage, progress, None)
                    .await?;
                Ok(report)
            }
            Err(error) => {
                let message = format!("{error:#}");
                if let Ok(source) = self.storage.get_source(source_id).await {
                    let mut metadata = RepoSourceMetadata::of(&source);
                    metadata.last_error = Some(message.clone());
                    let _ = self
                        .storage
                        .update_source(source_id, &source.name, "error", &metadata.to_value(), false)
                        .await;
                }
                let _ = self
                    .storage
                    .update_indexing_job(job_id, "failed", "failed", 0, Some(&message))
                    .await;
                Err(error)
            }
        }
    }

    /// The report of a sync that has nothing to do, or `None` when it has some. A sync has nothing
    /// to do when the commit, the include and exclude rules and the indexer version are those of the
    /// last complete sync (which left no file failed and was not stopped), and no file of the
    /// repository, nor the item standing for it, is waiting to be indexed.
    async fn nothing_to_sync(
        &self,
        source_id: &str,
        metadata: &RepoSourceMetadata,
        commit: &str,
    ) -> Result<Option<RepoSyncReport>> {
        let Some(previous) = metadata.last_report.as_ref() else {
            return Ok(None);
        };
        let same = metadata
            .last_synced_commit
            .as_ref()
            .is_some_and(|synced| synced.sha == commit)
            && metadata.last_synced_settings.as_ref() == Some(&metadata.settings)
            && metadata.last_synced_indexer_version == Some(INDEXER_VERSION)
            && !previous.cancelled
            && previous.index_failed == 0;
        if !same {
            return Ok(None);
        }
        if !self
            .storage
            .list_repo_artifacts_needing_index(source_id, INDEXER_VERSION)
            .await?
            .is_empty()
        {
            return Ok(None);
        }
        if let Some(artifact_id) = metadata.overview_artifact_id.as_deref() {
            if self
                .storage
                .get_artifact_summary(artifact_id)
                .await?
                .indexed_at
                .is_none()
            {
                return Ok(None);
            }
        }
        Ok(Some(RepoSyncReport {
            commit_sha: commit.to_owned(),
            files_in_tree: previous.files_in_tree,
            unchanged: previous.files_in_tree.saturating_sub(previous.skipped),
            skipped: previous.skipped,
            skipped_by_reason: previous.skipped_by_reason.clone(),
            nothing_changed: true,
            ..Default::default()
        }))
    }

    /// Who last changed each file at `commit`. After an earlier sync only the commits since it are
    /// read (`git log previous..commit`) and laid over the authors stored with the files; with no
    /// earlier sync, a history that no longer holds it, or too many new commits for the window, the
    /// full pass over the latest `LAST_CHANGE_COMMITS` commits decides.
    async fn last_changes_for_sync(
        &self,
        repo: &GitRepo,
        source_id: &str,
        metadata: &RepoSourceMetadata,
        commit: &str,
    ) -> Result<HashMap<String, LastChange>> {
        if let Some(previous) = metadata.last_synced_commit.as_ref() {
            if let Some(delta) = repo
                .last_changes_since(&previous.sha, commit, LAST_CHANGE_COMMITS)
                .await
                .unwrap_or(None)
            {
                let mut merged = self
                    .storage
                    .list_repo_file_entries(source_id)
                    .await?
                    .into_iter()
                    .filter_map(|file| {
                        Some((
                            file.path,
                            LastChange {
                                sha: file.last_change_commit?,
                                author_name: file.last_author?,
                                authored_at: file.last_changed_at?,
                            },
                        ))
                    })
                    .collect::<HashMap<_, _>>();
                merged.extend(delta);
                return Ok(merged);
            }
        }
        repo.last_changes(commit, LAST_CHANGE_COMMITS).await
    }

    async fn sync_repo(&self, source_id: &str, job_id: &str) -> Result<RepoSyncReport> {
        let source = self.repo_source_row(source_id).await?;
        let mut metadata = RepoSourceMetadata::of(&source);
        let root = source.root_uri.clone().unwrap_or_default();
        self.storage
            .update_indexing_job(job_id, "running", "reading_repository", 0, None)
            .await?;

        let repo = match &metadata.remote {
            Some(remote) => {
                self.storage
                    .update_indexing_job(job_id, "running", "fetching_repository", 0, None)
                    .await?;
                self.refresh_remote_clone(&source, remote, metadata.settings.branch.as_deref())
                    .await
                    .with_context(|| format!("the repository at {root} could not be fetched"))?
            }
            None => self
                .open_linked_repo(Path::new(&root))
                .await
                .with_context(|| format!("the repository at {root} could not be opened"))?,
        };
        let branch = metadata.settings.branch.clone();
        let commit = repo.resolve_commit(branch.as_deref()).await?;
        // The same commit, rules and indexer as the last complete sync, and nothing waiting to be
        // indexed: there is nothing to list, compare, read or write.
        if let Some(report) = self.nothing_to_sync(source_id, &metadata, &commit).await? {
            metadata.last_synced_at = Some(chrono_now());
            metadata.last_report = Some(report.clone());
            metadata.last_error = None;
            self.storage
                .update_source(source_id, &source.name, "ready", &metadata.to_value(), true)
                .await?;
            return Ok(report);
        }
        let info = repo.commit_info(&commit).await?;
        let branch = match branch {
            Some(branch) => Some(branch),
            None => repo.current_branch().await?,
        };
        let tree = repo.list_files(&commit).await?;
        // Who last changed each file: the commits since the previous sync, merged into what is
        // stored, or one pass over the whole history when there is no earlier sync or its commit is
        // not behind this one (a rewritten history). Without either, the files are indexed all the
        // same, only without an author.
        let last_changes = match self.last_changes_for_sync(&repo, source_id, &metadata, &commit).await {
            Ok(changes) => changes,
            Err(error) => {
                tracing::warn!(repository = %source_id, error = %format!("{error:#}"), "Who last changed each file could not be read");
                Default::default()
            }
        };
        let code_owners = read_code_owners(&repo, &tree).await;
        let rules = RepoFileRules {
            include: metadata.settings.include.clone(),
            exclude: metadata.settings.exclude.clone(),
        };

        let mut report = RepoSyncReport {
            commit_sha: commit.clone(),
            files_in_tree: tree.len(),
            ..Default::default()
        };
        let mut skips = SkipTally::default();
        let mut wanted = Vec::new();
        for file in tree {
            match classify_repo_file(&file.path, file.size_bytes, &rules) {
                Ok(kind) => wanted.push((file, kind)),
                Err(reason) => skips.add(reason, &file.path),
            }
        }

        // Compare the tree with what is stored. A stored file whose path is
        // gone but whose exact content shows up under a new path was renamed.
        let records = self.storage.list_repo_files(source_id).await?;
        let by_path = records
            .iter()
            .map(|record| (record.path.as_str(), record))
            .collect::<HashMap<_, _>>();
        let wanted_paths = wanted
            .iter()
            .map(|(file, _)| file.path.as_str())
            .collect::<HashSet<_>>();
        let mut vanished: HashMap<&str, Vec<&RepoFileRecord>> = HashMap::new();
        for record in &records {
            if !record.removed && !wanted_paths.contains(record.path.as_str()) {
                vanished.entry(record.blob_sha.as_str()).or_default().push(record);
            }
        }
        // A file renamed and edited in the same commit has a new blob id, so
        // only git's similarity detection can pair it with its old path.
        let renames_with_edits: HashMap<String, String> = match metadata.last_synced_commit.as_ref() {
            Some(previous) if previous.sha != commit => repo
                .renamed_files(&previous.sha, &commit)
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|(old, new)| (new, old))
                .collect(),
            _ => HashMap::new(),
        };
        let mut changes = Vec::new();
        for (file, kind) in &wanted {
            match by_path.get(file.path.as_str()) {
                Some(record) if !record.removed && record.blob_sha == file.blob_sha => {
                    report.unchanged += 1;
                }
                Some(record) => changes.push((file, kind, Change::Update { record })),
                None => {
                    let mut renamed_from = vanished
                        .get_mut(file.blob_sha.as_str())
                        .and_then(|records| records.pop());
                    if renamed_from.is_none() {
                        let old_path = renames_with_edits.get(file.path.as_str());
                        let candidate = old_path
                            .and_then(|old| by_path.get(old.as_str()))
                            .filter(|record| !record.removed && !wanted_paths.contains(record.path.as_str()));
                        if let Some(record) = candidate {
                            // It is renamed, not removed.
                            if let Some(group) = vanished.get_mut(record.blob_sha.as_str()) {
                                group.retain(|other| other.path != record.path);
                            }
                            renamed_from = Some(*record);
                        }
                    }
                    changes.push((
                        file,
                        kind,
                        match renamed_from {
                            Some(record) => Change::Rename { record },
                            None => Change::Add,
                        },
                    ));
                }
            }
        }
        let mut removals = vanished
            .into_values()
            .flatten()
            .map(|record| record.path.clone())
            .collect::<Vec<_>>();

        let mut progress = 0_usize;
        let planned = changes.len() + removals.len();
        self.storage.set_job_progress_total(job_id, planned as i64).await?;
        self.storage
            .update_indexing_job(job_id, "running", "storing_files", 0, None)
            .await?;

        let mut reader = repo.blob_reader().await?;
        for (file, kind, change) in changes {
            if progress % PROGRESS_EVERY == 0 && self.storage.is_job_cancel_requested(job_id).await? {
                report.cancelled = true;
                break;
            }
            let bytes = reader
                .read(&file.blob_sha)
                .await
                .with_context(|| format!("failed to read {} from git", file.path))?;
            if kind.needs_text_check && !is_indexable_text(&file.path, &bytes) {
                skips.add("binary or not UTF-8 text".to_owned(), &file.path);
                // A file that turned binary leaves the index like a deleted one.
                if let Change::Update { record } | Change::Rename { record } = change {
                    if !record.removed {
                        removals.push(record.path.clone());
                    }
                }
                progress += 1;
                continue;
            }
            let write = self
                .store_repo_blob(
                    &root,
                    &commit,
                    file,
                    kind,
                    &bytes,
                    last_changes.get(&file.path),
                    &code_owners.owners_of(&file.path),
                )
                .await?;
            match change {
                Change::Add => {
                    self.storage
                        .add_repo_file(&source.workspace_id, source_id, write)
                        .await?;
                    report.added += 1;
                }
                Change::Update { record } => {
                    self.storage
                        .update_repo_file(
                            source_id,
                            &record.artifact_id,
                            &record.path,
                            record.removed,
                            false,
                            write,
                        )
                        .await?;
                    if record.removed {
                        report.restored += 1;
                    } else {
                        report.updated += 1;
                    }
                }
                Change::Rename { record } => {
                    self.storage
                        .update_repo_file(source_id, &record.artifact_id, &record.path, false, true, write)
                        .await?;
                    report.renamed += 1;
                }
            }
            progress += 1;
            if progress % PROGRESS_EVERY == 0 {
                self.storage
                    .update_indexing_job(job_id, "running", "storing_files", progress as i64, None)
                    .await?;
            }
        }
        drop(reader);

        if !report.cancelled {
            for path in &removals {
                self.storage.remove_repo_file(source_id, path, &commit).await?;
                report.removed += 1;
                progress += 1;
            }
        }
        // Who last changed each tracked file (a revert changes the author
        // without changing the content, so every file is looked at).
        let last_rows = wanted
            .iter()
            .filter_map(|(file, _)| {
                last_changes.get(&file.path).map(|change| RepoLastChange {
                    path: file.path.clone(),
                    author_name: change.author_name.clone(),
                    authored_at: change.authored_at.clone(),
                    commit_sha: change.sha.clone(),
                })
            })
            .collect::<Vec<_>>();
        self.storage.set_repo_file_last_changes(source_id, &last_rows).await?;
        // The owners `CODEOWNERS` names, as hints (nothing is assigned).
        if code_owners.is_empty() {
            self.storage.clear_repo_file_code_owners(source_id).await?;
        } else {
            let owner_rows = wanted
                .iter()
                .map(|(file, _)| (file.path.clone(), code_owners.owners_of(&file.path)))
                .collect::<Vec<_>>();
            self.storage.set_repo_file_code_owners(source_id, &owner_rows).await?;
        }

        // Items that name these files (or their commits) may be older than
        // the files: look again at what the workspace's trackers mention.
        if !report.cancelled {
            if let Err(error) = self.refresh_workspace_mentions(&source.workspace_id).await {
                tracing::warn!(repository = %source_id, error = %format!("{error:#}"), "Mentions could not be refreshed");
            }
        }

        report.skipped = skips.total;
        report.skipped_by_reason = skips.into_counts();

        // The tree is stored; remember the commit before the slower indexing,
        // so an interrupted index pass does not repeat the file work.
        if !report.cancelled {
            let synced_commit = RepoCommit {
                sha: info.sha,
                summary: info.summary,
                author_name: info.author_name,
                committed_at: info.committed_at,
                branch,
            };
            let recent_commits = repo
                .recent_commits(&commit, RECENT_COMMITS)
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|commit| RepoCommit {
                    sha: commit.sha,
                    summary: commit.summary,
                    author_name: commit.author_name,
                    committed_at: commit.committed_at,
                    branch: None,
                })
                .collect();
            let (overview, overview_artifact_id) = self
                .write_repo_overview(
                    &source,
                    synced_commit.clone(),
                    recent_commits,
                    metadata.overview_artifact_id.as_deref(),
                )
                .await?;
            metadata.last_synced_commit = Some(synced_commit);
            metadata.last_synced_settings = Some(metadata.settings.clone());
            metadata.last_synced_indexer_version = Some(INDEXER_VERSION);
            metadata.last_synced_at = Some(chrono_now());
            metadata.overview = Some(overview);
            metadata.overview_artifact_id = Some(overview_artifact_id);
        }
        metadata.last_report = Some(report.clone());
        metadata.summary = self.latest_repo_summary(source_id).await?;
        let source = self
            .storage
            .update_source(source_id, &source.name, "syncing", &metadata.to_value(), false)
            .await?;

        if !report.cancelled {
            let pending = self
                .storage
                .list_repo_artifacts_needing_index(source_id, INDEXER_VERSION)
                .await?;
            self.storage
                .set_job_progress_total(job_id, (progress + pending.len()) as i64)
                .await?;
            self.storage
                .update_indexing_job(job_id, "running", "indexing", progress as i64, None)
                .await?;
            for (position, artifact) in pending.iter().enumerate() {
                if position % PROGRESS_EVERY == 0 && self.storage.is_job_cancel_requested(job_id).await? {
                    report.cancelled = true;
                    break;
                }
                match self.index_artifact_inner(artifact).await {
                    Ok(_) => {
                        let _ = self.storage.clear_index_failure(&artifact.id).await;
                        report.indexed += 1;
                    }
                    Err(error) => {
                        self.storage
                            .record_index_failure(&artifact.id, &format!("{error:#}"), 1)
                            .await?;
                        report.index_failed += 1;
                    }
                }
                progress += 1;
                if progress % PROGRESS_EVERY == 0 {
                    self.storage
                        .update_indexing_job(job_id, "running", "indexing", progress as i64, None)
                        .await?;
                }
            }
            // The item that stands for the repository is indexed last, once
            // its files are.
            if let (false, Some(artifact_id)) = (report.cancelled, &metadata.overview_artifact_id) {
                let artifact = self.storage.get_artifact_summary(artifact_id).await?;
                if artifact.indexed_at.is_none() {
                    match self.index_artifact_inner(&artifact).await {
                        Ok(_) => {
                            let _ = self.storage.clear_index_failure(artifact_id).await;
                        }
                        Err(error) => {
                            self.storage
                                .record_index_failure(artifact_id, &format!("{error:#}"), 1)
                                .await?;
                        }
                    }
                }
            }
            self.storage
                .update_indexing_job(job_id, "running", "indexing", progress as i64, None)
                .await?;
        }

        metadata.last_report = Some(report.clone());
        metadata.last_error = None;
        metadata.summary = self.latest_repo_summary(source_id).await?;
        self.storage
            .update_source(
                source_id,
                &source.name,
                "ready",
                &metadata.to_value(),
                !report.cancelled,
            )
            .await?;
        Ok(report)
    }

    /// Computes the repository's overview from its stored files and writes
    /// it as the content of the artifact that stands for the repository.
    async fn write_repo_overview(
        &self,
        source: &Source,
        commit: RepoCommit,
        recent_commits: Vec<RepoCommit>,
        existing_artifact_id: Option<&str>,
    ) -> Result<(RepoOverview, String)> {
        let files = self.storage.list_repo_file_entries(&source.id).await?;
        let readme = files
            .iter()
            .filter(|file| key_file_role(&file.path) == Some("readme"))
            .min_by_key(|file| !file.path.to_ascii_lowercase().ends_with(".md"));
        let excerpt = match readme {
            Some(file) => {
                let bytes = self.storage.read_artifact_blob(&file.artifact_id).await?;
                readme_excerpt(&String::from_utf8_lossy(&bytes))
            }
            None => None,
        };
        let mut manifests = std::collections::BTreeMap::new();
        let manifest_files = files.iter().filter(|file| {
            let name = file.path.rsplit('/').next().unwrap_or(&file.path);
            file.path.matches('/').count() <= 2
                && STACK_MANIFESTS.contains(&name)
                && file.size_bytes <= MAX_MANIFEST_BYTES
        });
        for file in manifest_files.take(MAX_STACK_MANIFESTS) {
            let bytes = self.storage.read_artifact_blob(&file.artifact_id).await?;
            manifests.insert(file.path.clone(), String::from_utf8_lossy(&bytes).into_owned());
        }
        let stack = detect_stack(&files, &manifests);
        let overview = compute_overview(commit, &files, excerpt, recent_commits, stack, chrono_now());
        let root = source.root_uri.clone().unwrap_or_default();
        let content = overview_markdown(&source.name, &root, &overview).into_bytes();
        let content_hash = StorageEngine::content_hash(&content);
        self.storage
            .store_blob(&content_hash, &content, Some("text/markdown"))
            .await?;
        let artifact_id = self
            .storage
            .write_repo_overview_artifact(
                &source.workspace_id,
                &source.id,
                existing_artifact_id,
                &source.name,
                &root,
                &content_hash,
                content.len() as i64,
                &json!({
                    "origin": "git_repo_overview",
                    "repo_root": root,
                    "commit": overview.commit.sha,
                }),
            )
            .await?;
        Ok((overview, artifact_id))
    }

    async fn store_repo_blob(
        &self,
        root: &str,
        commit: &str,
        file: &TreeFile,
        kind: &RepoFileKind,
        bytes: &[u8],
        last: Option<&LastChange>,
        owners: &[String],
    ) -> Result<RepoFileWrite> {
        let content_hash = StorageEngine::content_hash(bytes);
        let mut metadata = json!({
            "origin": "git_repo",
            "repo_root": root,
            "commit": commit,
            "blob_sha": file.blob_sha,
        });
        if !owners.is_empty() {
            metadata["code_owners"] = json!(owners);
        }
        if let Some(last) = last {
            metadata["last_author"] = json!(last.author_name);
            metadata["last_changed_at"] = json!(last.authored_at);
            metadata["last_change_commit"] = json!(last.sha);
        }
        self.storage
            .store_blob(&content_hash, bytes, kind.mime_type.as_deref())
            .await?;
        Ok(RepoFileWrite {
            title: file.path.rsplit('/').next().unwrap_or(&file.path).to_owned(),
            path: file.path.clone(),
            artifact_type: kind.artifact_type.clone(),
            language: kind.language.clone(),
            mime_type: kind.mime_type.clone(),
            content_hash,
            size_bytes: bytes.len() as i64,
            blob_sha: file.blob_sha.clone(),
            commit_sha: commit.to_owned(),
            metadata,
        })
    }

    pub(crate) async fn repo_source_row(&self, source_id: &str) -> Result<Source> {
        let source = self.storage.get_source(source_id).await?;
        if !matches!(source.source_type, SourceType::GitRepo) {
            repomemo_domain::bail_not_found!("Source was not found.");
        }
        Ok(source)
    }

    async fn build_repo_source(&self, source: Source, counts: &HashMap<String, (i64, i64)>) -> Result<RepoSource> {
        let metadata = RepoSourceMetadata::of(&source);
        let (file_count, indexed_file_count) = counts.get(&source.id).copied().unwrap_or_default();
        let active_job = self
            .storage
            .latest_source_job(&source.id, REPO_SYNC_JOB_KIND)
            .await?
            .filter(|job| job.status == "running");
        let remote = metadata.remote.is_some();
        let has_credential = remote
            && self
                .storage
                .repo_credential_info(&source.id)
                .await
                .ok()
                .flatten()
                .is_some();
        let webhook = self.storage.repo_webhook_info(&source.id).await.ok().flatten();
        Ok(RepoSource {
            id: source.id,
            workspace_id: source.workspace_id,
            name: source.name,
            root_path: source.root_uri.unwrap_or_default(),
            remote,
            has_credential,
            has_webhook: webhook.is_some(),
            webhook_last_received_at: webhook.and_then(|webhook| webhook.last_received_at),
            settings: metadata.settings,
            status: source.status,
            last_synced_commit: metadata.last_synced_commit,
            last_synced_at: metadata.last_synced_at,
            last_error: metadata.last_error,
            last_report: metadata.last_report,
            file_count,
            indexed_file_count,
            active_job,
            overview_artifact_id: metadata.overview_artifact_id,
            created_at: source.created_at,
        })
    }
}

#[derive(Default)]
struct SkipTally {
    total: usize,
    by_reason: BTreeMap<String, RepoSkipCount>,
}

impl SkipTally {
    fn add(&mut self, reason: String, path: &str) {
        self.total += 1;
        let entry = self.by_reason.entry(reason.clone()).or_insert_with(|| RepoSkipCount {
            reason,
            ..Default::default()
        });
        entry.count += 1;
        if entry.examples.len() < SKIP_EXAMPLES {
            entry.examples.push(path.to_owned());
        }
    }

    fn into_counts(self) -> Vec<RepoSkipCount> {
        let mut counts = self.by_reason.into_values().collect::<Vec<_>>();
        counts.sort_by_key(|skip| std::cmp::Reverse(skip.count));
        counts
    }
}

/// The repository's `CODEOWNERS`, if it has one a sync can read.
async fn read_code_owners(repo: &GitRepo, tree: &[TreeFile]) -> CodeOwners {
    for path in CODEOWNERS_PATHS {
        let Some(file) = tree.iter().find(|file| file.path == *path) else {
            continue;
        };
        if file.size_bytes > MAX_CODEOWNERS_BYTES {
            continue;
        }
        let Ok(mut reader) = repo.blob_reader().await else {
            return CodeOwners::default();
        };
        return match reader.read(&file.blob_sha).await {
            Ok(bytes) => CodeOwners::parse(&String::from_utf8_lossy(&bytes)),
            Err(_) => CodeOwners::default(),
        };
    }
    CodeOwners::default()
}

fn repo_name(root: &Path) -> String {
    root.file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("Repository")
        .to_owned()
}

pub(crate) fn normalize_settings(settings: RepoSettings) -> Result<RepoSettings> {
    let clean = |patterns: Vec<String>| -> Result<Vec<String>> {
        let patterns = patterns
            .into_iter()
            .map(|pattern| pattern.trim().replace('\\', "/"))
            .filter(|pattern| !pattern.is_empty() && !pattern.starts_with('#'))
            .collect::<Vec<_>>();
        if patterns.len() > MAX_PATTERNS {
            repomemo_domain::bail_invalid!("Use at most {MAX_PATTERNS} include and {MAX_PATTERNS} exclude patterns.");
        }
        Ok(patterns)
    };
    Ok(RepoSettings {
        branch: settings
            .branch
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty()),
        include: clean(settings.include)?,
        exclude: clean(settings.exclude)?,
    })
}

fn chrono_now() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "repomemo-{label}-{}",
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ))
    }

    fn git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=Test", "-c", "user.email=test@example.com"])
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    }

    fn write(dir: &Path, path: &str, content: &str) {
        let path = dir.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    async fn sync(core: &RepoMemoCore, source_id: &str) -> RepoSyncReport {
        let job = core.begin_repo_sync(source_id).await.unwrap();
        core.run_repo_sync(source_id, &job.id).await.unwrap()
    }

    /// V0.2.157: after a sync, the last authors come from the commits since
    /// it, merged with what was stored; a rewritten history is read in full.
    #[tokio::test]
    async fn the_last_authors_after_a_new_commit_keep_what_was_stored() {
        let repo_dir = unique_dir("repo-authors-repo");
        let data_dir = unique_dir("repo-authors-data");
        std::fs::create_dir_all(&repo_dir).unwrap();
        git(&repo_dir, &["init", "--quiet", "--initial-branch=main"]);
        write(&repo_dir, "a.md", "a\n");
        write(&repo_dir, "b.md", "b\n");
        git(&repo_dir, &["add", "."]);
        git(&repo_dir, &["commit", "--quiet", "-m", "first"]);
        let first_commit = String::from_utf8(
            Command::new("git")
                .arg("-C")
                .arg(&repo_dir)
                .args(["rev-parse", "HEAD"])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap()
        .trim()
        .to_owned();

        let core = RepoMemoCore::boot(data_dir.clone()).await.unwrap();
        let workspace = core.create_workspace("Authors".to_owned()).await.unwrap();
        let source = core
            .connect_local_repo(
                &workspace.id,
                &repo_dir.to_string_lossy(),
                None,
                RepoSettings::default(),
            )
            .await
            .unwrap();
        sync(&core, &source.id).await;
        let files = core.list_repo_files(&source.id).await.unwrap();
        assert!(files
            .iter()
            .all(|file| file.last_change_commit.as_deref() == Some(first_commit.as_str())));

        // A commit that changes one file: that file has the new commit, the other keeps the stored one.
        write(&repo_dir, "a.md", "a changed\n");
        git(&repo_dir, &["commit", "--quiet", "-am", "second"]);
        let report = sync(&core, &source.id).await;
        assert_eq!(report.updated, 1);
        let files = core.list_repo_files(&source.id).await.unwrap();
        let a = files.iter().find(|file| file.path == "a.md").unwrap();
        let b = files.iter().find(|file| file.path == "b.md").unwrap();
        assert_eq!(a.last_change_commit.as_deref(), Some(report.commit_sha.as_str()));
        assert_eq!(b.last_change_commit.as_deref(), Some(first_commit.as_str()));
        assert_eq!(b.last_author.as_deref(), Some("Test"));

        // A rewritten history is read in full: the files take the commits it now names.
        git(&repo_dir, &["checkout", "--quiet", "--orphan", "fresh"]);
        git(&repo_dir, &["add", "."]);
        git(&repo_dir, &["commit", "--quiet", "-m", "squashed"]);
        git(&repo_dir, &["branch", "--quiet", "-M", "main"]);
        let squashed = sync(&core, &source.id).await;
        let files = core.list_repo_files(&source.id).await.unwrap();
        assert!(files
            .iter()
            .all(|file| file.last_change_commit.as_deref() == Some(squashed.commit_sha.as_str())));

        drop(core);
        let _ = std::fs::remove_dir_all(data_dir);
        let _ = std::fs::remove_dir_all(repo_dir);
    }

    /// V0.2.156: a sync whose commit, rules and indexer are those of the last
    /// complete one stops after resolving the commit.
    #[tokio::test]
    async fn a_sync_with_nothing_new_stops_after_the_commit_is_resolved() {
        let repo_dir = unique_dir("repo-nothing-new-repo");
        let data_dir = unique_dir("repo-nothing-new-data");
        std::fs::create_dir_all(&repo_dir).unwrap();
        git(&repo_dir, &["init", "--quiet", "--initial-branch=main"]);
        write(&repo_dir, "README.md", "# Payments\n\nHow refunds work.\n");
        write(
            &repo_dir,
            "src/refund.rs",
            "pub fn issue_refund(amount: u64) -> u64 {\n    amount\n}\n",
        );
        write(&repo_dir, "notes.txt", "kept out by a rule later");
        git(&repo_dir, &["add", "."]);
        git(&repo_dir, &["commit", "--quiet", "-m", "Initial"]);

        let core = RepoMemoCore::boot(data_dir.clone()).await.unwrap();
        let workspace = core.create_workspace("Repo".to_owned()).await.unwrap();
        let source = core
            .connect_local_repo(
                &workspace.id,
                &repo_dir.to_string_lossy(),
                None,
                RepoSettings::default(),
            )
            .await
            .unwrap();

        let first = sync(&core, &source.id).await;
        assert!(!first.nothing_changed);
        assert_eq!(first.added, 3);

        // The same commit, rules and indexer: nothing is read, written or indexed.
        let second = sync(&core, &source.id).await;
        assert!(second.nothing_changed);
        assert_eq!(
            (second.added, second.updated, second.removed, second.indexed),
            (0, 0, 0, 0)
        );
        assert_eq!(second.files_in_tree, first.files_in_tree);
        assert_eq!(second.commit_sha, first.commit_sha);
        assert!(!core.repo_head(&source.id).await.unwrap().moved());
        assert_eq!(core.list_repo_files(&source.id).await.unwrap().len(), 3);

        // New rules are a change, though the commit is the same.
        core.update_repo_source(
            &source.id,
            None,
            RepoSettings {
                exclude: vec!["notes.txt".to_owned()],
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let third = sync(&core, &source.id).await;
        assert!(!third.nothing_changed, "a changed rule is worked through");
        assert_eq!(third.removed, 1);
        assert!(
            sync(&core, &source.id).await.nothing_changed,
            "and then there is nothing again"
        );

        // A new commit is a change.
        write(
            &repo_dir,
            "src/refund.rs",
            "pub fn issue_refund(amount: u64) -> u64 {\n    amount + 1\n}\n",
        );
        git(&repo_dir, &["commit", "--quiet", "-am", "Edit"]);
        let fifth = sync(&core, &source.id).await;
        assert!(!fifth.nothing_changed);
        assert_eq!(fifth.updated, 1);

        drop(core);
        let _ = std::fs::remove_dir_all(data_dir);
        let _ = std::fs::remove_dir_all(repo_dir);
    }

    #[tokio::test]
    async fn syncs_follow_commits_and_keep_artifact_identity() {
        let repo_dir = unique_dir("repo-sync-repo");
        let data_dir = unique_dir("repo-sync-data");
        std::fs::create_dir_all(&repo_dir).unwrap();
        git(&repo_dir, &["init", "--quiet", "--initial-branch=main"]);
        write(&repo_dir, "README.md", "# Payments\n\nHow refunds work.\n");
        write(
            &repo_dir,
            "src/refund.rs",
            "pub fn issue_refund(amount: u64) -> u64 {\n    amount\n}\n",
        );
        write(&repo_dir, "src/old_name.py", "def settle():\n    return 1\n");
        write(&repo_dir, "package-lock.json", "{}");
        write(&repo_dir, "notes.txt", "kept out by the exclude pattern");
        write(&repo_dir, "CODEOWNERS", "* @docs-team
/src/ @payments lead@example.com
");
        git(&repo_dir, &["add", "."]);
        git(&repo_dir, &["commit", "--quiet", "-m", "Initial"]);

        let core = RepoMemoCore::boot(data_dir.clone()).await.unwrap();
        let workspace = core.create_workspace("Repo".to_owned()).await.unwrap();
        let source = core
            .connect_local_repo(
                &workspace.id,
                &repo_dir.join("src").to_string_lossy(),
                None,
                RepoSettings {
                    exclude: vec!["notes.txt".to_owned(), "CODEOWNERS".to_owned()],
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(source.status, "pending");
        assert!(core
            .connect_local_repo(
                &workspace.id,
                &repo_dir.to_string_lossy(),
                None,
                RepoSettings::default()
            )
            .await
            .is_err());
        assert!(core.repo_head(&source.id).await.unwrap().moved(), "never synced");

        let first = sync(&core, &source.id).await;
        assert!(!core.repo_head(&source.id).await.unwrap().moved());
        assert_eq!(first.files_in_tree, 6);
        assert_eq!(first.added, 3);
        assert_eq!(first.skipped, 3);
        assert_eq!(first.indexed, 3);
        let files = core.list_repo_files(&source.id).await.unwrap();
        let paths = files.iter().map(|file| file.path.as_str()).collect::<Vec<_>>();
        assert_eq!(paths, vec!["README.md", "src/old_name.py", "src/refund.rs"]);
        assert!(files.iter().all(|file| file.indexed));
        // `CODEOWNERS` names owners as hints on the files it covers.
        let readme = core.storage().get_artifact(&files[0].artifact_id).await.unwrap();
        assert_eq!(readme.metadata["code_owners"], serde_json::json!(["@docs-team"]));
        let refund = core.storage().get_artifact(&files[2].artifact_id).await.unwrap();
        assert_eq!(refund.metadata["code_owners"], serde_json::json!(["@payments", "lead@example.com"]));
        // Who last changed each file comes from one pass over the history.
        assert!(files.iter().all(|file| file.last_author.as_deref() == Some("Test")));
        assert!(files.iter().all(|file| file.last_change_commit.is_some() && file.last_changed_at.is_some()));
        let first_change = files[2].last_change_commit.clone();
        let refund_id = files[2].artifact_id.clone();
        let renamed_id = files[1].artifact_id.clone();

        // The whole repository is one more evidence item, with its overview.
        let detail = core.repo_detail(&source.id).await.unwrap();
        let overview = detail.overview.unwrap();
        assert_eq!(overview.file_count, 3);
        assert_eq!(
            overview.readme_excerpt.as_deref(),
            Some("# Payments\n\nHow refunds work.")
        );
        assert_eq!(overview.recent_commits.len(), 1);
        let overview_id = detail.repository.overview_artifact_id.unwrap();
        let item = core.get_artifact(overview_id.clone()).await.unwrap();
        assert_eq!(item.summary.artifact_type, repomemo_domain::ArtifactType::Repository);
        assert!(item.summary.indexed_at.is_some());
        assert!(item.content_preview.unwrap().contains("## Structure"));

        let hits = core
            .search_workspace(repomemo_domain::SearchRequest {
                workspace_id: workspace.id.clone(),
                query: "issue_refund".to_owned(),
                limit: Some(5),
                artifact_types: Vec::new(),
                languages: Vec::new(),
                source_ids: Vec::new(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(hits.iter().any(|hit| hit.artifact_id == refund_id));

        // Nothing changed: nothing is written or indexed again.
        let again = sync(&core, &source.id).await;
        assert_eq!((again.added, again.updated, again.indexed), (0, 0, 0));
        assert_eq!(again.unchanged, 3);

        // Edit, rename, delete.
        write(
            &repo_dir,
            "src/refund.rs",
            "pub fn issue_partial_refund(amount: u64) -> u64 {\n    amount / 2\n}\n",
        );
        git(&repo_dir, &["mv", "src/old_name.py", "src/settlement.py"]);
        git(&repo_dir, &["rm", "--quiet", "README.md"]);
        git(&repo_dir, &["add", "."]);
        git(&repo_dir, &["commit", "--quiet", "-m", "Rework refunds"]);
        assert!(
            core.repo_head(&source.id).await.unwrap().moved(),
            "a new commit moves the branch"
        );

        let second = sync(&core, &source.id).await;
        assert_eq!((second.updated, second.renamed, second.removed), (1, 1, 1));
        let files = core.list_repo_files(&source.id).await.unwrap();
        let paths = files.iter().map(|file| file.path.as_str()).collect::<Vec<_>>();
        assert_eq!(paths, vec!["src/refund.rs", "src/settlement.py"]);
        assert_ne!(files[0].last_change_commit, first_change, "the edited file names its new last change");
        let artifact = core.storage().get_artifact(&files[0].artifact_id).await.unwrap();
        assert_eq!(artifact.metadata["last_author"], "Test");
        assert_eq!(files[0].artifact_id, refund_id);
        assert_eq!(files[1].artifact_id, renamed_id);

        let source = core.repo_source(&source.id).await.unwrap();
        assert_eq!(source.status, "ready");
        assert_eq!(source.file_count, 2);
        assert_eq!(source.indexed_file_count, 2);
        let commit = source.last_synced_commit.unwrap();
        assert_eq!(commit.summary, "Rework refunds");
        assert_eq!(commit.branch.as_deref(), Some("main"));
        let detail = core.repo_detail(&source.id).await.unwrap();
        assert_eq!(
            detail.repository.overview_artifact_id.as_deref(),
            Some(overview_id.as_str())
        );
        let overview = detail.overview.unwrap();
        assert_eq!(overview.file_count, 2);
        assert_eq!(overview.readme_excerpt, None);
        assert_eq!(overview.recent_commits[0].summary, "Rework refunds");

        // The removed README is kept for its links but no longer searchable,
        // and is never queued for indexing again.
        let hits = core
            .search_workspace(repomemo_domain::SearchRequest {
                workspace_id: workspace.id.clone(),
                query: "refunds".to_owned(),
                limit: Some(10),
                artifact_types: Vec::new(),
                languages: Vec::new(),
                source_ids: Vec::new(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(hits.iter().all(|hit| hit.path != "README.md"));
        assert!(core
            .storage
            .list_artifacts_needing_index(Some(&workspace.id), INDEXER_VERSION + 1, true)
            .await
            .unwrap()
            .iter()
            .all(|artifact| artifact.path != "README.md"));
        assert!(core.artifacts_needing_index().await.unwrap().is_empty());

        // Renamed and edited in the same commit: still the same artifact, and
        // results say which commit their text is from.
        git(&repo_dir, &["mv", "src/settlement.py", "src/payout.py"]);
        write(
            &repo_dir,
            "src/payout.py",
            "def settle():
    return 1
# paid out
",
        );
        git(&repo_dir, &["add", "."]);
        git(&repo_dir, &["commit", "--quiet", "-m", "Rename and edit"]);
        let third = sync(&core, &source.id).await;
        assert_eq!((third.renamed, third.added, third.removed), (1, 0, 0), "{third:?}");
        let files = core.list_repo_files(&source.id).await.unwrap();
        let payout = files.iter().find(|file| file.path == "src/payout.py").unwrap();
        assert_eq!(payout.artifact_id, renamed_id);
        let hits = core
            .search_workspace(repomemo_domain::SearchRequest {
                workspace_id: workspace.id.clone(),
                query: "paid".to_owned(),
                limit: Some(5),
                ..Default::default()
            })
            .await
            .unwrap();
        let hit = hits
            .iter()
            .find(|hit| hit.artifact_id == renamed_id)
            .expect("the renamed file is searchable");
        let synced = core.repo_source(&source.id).await.unwrap().last_synced_commit.unwrap();
        assert_eq!(hit.commit.as_deref(), Some(synced.sha.as_str()));

        assert!(core.summarize_repo(&source.id, "missing-provider").await.is_err());
        core.remove_repo_source(&source.id).await.unwrap();
        assert!(core.list_artifacts(workspace.id.clone()).await.unwrap().is_empty());

        drop(core);
        let _ = std::fs::remove_dir_all(data_dir);
        let _ = std::fs::remove_dir_all(repo_dir);
    }

    /// Roadmap R10 (V0.2.54): a repository of 100,000 files read by the sync engine. A remote link is a
    /// bare clone on the server that the engine reads exactly like this one, so what is measured here is
    /// everything after the clone; the transfer itself is git's and the host's. Not a check:
    /// `REPOMEMO_PERF_FILES` sets the size (default 100,000).
    ///
    /// `cargo test -p repomemo-api --release repo_sync_at_scale -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "a performance run, not a check"]
    async fn repo_sync_at_scale() {
        use std::io::Write;
        use std::time::Instant;
        let files: usize = std::env::var("REPOMEMO_PERF_FILES")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(100_000);
        let repo_dir = unique_dir("repo-scale-repo");
        let data_dir = unique_dir("repo-scale-data");
        std::fs::create_dir_all(&repo_dir).unwrap();
        git(&repo_dir, &["init", "--quiet", "--initial-branch=main"]);

        // One commit of `files` small source files, written with `git fast-import` (minutes faster than `git add`).
        let started = Instant::now();
        let stream = repo_dir.join("..").join(format!("fast-import-{files}.txt"));
        {
            let mut out = std::io::BufWriter::new(std::fs::File::create(&stream).unwrap());
            for index in 0..files {
                let body = format!(
                    "pub fn handler_{index}(input: u64) -> u64 {{\n    // module {} file {index}\n    input + {}\n}}\n",
                    index % 997,
                    index % 31
                );
                writeln!(out, "blob\nmark :{}\ndata {}\n{body}", index + 1, body.len()).unwrap();
            }
            writeln!(out, "commit refs/heads/main\ncommitter Test <test@example.com> 1700000000 +0000\ndata 7\nInitial").unwrap();
            for index in 0..files {
                writeln!(out, "M 100644 :{} src/m{:04}/f{index}.rs", index + 1, index % 997).unwrap();
            }
            writeln!(out, "done").unwrap();
        }
        let status = Command::new("git")
            .arg("-C")
            .arg(&repo_dir)
            .args(["fast-import", "--quiet", "--done"])
            .stdin(std::fs::File::open(&stream).unwrap())
            .status()
            .unwrap();
        assert!(status.success(), "git fast-import failed");
        println!("built a repository of {files} files in {:?}", started.elapsed());

        let core = RepoMemoCore::boot(data_dir.clone()).await.unwrap();
        let workspace = core.create_workspace("Scale".to_owned()).await.unwrap();
        let source = core
            .connect_local_repo(&workspace.id, &repo_dir.to_string_lossy(), None, RepoSettings::default())
            .await
            .unwrap();

        let started = Instant::now();
        let first = sync(&core, &source.id).await;
        let full = started.elapsed();
        println!(
            "first sync: {full:?} ({} in the tree, {} added, {} indexed, {} skipped)",
            first.files_in_tree, first.added, first.indexed, first.skipped
        );

        let started = Instant::now();
        let unchanged = sync(&core, &source.id).await;
        let nothing = started.elapsed();
        println!("sync with nothing changed: {nothing:?} ({} added, {} updated)", unchanged.added, unchanged.updated);

        write(&repo_dir, "src/m0001/f1.rs", "pub fn handler_1(input: u64) -> u64 {\n    input * 2\n}\n");
        git(&repo_dir, &["add", "src/m0001/f1.rs"]);
        git(&repo_dir, &["commit", "--quiet", "-m", "Change one file"]);
        let started = Instant::now();
        let changed = sync(&core, &source.id).await;
        let one_file = started.elapsed();
        println!("sync after one changed file: {one_file:?} ({} updated)", changed.updated);

        // The workspace answers at this size (targets of V0.1.74: search under 300 ms, map and health under 2 s).
        let mut times = Vec::new();
        for round in 0..20 {
            let started = Instant::now();
            let hits = core
                .search_workspace(repomemo_domain::SearchRequest {
                    workspace_id: workspace.id.clone(),
                    query: format!("handler_{}", round * 977),
                    limit: Some(10),
                    ..Default::default()
                })
                .await
                .unwrap();
            times.push(started.elapsed());
            assert!(!hits.is_empty(), "the search finds a function of the repository");
        }
        times.sort();
        let p95 = times[(times.len() * 95 / 100).min(times.len() - 1)];
        println!("keyword search p95 over 20 searches: {p95:?}");
        let started = Instant::now();
        let map = core.knowledge_map(&workspace.id).await.unwrap();
        let map_time = started.elapsed();
        let started = Instant::now();
        let health = core.workspace_health(&workspace.id).await.unwrap();
        let health_time = started.elapsed();
        println!("knowledge map, first load: {map_time:?} ({} nodes); health, first load: {health_time:?} ({} findings)", map.nodes.len(), health.findings.len());

        // A change touches one file; nothing about it should cost a tenth of the first run (as for the map in V0.2.8).
        assert!(nothing * 10 < full, "a sync with nothing changed took {nothing:?} against {full:?} for the first");
        assert!(one_file * 10 < full, "a sync after one changed file took {one_file:?} against {full:?} for the first");
        assert!(p95 < std::time::Duration::from_millis(300), "search p95 {p95:?}");
        assert!(map_time < std::time::Duration::from_secs(2) && health_time < std::time::Duration::from_secs(2));

        drop(core);
        let _ = std::fs::remove_dir_all(repo_dir);
        let _ = std::fs::remove_file(stream);
        let _ = std::fs::remove_dir_all(data_dir);
    }
}
