//! Public snapshots (V0.8.16): the preview and the record. There is no public route yet (V0.8.18).
//!
//! A snapshot is made from one file's text by [`make_snapshot`]: everything that could point at or name something private is
//! taken out and listed, so the publisher sees exactly what would be public. Publishing needs the review token of that preview,
//! which is the hash of the snapshot, so what is recorded is what was previewed and nothing else. Only text files of the workspace
//! can be published; mirrored tracker items, attachments and images cannot. It needs the operator's `REPOMEMO_PUBLIC_PUBLISHING`.

use crate::*;
use repomemo_domain::Publication;

const MAX_SNAPSHOT_BYTES: usize = 500_000;

#[derive(Debug, Serialize)]
pub(crate) struct SnapshotPreview {
    title: String,
    body_markdown: String,
    /// What was taken out, so nothing disappears silently.
    removed: Vec<String>,
    /// Send this back to publish exactly this text.
    review_token: String,
}

fn hash(text: &str) -> String {
    repomemo_storage::StorageEngine::content_hash(text.as_bytes())
}

/// The text a snapshot holds, and what was removed from it: images and raw HTML, comments in HTML, and every link that is not a
/// plain `https` address (a link to a file, task or app page is replaced by its text, so no private address survives).
pub(crate) fn make_snapshot(source: &str) -> (String, Vec<String>) {
    let mut removed = Vec::new();
    let mut out = String::new();
    let mut images = 0;
    let mut private_links = 0;
    let mut html = 0;
    let mut emails = 0;
    let mut fenced = false;
    for line in source.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if fenced {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        let mut rest = line;
        let mut clean = String::new();
        while let Some(open) = rest.find('<') {
            // Raw HTML (a tag or a comment) is not published.
            match rest[open..].find('>') {
                Some(close)
                    if rest[open + 1..]
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_ascii_alphabetic() || c == '/' || c == '!') =>
                {
                    clean.push_str(&rest[..open]);
                    rest = &rest[open + close + 1..];
                    html += 1;
                }
                _ => {
                    clean.push_str(&rest[..=open]);
                    rest = &rest[open + 1..];
                }
            }
        }
        clean.push_str(rest);
        let linked = rewrite_links(&clean, &mut images, &mut private_links);
        let masked = linked
            .split(' ')
            .map(|word| {
                let core = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '@');
                if core.contains('@')
                    && !core.starts_with('@')
                    && core.rsplit('@').next().is_some_and(|d| d.contains('.'))
                    && !word.contains("](")
                {
                    emails += 1;
                    word.replace(core, "[email removed]")
                } else {
                    word.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        out.push_str(&masked);
        out.push('\n');
    }
    if images > 0 {
        removed.push(format!("{images} image(s)"));
    }
    if private_links > 0 {
        removed.push(format!("{private_links} link(s) to files, pages or addresses that are not public plain https addresses (their text is kept)"));
    }
    if emails > 0 {
        removed.push(format!("{emails} email address(es)"));
    }
    if html > 0 {
        removed.push(format!("{html} piece(s) of raw HTML"));
    }
    (out.trim().to_owned(), removed)
}

fn rewrite_links(line: &str, images: &mut usize, private_links: &mut usize) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(at) = rest.find("](") {
        // Find the `[` that opens this link text.
        let Some(open) = rest[..at].rfind('[') else {
            out.push_str(&rest[..at + 2]);
            rest = &rest[at + 2..];
            continue;
        };
        let after = &rest[at + 2..];
        let Some(close) = after.find(')') else {
            out.push_str(&rest[..at + 2]);
            rest = after;
            continue;
        };
        let target = after[..close]
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .trim_matches(['<', '>']);
        let is_image = rest[..open].ends_with('!');
        let before = if is_image { &rest[..open - 1] } else { &rest[..open] };
        let text = &rest[open + 1..at];
        out.push_str(before);
        if is_image {
            *images += 1;
        } else if target.starts_with("https://") && !target.contains(['@', ' ']) {
            out.push_str(&format!("[{text}]({target})"));
        } else {
            *private_links += 1;
            out.push_str(text);
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

async fn load_publishable(
    state: &AppState,
    subject: &AuthenticatedSubject,
    artifact_id: &str,
) -> Result<(repomemo_domain::ArtifactSummary, String), ApiError> {
    if !state.public_publishing {
        return Err(ApiError::not_found("Public publishing is off on this server."));
    }
    let artifact = state
        .storage
        .get_artifact_summary(artifact_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_admin(state, subject, &artifact.workspace_id).await?;
    if state
        .storage
        .remote_item_info(artifact_id)
        .await
        .map_err(map_storage_error)?
        .is_some()
    {
        return Err(ApiError::bad_request(
            "An item mirrored from a tracker cannot be published: the tracker decides who may read it.",
        ));
    }
    // A mirrored item that came back through an archive is a plain file now, but its origin still says a tracker decides who may read it.
    let detail = state
        .storage
        .get_artifact(artifact_id)
        .await
        .map_err(map_storage_error)?;
    let origin = detail
        .metadata
        .get("origin")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    if matches!(
        origin,
        "github_issue" | "github_pull_request" | "gitlab_issue" | "gitlab_merge_request" | "jira_issue"
    ) {
        return Err(ApiError::bad_request(
            "An item mirrored from a tracker cannot be published: the tracker decides who may read it.",
        ));
    }
    let publishable = artifact
        .mime_type
        .as_deref()
        .is_some_and(|mime| mime.starts_with("text/"));
    if !publishable {
        return Err(ApiError::bad_request("Only a text or Markdown file can be published."));
    }
    let bytes = state
        .storage
        .read_artifact_blob(artifact_id)
        .await
        .map_err(map_storage_error)?;
    if bytes.len() > MAX_SNAPSHOT_BYTES {
        return Err(ApiError::bad_request("The file is too large to publish."));
    }
    Ok((artifact, String::from_utf8_lossy(&bytes).into_owned()))
}

pub(crate) async fn preview(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
) -> Result<Json<SnapshotPreview>, ApiError> {
    let (artifact, text) = load_publishable(&state, &subject, &artifact_id).await?;
    let (body, removed) = make_snapshot(&text);
    Ok(Json(SnapshotPreview {
        title: artifact.title,
        review_token: hash(&body),
        body_markdown: body,
        removed,
    }))
}

#[derive(Debug, Deserialize)]
pub(crate) struct PublishRequest {
    /// The token of the preview that was reviewed.
    review_token: String,
    #[serde(default)]
    title: Option<String>,
}

pub(crate) async fn publish(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
    Json(request): Json<PublishRequest>,
) -> Result<(StatusCode, Json<Publication>), ApiError> {
    let (artifact, text) = load_publishable(&state, &subject, &artifact_id).await?;
    let (body, _) = make_snapshot(&text);
    let snapshot_hash = hash(&body);
    if request.review_token != snapshot_hash {
        return Err(ApiError::conflict(
            "The file changed since it was previewed. Preview it again and review what would be public.",
        ));
    }
    if state
        .storage
        .live_publication_of(&artifact_id)
        .await
        .map_err(map_storage_error)?
        .is_some()
    {
        return Err(ApiError::conflict("This file is already published."));
    }
    let title: String = request
        .title
        .filter(|title| !title.trim().is_empty())
        .unwrap_or(artifact.title)
        .chars()
        .take(180)
        .collect();
    let publication = state
        .storage
        .create_publication(
            &artifact.workspace_id,
            &title,
            &body,
            &snapshot_hash,
            &artifact_id,
            &subject.user_id,
        )
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &artifact.workspace_id,
        &subject.user_id,
        "publication_created",
        "publication",
        Some(&publication.id),
        format!("Published a snapshot of {title}."),
    )
    .await;
    Ok((StatusCode::CREATED, Json(publication)))
}

#[derive(Debug, Serialize)]
pub(crate) struct PublicationView {
    #[serde(flatten)]
    publication: Publication,
    /// How the private original stands against the snapshot: `current`, `original_changed` (publish again to update) or
    /// `source_removed`. The snapshot itself never changes by itself.
    original: &'static str,
}

async fn view_of(state: &AppState, publication: Publication) -> Result<PublicationView, ApiError> {
    let original = match publication.source_artifact_id.as_deref() {
        None => "source_removed",
        Some(id) => match state.storage.get_artifact_summary(id).await {
            Err(_) => "source_removed",
            Ok(_) => match state.storage.read_artifact_blob(id).await {
                Ok(bytes) => {
                    let (body, _) = make_snapshot(&String::from_utf8_lossy(&bytes));
                    if hash(&body) == publication.snapshot_hash {
                        "current"
                    } else {
                        "original_changed"
                    }
                }
                Err(_) => "source_removed",
            },
        },
    };
    Ok(PublicationView { publication, original })
}

pub(crate) async fn list(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Vec<PublicationView>>, ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    let mut views = Vec::new();
    for publication in state
        .storage
        .list_publications(&workspace_id)
        .await
        .map_err(map_storage_error)?
    {
        views.push(view_of(&state, publication).await?);
    }
    Ok(Json(views))
}

async fn admin_publication(
    state: &AppState,
    subject: &AuthenticatedSubject,
    id: &str,
) -> Result<Publication, ApiError> {
    if !state.public_publishing {
        return Err(ApiError::not_found("Public publishing is off on this server."));
    }
    let publication = state.storage.publication(id).await.map_err(map_storage_error)?;
    require_workspace_admin(state, subject, &publication.workspace_id).await?;
    Ok(publication)
}

#[derive(Debug, Serialize)]
pub(crate) struct PublicPreview {
    status: String,
    version: i64,
    /// What an anonymous reader is served now: the title and text while it is `published`, nothing otherwise.
    served: bool,
    title: Option<String>,
    body_markdown: Option<String>,
    /// How long a shared cache may keep a page, so the publisher knows how soon a withdrawal is seen everywhere.
    cache_seconds: u32,
}

/// Shows the publisher exactly what readers get (V0.8.17): the snapshot while it is live, and nothing once it is withdrawn or stale.
pub(crate) async fn public_preview(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<PublicPreview>, ApiError> {
    let publication = admin_publication(&state, &subject, &id).await?;
    let served = publication.status == "published";
    let body = if served {
        Some(state.storage.publication_body(&id).await.map_err(map_storage_error)?)
    } else {
        None
    };
    Ok(Json(PublicPreview {
        status: publication.status,
        version: publication.version,
        served,
        title: served.then_some(publication.title),
        body_markdown: body,
        cache_seconds: 60,
    }))
}

pub(crate) async fn withdraw(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<PublicationView>, ApiError> {
    let publication = admin_publication(&state, &subject, &id).await?;
    let updated = state
        .storage
        .set_publication_status(&id, "withdrawn")
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &publication.workspace_id,
        &subject.user_id,
        "publication_withdrawn",
        "publication",
        Some(&id),
        format!("Withdrew the snapshot of {}.", publication.title),
    )
    .await;
    Ok(Json(view_of(&state, updated).await?))
}

/// Replaces the snapshot with the original as it is now, after the publisher reviewed the new preview, and makes it live again.
pub(crate) async fn republish(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<PublishRequest>,
) -> Result<Json<PublicationView>, ApiError> {
    let publication = admin_publication(&state, &subject, &id).await?;
    let Some(artifact_id) = publication.source_artifact_id.clone() else {
        return Err(ApiError::conflict(
            "The original was removed, so there is nothing to publish again.",
        ));
    };
    let (_, text) = load_publishable(&state, &subject, &artifact_id).await?;
    let (body, _) = make_snapshot(&text);
    let snapshot_hash = hash(&body);
    if request.review_token != snapshot_hash {
        return Err(ApiError::conflict(
            "The file changed since it was previewed. Preview it again and review what would be public.",
        ));
    }
    let updated = state
        .storage
        .replace_publication_snapshot(&id, &body, &snapshot_hash)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &publication.workspace_id,
        &subject.user_id,
        "publication_updated",
        "publication",
        Some(&id),
        format!("Published a new version of {}.", publication.title),
    )
    .await;
    Ok(Json(view_of(&state, updated).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_snapshot_keeps_text_and_public_links_and_drops_everything_private() {
        let source = "# Guide\nSee [the plan](/workspaces/w/artifacts/a1) and [docs](https://example.com/docs) and [rel](../x.md).\n![chart](img/a.png)\n<script>alert(1)</script> text <b>bold</b>\n```\n[kept](code.md)\n```\n[mail](mailto:a@b.c) [tok](https://u:p@x.y)";
        let (body, removed) = make_snapshot(source);
        assert!(
            body.contains("See the plan and [docs](https://example.com/docs) and rel."),
            "{body}"
        );
        assert!(
            !body.contains("artifacts/a1")
                && !body.contains("img/a.png")
                && !body.contains("<script>")
                && !body.contains("<b>"),
            "{body}"
        );
        assert!(body.contains("[kept](code.md)"), "code is left alone");
        assert!(body.contains("mail tok"), "{body}");
        assert_eq!(removed.len(), 3, "{removed:?}");
        assert!(make_snapshot("Written by ana@example.com today")
            .0
            .contains("[email removed]"));
    }
}
