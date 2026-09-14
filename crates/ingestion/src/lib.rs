use std::collections::VecDeque;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use flate2::read::DeflateDecoder;
use quick_xml::events::Event;
use quick_xml::Reader;
use repomemo_domain::{ArtifactType, ImportSkippedItem, SourceType};

const DEFAULT_MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;
const BINARY_SAMPLE_BYTES: usize = 8192;

const ACCEPTED_EXTENSIONS: &[&str] = &[
    "md", "mdx", "txt", "rs", "ts", "tsx", "js", "jsx", "py", "json", "toml", "yaml", "yml", "sql",
    "html", "css", "sh", "ps1", "doc", "docx", "png", "jpg", "jpeg", "gif", "webp", "svg", "bmp",
];

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp"];

pub fn is_image_extension(ext: &str) -> bool {
    IMAGE_EXTENSIONS.contains(&ext)
}

pub fn is_word_extension(ext: &str) -> bool {
    matches!(ext, "doc" | "docx")
}

pub fn is_word_document(path: &Path) -> bool {
    extension(path).is_some_and(|extension| is_word_extension(&extension))
}

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
        "txt" | "doc" | "docx" => Some(ArtifactType::File),
        value if IMAGE_EXTENSIONS.contains(&value) => Some(ArtifactType::Image),
        value if ACCEPTED_EXTENSIONS.contains(&value) => Some(ArtifactType::CodeFile),
        _ => None,
    }
}

pub fn detect_language(path: &Path) -> Option<String> {
    let extension = extension(path)?;
    let language = match extension.as_str() {
        "md" | "mdx" => "Markdown",
        "txt" => "Text",
        "doc" | "docx" => "Word",
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

    let is_image = matches!(artifact_type, ArtifactType::Image);

    if !is_image && !is_word_document(path) && is_probably_binary(path) {
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
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "json" => "application/json",
        "html" => "text/html",
        "css" => "text/css",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        _ => "text/plain",
    };

    Some(mime.to_owned())
}

/// Returns an indexable, human-readable representation for a Word document.
/// DOCX is decoded from its WordprocessingML body. Legacy DOC reads the
/// WordDocument stream, which covers the common text-only Word layouts.
pub fn extract_word_text(path: &Path, bytes: &[u8]) -> Result<Option<String>> {
    match extension(path).as_deref() {
        Some("docx") => extract_docx_text(bytes).map(Some),
        Some("doc") => extract_legacy_doc_text(bytes).map(Some),
        _ => Ok(None),
    }
}

const MAX_WORD_XML_BYTES: u64 = 16 * 1024 * 1024;

fn extract_docx_text(bytes: &[u8]) -> Result<String> {
    let document_xml = read_zip_member(bytes, "word/document.xml")
        .context("could not read word/document.xml from the DOCX file")?;
    let mut reader = Reader::from_reader(document_xml.as_slice());
    reader.config_mut().trim_text(false);
    let mut buffer = Vec::new();
    let mut text = String::new();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) if word_tag(event.name().as_ref(), b"p") => {
                if !text.is_empty() && !text.ends_with('\n') {
                    text.push('\n');
                }
            }
            Ok(Event::Empty(event)) if word_tag(event.name().as_ref(), b"tab") => text.push('\t'),
            Ok(Event::Empty(event))
                if word_tag(event.name().as_ref(), b"br")
                    || word_tag(event.name().as_ref(), b"cr") =>
            {
                text.push('\n')
            }
            Ok(Event::Text(event)) => {
                text.push_str(&event.decode().context("invalid XML text in DOCX")?)
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(anyhow::anyhow!("invalid WordprocessingML: {error}")),
            _ => {}
        }
        buffer.clear();
    }

    let text = normalize_word_text(&text);
    if text.is_empty() {
        bail!("The DOCX file contains no readable body text.");
    }
    Ok(text)
}

fn word_tag(name: &[u8], local_name: &[u8]) -> bool {
    name == local_name || name.strip_prefix(b"w:") == Some(local_name)
}

