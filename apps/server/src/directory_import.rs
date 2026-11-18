//! Knowledge migration (V0.8.11): a Markdown and asset directory brought into a workspace.
//!
//! The caller sends the directory as a list of files (`path` and `text` or `base64`). Like the task CSV import, **a dry run is
//! the default**: it reports, for every file, what would happen and what is wrong, and writes nothing. Committing is all or
//! skipping: one file with a problem refuses the whole import unless `skip_invalid` is true. A file is identified by the name of the
//! import (`source_key`) and its path, in a ledger, so sending the same directory again makes nothing twice, changes what changed
//! (keeping the file's id, comments and earlier versions) and carries on from the files already done after an interruption.
//!
//! Directories become folders. A relative Markdown link to another file of the import is rewritten to that file's address in
//! the app, so it keeps working; a link to a file that is not there, one that leaves the directory and one that cannot be read is
//! reported and left as it was. A path that is absolute, climbs out of the directory, holds a backslash or a control character, or
//! names a hidden file is reported as unsafe and never written. Types other than Markdown, text, common images and PDF are
//! reported as unsupported.

use std::collections::{HashMap, HashSet};

use base64::Engine;

use crate::*;

const MAX_FILES: usize = 500;
const MAX_FILE_BYTES: usize = 5 * 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 25 * 1024 * 1024;
const MAX_PATH_CHARS: usize = 300;
const MAX_SEGMENT_CHARS: usize = 80;
const BODY_LIMIT: usize = 40 * 1024 * 1024;

#[derive(Debug, Deserialize)]
pub(crate) struct ImportFile {
    path: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    base64: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DirectoryImportRequest {
    /// The name of the import: the same name sent again is the same import.
    source_key: String,
    /// Left out, a dry run: nothing is written.
    #[serde(default = "yes")]
    dry_run: bool,
    /// Import the files without a problem and report the others, instead of refusing everything.
    #[serde(default)]
    skip_invalid: bool,
    files: Vec<ImportFile>,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Markdown,
    Text,
    Image,
    Pdf,
    Unsupported,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Kind::Markdown => "markdown",
            Kind::Text => "text",
            Kind::Image | Kind::Pdf => "attachment",
            Kind::Unsupported => "unsupported",
        }
    }
}

fn classify(path: &str) -> (Kind, &'static str) {
    let extension = path.rsplit('.').next().unwrap_or_default().to_ascii_lowercase();
    match extension.as_str() {
        "md" | "markdown" => (Kind::Markdown, "text/markdown"),
        "txt" => (Kind::Text, "text/plain"),
        "png" => (Kind::Image, "image/png"),
        "jpg" | "jpeg" => (Kind::Image, "image/jpeg"),
        "gif" => (Kind::Image, "image/gif"),
        "webp" => (Kind::Image, "image/webp"),
        "pdf" => (Kind::Pdf, "application/pdf"),
        _ => (Kind::Unsupported, "application/octet-stream"),
    }
}

