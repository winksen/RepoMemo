use anyhow::Result;
use repomemo_domain::{ArtifactSummary, ArtifactType, Chunk};
use serde_json::json;
use sha2::{Digest, Sha256};

const MARKDOWN_TARGET_CHARS: usize = 1_600;
const TEXT_WINDOW_LINES: usize = 100;

#[derive(Debug, Clone)]
pub struct IndexArtifactOutput {
    pub chunks: Vec<Chunk>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
struct Line {
    number: i64,
    text: String,
}

#[derive(Debug, Clone)]
struct MarkdownSection {
    heading_path: Vec<String>,
    lines: Vec<Line>,
}

pub fn index_artifact(summary: &ArtifactSummary, bytes: &[u8]) -> Result<IndexArtifactOutput> {
    let mut warnings = Vec::new();
    let text = match String::from_utf8(bytes.to_vec()) {
        Ok(text) => text,
        Err(error) => {
            warnings.push(format!(
                "Artifact contained invalid UTF-8; decoded lossily for indexing: {error}"
            ));
            String::from_utf8_lossy(bytes).to_string()
        }
    };

    if text.trim().is_empty() {
        return Ok(IndexArtifactOutput {
            chunks: Vec::new(),
            warnings,
        });
    }

    let chunks = if matches!(
        summary.artifact_type,
        ArtifactType::MarkdownDoc | ArtifactType::Decision | ArtifactType::Runbook
    ) || matches!(summary.language.as_deref(), Some("Markdown"))
    {
        chunk_markdown(summary, &text)
    } else {
        chunk_by_line_windows(summary, &text)
    };

    Ok(IndexArtifactOutput { chunks, warnings })
}

fn chunk_markdown(summary: &ArtifactSummary, text: &str) -> Vec<Chunk> {
    let mut sections = split_markdown_sections(text);
    if sections.is_empty() {
        sections.push(MarkdownSection {
            heading_path: Vec::new(),
            lines: numbered_lines(text),
        });
    }

    let mut chunks = Vec::new();
    let mut next_index = 0_i64;

    for section in sections {
        let heading_path = if section.heading_path.is_empty() {
            None
        } else {
            Some(section.heading_path.join(" > "))
        };

        for window in split_section_by_chars(section.lines, MARKDOWN_TARGET_CHARS) {
            if let Some(chunk) = build_chunk(summary, next_index, window, heading_path.clone()) {
                chunks.push(chunk);
                next_index += 1;
            }
        }
    }

    chunks
}

fn chunk_by_line_windows(summary: &ArtifactSummary, text: &str) -> Vec<Chunk> {
    let lines = numbered_lines(text);
    let mut chunks = Vec::new();

    for (index, window) in lines.chunks(TEXT_WINDOW_LINES).enumerate() {
        if let Some(chunk) = build_chunk(summary, index as i64, window.to_vec(), None) {
            chunks.push(chunk);
        }
    }

    chunks
}

fn split_markdown_sections(text: &str) -> Vec<MarkdownSection> {
    let mut sections = Vec::new();
    let mut heading_stack: Vec<(usize, String)> = Vec::new();
    let mut current = MarkdownSection {
        heading_path: Vec::new(),
        lines: Vec::new(),
    };

    for line in numbered_lines(text) {
        if let Some((level, heading)) = parse_markdown_heading(&line.text) {
            if !current.lines.is_empty() {
                sections.push(current);
            }

            heading_stack.retain(|(existing_level, _)| *existing_level < level);
            heading_stack.push((level, heading));

            current = MarkdownSection {
                heading_path: heading_stack
                    .iter()
                    .map(|(_, heading)| heading.clone())
                    .collect(),
                lines: vec![line],
            };
        } else {
            current.lines.push(line);
        }
    }

    if !current.lines.is_empty() {
        sections.push(current);
    }

    sections
}

fn parse_markdown_heading(line: &str) -> Option<(usize, String)> {
    let trimmed = line.trim_start();
    let level = trimmed
        .chars()
        .take_while(|character| *character == '#')
        .count();

    if level == 0 || level > 6 {
        return None;
    }

    let rest = trimmed.get(level..)?;
    if !rest.starts_with(' ') {
        return None;
    }

    let heading = rest.trim().trim_matches('#').trim();
    if heading.is_empty() {
        return None;
    }

    Some((level, heading.to_owned()))
}

fn split_section_by_chars(lines: Vec<Line>, target_chars: usize) -> Vec<Vec<Line>> {
    let mut windows = Vec::new();
    let mut current = Vec::new();
    let mut current_chars = 0_usize;

    for line in lines {
        let line_chars = line.text.chars().count() + 1;
        if !current.is_empty() && current_chars + line_chars > target_chars {
            windows.push(current);
            current = Vec::new();
            current_chars = 0;
        }

        current_chars += line_chars;
        current.push(line);
    }

    if !current.is_empty() {
        windows.push(current);
    }

    windows
}

fn numbered_lines(text: &str) -> Vec<Line> {
    text.lines()
        .enumerate()
        .map(|(index, line)| Line {
            number: index as i64 + 1,
            text: line.to_owned(),
        })
        .collect()
}

fn build_chunk(
    summary: &ArtifactSummary,
    chunk_index: i64,
    lines: Vec<Line>,
    heading_path: Option<String>,
) -> Option<Chunk> {
    let start_line = lines.first()?.number;
    let end_line = lines.last()?.number;
    let text = lines
        .into_iter()
        .map(|line| line.text)
        .collect::<Vec<_>>()
        .join("\n");

    if text.trim().is_empty() {
        return None;
    }

    Some(Chunk {
        id: String::new(),
        artifact_id: summary.id.clone(),
        workspace_id: summary.workspace_id.clone(),
        chunk_index,
        token_count: Some(estimate_tokens(&text)),
        start_line: Some(start_line),
        end_line: Some(end_line),
        heading_path,
        content_hash: content_hash(text.as_bytes()),
        embedding_status: "not_configured".to_owned(),
        metadata: json!({
            "source_path": summary.path,
            "language": summary.language,
            "artifact_type": summary.artifact_type
        }),
        text,
    })
}

fn estimate_tokens(text: &str) -> i64 {
    let words = text.split_whitespace().count();
    words.max(1) as i64
}

fn content_hash(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use repomemo_domain::ArtifactType;

    #[test]
    fn markdown_chunks_preserve_heading_paths_and_lines() {
        let summary = artifact_summary(ArtifactType::MarkdownDoc, Some("Markdown"));
        let text = "# Intro\nhello\n\n## Setup\none\ntwo\n";

        let output = index_artifact(&summary, text.as_bytes()).unwrap();

        assert_eq!(output.chunks.len(), 2);
        assert_eq!(output.chunks[0].heading_path.as_deref(), Some("Intro"));
        assert_eq!(output.chunks[0].start_line, Some(1));
        assert_eq!(output.chunks[0].end_line, Some(3));
        assert_eq!(
            output.chunks[1].heading_path.as_deref(),
            Some("Intro > Setup")
        );
        assert_eq!(output.chunks[1].start_line, Some(4));
    }

    #[test]
    fn code_chunks_use_line_windows() {
        let summary = artifact_summary(ArtifactType::CodeFile, Some("Rust"));
        let text = (1..=205)
            .map(|index| format!("fn line_{index}() {{}}"))
            .collect::<Vec<_>>()
            .join("\n");

        let output = index_artifact(&summary, text.as_bytes()).unwrap();

        assert_eq!(output.chunks.len(), 3);
        assert_eq!(output.chunks[0].start_line, Some(1));
        assert_eq!(output.chunks[0].end_line, Some(100));
        assert_eq!(output.chunks[2].start_line, Some(201));
        assert_eq!(output.chunks[2].end_line, Some(205));
    }

    #[test]
    fn empty_files_produce_no_chunks() {
        let summary = artifact_summary(ArtifactType::File, Some("Text"));
        let output = index_artifact(&summary, b"  \n\n").unwrap();

        assert!(output.chunks.is_empty());
    }

    fn artifact_summary(artifact_type: ArtifactType, language: Option<&str>) -> ArtifactSummary {
        ArtifactSummary {
            id: "artifact-id".to_owned(),
            workspace_id: "workspace-id".to_owned(),
            source_id: "source-id".to_owned(),
            source_name: "source".to_owned(),
            artifact_type,
            title: "file.md".to_owned(),
            path: "file.md".to_owned(),
            content_hash: "hash".to_owned(),
            mime_type: None,
            language: language.map(str::to_owned),
            size_bytes: 42,
            created_at: "now".to_owned(),
            updated_at: "now".to_owned(),
            indexed_at: None,
        }
    }
}
