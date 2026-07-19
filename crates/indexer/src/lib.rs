use anyhow::Result;
use repomemo_domain::{ArtifactSummary, ArtifactType, Chunk, Symbol, SymbolKind};
use serde_json::json;
use sha2::{Digest, Sha256};
use tree_sitter::{Language, Node, Parser};

const MARKDOWN_TARGET_CHARS: usize = 1_600;
const TEXT_WINDOW_LINES: usize = 100;

#[derive(Debug, Clone)]
pub struct IndexArtifactOutput {
    pub chunks: Vec<Chunk>,
    pub symbols: Vec<Symbol>,
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
    if matches!(summary.artifact_type, ArtifactType::Image) {
        return Ok(IndexArtifactOutput {
            chunks: Vec::new(),
            symbols: Vec::new(),
            warnings: vec![
                "Images are not decoded as text. Run visual analysis with a vision-capable AI provider to make their content searchable."
                    .to_owned(),
            ],
        });
    }

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
            symbols: Vec::new(),
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

    let symbols = match extract_symbols(summary, &text) {
        Ok(symbols) => symbols,
        Err(error) => {
            warnings.push(format!("Symbol indexing was skipped: {error}"));
            Vec::new()
        }
    };

    Ok(IndexArtifactOutput {
        chunks,
        symbols,
        warnings,
    })
}

/// Turns a vision model's faithful description into the single searchable representation
/// for an image. Images deliberately have no line chunks or symbol index.
pub fn index_image_description(
    summary: &ArtifactSummary,
    description: &str,
) -> IndexArtifactOutput {
    let text = description.trim();
    if text.is_empty() {
        return IndexArtifactOutput {
            chunks: Vec::new(),
            symbols: Vec::new(),
            warnings: vec!["The vision provider returned no usable image description.".to_owned()],
        };
    }

    let chunk = Chunk {
        id: String::new(),
        artifact_id: summary.id.clone(),
        workspace_id: summary.workspace_id.clone(),
        chunk_index: 0,
        token_count: Some(estimate_tokens(text)),
        start_line: None,
        end_line: None,
        heading_path: Some("Visual description".to_owned()),
        content_hash: content_hash(text.as_bytes()),
        embedding_status: "not_configured".to_owned(),
        metadata: json!({
            "source_path": summary.path,
            "mime_type": summary.mime_type,
            "artifact_type": "image",
            "representation": "visual_description",
            "derived_from": "image"
        }),
        text: text.to_owned(),
    };

    IndexArtifactOutput {
        chunks: vec![chunk],
        symbols: Vec::new(),
        warnings: Vec::new(),
    }
}

fn extract_symbols(summary: &ArtifactSummary, text: &str) -> Result<Vec<Symbol>> {
    let language = match summary.language.as_deref() {
        Some("TypeScript") if summary.path.to_ascii_lowercase().ends_with(".tsx") => {
            tree_sitter_typescript::LANGUAGE_TSX.into()
        }
        Some("TypeScript") | Some("JavaScript") => {
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
        }
        Some("Python") => tree_sitter_python::LANGUAGE.into(),
        Some("Rust") => tree_sitter_rust::LANGUAGE.into(),
        _ => return Ok(Vec::new()),
    };

    parse_symbols(summary, text, language)
}

fn parse_symbols(summary: &ArtifactSummary, text: &str, language: Language) -> Result<Vec<Symbol>> {
    let mut parser = Parser::new();
    parser.set_language(&language)?;
    let tree = parser
        .parse(text, None)
        .ok_or_else(|| anyhow::anyhow!("parser returned no syntax tree"))?;
    let mut symbols = Vec::new();
    collect_symbols(tree.root_node(), summary, text, &mut symbols);
    symbols.sort_by_key(|symbol| (symbol.start_line.unwrap_or(i64::MAX), symbol.name.clone()));
    Ok(symbols)
}