/// A path inside the directory in a normal form, or why it is not safe.
fn normalize_path(raw: &str) -> Result<String, String> {
    let path = raw.trim();
    if path.is_empty() {
        return Err("The path is empty.".to_owned());
    }
    if path.chars().count() > MAX_PATH_CHARS {
        return Err(format!("The path is longer than {MAX_PATH_CHARS} characters."));
    }
    if path.starts_with('/') || path.contains(':') {
        return Err("The path is absolute: it must be relative to the directory.".to_owned());
    }
    if path.contains('\\') || path.chars().any(char::is_control) {
        return Err("The path holds a backslash or a control character.".to_owned());
    }
    let mut segments = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => continue,
            ".." => return Err("The path climbs out of the directory.".to_owned()),
            part if part.starts_with('.') => return Err("A hidden file or folder is not imported.".to_owned()),
            part if part.chars().count() > MAX_SEGMENT_CHARS => {
                return Err(format!(
                    "A name in the path is longer than {MAX_SEGMENT_CHARS} characters."
                ))
            }
            part if part.chars().any(|c| matches!(c, '<' | '>' | '"' | '|' | '?' | '*')) => {
                return Err("A name in the path holds a character that is not allowed.".to_owned())
            }
            part => segments.push(part),
        }
    }
    if segments.is_empty() {
        return Err("The path names no file.".to_owned());
    }
    Ok(segments.join("/"))
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Some(value) = std::str::from_utf8(&bytes[i + 1..i + 3])
                .ok()
                .and_then(|hex| u8::from_str_radix(hex, 16).ok())
            {
                out.push(value);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A relative link found in a Markdown file: where its target sits in the text and what it says.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LinkSpan {
    start: usize,
    end: usize,
    target: String,
}

/// The targets of `[text](target)` and `![alt](target)` outside fenced code, and how many `[[wiki links]]` there are (which are
/// not supported).
fn scan_links(text: &str) -> (Vec<LinkSpan>, usize) {
    let mut spans = Vec::new();
    let mut wiki = 0;
    let mut fenced = false;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
        } else if !fenced {
            wiki += line.matches("[[").count();
            for (at, _) in line.match_indices("](") {
                let start = at + 2;
                let rest = &line[start..];
                let (inner_start, end) = if let Some(stripped) = rest.strip_prefix('<') {
                    match stripped.find('>') {
                        Some(close) => (start + 1, start + 1 + close),
                        None => continue,
                    }
                } else {
                    match rest.find([')', ' ', '\t']) {
                        Some(close) => (start, start + close),
                        None => continue,
                    }
                };
                spans.push(LinkSpan {
                    start: offset + inner_start,
                    end: offset + end,
                    target: line[inner_start..end].to_owned(),
                });
            }
        }
        offset += line.len();
    }
    (spans, wiki)
}

#[derive(Debug, PartialEq, Eq)]
enum Resolved {
    /// A file of the import, with the part after `#`.
    File { path: String, fragment: String },
    /// A relative link to something the import does not hold.
    Missing,
    /// A link that leaves the directory.
    Unsafe,
    /// Not a link to a file: an address, a mail link or a position in this page.
    Other,
}

fn resolve_link(target: &str, from: &str, present: &HashSet<String>) -> Resolved {
    let target = target.trim();
    let has_scheme = target.split_once(':').is_some_and(|(scheme, _)| {
        !scheme.is_empty() && scheme.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
    });
    if target.is_empty() || target.starts_with('#') || target.starts_with("//") || has_scheme {
        return Resolved::Other;
    }
    let (path_part, fragment) = match target.split_once('#') {
        Some((path, fragment)) => (path, fragment.to_owned()),
        None => (target, String::new()),
    };
    let path_part = path_part.split('?').next().unwrap_or_default();
    let decoded = percent_decode(path_part);
    if decoded.starts_with('/') {
        return Resolved::Unsafe;
    }
    let mut stack: Vec<&str> = from
        .rsplit_once('/')
        .map(|(dir, _)| dir.split('/').collect())
        .unwrap_or_default();
    for part in decoded.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if stack.pop().is_none() {
                    return Resolved::Unsafe;
                }
            }
            part => stack.push(part),
        }
    }
    let joined = stack.join("/");
    for candidate in [joined.clone(), format!("{joined}.md")] {
        if present.contains(&candidate) {
            return Resolved::File {
                path: candidate,
                fragment,
            };
        }
    }
    Resolved::Missing
}

#[derive(Debug, Default, Clone, Serialize)]
pub(crate) struct LinkCounts {
    resolved: usize,
    missing: usize,
    external: usize,
    /// Links that leave the directory.
    unsafe_links: usize,
    /// `[[wiki links]]`, which are not supported and are left as written.
    wiki: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ImportEntry {
    path: String,
    /// `markdown`, `text`, `attachment` or `unsupported`.
    kind: &'static str,
    /// `create`, `replace`, `unchanged` or `skip`.
    action: &'static str,
    problems: Vec<String>,
    links: LinkCounts,
    /// Relative links to files the import does not hold.
    broken_links: Vec<String>,
    artifact_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct DirectoryImportReport {
    source_key: String,
    dry_run: bool,
    committed: bool,
    files: usize,
    create: usize,
    replace: usize,
    unchanged: usize,
    skipped: usize,
    folders: usize,
    links_resolved: usize,
    links_broken: usize,
    entries: Vec<ImportEntry>,
}

struct Planned {
    entry: ImportEntry,
    path: String,
    kind: Kind,
    mime: &'static str,
    bytes: Vec<u8>,
    spans: Vec<LinkSpan>,
}

fn check_key(key: &str) -> Result<String, ApiError> {
    let key = key.trim();
    if key.is_empty() || key.len() > 60 || !key.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)) {
        return Err(ApiError::bad_request(
            "Name the import with up to 60 letters, digits, dashes, dots or underscores.",
        ));
    }
    Ok(key.to_owned())
}

