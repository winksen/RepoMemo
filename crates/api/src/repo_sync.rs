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

use anyhow::{bail, Context, Result};
use repomemo_domain::{
    IndexingJobStatus, RepoAccessCheck, RepoCommit, RepoDetail, RepoFile, RepoOverview,
    RepoSettings, RepoSummary, RepoSkipCount, RepoSource,
    RepoSyncReport, Source, SourceType, REPO_SYNC_JOB_KIND,
};
use repomemo_git::{GitRepo, TreeFile};
use repomemo_indexer::INDEXER_VERSION;
use repomemo_ingestion::repo::{classify_repo_file, is_indexable_text, RepoFileKind, RepoFileRules};
use repomemo_storage::{RepoFileRecord, RepoFileWrite, StorageEngine};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::repo_overview::{
    compute_overview, key_file_role, overview_markdown, readme_excerpt, RECENT_COMMITS,
};
use crate::repo_stack::{detect_stack, MAX_MANIFEST_BYTES, MAX_STACK_MANIFESTS, STACK_MANIFESTS};
use crate::RepoMemoCore;

/// Skipped paths kept per reason in a sync report.
const SKIP_EXAMPLES: usize = 5;
/// Job progress is written every this many files, not after each one.
const PROGRESS_EVERY: usize = 20;
const MAX_PATTERNS: usize = 50;

/// What a repository source keeps in `sources.metadata_json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct RepoSourceMetadata {
    #[serde(default)]
    settings: RepoSettings,
    #[serde(default)]
    last_synced_commit: Option<RepoCommit>,
    #[serde(default)]
    last_synced_at: Option<String>,
    #[serde(default)]
    last_error: Option<String>,
    #[serde(default)]
    last_report: Option<RepoSyncReport>,
    /// What the repository held at the last complete sync.
    #[serde(default)]
    overview: Option<RepoOverview>,
    /// The artifact that stands for the whole repository.
    #[serde(default)]
    overview_artifact_id: Option<String>,
    /// The last AI summary, kept until it is regenerated.
    #[serde(default)]
    summary: Option<RepoSummary>,
}

impl RepoSourceMetadata {
    fn of(source: &Source) -> Self {
        serde_json::from_value(source.metadata.clone()).unwrap_or_default()
    }

