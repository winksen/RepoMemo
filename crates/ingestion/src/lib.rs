use std::collections::VecDeque;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::Result;
use repomemo_domain::{ArtifactType, ImportSkippedItem, SourceType};

const DEFAULT_MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;
const BINARY_SAMPLE_BYTES: usize = 8192;

const ACCEPTED_EXTENSIONS: &[&str] = &[
    "md", "mdx", "txt", "rs", "ts", "tsx", "js", "jsx", "py", "json", "toml", "yaml", "yml",
    "sql", "html", "css", "sh", "ps1",
];

const IGNORED_DIRECTORIES: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    ".vite",
];

#[derive(Debug, Clone)]
pub struct ImportOptions {
    pub max_file_bytes: u64,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            max_file_bytes: DEFAULT_MAX_FILE_BYTES,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImportCandidate {
    pub path: PathBuf,
    pub source_root: PathBuf,
    pub source_type: SourceType,
    pub relative_path: String,
    pub artifact_type: ArtifactType,
    pub language: Option<String>,
    pub mime_type: Option<String>,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Default)]
pub struct ImportDiscovery {
    pub scanned: usize,
    pub candidates: Vec<ImportCandidate>,
    pub skipped_items: Vec<ImportSkippedItem>,
}

pub fn discover_import_candidates(
    paths: &[PathBuf],
    options: &ImportOptions,
) -> Result<ImportDiscovery> {
    let mut discovery = ImportDiscovery::default();

    for path in paths {
        let path = normalize_path(path);
        match fs::metadata(&path) {
            Ok(metadata) if metadata.is_file() => {
                inspect_file(
                    &path,
                    &path,
                    SourceType::Upload,
                    metadata.len(),
                    options,
                    &mut discovery,
                );
            }
            Ok(metadata) if metadata.is_dir() => {
                if is_ignored_directory(&path) {
                    push_skip(&mut discovery, &path, "ignored directory");
                    continue;
                }
                inspect_directory(&path, options, &mut discovery)?;
            }
            Ok(_) => push_skip(&mut discovery, &path, "unsupported path type"),
            Err(error) => push_skip(
                &mut discovery,
                &path,
                &format!("could not read path metadata: {error}"),
            ),
        }
    }

    Ok(discovery)
}

pub fn accepted_extensions() -> &'static [&'static str] {
    ACCEPTED_EXTENSIONS
}

pub fn ignored_directories() -> &'static [&'static str] {
    IGNORED_DIRECTORIES
}

pub fn detect_artifact_type(path: &Path) -> Option<ArtifactType> {
    let extension = extension(path)?;
    match extension.as_str() {
        "md" | "mdx" => Some(ArtifactType::MarkdownDoc),
        "txt" => Some(ArtifactType::File),
        value if ACCEPTED_EXTENSIONS.contains(&value) => Some(ArtifactType::CodeFile),
        _ => None,
    }
}

pub fn detect_language(path: &Path) -> Option<String> {
    let extension = extension(path)?;
    let language = match extension.as_str() {
        "md" | "mdx" => "Markdown",
        "txt" => "Text",
        "rs" => "Rust",
        "ts" | "tsx" => "TypeScript",
        "js" | "jsx" => "JavaScript",
        "py" => "Python",
        "json" => "JSON",
        "toml" => "TOML",
        "yaml" | "yml" => "YAML",
        "sql" => "SQL",
        "html" => "HTML",
        "css" => "CSS",
        "sh" => "Shell",
        "ps1" => "PowerShell",
        _ => return None,
    };

    Some(language.to_owned())
}

pub fn is_probably_binary(path: &Path) -> bool {
    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(_) => return false,
    };
    let mut sample = vec![0; BINARY_SAMPLE_BYTES];
    let bytes_read = match file.read(&mut sample) {
        Ok(bytes_read) => bytes_read,
        Err(_) => return false,
    };

    sample[..bytes_read].contains(&0)
}

fn inspect_directory(
    root: &Path,
    options: &ImportOptions,
    discovery: &mut ImportDiscovery,
) -> Result<()> {
    let mut queue = VecDeque::from([root.to_path_buf()]);

    while let Some(directory) = queue.pop_front() {
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let path = entry.path();
            let metadata = match entry.metadata() {
                Ok(metadata) => metadata,
                Err(error) => {
                    push_skip(
                        discovery,
                        &path,
                        &format!("could not read path metadata: {error}"),
                    );
                    continue;
                }
            };

            if metadata.is_dir() {
                if is_ignored_directory(&path) {
                    push_skip(discovery, &path, "ignored directory");
                } else {
                    queue.push_back(path);
                }
                continue;
            }

            if metadata.is_file() {
                inspect_file(
                    &path,
                    root,
                    SourceType::Folder,
                    metadata.len(),
                    options,
                    discovery,
                );
            }
        }
    }

    Ok(())
}

