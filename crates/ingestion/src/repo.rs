//! Which files of a git repository become evidence.
//!
//! A repository's tracked tree already leaves out what `.gitignore` excludes,
//! so these rules only remove committed noise (lock files, minified bundles,
//! vendored code), files too large to be useful, and whatever the source's own
//! include and exclude patterns say. Decisions are made from the path and the
//! size reported by git, before any content is read; only files of an unknown
//! type need a look at their bytes.

use std::path::Path;

use repomemo_domain::ArtifactType;

use crate::{detect_artifact_type, detect_language, detect_mime, is_document};

/// Committed files larger than this are almost always generated or data.
pub const DEFAULT_REPO_MAX_FILE_BYTES: u64 = 1024 * 1024;

/// Applied to every repository on top of its own exclude patterns.
pub const DEFAULT_REPO_EXCLUDES: &[&str] = &[
    "node_modules/",
    "vendor/",
    "dist/",
    "build/",
    "target/",
    ".next/",
    ".vite/",
    "coverage/",
    "__pycache__/",
    ".git/",
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "Cargo.lock",
    "poetry.lock",
    "Pipfile.lock",
    "composer.lock",
    "Gemfile.lock",
    "go.sum",
    "*.min.js",
    "*.min.css",
    "*.map",
];

#[derive(Debug, Clone, Default)]
pub struct RepoFileRules {
    /// When not empty, a file must match one of these patterns.
    pub include: Vec<String>,
    /// Added to [`DEFAULT_REPO_EXCLUDES`].
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoFileKind {
    pub artifact_type: ArtifactType,
    pub language: Option<String>,
    pub mime_type: Option<String>,
    /// The type is unknown from the name alone; the content must be checked
    /// with [`is_indexable_text`] before the file is kept.
    pub needs_text_check: bool,
}

/// Decides from the path and size whether a tracked file is kept. Returns the
/// reason when it is skipped.
pub fn classify_repo_file(path: &str, size_bytes: u64, rules: &RepoFileRules) -> Result<RepoFileKind, String> {
    if !rules.include.is_empty() && !rules.include.iter().any(|pattern| pattern_matches(pattern, path)) {
        return Err("not matched by the include patterns".to_owned());
    }
    if let Some(pattern) = DEFAULT_REPO_EXCLUDES
        .iter()
        .copied()
        .chain(rules.exclude.iter().map(String::as_str))
        .find(|pattern| pattern_matches(pattern, path))
    {
        return Err(format!("excluded by {pattern}"));
    }
    if size_bytes > DEFAULT_REPO_MAX_FILE_BYTES {
        return Err("larger than 1 MB".to_owned());
    }

    let file_path = Path::new(path);
    match detect_artifact_type(file_path) {
        Some(ArtifactType::Image) => Err("images are not indexed from repositories".to_owned()),
        Some(artifact_type) => Ok(RepoFileKind {
            needs_text_check: false,
            language: detect_language(file_path),
            mime_type: detect_mime(file_path),
            artifact_type,
        }),
        None => {
            let language = repo_code_language(path);
            Ok(RepoFileKind {
                artifact_type: if language.is_some() {
                    ArtifactType::CodeFile
                } else {
                    ArtifactType::File
                },
                language: language.map(str::to_owned),
                mime_type: Some("text/plain".to_owned()),
                needs_text_check: true,
            })
        }
    }
}

/// Whether file content is plain text worth indexing. Office and PDF files are
/// binary but have their own extractors, so they are always accepted.
pub fn is_indexable_text(path: &str, bytes: &[u8]) -> bool {
    if is_document(Path::new(path)) {
        return true;
    }
    let sample = &bytes[..bytes.len().min(8192)];
    !sample.contains(&0) && std::str::from_utf8(bytes).is_ok()
}

/// Languages common in repositories that general uploads do not accept.
fn repo_code_language(path: &str) -> Option<&'static str> {
    let name = path.rsplit('/').next().unwrap_or(path);
    match name {
        "Dockerfile" => return Some("Dockerfile"),
        "Makefile" | "makefile" | "GNUmakefile" => return Some("Makefile"),
        _ => {}
    }
    let extension = name.rsplit_once('.')?.1.to_ascii_lowercase();
    Some(match extension.as_str() {
        "go" => "Go",
        "java" => "Java",
        "kt" | "kts" => "Kotlin",
        "c" | "h" => "C",
        "cc" | "cpp" | "cxx" | "hpp" | "hh" => "C++",
        "cs" => "C#",
        "rb" => "Ruby",
        "php" => "PHP",
        "swift" => "Swift",
        "scala" => "Scala",
        "vue" => "Vue",
        "svelte" => "Svelte",
        "xml" => "XML",
        "scss" | "sass" => "SCSS",
        "less" => "Less",
        "graphql" | "gql" => "GraphQL",
        "proto" => "Protocol Buffers",
        "gradle" => "Gradle",
        "ini" | "cfg" | "conf" => "INI",
        "bat" | "cmd" => "Batch",
        "lua" => "Lua",
        "dart" => "Dart",
        "ex" | "exs" => "Elixir",
        "r" => "R",
        "tf" => "Terraform",
        "mjs" | "cjs" => "JavaScript",
        "mts" | "cts" => "TypeScript",
        _ => return None,
    })
}

