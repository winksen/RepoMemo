//! A repository as one piece of evidence.
//!
//! Each sync computes an overview of what the repository holds (languages,
//! folders, key files, the README's opening, recent commits) from its stored
//! files, and writes it as the Markdown content of one `repository` artifact.
//! That artifact is what the Evidence ledger shows for the repository, and it
//! is indexed, so questions about the repository as a whole find it. The
//! overview needs no AI; an AI summary can be generated on request and is kept
//! until someone regenerates it.

use std::collections::BTreeMap;

use anyhow::{bail, Result};
use repomemo_ai::{provider_from_settings, AiProvider, GenerateRequest};
use repomemo_domain::{
    Citation, RepoCommit, RepoFile, RepoFolderShare, RepoKeyFile, RepoLanguageShare, RepoOverview,
    RepoStack, RepoSummary,
};
use serde_json::json;

use crate::RepoMemoCore;

const MAX_LANGUAGES: usize = 8;
const MAX_FOLDERS: usize = 12;
const MAX_KEY_FILES: usize = 16;
/// Key files kept per role, so a monorepo's many manifests do not crowd out
/// its entry points.
const KEY_FILES_PER_ROLE: [(&str, usize); 4] = [("readme", 2), ("manifest", 5), ("entry_point", 6), ("docs", 4)];
const README_EXCERPT_CHARS: usize = 1_800;
pub(crate) const RECENT_COMMITS: usize = 10;
/// Text sent to the AI provider for a summary, in total and per key file.
const SUMMARY_CONTEXT_CHARS: usize = 16_000;
const SUMMARY_FILE_CHARS: usize = 4_000;
const SUMMARY_KEY_FILES: usize = 8;

const MANIFESTS: &[&str] = &[
    "Cargo.toml", "package.json", "pyproject.toml", "setup.py", "requirements.txt", "go.mod",
    "pom.xml", "build.gradle", "build.gradle.kts", "Gemfile", "composer.json", "Dockerfile",
    "docker-compose.yml", "docker-compose.yaml", "Makefile", "CMakeLists.txt",
];
const ENTRY_POINTS: &[&str] = &[
    "main.rs", "lib.rs", "main.py", "__main__.py", "app.py", "manage.py", "main.go", "index.ts",
    "index.tsx", "index.js", "main.ts", "main.tsx", "main.js", "App.tsx", "App.jsx", "server.ts",
    "server.js", "Program.cs", "Main.java", "Application.java",
];

/// Why a file is worth opening first, if it is.
pub(crate) fn key_file_role(path: &str) -> Option<&'static str> {
    let depth = path.matches('/').count();
    let name = path.rsplit('/').next().unwrap_or(path);
    let lower = name.to_ascii_lowercase();
    if depth == 0 && lower.starts_with("readme") {
        return Some("readme");
    }
    if depth <= 2 && MANIFESTS.contains(&name) {
        return Some("manifest");
    }
    if depth <= 3 && ENTRY_POINTS.contains(&name) {
        return Some("entry_point");
    }
    let is_markdown = lower.ends_with(".md") || lower.ends_with(".mdx");
    let in_docs = path.starts_with("docs/") && depth == 1;
    if is_markdown && (depth == 0 || in_docs) {
        return Some("docs");
    }
    None
}

fn role_rank(role: &str) -> usize {
    ["readme", "manifest", "entry_point", "docs"]
        .iter()
        .position(|candidate| *candidate == role)
        .unwrap_or(4)
}

