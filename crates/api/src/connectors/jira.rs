//! Jira Cloud (roadmap V0.8.5): the issues of one project become markdown artifacts.
//!
//! Read-only, like the other connectors: only `GET` requests are made, so nothing changes in Jira. The account is a
//! person's email and an API token (stored as `email:token`, sent as HTTP basic authentication). A project whose issues
//! carry **security levels** is refused when it is connected, and an issue that carries one is never copied, because the
//! copy would show it to people Jira hides it from. Issues are read oldest update first with a cursor; a descriptive
//! text in Atlassian's document format is flattened to plain text.

use super::*;

/// The newest `updated` time seen is looked back from by this much, because JQL reads minutes in the time zone of the
/// account that owns the token. Issues read again are found unchanged by their content hash, never stored twice.
const JQL_OVERLAP_HOURS: i64 = 26;
/// Comments read for one issue.
const MAX_JIRA_COMMENTS: usize = 100;

/// `https://acme.atlassian.net/browse/KEY`, `.../browse/KEY-12`, `.../projects/KEY` or `.../jira/software/projects/KEY/...`;
/// `http` only for this machine.
pub(super) fn parse_jira_link(link: &str) -> Result<TrackerTarget> {
    let text = link.trim().trim_end_matches('/');
    let (scheme, rest) = text.split_once("://").ok_or_else(|| {
        repomemo_domain::invalid!("Enter the address of the project, such as https://acme.atlassian.net/browse/APP.")
    })?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "https" && scheme != "http" {
        repomemo_domain::bail_invalid!("Only https addresses can be connected.");
    }
    if text
        .chars()
        .any(|c| c.is_control() || c.is_whitespace() || matches!(c, '#' | '@' | '\\' | '%'))
    {
        repomemo_domain::bail_invalid!("That is not the address of a project.");
    }
    let text_without_query = rest.split('?').next().unwrap_or_default();
    let (authority, path) = text_without_query
        .split_once('/')
        .ok_or_else(|| repomemo_domain::invalid!("The address needs the project after the host."))?;
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
    let segments = path.split('/').filter(|part| !part.is_empty()).collect::<Vec<_>>();
    let key = segments
        .iter()
        .position(|part| matches!(*part, "browse" | "projects"))
        .and_then(|at| segments.get(at + 1))
        .map(|part| part.split('-').next().unwrap_or_default().to_owned())
        .unwrap_or_default();
    let valid = key.len() >= 2
        && key.len() <= 20
        && key.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        && key
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
    if !valid {
        repomemo_domain::bail_invalid!(
            "The address needs the project key after /browse/, such as https://acme.atlassian.net/browse/APP."
        );
    }
    let host_port = match port {
        Some(port) => format!("{host}:{port}"),
        None => host.clone(),
    };
    Ok(TrackerTarget {
        provider: "jira".to_owned(),
        api_base: format!("{scheme}://{host_port}/rest/api/3"),
        owner: host_port.clone(),
        repo: key.clone(),
        location: format!("{scheme}://{host_port}/browse/{key}"),
        host,
        port,
    })
}

/// An address under the API with its query encoded.
fn jira_url(base: &str, path: &str, params: &[(&str, &str)]) -> String {
    reqwest::Url::parse_with_params(&format!("{base}{path}"), params)
        .map(|url| url.to_string())
        .unwrap_or_else(|_| format!("{base}{path}"))
}

/// A person's email and token as `email:token`.
pub(super) fn split_jira_token(token: &str) -> Result<(&str, &str)> {
    match token.split_once(':') {
        Some((email, secret)) if !email.trim().is_empty() && !secret.trim().is_empty() => {
            Ok((email.trim(), secret.trim()))
        }
        _ => repomemo_domain::bail_invalid!(
            "Jira Cloud needs the account's email and an API token, written as email:token."
        ),
    }
}

/// Atlassian document format flattened to text: text, line breaks, mentions and links, with a line after each block.
fn adf_text(node: &Value, out: &mut String) {
    let kind = node.get("type").and_then(Value::as_str);
    match kind {
        Some("text") => out.push_str(node.get("text").and_then(Value::as_str).unwrap_or_default()),
        Some("hardBreak") => out.push('\n'),
        Some("mention") => out.push_str(
            node.get("attrs")
                .and_then(|attrs| attrs.get("text"))
                .and_then(Value::as_str)
                .unwrap_or("@someone"),
        ),
        Some("inlineCard") => out.push_str(
            node.get("attrs")
                .and_then(|attrs| attrs.get("url"))
                .and_then(Value::as_str)
                .unwrap_or_default(),
        ),
        _ => {}
    }
    if let Some(children) = node.get("content").and_then(Value::as_array) {
        for child in children {
            adf_text(child, out);
        }
    }
    if matches!(
        kind,
        Some("paragraph" | "heading" | "listItem" | "codeBlock" | "blockquote" | "tableRow")
    ) {
        out.push('\n');
    }
}

