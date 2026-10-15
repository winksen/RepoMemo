//! Connectors: trackers whose issues become evidence (roadmap F17a, V0.2.27,
//! decision 12).
//!
//! A connector is a source of type `connector`. The first one reads GitHub
//! issues (github.com or GitHub Enterprise Server) with an optional read-only
//! token: each issue, with its comments, becomes one markdown artifact that is
//! indexed like any file, searchable and citable. Syncs are incremental (the
//! tracker's `since`, oldest update first, so an interrupted sync resumes),
//! a closed issue becomes *outdated* and a reopened one active again.
//!
//! Outside rules (S6): the API host must be on `REPOMEMO_GIT_ALLOWED_HOSTS`
//! like repository hosts, only `https` is spoken (plain `http` only to this
//! machine, for a tracker running beside the server), redirects are not
//! followed, the token only ever goes to the host it was entered for, and a
//! response over 20 MiB is refused.

use std::time::Duration;

use anyhow::{Context, Result};
use repomemo_domain::{
    ArtifactType, ConnectorAccessCheck, ConnectorSource, ConnectorSyncReport, IndexingJobStatus, Source,
    CONNECTOR_SYNC_JOB_KIND,
};
use repomemo_indexer::INDEXER_VERSION;
use repomemo_storage::{ConnectorItemOutcome, ConnectorItemWrite, StorageEngine};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::RepoMemoCore;

/// Issues read in one sync before it stops and resumes at the next.
const MAX_PAGES_PER_SYNC: usize = 30;
const PER_PAGE: usize = 100;
/// Comment pages read for one issue (500 comments).
const MAX_COMMENT_PAGES: usize = 5;
const MAX_BODY_CHARS: usize = 100_000;
const MAX_COMMENT_CHARS: usize = 20_000;
const MAX_RESPONSE_BYTES: usize = 20 * 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Where a tracker is: derived from the link a person typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GitHubTarget {
    /// `https://api.github.com`, or `https://host/api/v3` for Enterprise.
    pub api_base: String,
    pub owner: String,
    pub repo: String,
    /// The link, normalised: what the connector is unique by.
    pub location: String,
    pub host: String,
    pub port: Option<u16>,
}

/// What a connector source keeps in `sources.metadata_json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ConnectorMetadata {
    #[serde(default)]
    provider: String,
    #[serde(default)]
    api_base: String,
    #[serde(default)]
    owner: String,
    #[serde(default)]
    repo: String,
    /// The tracker's update time of the last item stored: the next sync asks
    /// for what changed since.
    #[serde(default)]
    cursor: Option<String>,
    #[serde(default)]
    last_synced_at: Option<String>,
    #[serde(default)]
    last_error: Option<String>,
    #[serde(default)]
    last_report: Option<ConnectorSyncReport>,
}

impl ConnectorMetadata {
    fn of(source: &Source) -> Self {
        serde_json::from_value(source.metadata.clone()).unwrap_or_default()
    }

    fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or_else(|_| json!({}))
    }
}

/// `https://github.com/acme/app(.git)` or an Enterprise address of the same
/// shape. `http` is accepted for `localhost` and loopback addresses only.
pub(crate) fn parse_github_link(link: &str) -> Result<GitHubTarget> {
    let text = link.trim().trim_end_matches('/');
    let (scheme, rest) = text
        .split_once("://")
        .ok_or_else(|| repomemo_domain::invalid!("Enter the address of the repository, such as https://github.com/acme/app."))?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "https" && scheme != "http" {
        repomemo_domain::bail_invalid!("Only https addresses can be connected.");
    }
    if text.chars().any(|c| c.is_control() || c.is_whitespace() || c == '?' || c == '#' || c == '@' || c == '\\') {
        repomemo_domain::bail_invalid!("That is not the address of a repository.");
    }
    let (authority, path) = rest
        .split_once('/')
        .ok_or_else(|| repomemo_domain::invalid!("The address needs the owner and the repository after the host."))?;
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') || host.starts_with('[') => {
            let port = port
                .parse::<u16>()
                .map_err(|_| repomemo_domain::invalid!("The port is a number from 1 to 65535."))?;
            (host.trim_matches(['[', ']']).to_ascii_lowercase(), Some(port))
        }
        _ => (authority.to_ascii_lowercase(), None),
    };
    let local = matches!(host.as_str(), "localhost" | "127.0.0.1" | "::1");
    if scheme == "http" && !local {
        repomemo_domain::bail_invalid!("Only https addresses can be connected.");
    }
    let segments = path.split('/').filter(|segment| !segment.is_empty()).collect::<Vec<_>>();
    let [owner, repo] = segments.as_slice() else {
        repomemo_domain::bail_invalid!("The address needs the owner and the repository after the host.");
    };
    let repo = repo.strip_suffix(".git").unwrap_or(repo);
    let valid = |name: &str| {
        !name.is_empty()
            && name.len() <= 100
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            && !name.starts_with('.')
    };
    if !valid(owner) || !valid(repo) {
        repomemo_domain::bail_invalid!("That is not the address of a repository.");
    }
    let host_port = match port {
        Some(port) => format!("{host}:{port}"),
        None => host.clone(),
    };
    let api_base = if host == "github.com" || host == "www.github.com" {
        "https://api.github.com".to_owned()
    } else {
        format!("{scheme}://{host_port}/api/v3")
    };
    Ok(GitHubTarget {
        api_base,
        owner: (*owner).to_owned(),
        repo: repo.to_owned(),
        location: format!("{scheme}://{host_port}/{owner}/{repo}"),
        host: if host == "github.com" || host == "www.github.com" {
            "api.github.com".to_owned()
        } else {
            host
        },
        port: if host_is_github(&authority.to_ascii_lowercase()) { None } else { port },
    })
}

