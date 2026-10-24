//! GitLab, including self-hosted (roadmap F17c, V0.2.29).
//!
//! The same model as GitHub: a project's issues and merge requests become
//! markdown artifacts, synced incrementally (`updated_after`, oldest update
//! first), closed ones *outdated*, merged requests staying active. GitLab's
//! answers are reshaped into the GitHub-shaped records the renderers already
//! read, so both providers produce the same text and the same metadata.
//!
//! The token goes in `PRIVATE-TOKEN` and only to the API's own host, like
//! GitHub's. A self-hosted GitLab is told apart from GitHub Enterprise by the
//! address (a host with `gitlab` in it) or by the provider chosen when
//! connecting.

use super::*;

/// `https://gitlab.com/group/sub/project(.git)` or a self-hosted address of
/// the same shape; `http` only for this machine.
pub(super) fn parse_gitlab_link(link: &str) -> Result<TrackerTarget> {
    let text = link.trim().trim_end_matches('/');
    let (scheme, rest) = text.split_once("://").ok_or_else(|| {
        repomemo_domain::invalid!("Enter the address of the project, such as https://gitlab.com/group/project.")
    })?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "https" && scheme != "http" {
        repomemo_domain::bail_invalid!("Only https addresses can be connected.");
    }
    if text
        .chars()
        .any(|c| c.is_control() || c.is_whitespace() || matches!(c, '?' | '#' | '@' | '\\' | '%'))
    {
        repomemo_domain::bail_invalid!("That is not the address of a project.");
    }
    let (authority, path) = rest
        .split_once('/')
        .ok_or_else(|| repomemo_domain::invalid!("The address needs the group and the project after the host."))?;
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
    let mut segments = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if segments.len() < 2 || segments.len() > 10 {
        repomemo_domain::bail_invalid!("The address needs the group and the project after the host.");
    }
    if let Some(last) = segments.last_mut() {
        if let Some(stripped) = last.strip_suffix(".git") {
            *last = stripped.to_owned();
        }
    }
    let valid = |name: &String| {
        !name.is_empty()
            && name.len() <= 100
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            && !name.starts_with('.')
    };
    if !segments.iter().all(valid) {
        repomemo_domain::bail_invalid!("That is not the address of a project.");
    }
    let repo = segments.pop().unwrap_or_default();
    let owner = segments.join("/");
    let host_port = match port {
        Some(port) => format!("{host}:{port}"),
        None => host.clone(),
    };
    Ok(TrackerTarget {
        provider: "gitlab".to_owned(),
        api_base: format!("{scheme}://{host_port}/api/v4"),
        location: format!("{scheme}://{host_port}/{owner}/{repo}"),
        owner,
        repo,
        host,
        port,
    })
}

/// The project as the API addresses it: its path, with `/` as `%2F`.
pub(super) fn project_id(target: &TrackerTarget) -> String {
    format!("{}%2F{}", target.owner.replace('/', "%2F"), target.repo)
}

/// A GitLab issue or merge request in the shape the GitHub renderers read.
fn issue_like(item: &Value) -> Value {
    let state = match item.get("state").and_then(Value::as_str).unwrap_or("opened") {
        "closed" | "locked" | "merged" => "closed",
        _ => "open",
    };
    let username = |value: &Value| {
        value
            .get("username")
            .and_then(Value::as_str)
            .unwrap_or("ghost")
            .to_owned()
    };
    json!({
        "number": item.get("iid").cloned().unwrap_or(Value::Null),
        "title": text_of(item, "title"),
        "state": state,
        "user": {"login": item.get("author").map(username).unwrap_or_else(|| "ghost".to_owned())},
        "created_at": text_of(item, "created_at"),
        "updated_at": text_of(item, "updated_at"),
        "closed_at": item.get("closed_at").cloned().unwrap_or(Value::Null),
        "html_url": text_of(item, "web_url"),
        "body": text_of(item, "description"),
        "labels": item.get("labels").cloned().unwrap_or_else(|| json!([])),
        "assignees": item
            .get("assignees")
            .and_then(Value::as_array)
            .map(|people| people.iter().map(|person| json!({"login": username(person)})).collect::<Vec<_>>())
            .unwrap_or_default(),
        "comments": item.get("user_notes_count").cloned().unwrap_or(json!(0)),
    })
}

fn is_system(note: &Value) -> bool {
    note.get("system").and_then(Value::as_bool).unwrap_or(false)
}