/// A description or comment: plain text (older shape) or a document.
fn body_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(document @ Value::Object(_)) => {
            let mut out = String::new();
            adf_text(document, &mut out);
            out.trim().to_owned()
        }
        _ => String::new(),
    }
}

fn person(fields: &Value, key: &str) -> String {
    fields
        .get(key)
        .and_then(|user| user.get("displayName"))
        .and_then(Value::as_str)
        .unwrap_or("Unassigned")
        .to_owned()
}

fn named(fields: &Value, key: &str) -> String {
    fields
        .get(key)
        .and_then(|value| value.get("name"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// The markdown a Jira issue becomes.
pub(super) fn render_jira_issue(site: &str, issue: &Value, comments: &[Value]) -> String {
    let key = text_of(issue, "key");
    let fields = issue.get("fields").cloned().unwrap_or(Value::Null);
    let labels = fields
        .get("labels")
        .and_then(Value::as_array)
        .map(|labels| labels.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", "))
        .unwrap_or_default();
    let status = named(&fields, "status");
    let category = fields
        .get("status")
        .and_then(|status| status.get("statusCategory"))
        .and_then(|category| category.get("name"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mut out = format!("# {key} {}\n\n", text_of(&fields, "summary"));
    out.push_str(&format!(
        "- **Type:** {} · **Status:** {status} ({category}) · **Priority:** {}\n",
        named(&fields, "issuetype"),
        named(&fields, "priority")
    ));
    out.push_str(&format!(
        "- **Reporter:** {} · **Assignee:** {}\n",
        person(&fields, "reporter"),
        person(&fields, "assignee")
    ));
    if !labels.is_empty() {
        out.push_str(&format!("- **Labels:** {labels}\n"));
    }
    out.push_str(&format!(
        "- **Created:** {} · **Updated:** {}\n- **Link:** {site}/browse/{key}\n",
        text_of(&fields, "created"),
        text_of(&fields, "updated")
    ));
    let description = body_text(fields.get("description"));
    if !description.is_empty() {
        out.push_str(&format!("\n## Description\n\n{}\n", clip(&description, MAX_BODY_CHARS)));
    }
    if !comments.is_empty() {
        out.push_str("\n## Discussion\n");
        for comment in comments {
            let author = comment
                .get("author")
                .and_then(|user| user.get("displayName"))
                .and_then(Value::as_str)
                .unwrap_or("someone");
            out.push_str(&format!(
                "\n### {author}, {}\n\n{}\n",
                text_of(comment, "created"),
                clip(&body_text(comment.get("body")), MAX_BODY_CHARS)
            ));
        }
    }
    out
}

/// The JQL time to read from: the cursor minus the overlap, in UTC (`yyyy-MM-dd HH:mm`).
fn jql_since(cursor: &str) -> Option<String> {
    let parsed = chrono::DateTime::parse_from_str(cursor, "%Y-%m-%dT%H:%M:%S%.3f%z")
        .or_else(|_| chrono::DateTime::parse_from_rfc3339(cursor))
        .ok()?;
    Some(
        (parsed.with_timezone(&chrono::Utc) - chrono::Duration::hours(JQL_OVERLAP_HOURS))
            .format("%Y-%m-%d %H:%M")
            .to_string(),
    )
}

impl RepoMemoCore {
    /// Looks at a Jira project before it is connected: it exists, the account may browse it, and its issues carry no
    /// security level. Only `GET` requests are made.
    pub(super) async fn check_jira(
        &self,
        workspace_id: &str,
        target: &TrackerTarget,
        token: Option<&str>,
    ) -> Result<ConnectorAccessCheck> {
        let token = token.map(str::trim).filter(|token| !token.is_empty());
        let Some(token) = token else {
            repomemo_domain::bail_invalid!(
                "Jira Cloud needs the account's email and an API token, written as email:token."
            );
        };
        split_jira_token(token)?;
        let mut client = self.github_for(None, target).await?;
        client.token = Some(token.to_owned());
        let key = target.repo.as_str();
        let (project, _) = client
            .get(&jira_url(&client.api_base, &format!("/project/{key}"), &[]))
            .await?;
        let can_browse = client
            .get(&jira_url(
                &client.api_base,
                "/mypermissions",
                &[("projectKey", key), ("permissions", "BROWSE_PROJECTS")],
            ))
            .await
            .ok()
            .and_then(|(answer, _)| {
                answer
                    .get("permissions")
                    .and_then(|permissions| permissions.get("BROWSE_PROJECTS"))
                    .and_then(|permission| permission.get("havePermission"))
                    .and_then(Value::as_bool)
            })
            .unwrap_or(true);
        // A project with an issue security scheme hides some issues from some people. A failed look is not a refusal.
        let secured = client
            .get(&jira_url(
                &client.api_base,
                &format!("/project/{key}/issuesecuritylevelscheme"),
                &[],
            ))
            .await
            .ok()
            .and_then(|(scheme, _)| {
                scheme
                    .get("levels")
                    .and_then(Value::as_array)
                    .map(|levels| !levels.is_empty())
            })
            .unwrap_or(false);
        let jql = format!("project = \"{key}\" AND statusCategory != Done");
        let open_issues = client
            .get(&jira_url(
                &client.api_base,
                "/search/jql",
                &[("jql", &jql), ("fields", "key"), ("maxResults", "100")],
            ))
            .await
            .ok()
            .and_then(|(found, _)| found.get("issues").and_then(Value::as_array).map(Vec::len))
            .unwrap_or(0) as i64;
        let already_connected = self
            .storage
            .list_sources_of_type(Some(workspace_id), repomemo_domain::SourceType::Connector)
            .await?
            .iter()
            .any(|source| source.root_uri.as_deref() == Some(target.location.as_str()));
        let name = format!("{} ({key})", text_of(&project, "name"));
        let refusal = if !can_browse {
            Some(format!(
                "The account cannot browse project {key}, so nothing it reads can be shared with the workspace."
            ))
        } else if secured {
            Some(format!(
                "Project {key} uses issue security levels. Importing it would show restricted issues to every member of the workspace, including people Jira hides them from, so it cannot be connected."
            ))
        } else {
            None
        };
        let preview = self
            .connector_preview(workspace_id, "jira", &name, true, refusal)
            .await?;
        Ok(ConnectorAccessCheck {
            provider: "jira".to_owned(),
            name,
            location: target.location.clone(),
            open_issues,
            private: true,
            already_connected,
            preview,
        })
    }

    /// Reads what changed in a Jira project since the last sync, stores and indexes it. The page is saved as it is stored,
    /// so an interruption resumes after it.
    pub(super) async fn sync_jira(
        &self,
        source: &Source,
        mut metadata: ConnectorMetadata,
        job_id: &str,
    ) -> Result<ConnectorSyncReport> {
        let target = parse_jira_link(source.root_uri.as_deref().unwrap_or_default())?;
        let client = self.github_for(Some(&source.id), &target).await?;
        self.storage
            .update_indexing_job(job_id, "running", "reading_tracker", 0, None)
            .await?;
        let key = target.repo.as_str();
        let site = format!(
            "{}://{}",
            if target.api_base.starts_with("http://") {
                "http"
            } else {
                "https"
            },
            target.owner
        );
        let mut jql = format!("project = \"{key}\"");
        if let Some(since) = metadata.cursor.as_deref().and_then(jql_since) {
            jql.push_str(&format!(" AND updated >= \"{since}\""));
        }
        jql.push_str(" ORDER BY updated ASC");
        let mut report = ConnectorSyncReport::default();
        let mut page_token: Option<String> = None;
        let mut seen_tokens = std::collections::HashSet::new();
        let mut pages = 0;
        'pages: loop {
            if pages >= MAX_PAGES_PER_SYNC {
                report.more_to_fetch = true;
                break;
            }
            pages += 1;
            let mut params = vec![
                ("jql", jql.as_str()),
                (
                    "fields",
                    "summary,status,issuetype,priority,assignee,reporter,labels,created,updated,description,security",
                ),
                ("maxResults", "100"),
            ];
            if let Some(token) = page_token.as_deref() {
                params.push(("nextPageToken", token));
            }
            let (page, _) = client.get(&jira_url(&client.api_base, "/search/jql", &params)).await?;
            for issue in page
                .get("issues")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
            {
                if client.calls() >= MAX_REQUESTS_PER_SYNC {
                    report.more_to_fetch = true;
                    break 'pages;
                }
                report.items_seen += 1;
                let fields = issue.get("fields").cloned().unwrap_or(Value::Null);
                // An issue with a security level is never copied: Jira hides it from some people.
                let issue_key = text_of(&issue, "key");
                let remote_id = text_of(&issue, "id");
                if fields.get("security").is_some_and(|security| !security.is_null()) {
                    report.restricted_skipped += 1;
                    // A copy made before the issue was restricted goes too.
                    if self
                        .storage
                        .remove_connector_item(&source.id, &format!("issue:{remote_id}"))
                        .await?
                    {
                        report.removed += 1;
                    }
                    continue;
                }
                let (comment_page, _) = client
                    .get(&jira_url(
                        &client.api_base,
                        &format!("/issue/{issue_key}/comment"),
                        &[("maxResults", &MAX_JIRA_COMMENTS.to_string())],
                    ))
                    .await?;
                let comments = comment_page
                    .get("comments")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let content = render_jira_issue(&site, &issue, &comments).into_bytes();
                let content_hash = StorageEngine::content_hash(&content);
                self.storage
                    .store_blob(&content_hash, &content, Some("text/markdown"))
                    .await?;
                let updated = text_of(&fields, "updated");
                let done = fields
                    .get("status")
                    .and_then(|status| status.get("statusCategory"))
                    .and_then(|category| category.get("key"))
                    .and_then(Value::as_str)
                    == Some("done");
                let state = if done { "closed" } else { "open" };
                let number = issue_key
                    .rsplit('-')
                    .next()
                    .and_then(|number| number.parse::<i64>().ok())
                    .unwrap_or(0);
                let write = ConnectorItemWrite {
                    // Jira's own id never changes, even when an issue is renamed or moved.
                    external_id: format!("issue:{remote_id}"),
                    kind: "issue".to_owned(),
                    artifact_type: ArtifactType::Issue,
                    title: format!("{issue_key} {}", text_of(&fields, "summary")),
                    path: format!("issues/{issue_key}"),
                    content_hash,
                    size_bytes: content.len() as i64,
                    state: state.to_owned(),
                    remote_updated_at: updated.clone(),
                    metadata: json!({
                        "origin": "jira_issue",
                        "provider": "jira",
                        "repository": key,
                        "number": number,
                        "key": issue_key,
                        "url": format!("{site}/browse/{issue_key}"),
                        "state": state,
                        "author": person(&fields, "reporter"),
                        "opened_at": text_of(&fields, "created"),
                        "remote_updated_at": updated,
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
                    metadata.cursor = Some(updated);
                }
            }
            page_token = next_page_token(&page, &mut seen_tokens)?;
            // The page is stored: an interruption resumes after it.
            self.checkpoint_connector(source, &metadata).await?;
            if page_token.is_none() {
                break;
            }
        }
        // Only a sync that read everything can tell what is gone.
        if !report.more_to_fetch {
            self.reconcile_jira(source, &client, key, &mut report).await?;
        }
        self.finish_connector_sync(source, metadata, report, job_id).await
    }
}

/// The token of the next page, or none at the last. A token seen before is a tracker that repeats itself: the sync stops
/// instead of looping.
fn next_page_token(page: &Value, seen: &mut std::collections::HashSet<String>) -> Result<Option<String>> {
    if page.get("isLast").and_then(Value::as_bool).unwrap_or(false) {
        return Ok(None);
    }
    let Some(token) = page.get("nextPageToken").and_then(Value::as_str) else {
        return Ok(None);
    };
    if !seen.insert(token.to_owned()) {
        repomemo_domain::bail_invalid!(
            "The tracker repeated a page of results, so the sync stopped. Choose Sync now to try again."
        );
    }
    Ok(Some(token.to_owned()))
}

impl RepoMemoCore {
    /// Takes out of the evidence the issues Jira no longer lists for this account: deleted, moved to another project, or
    /// given a security level since. The whole project is listed (ids and security only); a list that is too long to read in
    /// one sync, or that would remove every issue kept, is not trusted and changes nothing (the access check handles a
    /// lost project).
    async fn reconcile_jira(
        &self,
        source: &Source,
        client: &GitHub,
        key: &str,
        report: &mut ConnectorSyncReport,
    ) -> Result<()> {
        let jql = format!("project = \"{key}\" ORDER BY created ASC");
        let mut visible = std::collections::HashSet::new();
        let mut seen_tokens = std::collections::HashSet::new();
        let mut token: Option<String> = None;
        let mut complete = false;
        for _ in 0..MAX_PAGES_PER_SYNC {
            let mut params = vec![("jql", jql.as_str()), ("fields", "security"), ("maxResults", "100")];
            if let Some(next) = token.as_deref() {
                params.push(("nextPageToken", next));
            }
            let (page, _) = client.get(&jira_url(&client.api_base, "/search/jql", &params)).await?;
            for issue in page.get("issues").and_then(Value::as_array).into_iter().flatten() {
                let restricted = issue
                    .get("fields")
                    .and_then(|fields| fields.get("security"))
                    .is_some_and(|security| !security.is_null());
                if !restricted {
                    visible.insert(format!("issue:{}", text_of(issue, "id")));
                }
            }
            token = next_page_token(&page, &mut seen_tokens)?;
            if token.is_none() {
                complete = true;
                break;
            }
        }
        if !complete {
            return Ok(());
        }
        let stored = self
            .storage
            .connector_external_ids(&source.id)
            .await?
            .into_iter()
            .filter(|id| id.starts_with("issue:"))
            .collect::<Vec<_>>();
        let gone = stored.iter().filter(|id| !visible.contains(*id)).collect::<Vec<_>>();
        if gone.is_empty() {
            return Ok(());
        }
        if gone.len() == stored.len() {
            tracing::warn!(connector = %source.name, "Jira listed none of the stored issues; nothing was removed");
            return Ok(());
        }
        for id in gone {
            if self.storage.remove_connector_item(&source.id, id).await? {
                report.removed += 1;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_addresses_name_the_site_and_the_key() {
        for link in [
            "https://acme.atlassian.net/browse/APP",
            "https://acme.atlassian.net/browse/APP-12",
            "https://acme.atlassian.net/jira/software/projects/APP/boards/1",
            "https://acme.atlassian.net/projects/APP?selectedItem=x",
        ] {
            let target = parse_jira_link(link).unwrap();
            assert_eq!(target.repo, "APP", "{link}");
            assert_eq!(target.api_base, "https://acme.atlassian.net/rest/api/3");
            assert_eq!(target.location, "https://acme.atlassian.net/browse/APP");
            assert_eq!(target.host, "acme.atlassian.net");
        }
        let local = parse_jira_link("http://127.0.0.1:9000/browse/APP").unwrap();
        assert_eq!(local.api_base, "http://127.0.0.1:9000/rest/api/3");
        assert_eq!(local.port, Some(9000));
        for refused in [
            "",
            "acme.atlassian.net/browse/APP",
            "http://acme.atlassian.net/browse/APP",
            "https://acme.atlassian.net/browse/app",
            "https://acme.atlassian.net/",
            "https://acme.atlassian.net/browse/A",
            "https://user@acme.atlassian.net/browse/APP",
        ] {
            assert!(parse_jira_link(refused).is_err(), "{refused:?}");
        }
    }

    #[test]
    fn an_account_is_an_email_and_a_token() {
        assert_eq!(
            split_jira_token("ana@acme.com:abc123").unwrap(),
            ("ana@acme.com", "abc123")
        );
        assert!(split_jira_token("abc123").is_err());
        assert!(split_jira_token("ana@acme.com:").is_err());
    }

    #[test]
    fn a_document_becomes_text_and_an_issue_becomes_markdown() {
        let issue = json!({
            "id": "10001", "key": "APP-7",
            "fields": {
                "summary": "Refunds double post", "labels": ["billing"], "created": "2026-01-01T00:00:00.000+0000",
                "updated": "2026-01-02T00:00:00.000+0000",
                "status": {"name": "In Progress", "statusCategory": {"name": "In Progress", "key": "indeterminate"}},
                "issuetype": {"name": "Bug"}, "priority": {"name": "High"},
                "reporter": {"displayName": "Ana"}, "assignee": null,
                "description": {"type": "doc", "content": [
                    {"type": "paragraph", "content": [{"type": "text", "text": "The ledger posts twice."}]},
                    {"type": "paragraph", "content": [{"type": "mention", "attrs": {"text": "@bo"}}]}
                ]}
            }
        });
        let comments = vec![json!({"author": {"displayName": "Bo"}, "created": "2026-01-03",
            "body": {"type": "doc", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "Reproduced."}]}]}})];
        let text = render_jira_issue("https://acme.atlassian.net", &issue, &comments);
        for expected in [
            "# APP-7 Refunds double post",
            "**Status:** In Progress (In Progress)",
            "**Assignee:** Unassigned",
            "The ledger posts twice.",
            "@bo",
            "### Bo, 2026-01-03",
            "Reproduced.",
            "https://acme.atlassian.net/browse/APP-7",
        ] {
            assert!(text.contains(expected), "{expected}\n{text}");
        }
    }

    #[test]
    fn the_cursor_is_read_back_with_an_overlap() {
        assert_eq!(
            jql_since("2026-01-02T10:30:00.000+0000").as_deref(),
            Some("2026-01-01 08:30")
        );
        assert_eq!(jql_since("not a time"), None);
    }
}