/// Computes the overview of a repository's current files.
pub(crate) fn compute_overview(
    commit: RepoCommit,
    files: &[RepoFile],
    readme_excerpt: Option<String>,
    recent_commits: Vec<RepoCommit>,
    stack: Option<RepoStack>,
    generated_at: String,
) -> RepoOverview {
    let mut languages: BTreeMap<String, (usize, i64)> = BTreeMap::new();
    let mut folders: BTreeMap<String, usize> = BTreeMap::new();
    let mut key_files = Vec::new();
    for file in files {
        let language = file.language.clone().unwrap_or_else(|| "Other".to_owned());
        let entry = languages.entry(language).or_default();
        entry.0 += 1;
        entry.1 += file.size_bytes;
        let folder = file
            .path
            .split_once('/')
            .map(|(folder, _)| folder.to_owned())
            .unwrap_or_default();
        *folders.entry(folder).or_default() += 1;
        if let Some(role) = key_file_role(&file.path) {
            key_files.push(RepoKeyFile {
                path: file.path.clone(),
                artifact_id: file.artifact_id.clone(),
                role: role.to_owned(),
            });
        }
    }

    let mut languages = languages
        .into_iter()
        .map(|(language, (files, bytes))| RepoLanguageShare { language, files, bytes })
        .collect::<Vec<_>>();
    languages.sort_by(|left, right| right.bytes.cmp(&left.bytes).then(left.language.cmp(&right.language)));
    if languages.len() > MAX_LANGUAGES {
        let rest = languages.split_off(MAX_LANGUAGES - 1);
        languages.push(RepoLanguageShare {
            language: "Other".to_owned(),
            files: rest.iter().map(|share| share.files).sum(),
            bytes: rest.iter().map(|share| share.bytes).sum(),
        });
    }
    let mut folders = folders
        .into_iter()
        .map(|(path, files)| RepoFolderShare { path, files })
        .collect::<Vec<_>>();
    folders.sort_by(|left, right| right.files.cmp(&left.files).then(left.path.cmp(&right.path)));
    folders.truncate(MAX_FOLDERS);
    key_files.sort_by(|left, right| {
        role_rank(&left.role)
            .cmp(&role_rank(&right.role))
            .then(left.path.matches('/').count().cmp(&right.path.matches('/').count()))
            .then(left.path.cmp(&right.path))
    });
    let mut kept_per_role: BTreeMap<String, usize> = BTreeMap::new();
    key_files.retain(|file| {
        let limit = KEY_FILES_PER_ROLE
            .iter()
            .find(|(role, _)| *role == file.role)
            .map(|(_, limit)| *limit)
            .unwrap_or(0);
        let kept = kept_per_role.entry(file.role.clone()).or_default();
        *kept += 1;
        *kept <= limit
    });
    key_files.truncate(MAX_KEY_FILES);

    RepoOverview {
        commit,
        generated_at,
        file_count: files.len(),
        total_bytes: files.iter().map(|file| file.size_bytes).sum(),
        languages,
        folders,
        key_files,
        readme_excerpt,
        recent_commits,
        stack,
    }
}

/// The opening of a README, without raw HTML tags and images, which do not
/// render outside the repository. The text inside HTML is kept.
pub(crate) fn readme_excerpt(text: &str) -> Option<String> {
    let mut kept: Vec<String> = Vec::new();
    let mut length = 0;
    let mut previous_blank = true;
    for raw_line in text.lines() {
        let had_markup = raw_line.contains('<');
        let line = strip_tags(raw_line);
        let trimmed = line.trim();
        if trimmed.starts_with("![") || trimmed.starts_with("[![") || (had_markup && trimmed.is_empty()) {
            continue;
        }
        let blank = trimmed.is_empty();
        if blank && previous_blank {
            continue;
        }
        if length + line.len() > README_EXCERPT_CHARS && !kept.is_empty() {
            // Stop at a paragraph boundary when possible.
            if let Some(last_blank) = kept.iter().rposition(|line| line.trim().is_empty()) {
                kept.truncate(last_blank);
            }
            break;
        }
        length += line.len() + 1;
        let line = if had_markup { trimmed.to_owned() } else { line };
        kept.push(line);
        previous_blank = blank;
    }
    let excerpt = kept.join("\n").trim().to_owned();
    (!excerpt.is_empty()).then_some(excerpt)
}

/// Removes `<...>` tags from a line, keeping the text between them.
fn strip_tags(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_tag = false;
    for character in line.chars() {
        match character {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(character),
            _ => {}
        }
    }
    out
}