/// Matches a repository path against a gitignore-like pattern:
///
/// - `name` or `*.ext` (no slash) matches the file name at any depth;
/// - `dir/` matches everything under a directory of that name at any depth;
/// - anything else is matched against the whole path, where `*` and `?` stay
///   within one path segment and `**` spans segments (`docs/**`, `**/*.md`).
pub fn pattern_matches(pattern: &str, path: &str) -> bool {
    let pattern = pattern.trim().trim_start_matches("./");
    if pattern.is_empty() {
        return false;
    }
    if let Some(directory) = pattern.strip_suffix('/') {
        let segments = path.split('/').collect::<Vec<_>>();
        let directories = &segments[..segments.len().saturating_sub(1)];
        if directory.contains('/') {
            return glob_matches(&format!("{}/**", directory.trim_start_matches('/')), path);
        }
        return directories.iter().any(|segment| glob_matches(directory, segment));
    }
    if !pattern.contains('/') {
        let name = path.rsplit('/').next().unwrap_or(path);
        return glob_matches(pattern, name);
    }
    glob_matches(pattern.trim_start_matches('/'), path)
}

/// The most steps one pattern may take on one path (V0.2.192). A pattern with many `*` or `**` can take time that grows
/// with each of them, and the patterns come from a person's form or from a template file; one that needs more than
/// this is not a sensible pattern, and it matches nothing.
const GLOB_STEPS_MAX: usize = 100_000;

fn glob_matches(pattern: &str, text: &str) -> bool {
    let mut budget = GLOB_STEPS_MAX;
    glob_matches_bytes(pattern.as_bytes(), text.as_bytes(), &mut budget)
}

