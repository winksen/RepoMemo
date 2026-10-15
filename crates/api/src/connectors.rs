//! Connectors: trackers whose issues become evidence (roadmap F17a, V0.2.27,
//! decision 12).
//!
//! A connector is a source of type `connector`. The first one reads GitHub
//! issues and pull requests (github.com or GitHub Enterprise Server) with an
//! optional read-only token: each issue, with its comments, and each pull
//! request, with its commits, reviews and discussion, becomes one markdown
//! artifact that is indexed like any file, searchable and citable. Syncs are
//! incremental (the tracker's `since`, oldest update first, so an interrupted
//! sync resumes). A closed issue or an abandoned pull request becomes
//! *outdated* and a reopened one active again; a merged pull request stays.
//!
//! GitLab (V0.2.29) lives in [`gitlab`]: the same model, reshaped to the
//! records the renderers here read.
//!
//! Outside rules (S6): the API host must be on `REPOMEMO_GIT_ALLOWED_HOSTS`
//! like repository hosts, only `https` is spoken (plain `http` only to this
//! machine, for a tracker running beside the server), redirects are not
//! followed, the token only ever goes to the host it was entered for, and a
//! response over 20 MiB is refused.

use std::sync::atomic::{AtomicUsize, Ordering};
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

mod gitlab;
use gitlab::{parse_gitlab_link, project_id};