/// The Markdown content of the repository's evidence item. It carries no
/// timestamp of its own, so it only changes when the repository does.
pub(crate) fn overview_markdown(name: &str, root: &str, overview: &RepoOverview) -> String {
    let commit = &overview.commit;
    let short = &commit.sha[..commit.sha.len().min(7)];
    let branch = commit
        .branch
        .as_deref()
        .map(|branch| format!(" on branch `{branch}`"))
        .unwrap_or_default();
    let mut out = format!(
        "# {name}\n\nGit repository `{root}`{branch}, at commit `{short}`: {} ({}, {}).\n\n## At a glance\n\n- {} files indexed, {}\n",
        commit.summary,
        commit.author_name,
        commit.committed_at.split('T').next().unwrap_or(&commit.committed_at),
        overview.file_count,
        format_bytes(overview.total_bytes),
    );
    if let Some(stack) = &overview.stack {
        let technologies = stack
            .technologies
            .iter()
            .map(|technology| technology.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("- Project type: {} (built with {technologies})\n", stack.summary));
    }
    if !overview.languages.is_empty() {
        let languages = overview
            .languages
            .iter()
            .map(|share| format!("{} ({} files)", share.language, share.files))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("- Languages: {languages}\n"));
    }
    if !overview.folders.is_empty() {
        out.push_str("\n## Structure\n\n| Folder | Files |\n|---|---|\n");
        for folder in &overview.folders {
            let label = if folder.path.is_empty() {
                "(repository root)".to_owned()
            } else {
                format!("`{}/`", folder.path)
            };
            out.push_str(&format!("| {label} | {} |\n", folder.files));
        }
    }
    if !overview.key_files.is_empty() {
        out.push_str("\n## Key files\n\n");
        for file in &overview.key_files {
            out.push_str(&format!("- `{}` ({})\n", file.path, file.role.replace('_', " ")));
        }
    }
    if let Some(readme) = &overview.readme_excerpt {
        out.push_str(&format!("\n## From the README\n\n{readme}\n"));
    }
    if !overview.recent_commits.is_empty() {
        out.push_str("\n## Recent commits\n\n");
        for commit in &overview.recent_commits {
            out.push_str(&format!(
                "- `{}` {} ({}, {})\n",
                &commit.sha[..commit.sha.len().min(7)],
                commit.summary,
                commit.author_name,
                commit.committed_at.split('T').next().unwrap_or(&commit.committed_at)
            ));
        }
    }
    out
}

fn format_bytes(bytes: i64) -> String {
    let bytes = bytes.max(0) as f64;
    if bytes >= 1024.0 * 1024.0 {
        format!("{:.1} MB", bytes / 1024.0 / 1024.0)
    } else if bytes >= 1024.0 {
        format!("{:.0} KB", bytes / 1024.0)
    } else {
        format!("{bytes} bytes")
    }
}