fn host_is_github(authority: &str) -> bool {
    authority == "github.com" || authority == "www.github.com"
}

/// A GitHub REST client for one target.
struct GitHub {
    http: reqwest::Client,
    api_base: String,
    token: Option<String>,
}

impl GitHub {
    fn new(api_base: &str, token: Option<String>) -> Result<Self> {
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(REQUEST_TIMEOUT)
            .user_agent("RepoMemo")
            .build()
            .context("the HTTP client could not be created")?;
        Ok(Self {
            http,
            api_base: api_base.trim_end_matches('/').to_owned(),
            token,
        })
    }

    /// One GET, as JSON, and the address of the next page when there is one.
    /// A next page on another host is never followed, so the token cannot be
    /// sent there.
    async fn get(&self, url: &str) -> Result<(Value, Option<String>)> {
        if !url.starts_with(&self.api_base) {
            repomemo_domain::bail_invalid!("The tracker pointed to an address outside its own API.");
        }
        let mut request = self
            .http
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        let response = request.send().await.map_err(|error| {
            // The error text can carry the address, never the token.
            repomemo_domain::invalid!("The tracker could not be reached: {}", error.without_url())
        })?;
        let status = response.status();
        let remaining = response
            .headers()
            .get("x-ratelimit-remaining")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let next = response
            .headers()
            .get("link")
            .and_then(|value| value.to_str().ok())
            .and_then(next_link);
        if !status.is_success() {
            if status.as_u16() == 401 {
                repomemo_domain::bail_invalid!(
                    "The tracker refused the token. Check that it is valid, not expired, and can read issues."
                );
            }
            if status.as_u16() == 429 || (status.as_u16() == 403 && remaining.as_deref() == Some("0")) {
                repomemo_domain::bail_invalid!(
                    "The tracker's rate limit is reached. The sync resumes at the next try."
                );
            }
            if status.as_u16() == 403 {
                repomemo_domain::bail_invalid!("The tracker refused the request: the token cannot read this repository's issues.");
            }
            if status.as_u16() == 404 {
                repomemo_domain::bail_invalid!(
                    "The tracker does not know this repository, or the token cannot see it."
                );
            }
            repomemo_domain::bail_invalid!("The tracker answered {status}.");
        }
        let bytes = response.bytes().await.context("the tracker's answer could not be read")?;
        if bytes.len() > MAX_RESPONSE_BYTES {
            repomemo_domain::bail_invalid!("The tracker's answer is too large.");
        }
        let value = serde_json::from_slice::<Value>(&bytes).context("the tracker's answer is not JSON")?;
        Ok((value, next))
    }
}

/// The `rel="next"` address of a `Link` header.
fn next_link(header: &str) -> Option<String> {
    header.split(',').find_map(|part| {
        let (url, rel) = part.split_once(';')?;
        rel.contains("rel=\"next\"")
            .then(|| url.trim().trim_start_matches('<').trim_end_matches('>').to_owned())
    })
}

fn text_of(value: &Value, key: &str) -> String {
    value.get(key).and_then(Value::as_str).unwrap_or_default().to_owned()
}