/// Notes as GitHub-shaped comments, without the system ones ("changed the
/// description", "added 1 commit").
fn comments_like(notes: &[Value]) -> Vec<Value> {
    notes
        .iter()
        .filter(|note| !is_system(note) && note.get("type").and_then(Value::as_str) != Some("DiffNote"))
        .map(|note| {
            json!({
                "user": {"login": note.get("author").and_then(|a| a.get("username")).cloned().unwrap_or(json!("ghost"))},
                "created_at": text_of(note, "created_at"),
                "body": text_of(note, "body"),
            })
        })
        .collect()
}

fn review_comments_like(notes: &[Value]) -> Vec<Value> {
    notes
        .iter()
        .filter(|note| !is_system(note) && note.get("type").and_then(Value::as_str) == Some("DiffNote"))
        .map(|note| {
            let position = note.get("position").cloned().unwrap_or(Value::Null);
            json!({
                "user": {"login": note.get("author").and_then(|a| a.get("username")).cloned().unwrap_or(json!("ghost"))},
                "created_at": text_of(note, "created_at"),
                "body": text_of(note, "body"),
                "path": position.get("new_path").cloned().unwrap_or(Value::Null),
                "line": position.get("new_line").cloned().unwrap_or(Value::Null),
            })
        })
        .collect()
}

fn pull_like(merge_request: &Value, detail: &Value, commits: &[Value]) -> Value {
    let merged = merge_request.get("state").and_then(Value::as_str) == Some("merged");
    let figure = detail
        .get("changes_count")
        .and_then(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .or_else(|| value.as_i64().map(|n| n.to_string()))
        })
        .and_then(|text| text.trim_end_matches('+').parse::<i64>().ok())
        .unwrap_or(0);
    json!({
        "merged": merged,
        "merged_at": merge_request.get("merged_at").cloned().unwrap_or(Value::Null),
        "base": {"ref": text_of(merge_request, "target_branch")},
        "head": {"ref": text_of(merge_request, "source_branch")},
        "commits": commits.len(),
        "changed_files": figure,
        "additions": 0,
        "deletions": 0,
    })
}