fn collect_symbols(
    node: Node<'_>,
    summary: &ArtifactSummary,
    text: &str,
    output: &mut Vec<Symbol>,
) {
    if let Some((kind, name_node)) = symbol_descriptor(node, summary.language.as_deref()) {
        if let Ok(name) = name_node.utf8_text(text.as_bytes()) {
            let name = name.trim();
            if !name.is_empty() {
                let start_line = node.start_position().row as i64 + 1;
                let end_line = node.end_position().row as i64 + 1;
                output.push(Symbol {
                    id: String::new(),
                    artifact_id: summary.id.clone(),
                    workspace_id: summary.workspace_id.clone(),
                    kind,
                    name: name.to_owned(),
                    signature: symbol_signature(node, text),
                    start_line: Some(start_line),
                    end_line: Some(end_line),
                    metadata: json!({ "language": summary.language, "source_path": summary.path }),
                });
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_symbols(child, summary, text, output);
    }
}

fn symbol_descriptor<'a>(node: Node<'a>, language: Option<&str>) -> Option<(SymbolKind, Node<'a>)> {
    let name = node.child_by_field_name("name")?;
    let kind = match (language, node.kind()) {
        (Some("TypeScript") | Some("JavaScript"), "function_declaration") => SymbolKind::Function,
        (Some("TypeScript") | Some("JavaScript"), "class_declaration") => SymbolKind::Class,
        (Some("TypeScript") | Some("JavaScript"), "interface_declaration") => SymbolKind::Interface,
        (Some("TypeScript") | Some("JavaScript"), "enum_declaration") => SymbolKind::Enum,
        (Some("TypeScript") | Some("JavaScript"), "method_definition") => SymbolKind::Method,
        (Some("Python"), "function_definition") if has_ancestor_kind(node, "class_definition") => {
            SymbolKind::Method
        }
        (Some("Python"), "function_definition") => SymbolKind::Function,
        (Some("Python"), "class_definition") => SymbolKind::Class,
        (Some("Rust"), "function_item") if has_ancestor_kind(node, "impl_item") => {
            SymbolKind::Method
        }
        (Some("Rust"), "function_item") => SymbolKind::Function,
        (Some("Rust"), "enum_item") => SymbolKind::Enum,
        (Some("Rust"), "struct_item") => SymbolKind::Class,
        (Some("Rust"), "trait_item") => SymbolKind::Interface,
        _ => return None,
    };
    Some((kind, name))
}

fn has_ancestor_kind(mut node: Node<'_>, kind: &str) -> bool {
    while let Some(parent) = node.parent() {
        if parent.kind() == kind {
            return true;
        }
        node = parent;
    }
    false
}

fn symbol_signature(node: Node<'_>, text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let end = node
        .child_by_field_name("body")
        .map(|body| body.start_byte())
        .unwrap_or_else(|| node.end_byte())
        .min(node.start_byte().saturating_add(400));
    let signature = std::str::from_utf8(bytes.get(node.start_byte()..end)?).ok()?;
    let signature = signature.split_whitespace().collect::<Vec<_>>().join(" ");
    (!signature.is_empty()).then_some(signature)
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

    #[test]
    fn images_are_not_lossily_decoded_into_line_chunks() {
        let summary = artifact_summary(ArtifactType::Image, None);
        let output = index_artifact(&summary, &[0x89, b'P', b'N', b'G', 0, 1]).unwrap();

        assert!(output.chunks.is_empty());
        assert_eq!(output.warnings.len(), 1);
    }

    #[test]
    fn visual_description_is_a_single_non_line_chunk() {
        let summary = artifact_summary(ArtifactType::Image, None);
        let output = index_image_description(&summary, "A login screen with an email field.");

        assert_eq!(output.chunks.len(), 1);
        assert_eq!(
            output.chunks[0].heading_path.as_deref(),
            Some("Visual description")
        );
        assert_eq!(output.chunks[0].start_line, None);
        assert_eq!(
            output.chunks[0].metadata["representation"],
            "visual_description"
        );
    }

    #[test]
    fn typescript_symbols_include_functions_classes_interfaces_and_methods() {
        let summary = artifact_summary(ArtifactType::CodeFile, Some("TypeScript"));
        let text = "interface Store { read(): string }\nclass MemoryStore { read(): string { return 'ok' } }\nfunction createStore(): Store { return new MemoryStore() }";
        let output = index_artifact(&summary, text.as_bytes()).unwrap();

        assert!(has_symbol(&output, "Store", SymbolKind::Interface, 1));
        assert!(has_symbol(&output, "MemoryStore", SymbolKind::Class, 2));
        assert!(has_symbol(&output, "read", SymbolKind::Method, 2));
        assert!(has_symbol(&output, "createStore", SymbolKind::Function, 3));
    }

    #[test]
    fn python_symbols_distinguish_methods() {
        let summary = artifact_summary(ArtifactType::CodeFile, Some("Python"));
        let text = "class Index:\n    def search(self, query):\n        return query\n\ndef build_index():\n    return Index()\n";
        let output = index_artifact(&summary, text.as_bytes()).unwrap();

        assert!(has_symbol(&output, "Index", SymbolKind::Class, 1));
        assert!(has_symbol(&output, "search", SymbolKind::Method, 2));
        assert!(has_symbol(&output, "build_index", SymbolKind::Function, 5));
    }

    #[test]
    fn rust_symbols_include_enums_functions_and_impl_methods() {
        let summary = artifact_summary(ArtifactType::CodeFile, Some("Rust"));
        let text = "enum State { Ready }\nstruct Index;\nimpl Index { fn search(&self) {} }\nfn build() -> Index { Index }\n";
        let output = index_artifact(&summary, text.as_bytes()).unwrap();

        assert!(has_symbol(&output, "State", SymbolKind::Enum, 1));
        assert!(has_symbol(&output, "Index", SymbolKind::Class, 2));
        assert!(has_symbol(&output, "search", SymbolKind::Method, 3));
        assert!(has_symbol(&output, "build", SymbolKind::Function, 4));
    }

    #[test]
    fn malformed_code_still_produces_text_chunks() {
        let summary = artifact_summary(ArtifactType::CodeFile, Some("TypeScript"));
        let text = "function incomplete( {\n  return value\n";
        let output = index_artifact(&summary, text.as_bytes()).unwrap();

        assert_eq!(output.chunks.len(), 1);
        assert_eq!(output.chunks[0].start_line, Some(1));
    }

    fn has_symbol(output: &IndexArtifactOutput, name: &str, kind: SymbolKind, line: i64) -> bool {
        output.symbols.iter().any(|symbol| {
            symbol.name == name
                && std::mem::discriminant(&symbol.kind) == std::mem::discriminant(&kind)
                && symbol.start_line == Some(line)
        })
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