/// Issues read in one sync before it stops and resumes at the next.
const MAX_PAGES_PER_SYNC: usize = 30;
const PER_PAGE: usize = 100;
/// Comment pages read for one issue (500 comments).
const MAX_COMMENT_PAGES: usize = 5;
const MAX_BODY_CHARS: usize = 100_000;
const MAX_COMMENT_CHARS: usize = 20_000;
const MAX_RESPONSE_BYTES: usize = 20 * 1024 * 1024;
/// Requests one sync makes before it stops and resumes at the next (a pull
/// request alone takes five), so a large tracker cannot use up the token's
/// hourly allowance in one go.
const MAX_REQUESTS_PER_SYNC: usize = 500;
/// Commits listed for one pull request.
const MAX_PULL_COMMITS: usize = 250;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Where a tracker is: derived from the link a person typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TrackerTarget {
    /// `github` or `gitlab`.
    pub provider: String,
    /// `https://api.github.com`, `https://host/api/v3` for Enterprise, or
    /// `https://host/api/v4` for GitLab.
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
    /// for what changed since. GitHub has one feed for issues and pull
    /// requests; GitLab has two, and merge requests use `pulls_cursor`.
    #[serde(default)]
    cursor: Option<String>,
    #[serde(default)]
    pulls_cursor: Option<String>,
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
pub(crate) fn parse_github_link(link: &str) -> Result<TrackerTarget> {
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
    Ok(TrackerTarget {
        provider: "github".to_owned(),
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

/// `github` or `gitlab`, told by the person's choice or by the address: a
/// host with `gitlab` in its name is GitLab, and any other is GitHub (or
/// GitHub Enterprise Server).
pub(crate) fn parse_tracker_link(link: &str, provider: Option<&str>) -> Result<TrackerTarget> {
    let provider = match provider.map(str::trim).filter(|value| !value.is_empty()) {
        Some("github") => "github",
        Some("gitlab") => "gitlab",
        Some(_) => repomemo_domain::bail_invalid!("The tracker is GitHub or GitLab."),
        None => {
            let host = link
                .split_once("://")
                .map(|(_, rest)| rest.split('/').next().unwrap_or_default().to_ascii_lowercase())
                .unwrap_or_default();
            if host.contains("gitlab") {
                "gitlab"
            } else {
                "github"
            }
        }
    };
    if provider == "gitlab" {
        parse_gitlab_link(link)
    } else {
        parse_github_link(link)
    }
}

/// The address that describes a project (repository) and answers whether the
/// token can see it.
fn project_url(target: &TrackerTarget) -> String {
    if target.provider == "gitlab" {
        format!("{}/projects/{}", target.api_base, project_id(target))
    } else {
        format!("{}/repos/{}/{}", target.api_base, target.owner, target.repo)
    }
}

/// A GitHub REST client for one target.
struct GitHub {
    http: reqwest::Client,
    api_base: String,
    token: Option<String>,
    /// GitLab: the token goes in `PRIVATE-TOKEN` and no GitHub headers are sent.
    gitlab: bool,
    /// Requests made so far.
    calls: AtomicUsize,
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
            gitlab: false,
            calls: AtomicUsize::new(0),
        })
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::Relaxed)
    }

    /// One GET, as JSON, and the address of the next page when there is one.
    /// A next page on another host is never followed, so the token cannot be
    /// sent there.
    async fn get(&self, url: &str) -> Result<(Value, Option<String>)> {
        if !url.starts_with(&self.api_base) {
            repomemo_domain::bail_invalid!("The tracker pointed to an address outside its own API.");
        }
        self.calls.fetch_add(1, Ordering::Relaxed);
        let mut request = self.http.get(url);
        if self.gitlab {
            request = request.header("Accept", "application/json");
            if let Some(token) = &self.token {
                request = request.header("PRIVATE-TOKEN", token);
            }
        } else {
            request = request
                .header("Accept", "application/vnd.github+json")
                .header("X-GitHub-Api-Version", "2022-11-28");
            if let Some(token) = &self.token {
                request = request.bearer_auth(token);
            }
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

/// Everything a pull request is made of beyond its description.
pub(crate) struct PullDetail {
    pull: Value,
    commits: Vec<Value>,
    reviews: Vec<Value>,
    review_comments: Vec<Value>,
    comments: Vec<Value>,
}

impl PullDetail {
    fn merged(&self) -> bool {
        self.pull.get("merged").and_then(Value::as_bool).unwrap_or(false)
            || self.pull.get("merged_at").is_some_and(|value| !value.is_null())
    }

    /// The full commit ids, oldest first as the tracker lists them.
    fn commit_ids(&self) -> Vec<String> {
        self.commits
            .iter()
            .filter_map(|commit| commit.get("sha").and_then(Value::as_str).map(str::to_owned))
            .collect()
    }
}

/// The markdown a pull request becomes: its details, description, commits,
/// reviews, inline review comments and discussion.
pub(crate) fn render_pull(owner: &str, repo: &str, issue: &Value, detail: &PullDetail) -> String {
    let pull = &detail.pull;
    let number = issue.get("number").and_then(Value::as_i64).unwrap_or(0);
    let state = if detail.merged() { "merged".to_owned() } else { text_of(issue, "state") };
    let branch = |side: &str| {
        pull.get(side)
            .and_then(|value| value.get("ref"))
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_owned()
    };
    let mut out = format!("# {}\n\n", text_of(issue, "title"));
    out.push_str(&format!(
        "Pull request #{number} in {owner}/{repo} · {state} · opened by @{} on {} · {} ← {}",
        user_of(issue, "user"),
        text_of(issue, "created_at"),
        branch("base"),
        branch("head")
    ));
    if let Some(merged) = pull.get("merged_at").and_then(Value::as_str) {
        out.push_str(&format!(" · merged {merged}"));
    }
    out.push('\n');
    let figure = |key: &str| pull.get(key).and_then(Value::as_i64).unwrap_or(0);
    out.push_str(&format!(
        "\n{} commits · {} files changed · +{} −{}\n",
        figure("commits"),
        figure("changed_files"),
        figure("additions"),
        figure("deletions")
    ));
    let url = text_of(issue, "html_url");
    if !url.is_empty() {
        out.push_str(&format!("\n{url}\n"));
    }
    let body = text_of(issue, "body");
    if !body.trim().is_empty() {
        out.push_str(&format!("\n{}\n", clip(body.trim(), MAX_BODY_CHARS)));
    }
    if !detail.commits.is_empty() {
        out.push_str("\n## Commits\n\n");
        for commit in &detail.commits {
            let sha = text_of(commit, "sha");
            let message = commit
                .get("commit")
                .and_then(|inner| inner.get("message"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .lines()
                .next()
                .unwrap_or_default();
            let author = commit
                .get("commit")
                .and_then(|inner| inner.get("author"))
                .and_then(|author| author.get("name"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            out.push_str(&format!("- {} {message} ({author})\n", &sha[..sha.len().min(12)]));
        }
    }
    if !detail.reviews.is_empty() {
        out.push_str("\n## Reviews\n");
        for review in &detail.reviews {
            let body = text_of(review, "body");
            out.push_str(&format!(
                "\n### @{} · {} · {}\n",
                user_of(review, "user"),
                text_of(review, "state").to_lowercase().replace('_', " "),
                text_of(review, "submitted_at")
            ));
            if !body.trim().is_empty() {
                out.push_str(&format!("\n{}\n", clip(body.trim(), MAX_COMMENT_CHARS)));
            }
        }
    }
    if !detail.review_comments.is_empty() {
        out.push_str("\n## Review comments\n");
        for comment in &detail.review_comments {
            let line = comment
                .get("line")
                .or_else(|| comment.get("original_line"))
                .and_then(Value::as_i64)
                .map(|line| format!(":{line}"))
                .unwrap_or_default();
            out.push_str(&format!(
                "\n### @{} on {}{line} · {}\n\n{}\n",
                user_of(comment, "user"),
                text_of(comment, "path"),
                text_of(comment, "created_at"),
                clip(text_of(comment, "body").trim(), MAX_COMMENT_CHARS)
            ));
        }
    }
    if !detail.comments.is_empty() {
        out.push_str("\n## Discussion\n");
        for comment in &detail.comments {
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
    async fn github_for(&self, source_id: Option<&str>, target: &TrackerTarget) -> Result<GitHub> {
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
        let mut client = GitHub::new(&target.api_base, token)?;
        client.gitlab = target.provider == "gitlab";
        Ok(client)
    }

    /// Looks at a tracker link before it is connected, with the token that
    /// would be used. Nothing is stored. `provider` is `github` or `gitlab`;
    /// without it a host with `gitlab` in its name is GitLab and any other is
    /// GitHub.
    pub async fn check_tracker_link(
        &self,
        workspace_id: &str,
        link: &str,
        provider: Option<&str>,
        token: Option<&str>,
    ) -> Result<ConnectorAccessCheck> {
        if !self.storage.workspace_exists(workspace_id).await? {
            repomemo_domain::bail_not_found!("Workspace was not found.");
        }
        let target = parse_tracker_link(link, provider)?;
        if target.provider == "gitlab" {
            return self.check_gitlab(workspace_id, &target, token).await;
        }
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

    /// Connects a repository's (or project's) issues and pull requests to a
    /// workspace. Nothing is read until a sync runs.
    pub async fn connect_tracker(
        &self,
        workspace_id: &str,
        link: &str,
        provider: Option<&str>,
        name: Option<String>,
        token: Option<String>,
        actor_user_id: Option<&str>,
    ) -> Result<ConnectorSource> {
        let check = self
            .check_tracker_link(workspace_id, link, provider, token.as_deref())
            .await?;
        let target = parse_tracker_link(link, provider)?;
        let name = name
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| format!("{} issues", check.name));
        let metadata = ConnectorMetadata {
            provider: target.provider.clone(),
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
        let target = if metadata.provider == "gitlab" {
            parse_gitlab_link(source.root_uri.as_deref().unwrap_or_default())?
        } else {
            parse_github_link(source.root_uri.as_deref().unwrap_or_default())?
        };
        let mut client = self.github_for(None, &target).await?;
        client.token = Some(token.trim().to_owned());
        client.get(&project_url(&target)).await?;
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
        if metadata.provider == "gitlab" {
            return self.sync_gitlab(&source, metadata, job_id).await;
        }
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
        'pages: while let Some(page_url) = url.take() {
            if pages >= MAX_PAGES_PER_SYNC {
                report.more_to_fetch = true;
                break;
            }
            pages += 1;
            let (page, next) = client.get(&page_url).await?;
            for issue in page.as_array().cloned().unwrap_or_default() {
                // A pull request costs several requests: stop before the
                // allowance goes, and resume from the cursor next time.
                if client.calls() >= MAX_REQUESTS_PER_SYNC {
                    report.more_to_fetch = true;
                    break 'pages;
                }
                // The issues feed also lists pull requests, with the same
                // number space.
                let is_pull = issue.get("pull_request").is_some();
                report.items_seen += 1;
                let number = issue.get("number").and_then(Value::as_i64).unwrap_or(0);
                let updated = text_of(&issue, "updated_at");
                let (content, state, artifact_type, kind, path, origin, commits) = if is_pull {
                    report.pull_requests_seen += 1;
                    let detail = self.github_pull(&client, &target, number, &issue).await?;
                    let state = if detail.merged() {
                        "merged".to_owned()
                    } else if text_of(&issue, "state") == "closed" {
                        "closed".to_owned()
                    } else {
                        "open".to_owned()
                    };
                    (
                        render_pull(&target.owner, &target.repo, &issue, &detail),
                        state,
                        ArtifactType::Pr,
                        "pull_request",
                        format!("pulls/{number}"),
                        "github_pull_request",
                        detail.commit_ids(),
                    )
                } else {
                    let comments = if issue.get("comments").and_then(Value::as_i64).unwrap_or(0) > 0 {
                        self.github_comments(&client, &target, number).await?
                    } else {
                        Vec::new()
                    };
                    let state = if text_of(&issue, "state") == "closed" { "closed" } else { "open" };
                    (
                        render_issue(&target.owner, &target.repo, &issue, &comments),
                        state.to_owned(),
                        ArtifactType::Issue,
                        "issue",
                        format!("issues/{number}"),
                        "github_issue",
                        Vec::new(),
                    )
                };
                let content = content.into_bytes();
                let content_hash = StorageEngine::content_hash(&content);
                self.storage
                    .store_blob(&content_hash, &content, Some("text/markdown"))
                    .await?;
                let item = ConnectorItemWrite {
                    external_id: format!("{}:{number}", if is_pull { "pr" } else { "issue" }),
                    kind: kind.to_owned(),
                    artifact_type,
                    title: format!("#{number} {}", text_of(&issue, "title")),
                    path,
                    content_hash,
                    size_bytes: content.len() as i64,
                    state: state.clone(),
                    remote_updated_at: updated.clone(),
                    metadata: json!({
                        "origin": origin,
                        "provider": "github",
                        "repository": format!("{}/{}", target.owner, target.repo),
                        "number": number,
                        "url": text_of(&issue, "html_url"),
                        "state": state,
                        "author": user_of(&issue, "user"),
                        "opened_at": text_of(&issue, "created_at"),
                        "closed_at": issue.get("closed_at").cloned().unwrap_or(Value::Null),
                        "remote_updated_at": updated,
                        "commits": commits,
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

        self.finish_connector_sync(&source, metadata, report, job_id).await
    }

    /// Indexes what a sync stored, in the same job like a repository's files,
    /// and records the outcome on the source.
    async fn finish_connector_sync(
        &self,
        source: &Source,
        mut metadata: ConnectorMetadata,
        mut report: ConnectorSyncReport,
        job_id: &str,
    ) -> Result<ConnectorSyncReport> {
        let source_id = source.id.as_str();
        // What the items name (files, symbols, commits), so a file's page can
        // list the items that mention it.
        if report.added + report.updated > 0 {
            if let Err(error) = self.refresh_workspace_mentions(&source.workspace_id).await {
                tracing::warn!(connector = %source.name, error = %format!("{error:#}"), "Mentions could not be refreshed");
            }
        }
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

    async fn github_comments(&self, client: &GitHub, target: &TrackerTarget, number: i64) -> Result<Vec<Value>> {
        self.github_list(
            client,
            format!(
                "{}/repos/{}/{}/issues/{number}/comments?per_page={PER_PAGE}",
                client.api_base, target.owner, target.repo
            ),
            MAX_COMMENT_PAGES,
        )
        .await
    }

    /// Up to `max_pages` pages of one list, as one list.
    async fn github_list(&self, client: &GitHub, first: String, max_pages: usize) -> Result<Vec<Value>> {
        let mut items = Vec::new();
        let mut url = Some(first);
        let mut pages = 0;
        while let Some(page_url) = url.take() {
            if pages >= max_pages {
                break;
            }
            pages += 1;
            let (page, next) = client.get(&page_url).await?;
            items.extend(page.as_array().cloned().unwrap_or_default());
            url = next;
        }
        Ok(items)
    }

    /// A pull request's own record, commits, reviews, inline review comments
    /// and conversation.
    async fn github_pull(&self, client: &GitHub, target: &TrackerTarget, number: i64, _issue: &Value) -> Result<PullDetail> {
        let base = format!("{}/repos/{}/{}", client.api_base, target.owner, target.repo);
        let (pull, _) = client.get(&format!("{base}/pulls/{number}")).await?;
        let commit_pages = MAX_PULL_COMMITS.div_ceil(PER_PAGE);
        let commits = self
            .github_list(client, format!("{base}/pulls/{number}/commits?per_page={PER_PAGE}"), commit_pages)
            .await?;
        let reviews = self
            .github_list(client, format!("{base}/pulls/{number}/reviews?per_page={PER_PAGE}"), MAX_COMMENT_PAGES)
            .await?;
        let review_comments = self
            .github_list(client, format!("{base}/pulls/{number}/comments?per_page={PER_PAGE}"), MAX_COMMENT_PAGES)
            .await?;
        let comments = self.github_comments(client, target, number).await?;
        Ok(PullDetail {
            pull,
            commits: commits.into_iter().take(MAX_PULL_COMMITS).collect(),
            reviews,
            review_comments,
            comments,
        })
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
    fn a_pull_request_becomes_markdown_with_its_commits_and_reviews() {
        let issue = json!({
            "number": 8, "title": "Fix double posting", "state": "closed", "user": {"login": "bo"},
            "created_at": "2026-01-05T00:00:00Z", "html_url": "https://github.com/acme/app/pull/8",
            "body": "Posts once per refund."
        });
        let detail = PullDetail {
            pull: json!({"merged": true, "merged_at": "2026-01-07T00:00:00Z", "base": {"ref": "main"},
                         "head": {"ref": "fix-double"}, "commits": 2, "changed_files": 3, "additions": 40, "deletions": 5}),
            commits: vec![
                json!({"sha": "0123456789abcdef0123456789abcdef01234567", "commit": {"message": "Guard the ledger\n\nlong", "author": {"name": "Bo"}}}),
            ],
            reviews: vec![json!({"user": {"login": "ana"}, "state": "CHANGES_REQUESTED", "submitted_at": "2026-01-06T00:00:00Z", "body": "Add a test."})],
            review_comments: vec![json!({"user": {"login": "ana"}, "path": "src/ledger.rs", "line": 42, "created_at": "2026-01-06T00:00:00Z", "body": "Why twice?"})],
            comments: vec![json!({"user": {"login": "bo"}, "created_at": "2026-01-06T01:00:00Z", "body": "Done."})],
        };
        assert!(detail.merged());
        assert_eq!(detail.commit_ids(), vec!["0123456789abcdef0123456789abcdef01234567".to_owned()]);
        let text = render_pull("acme", "app", &issue, &detail);
        assert!(text.contains("Pull request #8 in acme/app · merged · opened by @bo"));
        assert!(text.contains("main ← fix-double · merged 2026-01-07T00:00:00Z"));
        assert!(text.contains("2 commits · 3 files changed · +40 −5"));
        assert!(text.contains("- 0123456789ab Guard the ledger (Bo)"));
        assert!(text.contains("### @ana · changes requested · 2026-01-06T00:00:00Z\n\nAdd a test."));
        assert!(text.contains("### @ana on src/ledger.rs:42"));
        assert!(text.contains("## Discussion"));
    }

    #[test]
    fn the_next_page_comes_from_the_link_header() {
        let header = "<https://api.github.com/x?page=2>; rel=\"next\", <https://api.github.com/x?page=9>; rel=\"last\"";
        assert_eq!(next_link(header).as_deref(), Some("https://api.github.com/x?page=2"));
        assert_eq!(next_link("<https://api.github.com/x?page=1>; rel=\"prev\""), None);
        assert_eq!(next_link(""), None);
    }
}