/// GitLab's approvals of a merge request as GitHub-shaped reviews: an approval carries no date or text.
fn approvals_like(approvals: &Value) -> Vec<Value> {
    approvals
        .get("approved_by")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .map(|entry| {
                    json!({
                        "user": {"login": entry.get("user").and_then(|user| user.get("username")).cloned().unwrap_or(json!("ghost"))},
                        "state": "APPROVED",
                        "submitted_at": "",
                        "body": "",
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn commits_like(commits: &[Value]) -> Vec<Value> {
    commits
        .iter()
        .map(|commit| {
            json!({
                "sha": text_of(commit, "id"),
                "commit": {
                    "message": text_of(commit, "title"),
                    "author": {"name": text_of(commit, "author_name")},
                },
            })
        })
        .collect()
}

impl RepoMemoCore {
    /// What GitLab answers for a project, before it is connected.
    pub(super) async fn check_gitlab(
        &self,
        workspace_id: &str,
        target: &TrackerTarget,
        token: Option<&str>,
    ) -> Result<ConnectorAccessCheck> {
        let mut client = self.github_for(None, target).await?;
        client.token = token
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .map(str::to_owned);
        let (project, _) = client
            .get(&format!("{}/projects/{}", client.api_base, project_id(target)))
            .await?;
        let already_connected = self
            .storage
            .list_sources_of_type(Some(workspace_id), repomemo_domain::SourceType::Connector)
            .await?
            .iter()
            .any(|source| source.root_uri.as_deref() == Some(target.location.as_str()));
        Ok(ConnectorAccessCheck {
            provider: "gitlab".to_owned(),
            name: text_of(&project, "path_with_namespace"),
            location: target.location.clone(),
            open_issues: project.get("open_issues_count").and_then(Value::as_i64).unwrap_or(0),
            private: project.get("visibility").and_then(Value::as_str) != Some("public"),
            already_connected,
        })
    }

    /// Reads what changed in a GitLab project since the last sync, stores and
    /// indexes it.
    pub(super) async fn sync_gitlab(
        &self,
        source: &Source,
        mut metadata: ConnectorMetadata,
        job_id: &str,
    ) -> Result<ConnectorSyncReport> {
        let target = parse_gitlab_link(source.root_uri.as_deref().unwrap_or_default())?;
        let client = self.github_for(Some(&source.id), &target).await?;
        self.storage
            .update_indexing_job(job_id, "running", "reading_tracker", 0, None)
            .await?;
        let base = format!("{}/projects/{}", client.api_base, project_id(&target));
        let mut report = ConnectorSyncReport::default();

        'endpoints: for merge_requests in [false, true] {
            let endpoint = if merge_requests { "merge_requests" } else { "issues" };
            let cursor = if merge_requests {
                metadata.pulls_cursor.clone()
            } else {
                metadata.cursor.clone()
            };
            let mut url = Some({
                let mut url =
                    format!("{base}/{endpoint}?scope=all&state=all&order_by=updated_at&sort=asc&per_page={PER_PAGE}");
                if let Some(since) = &cursor {
                    url.push_str(&format!("&updated_after={}", since.replace('+', "%2B")));
                }
                url
            });
            let mut pages = 0;
            while let Some(page_url) = url.take() {
                if pages >= MAX_PAGES_PER_SYNC {
                    report.more_to_fetch = true;
                    break 'endpoints;
                }
                pages += 1;
                let (page, next) = client.get(&page_url).await?;
                for item in page.as_array().cloned().unwrap_or_default() {
                    if client.calls() >= MAX_REQUESTS_PER_SYNC {
                        report.more_to_fetch = true;
                        break 'endpoints;
                    }
                    report.items_seen += 1;
                    let iid = item.get("iid").and_then(Value::as_i64).unwrap_or(0);
                    let updated = text_of(&item, "updated_at");
                    let issue = issue_like(&item);
                    let notes = if item.get("user_notes_count").and_then(Value::as_i64).unwrap_or(0) > 0
                        || merge_requests
                    {
                        self.github_list(
                            &client,
                            format!("{base}/{endpoint}/{iid}/notes?sort=asc&order_by=created_at&per_page={PER_PAGE}"),
                            MAX_COMMENT_PAGES,
                        )
                        .await?
                    } else {
                        Vec::new()
                    };
                    let (content, state, artifact_type, kind, path, origin, commit_ids, review) = if merge_requests {
                        report.pull_requests_seen += 1;
                        let (detail, _) = client.get(&format!("{base}/merge_requests/{iid}")).await?;
                        let commits = self
                            .github_list(
                                &client,
                                format!("{base}/merge_requests/{iid}/commits?per_page={PER_PAGE}"),
                                MAX_PULL_COMMITS.div_ceil(PER_PAGE),
                            )
                            .await?;
                        let commits = commits_like(&commits);
                        // Approvals are a nicety: a project that does not offer them, or a refusal here, must not
                        // stop the sync or pause the connector.
                        let approvals = client
                            .get(&format!("{base}/merge_requests/{iid}/approvals"))
                            .await
                            .map(|(value, _)| value)
                            .unwrap_or(Value::Null);
                        let pull = PullDetail {
                            pull: pull_like(&item, &detail, &commits),
                            commits,
                            reviews: approvals_like(&approvals),
                            review_comments: review_comments_like(&notes),
                            comments: comments_like(&notes),
                        };
                        let state = if pull.merged() {
                            "merged"
                        } else if issue.get("state").and_then(Value::as_str) == Some("closed") {
                            "closed"
                        } else {
                            "open"
                        };
                        (
                            render_pull(&target.owner, &target.repo, &issue, &pull),
                            state.to_owned(),
                            ArtifactType::Pr,
                            "merge_request",
                            format!("merge_requests/{iid}"),
                            "gitlab_merge_request",
                            pull.commit_ids(),
                            pull.review_outcome(),
                        )
                    } else {
                        let state = if issue.get("state").and_then(Value::as_str) == Some("closed") {
                            "closed"
                        } else {
                            "open"
                        };
                        (
                            render_issue(&target.owner, &target.repo, &issue, &comments_like(&notes)),
                            state.to_owned(),
                            ArtifactType::Issue,
                            "issue",
                            format!("issues/{iid}"),
                            "gitlab_issue",
                            Vec::new(),
                            ReviewOutcome::default(),
                        )
                    };
                    let content = content.into_bytes();
                    let content_hash = StorageEngine::content_hash(&content);
                    self.storage
                        .store_blob(&content_hash, &content, Some("text/markdown"))
                        .await?;
                    let write = ConnectorItemWrite {
                        external_id: format!("{}:{iid}", if merge_requests { "mr" } else { "issue" }),
                        kind: kind.to_owned(),
                        artifact_type,
                        title: format!("#{iid} {}", text_of(&item, "title")),
                        path,
                        content_hash,
                        size_bytes: content.len() as i64,
                        state: state.clone(),
                        remote_updated_at: updated.clone(),
                        metadata: json!({
                            "origin": origin,
                            "provider": "gitlab",
                            "repository": format!("{}/{}", target.owner, target.repo),
                            "number": iid,
                            "url": text_of(&item, "web_url"),
                            "state": state,
                            "author": user_of(&issue, "user"),
                            "opened_at": text_of(&item, "created_at"),
                            "closed_at": item.get("closed_at").cloned().unwrap_or(Value::Null),
                            "remote_updated_at": updated,
                            "commits": commit_ids,
                            "approved_by": review.approved_by,
                            "changes_requested_by": review.changes_requested_by,
                        }),
                    };
                    let (_, outcome) = self
                        .storage
                        .write_connector_item(&source.workspace_id, &source.id, &write)
                        .await?;
                    match outcome {
                        ConnectorItemOutcome::Added => report.added += 1,
                        ConnectorItemOutcome::Updated => report.updated += 1,
                        ConnectorItemOutcome::Unchanged => report.unchanged += 1,
                    }
                    if !updated.is_empty() {
                        if merge_requests {
                            metadata.pulls_cursor = Some(updated);
                        } else {
                            metadata.cursor = Some(updated);
                        }
                    }
                }
                url = next;
            }
        }
        self.finish_connector_sync(source, metadata, report, job_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_addresses_keep_their_groups() {
        let hosted = parse_gitlab_link("https://gitlab.com/acme/platform/ledger.git").unwrap();
        assert_eq!(hosted.api_base, "https://gitlab.com/api/v4");
        assert_eq!(
            (hosted.owner.as_str(), hosted.repo.as_str()),
            ("acme/platform", "ledger")
        );
        assert_eq!(project_id(&hosted), "acme%2Fplatform%2Fledger");
        assert_eq!(hosted.location, "https://gitlab.com/acme/platform/ledger");

        let own = parse_gitlab_link("https://git.corp.example:8443/team/app").unwrap();
        assert_eq!(own.api_base, "https://git.corp.example:8443/api/v4");
        assert_eq!((own.host.as_str(), own.port), ("git.corp.example", Some(8443)));

        for refused in [
            "",
            "gitlab.com/acme/app",
            "http://gitlab.com/acme/app",
            "https://gitlab.com/acme",
            "https://gitlab.com/acme/%2e%2e/app",
            "https://user@gitlab.com/acme/app",
            "https://gitlab.com/acme/app?x=1",
            "https://gitlab.com/../app",
        ] {
            assert!(parse_gitlab_link(refused).is_err(), "{refused:?}");
        }
    }

    #[test]
    fn gitlab_records_become_the_shapes_the_renderers_read() {
        let issue = issue_like(&json!({
            "iid": 4, "title": "Rounding", "state": "closed", "author": {"username": "ana"},
            "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-02T00:00:00Z",
            "closed_at": "2026-01-02T00:00:00Z", "web_url": "https://gitlab.com/a/b/-/issues/4",
            "description": "Round half up.", "labels": ["bug"], "assignees": [{"username": "bo"}],
            "user_notes_count": 2
        }));
        let text = render_issue("a", "b", &issue, &[]);
        assert!(text.contains("Issue #4 in a/b · closed · opened by @ana"));
        assert!(text.contains("Labels: bug"));
        assert!(text.contains("Assignees: @bo"));

        let notes = vec![
            json!({"system": true, "body": "added 1 commit", "author": {"username": "bo"}, "created_at": "x"}),
            json!({"system": false, "body": "Looks fine", "author": {"username": "bo"}, "created_at": "2026-01-03T00:00:00Z"}),
            json!({"system": false, "type": "DiffNote", "body": "Why twice?", "author": {"username": "ana"},
                   "created_at": "2026-01-03T01:00:00Z", "position": {"new_path": "src/a.rs", "new_line": 9}}),
        ];
        assert_eq!(comments_like(&notes).len(), 1);
        let review = review_comments_like(&notes);
        assert_eq!(review.len(), 1);
        assert_eq!(review[0]["path"], "src/a.rs");
        assert_eq!(review[0]["line"], 9);

        let pull = pull_like(
            &json!({"state": "merged", "merged_at": "2026-01-05T00:00:00Z", "target_branch": "main", "source_branch": "fix"}),
            &json!({"changes_count": "12+"}),
            &commits_like(&[json!({"id": "abc", "title": "Guard", "author_name": "Bo"})]),
        );
        assert_eq!(pull["merged"], true);
        assert_eq!(pull["changed_files"], 12);
        assert_eq!(pull["base"]["ref"], "main");
        assert_eq!(pull["commits"], 1);
    }
}