fn read_zip_member(bytes: &[u8], member_name: &str) -> Result<Vec<u8>> {
    let eocd_start = bytes.len().saturating_sub(65_557);
    let eocd = bytes[eocd_start..]
        .windows(4)
        .rposition(|window| window == [0x50, 0x4b, 0x05, 0x06])
        .map(|offset| eocd_start + offset)
        .ok_or_else(|| anyhow::anyhow!("DOCX ZIP end record was not found"))?;
    let central_directory_offset = le_u32(bytes, eocd + 16)? as usize;
    let entries = le_u16(bytes, eocd + 10)? as usize;
    let mut cursor = central_directory_offset;

    for _ in 0..entries {
        if bytes.get(cursor..cursor + 4) != Some(&[0x50, 0x4b, 0x01, 0x02]) {
            bail!("DOCX ZIP central directory is malformed");
        }
        let compression = le_u16(bytes, cursor + 10)?;
        let compressed_size = le_u32(bytes, cursor + 20)? as usize;
        let filename_length = le_u16(bytes, cursor + 28)? as usize;
        let extra_length = le_u16(bytes, cursor + 30)? as usize;
        let comment_length = le_u16(bytes, cursor + 32)? as usize;
        let local_header_offset = le_u32(bytes, cursor + 42)? as usize;
        let filename_start = cursor + 46;
        let filename_end = filename_start + filename_length;
        let filename = std::str::from_utf8(
            bytes
                .get(filename_start..filename_end)
                .ok_or_else(|| anyhow::anyhow!("DOCX ZIP filename is truncated"))?,
        )?;
        cursor = filename_end + extra_length + comment_length;

        if filename != member_name {
            continue;
        }
        if bytes.get(local_header_offset..local_header_offset + 4)
            != Some(&[0x50, 0x4b, 0x03, 0x04])
        {
            bail!("DOCX ZIP local entry is malformed");
        }
        let local_name_length = le_u16(bytes, local_header_offset + 26)? as usize;
        let local_extra_length = le_u16(bytes, local_header_offset + 28)? as usize;
        let data_start = local_header_offset + 30 + local_name_length + local_extra_length;
        let data_end = data_start + compressed_size;
        let compressed = bytes
            .get(data_start..data_end)
            .ok_or_else(|| anyhow::anyhow!("DOCX ZIP entry is truncated"))?;
        let mut output = Vec::new();
        match compression {
            0 => output.extend_from_slice(compressed),
            8 => {
                DeflateDecoder::new(compressed)
                    .take(MAX_WORD_XML_BYTES + 1)
                    .read_to_end(&mut output)?;
            }
            _ => bail!("DOCX uses unsupported ZIP compression method {compression}"),
        }
        if output.len() as u64 > MAX_WORD_XML_BYTES {
            bail!("DOCX body exceeds the 16 MiB extraction limit");
        }
        return Ok(output);
    }
    bail!("DOCX file does not contain word/document.xml")
}

fn le_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| anyhow::anyhow!("DOCX ZIP metadata is truncated"))?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

fn le_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| anyhow::anyhow!("DOCX ZIP metadata is truncated"))?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

fn extract_legacy_doc_text(bytes: &[u8]) -> Result<String> {
    let mut compound = cfb::CompoundFile::open(Cursor::new(bytes))
        .context("legacy .doc files must use the Microsoft Compound File format")?;
    let stream = compound
        .open_stream("/WordDocument")
        .context("legacy .doc is missing its WordDocument stream")?;
    let mut word_document = Vec::new();
    stream
        .take(MAX_WORD_XML_BYTES + 1)
        .read_to_end(&mut word_document)?;
    if word_document.len() as u64 > MAX_WORD_XML_BYTES {
        bail!("legacy .doc body exceeds the 16 MiB extraction limit");
    }

    let fc_min = le_u32(&word_document, 24).unwrap_or(0) as usize;
    let fc_mac = le_u32(&word_document, 28).unwrap_or(word_document.len() as u32) as usize;
    let body = word_document.get(fc_min..fc_mac).unwrap_or(&word_document);
    let likely_utf16 = body
        .chunks(2)
        .take(128)
        .filter(|pair| pair.len() == 2 && pair[1] == 0)
        .count()
        > 12;
    let text = if likely_utf16 {
        let units = body
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        String::from_utf16_lossy(&units)
    } else {
        body.iter().map(|byte| *byte as char).collect()
    };
    let text = normalize_word_text(&text);
    if text.is_empty() {
        bail!("The legacy .doc file contains no readable body text.");
    }
    Ok(text)
}

fn normalize_word_text(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            '\r' | '\n' => '\n',
            '\t' => '\t',
            character if character.is_control() => ' ',
            character => character,
        })
        .collect::<String>()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
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
        assert!(matches!(
            detect_artifact_type(Path::new("photo.png")),
            Some(ArtifactType::Image)
        ));
        assert!(detect_artifact_type(Path::new("blob.bin")).is_none());
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

    #[test]
    fn extracts_paragraphs_from_docx_body() {
        let xml = br#"<?xml version="1.0"?><w:document xmlns:w="urn:word"><w:body><w:p><w:r><w:t>Project brief</w:t></w:r></w:p><w:p><w:r><w:t>Searchable evidence</w:t></w:r></w:p></w:body></w:document>"#;
        let text = extract_word_text(Path::new("brief.docx"), &stored_docx(xml))
            .unwrap()
            .unwrap();

        assert_eq!(text, "Project brief\nSearchable evidence");
    }

    fn stored_docx(xml: &[u8]) -> Vec<u8> {
        let name = b"word/document.xml";
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04]);
        bytes.extend_from_slice(&20_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&(xml.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(xml.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(name.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(name);
        bytes.extend_from_slice(xml);

        let central_directory_offset = bytes.len() as u32;
        bytes.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02]);
        bytes.extend_from_slice(&20_u16.to_le_bytes());
        bytes.extend_from_slice(&20_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&(xml.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(xml.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(name.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(name);

        let central_directory_size = bytes.len() as u32 - central_directory_offset;
        bytes.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06]);
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&central_directory_size.to_le_bytes());
        bytes.extend_from_slice(&central_directory_offset.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes
    }

    fn temp_path(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{nanos}"))
    }
}