    fn to_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_else(|_| json!({}))
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
            bail!("Workspace was not found.");
        }
        let settings = normalize_settings(settings)?;
        let repo = open_repo_link(link).await?;
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
            bail!("Workspace was not found.");
        }
        let settings = normalize_settings(settings)?;
        let repo = open_repo_link(link).await?;
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
        let counts = self
            .storage
            .repo_source_file_counts(&source.workspace_id)
            .await?;
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
        if settings.branch != RepoSourceMetadata::of(&source).settings.branch {
            let repo = GitRepo::open(Path::new(source.root_uri.as_deref().unwrap_or_default())).await?;
            repo.resolve_commit(settings.branch.as_deref()).await?;
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
        self.storage.delete_source(source_id).await
    }

    pub async fn list_repo_files(&self, source_id: &str) -> Result<Vec<RepoFile>> {
        self.repo_source_row(source_id).await?;
        self.storage.list_repo_file_entries(source_id).await
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

    async fn sync_repo(&self, source_id: &str, job_id: &str) -> Result<RepoSyncReport> {
        let source = self.repo_source_row(source_id).await?;
        let mut metadata = RepoSourceMetadata::of(&source);
        let root = source.root_uri.clone().unwrap_or_default();
        self.storage
            .update_indexing_job(job_id, "running", "reading_repository", 0, None)
            .await?;

        let repo = GitRepo::open(Path::new(&root))
            .await
            .with_context(|| format!("the repository at {root} could not be opened"))?;
        let branch = metadata.settings.branch.clone();
        let commit = repo.resolve_commit(branch.as_deref()).await?;
        let info = repo.commit_info(&commit).await?;
        let branch = match branch {
            Some(branch) => Some(branch),
            None => repo.current_branch().await?,
        };
        let tree = repo.list_files(&commit).await?;
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
        let mut changes = Vec::new();
        for (file, kind) in &wanted {
            match by_path.get(file.path.as_str()) {
                Some(record) if !record.removed && record.blob_sha == file.blob_sha => {
                    report.unchanged += 1;
                }
                Some(record) => changes.push((file, kind, Change::Update { record })),
                None => {
                    let renamed_from = vanished
                        .get_mut(file.blob_sha.as_str())
                        .and_then(|records| records.pop());
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
        self.storage
            .set_job_progress_total(job_id, planned as i64)
            .await?;
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
                .store_repo_blob(&root, &commit, file, kind, &bytes)
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
                        .update_repo_file(source_id, &record.artifact_id, &record.path, record.removed, false, write)
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
            .update_source(source_id, &source.name, "ready", &metadata.to_value(), !report.cancelled)
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
    ) -> Result<RepoFileWrite> {
        let content_hash = StorageEngine::content_hash(bytes);
        self.storage
            .store_blob(&content_hash, bytes, kind.mime_type.as_deref())
            .await?;
        Ok(RepoFileWrite {
            title: file
                .path
                .rsplit('/')
                .next()
                .unwrap_or(&file.path)
                .to_owned(),
            path: file.path.clone(),
            artifact_type: kind.artifact_type.clone(),
            language: kind.language.clone(),
            mime_type: kind.mime_type.clone(),
            content_hash,
            size_bytes: bytes.len() as i64,
            blob_sha: file.blob_sha.clone(),
            commit_sha: commit.to_owned(),
            metadata: json!({
                "origin": "git_repo",
                "repo_root": root,
                "commit": commit,
                "blob_sha": file.blob_sha,
            }),
        })
    }

    async fn repo_source_row(&self, source_id: &str) -> Result<Source> {
        let source = self.storage.get_source(source_id).await?;
        if !matches!(source.source_type, SourceType::GitRepo) {
            bail!("Source was not found.");
        }
        Ok(source)
    }

    async fn build_repo_source(
        &self,
        source: Source,
        counts: &HashMap<String, (i64, i64)>,
    ) -> Result<RepoSource> {
        let metadata = RepoSourceMetadata::of(&source);
        let (file_count, indexed_file_count) = counts.get(&source.id).copied().unwrap_or_default();
        let active_job = self
            .storage
            .latest_source_job(&source.id, REPO_SYNC_JOB_KIND)
            .await?
            .filter(|job| job.status == "running");
        Ok(RepoSource {
            id: source.id,
            workspace_id: source.workspace_id,
            name: source.name,
            root_path: source.root_uri.unwrap_or_default(),
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
        let entry = self
            .by_reason
            .entry(reason.clone())
            .or_insert_with(|| RepoSkipCount {
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
        counts.sort_by(|left, right| right.count.cmp(&left.count));
        counts
    }
}

/// Opens the repository behind a link, explaining in plain words why the
/// server cannot read it when it cannot.
async fn open_repo_link(link: &str) -> Result<GitRepo> {
    // "Copy as path" in Windows Explorer wraps the path in quotes.
    let link = link.trim().trim_matches('"').trim();
    if link.is_empty() {
        bail!("Enter the folder of a git repository on the server.");
    }
    let lower = link.to_ascii_lowercase();
    let is_url = ["http://", "https://", "ssh://", "git://", "git@"]
        .iter()
        .any(|prefix| lower.starts_with(prefix));
    if is_url {
        bail!(
            "Remote repository links are not supported yet. Enter the folder of a checkout on the server, such as C:\\code\\my-repo or /srv/code/my-repo."
        );
    }
    let path = PathBuf::from(link.strip_prefix("file://").unwrap_or(link));
    if !path.is_dir() {
        bail!(
            "The folder {} does not exist on the server, or the server cannot see it.",
            path.display()
        );
    }
    if let Err(error) = std::fs::read_dir(&path) {
        if error.kind() == std::io::ErrorKind::PermissionDenied {
            bail!("The server has no read access to {}.", path.display());
        }
        bail!("The folder {} cannot be read: {error}", path.display());
    }
    GitRepo::open(&path).await
}

fn repo_name(root: &Path) -> String {
    root.file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("Repository")
        .to_owned()
}

fn normalize_settings(settings: RepoSettings) -> Result<RepoSettings> {
    let clean = |patterns: Vec<String>| -> Result<Vec<String>> {
        let patterns = patterns
            .into_iter()
            .map(|pattern| pattern.trim().replace('\\', "/"))
            .filter(|pattern| !pattern.is_empty() && !pattern.starts_with('#'))
            .collect::<Vec<_>>();
        if patterns.len() > MAX_PATTERNS {
            bail!("Use at most {MAX_PATTERNS} include and {MAX_PATTERNS} exclude patterns.");
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
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
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

    #[tokio::test]
    async fn syncs_follow_commits_and_keep_artifact_identity() {
        let repo_dir = unique_dir("repo-sync-repo");
        let data_dir = unique_dir("repo-sync-data");
        std::fs::create_dir_all(&repo_dir).unwrap();
        git(&repo_dir, &["init", "--quiet", "--initial-branch=main"]);
        write(&repo_dir, "README.md", "# Payments\n\nHow refunds work.\n");
        write(&repo_dir, "src/refund.rs", "pub fn issue_refund(amount: u64) -> u64 {\n    amount\n}\n");
        write(&repo_dir, "src/old_name.py", "def settle():\n    return 1\n");
        write(&repo_dir, "package-lock.json", "{}");
        write(&repo_dir, "notes.txt", "kept out by the exclude pattern");
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
                    exclude: vec!["notes.txt".to_owned()],
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(source.status, "pending");
        assert!(core
            .connect_local_repo(&workspace.id, &repo_dir.to_string_lossy(), None, RepoSettings::default())
            .await
            .is_err());

        let first = sync(&core, &source.id).await;
        assert_eq!(first.files_in_tree, 5);
        assert_eq!(first.added, 3);
        assert_eq!(first.skipped, 2);
        assert_eq!(first.indexed, 3);
        let files = core.list_repo_files(&source.id).await.unwrap();
        let paths = files.iter().map(|file| file.path.as_str()).collect::<Vec<_>>();
        assert_eq!(paths, vec!["README.md", "src/old_name.py", "src/refund.rs"]);
        assert!(files.iter().all(|file| file.indexed));
        let refund_id = files[2].artifact_id.clone();
        let renamed_id = files[1].artifact_id.clone();

        // The whole repository is one more evidence item, with its overview.
        let detail = core.repo_detail(&source.id).await.unwrap();
        let overview = detail.overview.unwrap();
        assert_eq!(overview.file_count, 3);
        assert_eq!(overview.readme_excerpt.as_deref(), Some("# Payments\n\nHow refunds work."));
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
            })
            .await
            .unwrap();
        assert!(hits.iter().any(|hit| hit.artifact_id == refund_id));

        // Nothing changed: nothing is written or indexed again.
        let again = sync(&core, &source.id).await;
        assert_eq!((again.added, again.updated, again.indexed), (0, 0, 0));
        assert_eq!(again.unchanged, 3);

        // Edit, rename, delete.
        write(&repo_dir, "src/refund.rs", "pub fn issue_partial_refund(amount: u64) -> u64 {\n    amount / 2\n}\n");
        git(&repo_dir, &["mv", "src/old_name.py", "src/settlement.py"]);
        git(&repo_dir, &["rm", "--quiet", "README.md"]);
        git(&repo_dir, &["add", "."]);
        git(&repo_dir, &["commit", "--quiet", "-m", "Rework refunds"]);

        let second = sync(&core, &source.id).await;
        assert_eq!((second.updated, second.renamed, second.removed), (1, 1, 1));
        let files = core.list_repo_files(&source.id).await.unwrap();
        let paths = files.iter().map(|file| file.path.as_str()).collect::<Vec<_>>();
        assert_eq!(paths, vec!["src/refund.rs", "src/settlement.py"]);
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
        assert_eq!(detail.repository.overview_artifact_id.as_deref(), Some(overview_id.as_str()));
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

        assert!(core.summarize_repo(&source.id, "missing-provider").await.is_err());
        core.remove_repo_source(&source.id).await.unwrap();
        assert!(core.list_artifacts(workspace.id.clone()).await.unwrap().is_empty());

        drop(core);
        let _ = std::fs::remove_dir_all(data_dir);
        let _ = std::fs::remove_dir_all(repo_dir);
    }
}