fn user_of(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|user| user.get("login"))
        .and_then(Value::as_str)
        .unwrap_or("ghost")
        .to_owned()
}

fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        let mut clipped = text.chars().take(max).collect::<String>();
        clipped.push_str("\n\n[… cut off]");
        clipped
    }
}

/// The markdown an issue becomes: its details, body and discussion.
pub(crate) fn render_issue(owner: &str, repo: &str, issue: &Value, comments: &[Value]) -> String {
    let number = issue.get("number").and_then(Value::as_i64).unwrap_or(0);
    let labels = issue
        .get("labels")
        .and_then(Value::as_array)
        .map(|labels| {
            labels
                .iter()
                .filter_map(|label| label.get("name").and_then(Value::as_str).or_else(|| label.as_str()))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let assignees = issue
        .get("assignees")
        .and_then(Value::as_array)
        .map(|people| {
            people
                .iter()
                .filter_map(|person| person.get("login").and_then(Value::as_str))
                .map(|login| format!("@{login}"))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let mut out = format!("# {}\n\n", text_of(issue, "title"));
    out.push_str(&format!(
        "Issue #{number} in {owner}/{repo} · {} · opened by @{} on {}",
        text_of(issue, "state"),
        user_of(issue, "user"),
        text_of(issue, "created_at")
    ));
    if let Some(closed) = issue.get("closed_at").and_then(Value::as_str) {
        out.push_str(&format!(" · closed {closed}"));
    }
    out.push('\n');
    if !labels.is_empty() {
        out.push_str(&format!("\nLabels: {labels}\n"));
    }
    if !assignees.is_empty() {
        out.push_str(&format!("\nAssignees: {assignees}\n"));
    }
    let url = text_of(issue, "html_url");
    if !url.is_empty() {
        out.push_str(&format!("\n{url}\n"));
    }
    let body = text_of(issue, "body");
    if !body.trim().is_empty() {
        out.push_str(&format!("\n{}\n", clip(body.trim(), MAX_BODY_CHARS)));
    }
    if !comments.is_empty() {
        out.push_str("\n## Discussion\n");
        for comment in comments {
            out.push_str(&format!(
                "\n### @{} · {}\n\n{}\n",
                user_of(comment, "user"),
                text_of(comment, "created_at"),
                clip(text_of(comment, "body").trim(), MAX_COMMENT_CHARS)
            ));
        }
    }
    out
}

impl RepoMemoCore {
    async fn github_for(&self, source_id: Option<&str>, target: &GitHubTarget) -> Result<GitHub> {
        self.remote_git_policy.check_api_host(&target.host, target.port)?;
        let token = match source_id {
            Some(id) => self
                .storage
                .open_repo_credential(id)
                .await?
                .filter(|secret| secret.kind == "token")
                .map(|secret| secret.expose().to_owned()),
            None => None,
        };
        GitHub::new(&target.api_base, token)
    }

    /// Looks at a tracker link before it is connected, with the token that
    /// would be used. Nothing is stored.
    pub async fn check_github_issues_link(
        &self,
        workspace_id: &str,
        link: &str,
        token: Option<&str>,
    ) -> Result<ConnectorAccessCheck> {
        if !self.storage.workspace_exists(workspace_id).await? {
            repomemo_domain::bail_not_found!("Workspace was not found.");
        }
        let target = parse_github_link(link)?;
        let mut client = self.github_for(None, &target).await?;
        client.token = token.map(str::trim).filter(|token| !token.is_empty()).map(str::to_owned);
        let (repository, _) = client
            .get(&format!("{}/repos/{}/{}", client.api_base, target.owner, target.repo))
            .await?;
        let already_connected = self
            .storage
            .list_sources_of_type(Some(workspace_id), repomemo_domain::SourceType::Connector)
            .await?
            .iter()
            .any(|source| source.root_uri.as_deref() == Some(target.location.as_str()));
        Ok(ConnectorAccessCheck {
            provider: "github".to_owned(),
            name: text_of(&repository, "full_name"),
            location: target.location,
            open_issues: repository.get("open_issues_count").and_then(Value::as_i64).unwrap_or(0),
            private: repository.get("private").and_then(Value::as_bool).unwrap_or(false),
            already_connected,
        })
    }

    /// Connects a repository's GitHub issues to a workspace. Nothing is read
    /// until a sync runs.
    pub async fn connect_github_issues(
        &self,
        workspace_id: &str,
        link: &str,
        name: Option<String>,
        token: Option<String>,
        actor_user_id: Option<&str>,
    ) -> Result<ConnectorSource> {
        let check = self
            .check_github_issues_link(workspace_id, link, token.as_deref())
            .await?;
        let target = parse_github_link(link)?;
        let name = name
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| format!("{} issues", check.name));
        let metadata = ConnectorMetadata {
            provider: "github".to_owned(),
            api_base: target.api_base.clone(),
            owner: target.owner.clone(),
            repo: target.repo.clone(),
            ..Default::default()
        };
        let source = self
            .storage
            .create_connector_source(workspace_id, &name, &target.location, &metadata.to_value())
            .await?;
        if let Some(token) = token.as_deref().map(str::trim).filter(|token| !token.is_empty()) {
            if let Err(error) = self
                .storage
                .set_repo_credential(&source.id, "token", None, token, actor_user_id)
                .await
            {
                let _ = self.storage.delete_source(&source.id).await;
                return Err(error);
            }
        }
        self.connector_source(&source.id).await
    }

    pub async fn connector_source(&self, source_id: &str) -> Result<ConnectorSource> {
        let source = self.connector_source_row(source_id).await?;
        self.build_connector(source).await
    }

    pub async fn list_connectors(&self, workspace_id: &str) -> Result<Vec<ConnectorSource>> {
        let mut connectors = Vec::new();
        for source in self
            .storage
            .list_sources_of_type(Some(workspace_id), repomemo_domain::SourceType::Connector)
            .await?
        {
            connectors.push(self.build_connector(source).await?);
        }
        Ok(connectors)
    }

    pub async fn list_all_connector_ids(&self) -> Result<Vec<String>> {
        Ok(self
            .storage
            .list_sources_of_type(None, repomemo_domain::SourceType::Connector)
            .await?
            .into_iter()
            .map(|source| source.id)
            .collect())
    }

    async fn connector_source_row(&self, source_id: &str) -> Result<Source> {
        let source = self.storage.get_source(source_id).await?;
        if !matches!(source.source_type, repomemo_domain::SourceType::Connector) {
            repomemo_domain::bail_not_found!("Connector was not found.");
        }
        Ok(source)
    }

    async fn build_connector(&self, source: Source) -> Result<ConnectorSource> {
        let metadata = ConnectorMetadata::of(&source);
        let (open_items, closed_items) = self.storage.connector_item_counts(&source.id).await?;
        let active_job = self
            .storage
            .latest_source_job(&source.id, CONNECTOR_SYNC_JOB_KIND)
            .await?
            .filter(|job| job.status == "running");
        let has_credential = self.storage.repo_credential_info(&source.id).await?.is_some();
        Ok(ConnectorSource {
            id: source.id,
            workspace_id: source.workspace_id,
            name: source.name,
            provider: metadata.provider,
            location: source.root_uri.unwrap_or_default(),
            status: source.status,
            last_synced_at: metadata.last_synced_at,
            last_error: metadata.last_error,
            last_report: metadata.last_report,
            open_items,
            closed_items,
            has_credential,
            active_job,
            created_at: source.created_at,
        })
    }

    /// Replaces the token of a connector. It has to work before it replaces
    /// the old one.
    pub async fn set_connector_token(&self, source_id: &str, token: &str, actor_user_id: Option<&str>) -> Result<()> {
        let source = self.connector_source_row(source_id).await?;
        let metadata = ConnectorMetadata::of(&source);
        let target = parse_github_link(source.root_uri.as_deref().unwrap_or_default())?;
        let mut client = self.github_for(None, &target).await?;
        client.token = Some(token.trim().to_owned());
        client
            .get(&format!("{}/repos/{}/{}", metadata.api_base, metadata.owner, metadata.repo))
            .await?;
        self.storage
            .set_repo_credential(source_id, "token", None, token, actor_user_id)
            .await?;
        Ok(())
    }

    pub async fn remove_connector(&self, source_id: &str) -> Result<()> {
        self.connector_source_row(source_id).await?;
        self.storage.delete_source(source_id).await
    }

    /// Records a queued sync. Run it with [`RepoMemoCore::run_connector_sync`].
    pub async fn begin_connector_sync(&self, source_id: &str) -> Result<IndexingJobStatus> {
        let source = self.connector_source_row(source_id).await?;
        let job = self
            .storage
            .create_job(&source.workspace_id, Some(source_id), CONNECTOR_SYNC_JOB_KIND, "queued", None)
            .await?;
        self.storage.set_source_status(source_id, "syncing").await?;
        Ok(job)
    }

    /// Reads what changed in the tracker since the last sync, stores it and
    /// indexes it, reporting progress on `job_id`.
    pub async fn run_connector_sync(&self, source_id: &str, job_id: &str) -> Result<ConnectorSyncReport> {
        match self.sync_connector(source_id, job_id).await {
            Ok(report) => {
                self.storage
                    .update_indexing_job(job_id, "completed", "synced", 0, None)
                    .await?;
                Ok(report)
            }
            Err(error) => {
                let message = format!("{error:#}");
                if let Ok(source) = self.storage.get_source(source_id).await {
                    let mut metadata = ConnectorMetadata::of(&source);
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

    async fn sync_connector(&self, source_id: &str, job_id: &str) -> Result<ConnectorSyncReport> {
        let source = self.connector_source_row(source_id).await?;
        let mut metadata = ConnectorMetadata::of(&source);
        let target = parse_github_link(source.root_uri.as_deref().unwrap_or_default())?;
        let client = self.github_for(Some(source_id), &target).await?;
        self.storage
            .update_indexing_job(job_id, "running", "reading_tracker", 0, None)
            .await?;

        let mut report = ConnectorSyncReport::default();
        let mut url = {
            let mut url = format!(
                "{}/repos/{}/{}/issues?state=all&sort=updated&direction=asc&per_page={PER_PAGE}",
                client.api_base, target.owner, target.repo
            );
            if let Some(since) = &metadata.cursor {
                url.push_str(&format!("&since={}", since.replace('+', "%2B")));
            }
            Some(url)
        };
        let mut pages = 0;
        while let Some(page_url) = url.take() {
            if pages >= MAX_PAGES_PER_SYNC {
                report.more_to_fetch = true;
                break;
            }
            pages += 1;
            let (page, next) = client.get(&page_url).await?;
            for issue in page.as_array().cloned().unwrap_or_default() {
                // The issues list also holds pull requests; those are V0.2.28.
                if issue.get("pull_request").is_some() {
                    continue;
                }
                report.items_seen += 1;
                let number = issue.get("number").and_then(Value::as_i64).unwrap_or(0);
                let updated = text_of(&issue, "updated_at");
                let comments = if issue.get("comments").and_then(Value::as_i64).unwrap_or(0) > 0 {
                    self.github_comments(&client, &target, number).await?
                } else {
                    Vec::new()
                };
                let content = render_issue(&target.owner, &target.repo, &issue, &comments).into_bytes();
                let content_hash = StorageEngine::content_hash(&content);
                self.storage
                    .store_blob(&content_hash, &content, Some("text/markdown"))
                    .await?;
                let state = if text_of(&issue, "state") == "closed" { "closed" } else { "open" };
                let item = ConnectorItemWrite {
                    external_id: format!("issue:{number}"),
                    kind: "issue".to_owned(),
                    artifact_type: ArtifactType::Issue,
                    title: format!("#{number} {}", text_of(&issue, "title")),
                    path: format!("issues/{number}"),
                    content_hash,
                    size_bytes: content.len() as i64,
                    state: state.to_owned(),
                    remote_updated_at: updated.clone(),
                    metadata: json!({
                        "origin": "github_issue",
                        "provider": "github",
                        "repository": format!("{}/{}", target.owner, target.repo),
                        "number": number,
                        "url": text_of(&issue, "html_url"),
                        "state": state,
                        "author": user_of(&issue, "user"),
                        "opened_at": text_of(&issue, "created_at"),
                        "closed_at": issue.get("closed_at").cloned().unwrap_or(Value::Null),
                        "remote_updated_at": updated,
                    }),
                };
                let (_, outcome) = self
                    .storage
                    .write_connector_item(&source.workspace_id, source_id, &item)
                    .await?;
                match outcome {
                    ConnectorItemOutcome::Added => report.added += 1,
                    ConnectorItemOutcome::Updated => report.updated += 1,
                    ConnectorItemOutcome::Unchanged => report.unchanged += 1,
                }
                if !updated.is_empty() {
                    metadata.cursor = Some(updated);
                }
            }
            url = next;
        }

        // Indexed in the same job, like a repository's files.
        let pending = self
            .storage
            .list_connector_artifacts_needing_index(source_id, INDEXER_VERSION)
            .await?;
        self.storage
            .set_job_progress_total(job_id, pending.len() as i64)
            .await?;
        self.storage
            .update_indexing_job(job_id, "running", "indexing", 0, None)
            .await?;
        for (position, artifact) in pending.iter().enumerate() {
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
            if position % 20 == 19 {
                self.storage
                    .update_indexing_job(job_id, "running", "indexing", (position + 1) as i64, None)
                    .await?;
            }
        }

        metadata.last_report = Some(report.clone());
        metadata.last_error = None;
        metadata.last_synced_at = Some(chrono::Utc::now().to_rfc3339());
        self.storage
            .update_source(source_id, &source.name, "ready", &metadata.to_value(), true)
            .await?;
        Ok(report)
    }

    async fn github_comments(&self, client: &GitHub, target: &GitHubTarget, number: i64) -> Result<Vec<Value>> {
        let mut comments = Vec::new();
        let mut url = Some(format!(
            "{}/repos/{}/{}/issues/{number}/comments?per_page={PER_PAGE}",
            client.api_base, target.owner, target.repo
        ));
        let mut pages = 0;
        while let Some(page_url) = url.take() {
            if pages >= MAX_COMMENT_PAGES {
                break;
            }
            pages += 1;
            let (page, next) = client.get(&page_url).await?;
            comments.extend(page.as_array().cloned().unwrap_or_default());
            url = next;
        }
        Ok(comments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_name_the_api_and_the_repository() {
        let github = parse_github_link("https://github.com/acme/app.git/").unwrap();
        assert_eq!(github.api_base, "https://api.github.com");
        assert_eq!((github.owner.as_str(), github.repo.as_str()), ("acme", "app"));
        assert_eq!(github.location, "https://github.com/acme/app");
        assert_eq!(github.host, "api.github.com");

        let enterprise = parse_github_link("https://ghe.corp.example:8443/team/svc").unwrap();
        assert_eq!(enterprise.api_base, "https://ghe.corp.example:8443/api/v3");
        assert_eq!((enterprise.host.as_str(), enterprise.port), ("ghe.corp.example", Some(8443)));

        let local = parse_github_link("http://127.0.0.1:9999/o/r").unwrap();
        assert_eq!(local.api_base, "http://127.0.0.1:9999/api/v3");

        for refused in [
            "",
            "github.com/acme/app",
            "http://github.com/acme/app",
            "ftp://github.com/acme/app",
            "https://github.com/acme",
            "https://github.com/acme/app/issues",
            "https://user@github.com/acme/app",
            "https://github.com/acme/app?x=1",
            "https://github.com/../app",
        ] {
            assert!(parse_github_link(refused).is_err(), "{refused:?}");
        }
    }

    #[test]
    fn an_issue_becomes_markdown_with_its_discussion() {
        let issue = json!({
            "number": 7, "title": "Refunds double post", "state": "closed",
            "user": {"login": "ana"}, "created_at": "2026-01-02T03:04:05Z", "closed_at": "2026-01-09T00:00:00Z",
            "html_url": "https://github.com/acme/app/issues/7", "body": "Seen on staging.",
            "labels": [{"name": "bug"}, {"name": "payments"}], "assignees": [{"login": "bo"}]
        });
        let comments = vec![json!({"user": {"login": "bo"}, "created_at": "2026-01-03T00:00:00Z", "body": "Fixed in #8."})];
        let text = render_issue("acme", "app", &issue, &comments);
        assert!(text.starts_with("# Refunds double post\n"));
        assert!(text.contains("Issue #7 in acme/app · closed · opened by @ana"));
        assert!(text.contains("Labels: bug, payments"));
        assert!(text.contains("Assignees: @bo"));
        assert!(text.contains("Seen on staging."));
        assert!(text.contains("## Discussion"));
        assert!(text.contains("### @bo · 2026-01-03T00:00:00Z\n\nFixed in #8."));
    }

    #[test]
    fn the_next_page_comes_from_the_link_header() {
        let header = "<https://api.github.com/x?page=2>; rel=\"next\", <https://api.github.com/x?page=9>; rel=\"last\"";
        assert_eq!(next_link(header).as_deref(), Some("https://api.github.com/x?page=2"));
        assert_eq!(next_link("<https://api.github.com/x?page=1>; rel=\"prev\""), None);
        assert_eq!(next_link(""), None);
    }
}