/// Reads each file, finds its problems and its links. Nothing is written.
fn plan(files: Vec<ImportFile>) -> Result<Vec<Planned>, ApiError> {
    if files.is_empty() || files.len() > MAX_FILES {
        return Err(ApiError::bad_request(format!(
            "Send between 1 and {MAX_FILES} files in one import."
        )));
    }
    let mut total = 0;
    let mut seen = HashSet::new();
    let mut planned = Vec::new();
    for file in files {
        let mut problems = Vec::new();
        let normalized = normalize_path(&file.path);
        let path = normalized.clone().unwrap_or_else(|_| file.path.clone());
        let (kind, mime) = classify(&path);
        let mut kind = kind;
        match &normalized {
            Err(reason) => {
                problems.push(reason.clone());
                kind = Kind::Unsupported;
            }
            Ok(path) => {
                if !seen.insert(path.to_ascii_lowercase()) {
                    problems.push("The same path appears twice.".to_owned());
                }
                if kind == Kind::Unsupported {
                    problems.push(
                        "This type of file is not supported (Markdown, text, png, jpg, gif, webp and pdf are)."
                            .to_owned(),
                    );
                }
            }
        }
        let mut bytes = Vec::new();
        if problems.is_empty() {
            match (&file.text, &file.base64) {
                (Some(text), _) if matches!(kind, Kind::Markdown | Kind::Text) => bytes = text.clone().into_bytes(),
                (_, Some(encoded)) => match base64::engine::general_purpose::STANDARD.decode(encoded.trim()) {
                    Ok(decoded) => bytes = decoded,
                    Err(_) => problems.push("The content is not valid base64.".to_owned()),
                },
                _ => problems.push("The file has no content: send text, or base64 for an attachment.".to_owned()),
            }
        }
        if bytes.len() > MAX_FILE_BYTES {
            problems.push(format!(
                "The file is larger than {} MiB.",
                MAX_FILE_BYTES / (1024 * 1024)
            ));
        }
        total += bytes.len();
        planned.push(Planned {
            entry: ImportEntry {
                path: path.clone(),
                kind: kind.label(),
                action: "skip",
                problems,
                links: LinkCounts::default(),
                broken_links: Vec::new(),
                artifact_id: None,
            },
            path,
            kind,
            mime,
            bytes,
            spans: Vec::new(),
        });
    }
    if total > MAX_TOTAL_BYTES {
        return Err(ApiError::bad_request(format!(
            "One import can hold up to {} MiB.",
            MAX_TOTAL_BYTES / (1024 * 1024)
        )));
    }
    // Links are read against the files that are fit to import.
    let present: HashSet<String> = planned
        .iter()
        .filter(|file| file.entry.problems.is_empty())
        .map(|file| file.path.clone())
        .collect();
    for file in planned.iter_mut().filter(|file| file.entry.problems.is_empty()) {
        if file.kind != Kind::Markdown {
            continue;
        }
        let text = String::from_utf8_lossy(&file.bytes).into_owned();
        let (spans, wiki) = scan_links(&text);
        file.entry.links.wiki = wiki;
        for span in spans {
            match resolve_link(&span.target, &file.path, &present) {
                Resolved::File { .. } => {
                    file.entry.links.resolved += 1;
                    file.spans.push(span);
                }
                Resolved::Missing => {
                    file.entry.links.missing += 1;
                    file.entry.broken_links.push(span.target);
                }
                Resolved::Unsafe => {
                    file.entry.links.unsafe_links += 1;
                    file.entry.broken_links.push(span.target);
                }
                Resolved::Other => file.entry.links.external += 1,
            }
        }
    }
    Ok(planned)
}

/// The Markdown with each resolved link pointing at the file's address in the app.
fn rewrite(
    text: &str,
    from: &str,
    spans: &[LinkSpan],
    present: &HashSet<String>,
    ids: &HashMap<String, String>,
    workspace_id: &str,
) -> String {
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0;
    for span in spans {
        if let Resolved::File { path, fragment } = resolve_link(&span.target, from, present) {
            if let Some(id) = ids.get(&path) {
                out.push_str(&text[cursor..span.start]);
                out.push_str(&format!("/workspaces/{workspace_id}/artifacts/{id}"));
                if !fragment.is_empty() {
                    out.push('#');
                    out.push_str(&fragment);
                }
                cursor = span.end;
            }
        }
    }
    out.push_str(&text[cursor..]);
    out
}