fn glob_matches_bytes(pattern: &[u8], text: &[u8], budget: &mut usize) -> bool {
    if *budget == 0 {
        return false;
    }
    *budget -= 1;
    let Some((&first, rest)) = pattern.split_first() else {
        return text.is_empty();
    };
    match first {
        b'*' if rest.first() == Some(&b'*') => {
            let rest = &rest[1..];
            // "**/" also matches zero directories.
            if rest.first() == Some(&b'/') && glob_matches_bytes(&rest[1..], text, budget) {
                return true;
            }
            (0..=text.len()).any(|start| glob_matches_bytes(rest, &text[start..], budget))
        }
        b'*' => {
            // A single star stays within one path segment.
            let segment_end = text.iter().position(|byte| *byte == b'/').unwrap_or(text.len());
            (0..=segment_end).any(|start| glob_matches_bytes(rest, &text[start..], budget))
        }
        b'?' => text.first().is_some_and(|byte| *byte != b'/') && glob_matches_bytes(rest, &text[1..], budget),
        _ => text.first() == Some(&first) && glob_matches_bytes(rest, &text[1..], budget),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// V0.2.192: a pattern with many stars used to take time that grew with every one of them; it now gives up after a
    /// fixed amount of work and matches nothing, and a sensible pattern is not affected.
    #[test]
    fn a_pattern_of_many_stars_gives_up_instead_of_stalling_a_sync() {
        let path = format!("{}/file.txt", "a".repeat(200));
        assert!(!pattern_matches("*a*a*a*a*a*a*a*a*a*a*a*a*a*a*b", &path));
        assert!(!pattern_matches("**/**/**/**/**/**/**/**/**/**/x", &"d/".repeat(100)));
        assert!(!pattern_matches("a**a**a**a**a**a**a**a**a**a**b", &"a".repeat(300)));
        // Ordinary patterns, long paths included, are well inside the limit.
        let deep = format!("{}file.md", "folder/".repeat(60));
        assert!(pattern_matches("**/*.md", &deep));
        assert!(pattern_matches("folder/**/file.md", &deep));
        assert!(pattern_matches("*.min.js", "web/static/app.min.js"));
    }

    #[test]
    fn patterns_follow_gitignore_conventions() {
        assert!(pattern_matches("*.min.js", "web/static/app.min.js"));
        assert!(!pattern_matches("*.min.js", "web/static/app.js"));
        assert!(pattern_matches("Cargo.lock", "Cargo.lock"));
        assert!(pattern_matches("Cargo.lock", "crates/tool/Cargo.lock"));
        assert!(pattern_matches("node_modules/", "web/node_modules/react/index.js"));
        assert!(!pattern_matches("node_modules/", "docs/node_modules.md"));
        assert!(pattern_matches("docs/**", "docs/guide/setup.md"));
        assert!(!pattern_matches("docs/**", "src/docs.rs"));
        assert!(pattern_matches("**/*.test.ts", "src/a/b.test.ts"));
        assert!(pattern_matches("**/*.test.ts", "b.test.ts"));
        assert!(pattern_matches("src/*.rs", "src/main.rs"));
        assert!(!pattern_matches("src/*.rs", "src/bin/tool.rs"));
        assert!(pattern_matches("apps/web/", "apps/web/src/main.tsx"));
        assert!(!pattern_matches("apps/web/", "apps/website/main.tsx"));
    }

    #[test]
    fn repository_files_are_classified_before_reading() {
        let rules = RepoFileRules::default();
        let rust = classify_repo_file("src/main.rs", 100, &rules).unwrap();
        assert_eq!(rust.artifact_type, ArtifactType::CodeFile);
        assert!(!rust.needs_text_check);

        // Go and Java are read for symbols, like Rust, so they need no text check.
        let go = classify_repo_file("cmd/server/main.go", 100, &rules).unwrap();
        assert_eq!(go.language.as_deref(), Some("Go"));
        assert_eq!(go.artifact_type, ArtifactType::CodeFile);
        assert!(!go.needs_text_check);

        let csharp = classify_repo_file("src/Program.cs", 100, &rules).unwrap();
        assert_eq!(csharp.language.as_deref(), Some("C#"));
        assert_eq!(csharp.artifact_type, ArtifactType::CodeFile);
        assert!(!csharp.needs_text_check);

        let ruby = classify_repo_file("lib/app.rb", 100, &rules).unwrap();
        assert_eq!(ruby.language.as_deref(), Some("Ruby"));
        assert!(ruby.needs_text_check);

        assert!(classify_repo_file("package-lock.json", 100, &rules).is_err());
        assert!(classify_repo_file("assets/logo.png", 100, &rules).is_err());
        assert!(classify_repo_file("data/huge.json", 5 * 1024 * 1024, &rules).is_err());

        let docs_only = RepoFileRules {
            include: vec!["docs/**".to_owned()],
            exclude: vec!["docs/drafts/".to_owned()],
        };
        assert!(classify_repo_file("docs/setup.md", 10, &docs_only).is_ok());
        assert!(classify_repo_file("src/main.rs", 10, &docs_only).is_err());
        assert!(classify_repo_file("docs/drafts/idea.md", 10, &docs_only).is_err());
    }

    #[test]
    fn unknown_files_must_be_text() {
        assert!(is_indexable_text("LICENSE", b"MIT License"));
        assert!(!is_indexable_text("data.bin", &[0, 159, 146, 150]));
        assert!(!is_indexable_text("latin1.txt", &[0xe9, 0x74, 0xe9]));
    }
}