fn inspect_file(
    path: &Path,
    source_root: &Path,
    source_type: SourceType,
    size_bytes: u64,
    options: &ImportOptions,
    discovery: &mut ImportDiscovery,
) {
    discovery.scanned += 1;

    if size_bytes > options.max_file_bytes {
        push_skip(discovery, path, "file is larger than 5 MB");
        return;
    }

    let Some(artifact_type) = detect_artifact_type(path) else {
        push_skip(discovery, path, "unsupported file extension");
        return;
    };

    if is_probably_binary(path) {
        push_skip(discovery, path, "binary file");
        return;
    }

    discovery.candidates.push(ImportCandidate {
        path: path.to_path_buf(),
        source_root: source_root.to_path_buf(),
        source_type,
        relative_path: relative_path(path, source_root),
        artifact_type,
        language: detect_language(path),
        mime_type: detect_mime(path),
        size_bytes,
    });
}

fn relative_path(path: &Path, source_root: &Path) -> String {
    if source_root.is_file() {
        return path
            .file_name()
            .and_then(|value| value.to_str())
            .map(str::to_owned)
            .unwrap_or_else(|| path.to_string_lossy().to_string())
            .replace('\\', "/");
    }

    path.strip_prefix(source_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn detect_mime(path: &Path) -> Option<String> {
    let extension = extension(path)?;
    let mime = match extension.as_str() {
        "md" | "mdx" => "text/markdown",
        "txt" => "text/plain",
        "json" => "application/json",
        "html" => "text/html",
        "css" => "text/css",
        _ => "text/plain",
    };

    Some(mime.to_owned())
}

fn extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
}

fn is_ignored_directory(path: &Path) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(|name| IGNORED_DIRECTORIES.contains(&name))
        .unwrap_or(false)
}

fn normalize_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn push_skip(discovery: &mut ImportDiscovery, path: &Path, reason: &str) {
    discovery.skipped_items.push(ImportSkippedItem {
        path: path.to_string_lossy().to_string(),
        reason: reason.to_owned(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn detects_supported_artifact_types() {
        assert!(matches!(
            detect_artifact_type(Path::new("README.md")),
            Some(ArtifactType::MarkdownDoc)
        ));
        assert!(matches!(
            detect_artifact_type(Path::new("main.rs")),
            Some(ArtifactType::CodeFile)
        ));
        assert!(matches!(
            detect_artifact_type(Path::new("notes.txt")),
            Some(ArtifactType::File)
        ));
        assert!(detect_artifact_type(Path::new("photo.png")).is_none());
    }

    #[test]
    fn detects_language_from_extension() {
        assert_eq!(
            detect_language(Path::new("component.tsx")).as_deref(),
            Some("TypeScript")
        );
        assert_eq!(
            detect_language(Path::new("script.py")).as_deref(),
            Some("Python")
        );
        assert_eq!(detect_language(Path::new("unknown.bin")), None);
    }

    #[test]
    fn discovers_files_and_skips_ignored_directories() {
        let root = temp_path("repomemo-ingestion-discovery");
        let ignored = root.join("node_modules");
        fs::create_dir_all(&ignored).unwrap();
        fs::write(root.join("README.md"), "# RepoMemo").unwrap();
        fs::write(ignored.join("package.json"), "{}").unwrap();

        let discovery =
            discover_import_candidates(&[root.clone()], &ImportOptions::default()).unwrap();

        assert_eq!(discovery.scanned, 1);
        assert_eq!(discovery.candidates.len(), 1);
        assert_eq!(discovery.candidates[0].relative_path, "README.md");
        assert!(discovery
            .skipped_items
            .iter()
            .any(|item| item.reason == "ignored directory"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn detects_binary_sample() {
        let root = temp_path("repomemo-ingestion-binary");
        fs::create_dir_all(&root).unwrap();
        let file = root.join("binary.txt");
        fs::write(&file, [65, 0, 66]).unwrap();

        assert!(is_probably_binary(&file));

        fs::remove_dir_all(root).unwrap();
    }

    fn temp_path(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{nanos}"))
    }
}