impl RepoMemoCore {
    /// Writes an AI summary of a repository from its overview and the opening
    /// of its key files, with citations, and keeps it on the repository.
    pub async fn summarize_repo(&self, source_id: &str, provider_id: &str) -> Result<RepoSummary> {
        let detail = self.repo_detail(source_id).await?;
        let repository = detail.repository;
        let Some(overview) = detail.overview else {
            bail!("Sync the repository before summarizing it.");
        };
        let settings = self.storage.get_provider_settings(provider_id).await?;
        if !settings.enabled {
            bail!("Enable this AI provider before summarizing. No content was sent.");
        }
        if settings.workspace_id.as_deref() != Some(repository.workspace_id.as_str()) {
            bail!("The selected provider belongs to a different workspace.");
        }
        let provider_name = settings.name.clone();
        let provider = provider_from_settings(settings)?;

        let mut context = vec![format!(
            "Repository overview\n{}",
            overview_markdown(&repository.name, &repository.root_path, &overview)
        )];
        let mut used = context[0].len();
        let mut citations = Vec::new();
        for file in overview.key_files.iter().take(SUMMARY_KEY_FILES) {
            if used >= SUMMARY_CONTEXT_CHARS {
                break;
            }
            let chunks = self.storage.list_chunks_for_artifact(&file.artifact_id).await?;
            let mut text = String::new();
            for chunk in &chunks {
                if text.len() >= SUMMARY_FILE_CHARS || used + text.len() >= SUMMARY_CONTEXT_CHARS {
                    break;
                }
                text.push_str(&chunk.text);
                text.push('\n');
                citations.push(Citation {
                    artifact_id: file.artifact_id.clone(),
                    chunk_id: Some(chunk.id.clone()),
                    title: file.path.rsplit('/').next().unwrap_or(&file.path).to_owned(),
                    path: file.path.clone(),
                    start_line: chunk.start_line,
                    end_line: chunk.end_line,
                    confidence: None,
                });
            }
            if !text.is_empty() {
                let text = text.chars().take(SUMMARY_FILE_CHARS).collect::<String>();
                used += text.len();
                context.push(format!("File {} ({})\n{text}", file.path, file.role.replace('_', " ")));
            }
        }

        let summary_markdown = provider
            .generate(GenerateRequest {
                system: Some("You describe software repositories faithfully for engineers. Use only the supplied material, keep names exact, never add facts, and say plainly when something is not covered.".to_owned()),
                prompt: format!(
                    "Write an overview of the repository '{}' for an engineer who has never seen it. Open with one or two sentences on what it is and what it does. Then short Markdown sections: **Main parts** (the important folders or components and their role), **Technologies**, **How to build or run it** (only if the material says), and **Where to start reading** (files). Be concise.",
                    repository.name
                ),
                context: context.join("\n\n---\n\n"),
                options: json!({ "temperature": 0.2 }),
            })
            .await?;

        let summary = RepoSummary {
            summary_markdown,
            citations,
            warnings: vec!["Generated by the configured AI provider from the repository overview and the opening of its key files. Verify against the cited files before relying on it.".to_owned()],
            provider_name,
            commit_sha: overview.commit.sha.clone(),
            generated_at: chrono::Utc::now().to_rfc3339(),
        };
        self.store_repo_summary(source_id, summary.clone()).await?;
        Ok(summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use repomemo_domain::ArtifactType;

    fn file(path: &str, language: Option<&str>, size: i64) -> RepoFile {
        RepoFile {
            path: path.to_owned(),
            artifact_id: format!("id-{path}"),
            artifact_type: ArtifactType::CodeFile,
            language: language.map(str::to_owned),
            size_bytes: size,
            indexed: true,
            index_failure: None,
            commit_sha: "abc".to_owned(),
        }
    }

    #[test]
    fn overview_counts_languages_folders_and_key_files() {
        let commit = RepoCommit {
            sha: "0123456789abcdef".to_owned(),
            summary: "Add ledger".to_owned(),
            author_name: "Ada".to_owned(),
            committed_at: "2026-10-01T10:00:00Z".to_owned(),
            branch: Some("main".to_owned()),
        };
        let files = vec![
            file("README.md", Some("Markdown"), 300),
            file("Cargo.toml", Some("TOML"), 100),
            file("src/main.rs", Some("Rust"), 2_000),
            file("src/ledger.rs", Some("Rust"), 4_000),
            file("docs/setup.md", Some("Markdown"), 500),
            file("docs/deep/notes.md", Some("Markdown"), 50),
            file("LICENSE", None, 1_000),
            file("tools/a/Cargo.toml", Some("TOML"), 10),
            file("tools/b/Cargo.toml", Some("TOML"), 10),
            file("tools/c/Cargo.toml", Some("TOML"), 10),
            file("tools/d/Cargo.toml", Some("TOML"), 10),
            file("tools/e/Cargo.toml", Some("TOML"), 10),
        ];
        let overview = compute_overview(commit, &files, Some("# Payments".to_owned()), Vec::new(), None, "now".to_owned());
        assert_eq!(overview.file_count, 12);
        assert_eq!(overview.total_bytes, 8_000);
        assert_eq!(overview.languages[0].language, "Rust");
        assert_eq!(overview.languages[0].files, 2);
        assert_eq!(overview.folders[0], RepoFolderShare { path: "tools".to_owned(), files: 5 });
        let key = overview.key_files.iter().map(|file| (file.path.as_str(), file.role.as_str())).collect::<Vec<_>>();
        // Five manifests at most: the root one first, so the entry point stays.
        assert_eq!(
            key,
            vec![
                ("README.md", "readme"),
                ("Cargo.toml", "manifest"),
                ("tools/a/Cargo.toml", "manifest"),
                ("tools/b/Cargo.toml", "manifest"),
                ("tools/c/Cargo.toml", "manifest"),
                ("tools/d/Cargo.toml", "manifest"),
                ("src/main.rs", "entry_point"),
                ("docs/setup.md", "docs"),
            ]
        );

        let markdown = overview_markdown("payments", "C:/code/payments", &overview);
        assert!(markdown.starts_with("# payments\n"));
        assert!(markdown.contains("on branch `main`, at commit `0123456`"));
        assert!(markdown.contains("| `src/` | 2 |"));
        assert!(markdown.contains("## From the README\n\n# Payments"));
        assert!(!markdown.contains("now"));
    }

    #[test]
    fn readme_excerpts_drop_html_and_images() {
        let text = "<p align=\"center\">\n  <img src=\"logo.svg\" />\n</p>\n\n<p align=\"center\">\n  <strong>Refunds, done right.</strong><br />\n</p>\n\n![badge](x.svg)\n\n# Payments\n\nIssues refunds.\n";
        assert_eq!(readme_excerpt(text).as_deref(), Some("Refunds, done right.\n\n# Payments\n\nIssues refunds."));
        assert_eq!(readme_excerpt("<div></div>\n"), None);
    }
}