fn title_of(path: &str, text: Option<&str>) -> String {
    let heading = text
        .and_then(|text| text.lines().find_map(|line| line.strip_prefix("# ")))
        .map(str::trim)
        .filter(|title| !title.is_empty());
    let title = heading.map(str::to_owned).unwrap_or_else(|| {
        let name = path.rsplit('/').next().unwrap_or(path);
        name.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(name).to_owned()
    });
    title.chars().take(180).collect()
}

pub(crate) async fn import_directory(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<DirectoryImportRequest>,
) -> Result<(StatusCode, Json<DirectoryImportReport>), ApiError> {
    require_workspace_write(&state, &subject, &workspace_id).await?;
    let source_key = check_key(&request.source_key)?;
    let (dry_run, skip_invalid) = (request.dry_run, request.skip_invalid);
    let mut planned = plan(request.files)?;
    let earlier = state
        .storage
        .directory_import_files(&workspace_id, &source_key)
        .await
        .map_err(map_storage_error)?;
    for file in planned.iter_mut().filter(|file| file.entry.problems.is_empty()) {
        let hash = repomemo_storage::StorageEngine::content_hash(&file.bytes);
        file.entry.action = match earlier.get(&file.path) {
            None => "create",
            Some(found) if found.source_hash == hash => {
                file.entry.artifact_id = Some(found.artifact_id.clone());
                "unchanged"
            }
            Some(found) => {
                file.entry.artifact_id = Some(found.artifact_id.clone());
                "replace"
            }
        };
    }
    let invalid = planned.iter().filter(|file| !file.entry.problems.is_empty()).count();
    let report = |planned: &[Planned], committed: bool, folders: usize| DirectoryImportReport {
        source_key: source_key.clone(),
        dry_run,
        committed,
        files: planned.len(),
        create: planned.iter().filter(|f| f.entry.action == "create").count(),
        replace: planned.iter().filter(|f| f.entry.action == "replace").count(),
        unchanged: planned.iter().filter(|f| f.entry.action == "unchanged").count(),
        skipped: planned.iter().filter(|f| f.entry.action == "skip").count(),
        folders,
        links_resolved: planned.iter().map(|f| f.entry.links.resolved).sum(),
        links_broken: planned
            .iter()
            .map(|f| f.entry.links.missing + f.entry.links.unsafe_links)
            .sum(),
        entries: planned.iter().map(|f| f.entry.clone()).collect(),
    };
    let folders_needed: HashSet<String> = planned
        .iter()
        .filter(|file| file.entry.problems.is_empty())
        .filter_map(|file| file.path.rsplit_once('/').map(|(dir, _)| dir.to_owned()))
        .flat_map(|dir| {
            let mut all = Vec::new();
            let mut current = String::new();
            for part in dir.split('/') {
                if !current.is_empty() {
                    current.push('/');
                }
                current.push_str(part);
                all.push(current.clone());
            }
            all
        })
        .collect();
    if dry_run {
        return Ok((StatusCode::OK, Json(report(&planned, false, folders_needed.len()))));
    }
    if invalid > 0 && !skip_invalid {
        return Ok((
            StatusCode::CONFLICT,
            Json(report(&planned, false, folders_needed.len())),
        ));
    }

    let writes: u64 = planned
        .iter()
        .filter(|file| matches!(file.entry.action, "create" | "replace"))
        .map(|file| file.bytes.len() as u64)
        .sum();
    quotas::check_incoming(&state, &subject, &workspace_id, writes, false).await?;
    let source = state
        .storage
        .create_or_get_source(
            &workspace_id,
            repomemo_domain::SourceType::Manual,
            &format!("Import: {source_key}"),
            None,
        )
        .await
        .map_err(map_storage_error)?;

    // Folders: one for each directory, made when it is not there.
    let mut folder_ids: HashMap<String, String> = HashMap::new();
    let mut dirs: Vec<&String> = folders_needed.iter().collect();
    dirs.sort_by_key(|dir| dir.matches('/').count());
    for dir in dirs {
        let (parent, name) = match dir.rsplit_once('/') {
            Some((parent, name)) => (folder_ids.get(parent).cloned(), name),
            None => (None, dir.as_str()),
        };
        let id = state
            .storage
            .ensure_folder(&workspace_id, parent.as_deref(), name, Some(&subject.user_id))
            .await
            .map_err(map_storage_error)?;
        folder_ids.insert(dir.clone(), id);
    }

    // First pass: every file is stored or replaced as it was sent, and recorded as done.
    let mut ids: HashMap<String, String> = HashMap::new();
    let mut touched: Vec<String> = Vec::new();
    for file in planned.iter_mut().filter(|file| file.entry.problems.is_empty()) {
        let hash = repomemo_storage::StorageEngine::content_hash(&file.bytes);
        state
            .storage
            .store_blob(&hash, &file.bytes, Some(file.mime))
            .await
            .map_err(map_storage_error)?;
        let id = match file.entry.action {
            "unchanged" => file.entry.artifact_id.clone().unwrap_or_default(),
            "replace" => {
                let id = file.entry.artifact_id.clone().unwrap_or_default();
                state
                    .storage
                    .replace_artifact_content(
                        &id,
                        &hash,
                        file.bytes.len() as i64,
                        Some(file.mime),
                        Some(&subject.user_id),
                    )
                    .await
                    .map_err(map_storage_error)?;
                touched.push(id.clone());
                id
            }
            _ => {
                let text = (file.kind == Kind::Markdown || file.kind == Kind::Text)
                    .then(|| String::from_utf8_lossy(&file.bytes).into_owned());
                let stored = state
                    .storage
                    .store_artifact(repomemo_storage::NewArtifact {
                        workspace_id: workspace_id.clone(),
                        source_id: source.id.clone(),
                        artifact_type: match file.kind {
                            Kind::Markdown => repomemo_domain::ArtifactType::MarkdownDoc,
                            Kind::Image => repomemo_domain::ArtifactType::Image,
                            _ => repomemo_domain::ArtifactType::File,
                        },
                        title: title_of(&file.path, text.as_deref()),
                        path: file.path.clone(),
                        content_hash: hash.clone(),
                        mime_type: Some(file.mime.to_owned()),
                        language: (file.kind == Kind::Markdown).then(|| "Markdown".to_owned()),
                        size_bytes: file.bytes.len() as i64,
                        metadata: serde_json::json!({ "origin": "directory_import", "source_key": source_key, "path": file.path }),
                    })
                    .await
                    .map_err(map_storage_error)?;
                if let Some((dir, _)) = file.path.rsplit_once('/') {
                    state
                        .storage
                        .set_artifact_folder(&stored.artifact.id, folder_ids.get(dir).map(String::as_str))
                        .await
                        .map_err(map_storage_error)?;
                }
                touched.push(stored.artifact.id.clone());
                stored.artifact.id
            }
        };
        state
            .storage
            .record_directory_import_file(&workspace_id, &source_key, &file.path, &id, &hash)
            .await
            .map_err(map_storage_error)?;
        file.entry.artifact_id = Some(id.clone());
        ids.insert(file.path.clone(), id);
    }

    // Second pass: links to files of the import now point at their addresses in the app.
    let present: HashSet<String> = ids.keys().cloned().collect();
    for file in planned
        .iter()
        .filter(|file| file.entry.problems.is_empty() && file.kind == Kind::Markdown && !file.spans.is_empty())
    {
        let original = String::from_utf8_lossy(&file.bytes).into_owned();
        let rewritten = rewrite(&original, &file.path, &file.spans, &present, &ids, &workspace_id);
        let bytes = rewritten.into_bytes();
        let hash = repomemo_storage::StorageEngine::content_hash(&bytes);
        let Some(id) = ids.get(&file.path) else { continue };
        let current = state
            .storage
            .get_artifact_summary(id)
            .await
            .map_err(map_storage_error)?;
        if current.content_hash == hash {
            continue;
        }
        state
            .storage
            .store_blob(&hash, &bytes, Some("text/markdown"))
            .await
            .map_err(map_storage_error)?;
        state
            .storage
            .replace_artifact_content(
                id,
                &hash,
                bytes.len() as i64,
                Some("text/markdown"),
                Some(&subject.user_id),
            )
            .await
            .map_err(map_storage_error)?;
        if !touched.contains(id) {
            touched.push(id.clone());
        }
    }
    for id in &touched {
        if let Ok(artifact) = state.storage.get_artifact_summary(id).await {
            state.index_queue.enqueue(&artifact, Some(&subject.user_id));
        }
    }
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "directory_imported",
        "import",
        None,
        format!("Imported {} files from {source_key}.", ids.len()),
    )
    .await;
    Ok((StatusCode::OK, Json(report(&planned, true, folder_ids.len()))))
}

/// The most the route accepts.
pub(crate) fn body_limit() -> usize {
    BODY_LIMIT
}

#[cfg(test)]
mod tests {
    use super::*;

    fn present(paths: &[&str]) -> HashSet<String> {
        paths.iter().map(|path| (*path).to_owned()).collect()
    }

    #[test]
    fn paths_are_kept_inside_the_directory() {
        assert_eq!(normalize_path("docs/./guide.md").unwrap(), "docs/guide.md");
        for bad in [
            "/etc/passwd",
            "../secret.md",
            "a/../../b.md",
            "a\\b.md",
            ".git/config",
            "C:/x.md",
            "a/.hidden.md",
            "",
            "a/b?.md",
        ] {
            assert!(normalize_path(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn links_resolve_against_the_files_of_the_import() {
        let files = present(&["docs/guide.md", "docs/img/a.png", "readme.md", "docs/My page.md"]);
        assert_eq!(
            resolve_link("img/a.png", "docs/guide.md", &files),
            Resolved::File {
                path: "docs/img/a.png".to_owned(),
                fragment: String::new()
            }
        );
        assert_eq!(
            resolve_link("../readme.md#intro", "docs/guide.md", &files),
            Resolved::File {
                path: "readme.md".to_owned(),
                fragment: "intro".to_owned()
            }
        );
        assert_eq!(
            resolve_link("My%20page", "docs/guide.md", &files),
            Resolved::File {
                path: "docs/My page.md".to_owned(),
                fragment: String::new()
            }
        );
        assert_eq!(resolve_link("gone.md", "docs/guide.md", &files), Resolved::Missing);
        assert_eq!(resolve_link("../../x.md", "docs/guide.md", &files), Resolved::Unsafe);
        assert_eq!(resolve_link("/etc/passwd", "docs/guide.md", &files), Resolved::Unsafe);
        for other in ["https://example.com", "mailto:a@b.c", "#top", "//cdn.example/x", ""] {
            assert_eq!(resolve_link(other, "docs/guide.md", &files), Resolved::Other, "{other}");
        }
    }

    #[test]
    fn links_are_found_outside_code_and_rewritten_in_place() {
        let text = "# T\nSee [guide](docs/guide.md#a) and ![pic](img/a.png \"t\").\n```\n[no](skipped.md)\n```\n[[wiki]] [web](https://x.y)\n";
        let (spans, wiki) = scan_links(text);
        assert_eq!(wiki, 1);
        assert_eq!(
            spans.iter().map(|span| span.target.as_str()).collect::<Vec<_>>(),
            vec!["docs/guide.md#a", "img/a.png", "https://x.y"]
        );
        let files = present(&["docs/guide.md", "img/a.png"]);
        let ids = HashMap::from([
            ("docs/guide.md".to_owned(), "G1".to_owned()),
            ("img/a.png".to_owned(), "P1".to_owned()),
        ]);
        let out = rewrite(text, "index.md", &spans, &files, &ids, "W");
        assert!(out.contains("[guide](/workspaces/W/artifacts/G1#a)"), "{out}");
        assert!(out.contains("![pic](/workspaces/W/artifacts/P1 \"t\")"), "{out}");
        assert!(
            out.contains("[no](skipped.md)") && out.contains("[web](https://x.y)"),
            "{out}"
        );
    }

    #[test]
    fn a_title_is_the_first_heading_or_the_file_name() {
        assert_eq!(title_of("docs/guide.md", Some("intro\n# The Guide\n")), "The Guide");
        assert_eq!(title_of("docs/guide.md", Some("no heading")), "guide");
    }

    #[test]
    fn percent_decoding_reads_escaped_bytes() {
        assert_eq!(percent_decode("a%20b%2Fc"), "a b/c");
        assert_eq!(percent_decode("100%"), "100%");
    }
}
