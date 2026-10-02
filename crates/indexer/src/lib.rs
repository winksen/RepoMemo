use anyhow::Result;
use repomemo_domain::{ArtifactSummary, ArtifactType, Chunk, Symbol, SymbolKind};
use repomemo_ingestion::extract_document_text;
use serde_json::json;
use sha2::{Digest, Sha256};
use tree_sitter::{Language, Node, Parser, Tree};

const MARKDOWN_TARGET_CHARS: usize = 1_600;
const TEXT_WINDOW_LINES: usize = 100;
const CODE_TARGET_CHARS: usize = 1_800;
/// Code units larger than this are split along their own structure (methods of
/// a class, items of an impl block) instead of being kept whole.
const CODE_SPLIT_CHARS: usize = 3_600;
const CODE_MAX_NESTING: usize = 3;

/// Version of the chunking and symbol logic. Bump it whenever a change would
/// produce different chunks, so stored indexes are refreshed in the background.
/// Chunks whose text is unchanged keep their identity when re-indexed.
pub const INDEXER_VERSION: i64 = 2;

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
    let text = match extract_document_text(std::path::Path::new(&summary.path), bytes)? {
        Some(text) => {
            warnings.push("Document text was extracted locally for indexing.".to_owned());
            text
        }
        None => match String::from_utf8(bytes.to_vec()) {
            Ok(text) => text,
            Err(error) => {
                warnings.push(format!(
                    "Artifact contained invalid UTF-8; decoded lossily for indexing: {error}"
                ));
                String::from_utf8_lossy(bytes).to_string()
            }
        },
    };

    if text.trim().is_empty() {
        return Ok(IndexArtifactOutput {
            chunks: Vec::new(),
            symbols: Vec::new(),
            warnings,
        });
    }

    let tree = match tree_sitter_language(summary) {
        Some(language) => match parse_tree(&language, &text) {
            Ok(tree) => Some(tree),
            Err(error) => {
                warnings.push(format!("Symbol indexing was skipped: {error}"));
                None
            }
        },
        None => None,
    };

    let is_markdown = matches!(
        summary.artifact_type,
        ArtifactType::MarkdownDoc
            | ArtifactType::Note
            | ArtifactType::Decision
            | ArtifactType::Runbook
    ) || matches!(summary.language.as_deref(), Some("Markdown"));
    let chunks = if is_markdown {
        chunk_markdown(summary, &text)
    } else {
        tree.as_ref()
            .and_then(|tree| chunk_code_by_structure(summary, &text, tree))
            .unwrap_or_else(|| chunk_by_line_windows(summary, &text))
    };

    let symbols = tree
        .as_ref()
        .map(|tree| symbols_from_tree(tree, summary, &text))
        .unwrap_or_default();

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

fn tree_sitter_language(summary: &ArtifactSummary) -> Option<Language> {
    Some(match summary.language.as_deref() {
        Some("TypeScript") if summary.path.to_ascii_lowercase().ends_with(".tsx") => {
            tree_sitter_typescript::LANGUAGE_TSX.into()
        }
        Some("TypeScript") | Some("JavaScript") => {
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
        }
        Some("Python") => tree_sitter_python::LANGUAGE.into(),
        Some("Rust") => tree_sitter_rust::LANGUAGE.into(),
        _ => return None,
    })
}

fn parse_tree(language: &Language, text: &str) -> Result<Tree> {
    let mut parser = Parser::new();
    parser.set_language(language)?;
    parser
        .parse(text, None)
        .ok_or_else(|| anyhow::anyhow!("parser returned no syntax tree"))
}

fn symbols_from_tree(tree: &Tree, summary: &ArtifactSummary, text: &str) -> Vec<Symbol> {
    let mut symbols = Vec::new();
    collect_symbols(tree.root_node(), summary, text, &mut symbols);
    symbols.sort_by_key(|symbol| (symbol.start_line.unwrap_or(i64::MAX), symbol.name.clone()));
    symbols
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

/// A run of source rows that belongs to one declaration (or one loose statement).
#[derive(Debug, Clone)]
struct CodeUnit {
    start_row: usize,
    end_row: usize,
    label: Option<String>,
    context: Vec<String>,
    /// Comments and attributes stay attached to the declaration that follows.
    leads_next: bool,
}

#[derive(Debug)]
struct CodeGroup {
    context: Vec<String>,
    labels: Vec<String>,
    start_row: usize,
    end_row: usize,
}

/// Chunks source code along declaration boundaries so a function, class or
/// impl block is not cut in half. Small neighbours are packed together up to
/// `CODE_TARGET_CHARS`, and oversized containers are split along their members.
/// Every chunk records the enclosing scope and declared names in its heading
/// path, which is also searchable. Returns `None` when the tree has nothing to
/// anchor on, so the caller can fall back to plain line windows.
fn chunk_code_by_structure(
    summary: &ArtifactSummary,
    text: &str,
    tree: &Tree,
) -> Option<Vec<Chunk>> {
    let lines = numbered_lines(text);
    if lines.is_empty() {
        return None;
    }

    let mut units = Vec::new();
    collect_code_units(tree.root_node(), text, &[], 0, &mut units);
    if units.is_empty() {
        return None;
    }
    let units = merge_leading_comments(close_unit_gaps(units, lines.len()));

    let mut chunks = Vec::new();
    let mut next_index = 0_i64;
    for group in pack_code_units(&units, &lines) {
        let heading_path = code_heading(&group.context, &group.labels);
        let rows = lines[group.start_row..=group.end_row].to_vec();
        let rows_chars: usize = rows.iter().map(|line| line.text.chars().count() + 1).sum();
        let windows = if rows_chars > CODE_SPLIT_CHARS {
            // A single declaration with no structure left to split on.
            split_section_by_chars(rows, CODE_TARGET_CHARS)
        } else {
            vec![rows]
        };
        for window in windows {
            if let Some(chunk) = build_chunk(summary, next_index, window, heading_path.clone()) {
                chunks.push(chunk);
                next_index += 1;
            }
        }
    }

    (!chunks.is_empty()).then_some(chunks)
}

fn collect_code_units(
    container: Node<'_>,
    text: &str,
    context: &[String],
    depth: usize,
    out: &mut Vec<CodeUnit>,
) {
    let mut cursor = container.walk();
    for child in container.named_children(&mut cursor) {
        let target = declaration_target(child);
        let label = unit_label(target, text);
        let start_row = child.start_position().row;
        let end_row = child.end_position().row;
        let leads_next = child.kind().contains("comment") || child.kind() == "attribute_item";
        let size = child.end_byte().saturating_sub(child.start_byte());

        if size > CODE_SPLIT_CHARS && depth < CODE_MAX_NESTING {
            if let Some(body) = target.child_by_field_name("body") {
                let first_member_row = body
                    .named_child(0)
                    .map(|member| member.start_position().row)
                    .unwrap_or(start_row);
                if body.named_child_count() > 1 && first_member_row > start_row {
                    let header_index = out.len();
                    out.push(CodeUnit {
                        start_row,
                        end_row: first_member_row - 1,
                        label: label.clone(),
                        context: context.to_vec(),
                        leads_next: false,
                    });
                    let mut inner_context = context.to_vec();
                    inner_context.push(label.clone().unwrap_or_else(|| target.kind().to_owned()));
                    collect_code_units(body, text, &inner_context, depth + 1, out);
                    if out.len() > header_index + 1 {
                        // Closing braces belong to the last member.
                        if let Some(last) = out.last_mut() {
                            last.end_row = last.end_row.max(end_row);
                        }
                        continue;
                    }
                    // Nothing to split on after all: drop the header and keep it whole.
                    out.truncate(header_index);
                }
            }
        }

        out.push(CodeUnit {
            start_row,
            end_row,
            label,
            context: context.to_vec(),
            leads_next,
        });
    }
}

/// Looks through wrappers such as `export` and Python decorators to the
/// declaration that carries the name and body.
fn declaration_target(node: Node<'_>) -> Node<'_> {
    match node.kind() {
        "export_statement" => node.child_by_field_name("declaration").unwrap_or(node),
        "decorated_definition" => node.child_by_field_name("definition").unwrap_or(node),
        _ => node,
    }
}

fn unit_label(node: Node<'_>, text: &str) -> Option<String> {
    let read = |node: Node<'_>| {
        node.utf8_text(text.as_bytes()).ok().map(|value| {
            let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
            value.chars().take(80).collect::<String>()
        })
    };
    match node.kind() {
        "impl_item" => {
            let type_name = read(node.child_by_field_name("type")?)?;
            Some(match node.child_by_field_name("trait").and_then(read) {
                Some(trait_name) => format!("impl {trait_name} for {type_name}"),
                None => format!("impl {type_name}"),
            })
        }
        "lexical_declaration" | "variable_declaration" => {
            let mut cursor = node.walk();
            let declarator = node
                .named_children(&mut cursor)
                .find(|child| child.kind() == "variable_declarator")?;
            // Destructuring patterns such as `const [a, setA] = useState()` are not names.
            let name = declarator.child_by_field_name("name")?;
            (name.kind() == "identifier").then(|| read(name)).flatten()
        }
        _ => read(node.child_by_field_name("name")?),
    }
}

/// Makes the units cover every row exactly once: blank lines and gaps join the
/// following unit, and trailing rows join the last one. Units that share rows
/// with their predecessor (several declarations on one line) are folded in.
fn close_unit_gaps(units: Vec<CodeUnit>, total_rows: usize) -> Vec<CodeUnit> {
    let mut closed: Vec<CodeUnit> = Vec::with_capacity(units.len());
    for mut unit in units {
        let next_start = closed.last().map_or(0, |previous| previous.end_row + 1);
        if unit.end_row < next_start {
            if let Some(previous) = closed.last_mut() {
                previous.label = match (previous.label.take(), unit.label) {
                    (Some(first), Some(second)) => Some(format!("{first}, {second}")),
                    (first, second) => first.or(second),
                };
            }
            continue;
        }
        unit.start_row = next_start;
        closed.push(unit);
    }
    if let Some(last) = closed.last_mut() {
        last.end_row = total_rows.saturating_sub(1);
    }
    closed
}

fn merge_leading_comments(units: Vec<CodeUnit>) -> Vec<CodeUnit> {
    let mut merged: Vec<CodeUnit> = Vec::with_capacity(units.len());
    let mut leading: Option<CodeUnit> = None;
    for unit in units {
        let unit = match leading.take() {
            Some(lead) if lead.context == unit.context => CodeUnit {
                start_row: lead.start_row,
                ..unit
            },
            Some(lead) => {
                merged.push(lead);
                unit
            }
            None => unit,
        };
        if unit.leads_next {
            leading = Some(unit);
        } else {
            merged.push(unit);
        }
    }
    merged.extend(leading);
    merged
}

fn pack_code_units(units: &[CodeUnit], lines: &[Line]) -> Vec<CodeGroup> {
    let row_chars = |unit: &CodeUnit| -> usize {
        lines[unit.start_row..=unit.end_row]
            .iter()
            .map(|line| line.text.chars().count() + 1)
            .sum()
    };

    let mut groups: Vec<CodeGroup> = Vec::new();
    let mut current_chars = 0_usize;
    for unit in units {
        let unit_chars = row_chars(unit);
        match groups.last_mut() {
            Some(group)
                if group.context == unit.context
                    && current_chars + unit_chars <= CODE_TARGET_CHARS =>
            {
                group.end_row = unit.end_row;
                group.labels.extend(unit.label.clone());
                current_chars += unit_chars;
            }
            _ => {
                groups.push(CodeGroup {
                    context: unit.context.clone(),
                    labels: unit.label.iter().cloned().collect(),
                    start_row: unit.start_row,
                    end_row: unit.end_row,
                });
                current_chars = unit_chars;
            }
        }
    }
    groups
}

fn code_heading(context: &[String], labels: &[String]) -> Option<String> {
    let mut parts = context.to_vec();
    if !labels.is_empty() {
        let mut declared = labels.iter().take(4).cloned().collect::<Vec<_>>().join(", ");
        if labels.len() > 4 {
            declared.push_str(", …");
        }
        parts.push(declared);
    }
    (!parts.is_empty()).then(|| parts.join(" > "))
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
    fn unsupported_languages_use_line_windows() {
        let summary = artifact_summary(ArtifactType::CodeFile, Some("Go"));
        let text = (1..=205)
            .map(|index| format!("func line{index}() {{}}"))
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
    fn small_declarations_are_packed_and_named() {
        let summary = artifact_summary(ArtifactType::CodeFile, Some("Rust"));
        let text = "use std::fmt;\n\n/// Adds.\nfn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n\nfn sub(a: i32, b: i32) -> i32 {\n    a - b\n}\n";

        let output = index_artifact(&summary, text.as_bytes()).unwrap();

        assert_eq!(output.chunks.len(), 1);
        assert_eq!(output.chunks[0].heading_path.as_deref(), Some("add, sub"));
        assert_covers_all_lines(&output.chunks, text);
    }

    #[test]
    fn large_impl_blocks_are_split_by_method_with_scope_in_the_heading() {
        let summary = artifact_summary(ArtifactType::CodeFile, Some("Rust"));
        let padding = "        let _padding = \"........................................\";\n".repeat(8);
        let methods = (1..=12)
            .map(|index| {
                format!(
                    "    /// Method {index}.\n    fn method_{index}(&self) -> usize {{\n{padding}        {index}\n    }}\n"
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let text = format!("struct Store;\n\nimpl Store {{\n{methods}}}\n");

        let output = index_artifact(&summary, text.as_bytes()).unwrap();

        assert!(output.chunks.len() > 3);
        assert!(output
            .chunks
            .iter()
            .skip(1)
            .all(|chunk| chunk.heading_path.as_deref().unwrap().starts_with("impl Store")));
        // No method is cut in half: every chunk that opens a method also closes it.
        for chunk in &output.chunks {
            let opens = chunk.text.matches("fn method_").count();
            let closes = chunk.text.matches("\n    }").count();
            assert!(closes >= opens, "chunk cut a method: {}", chunk.text);
        }
        assert_covers_all_lines(&output.chunks, &text);
    }

    #[test]
    fn python_decorators_and_class_methods_keep_their_scope() {
        let summary = artifact_summary(ArtifactType::CodeFile, Some("Python"));
        let body = "        value = 1\n".repeat(200);
        let text = format!(
            "import os\n\n@cache\ndef first():\n    return 1\n\nclass Index:\n    def one(self):\n{body}\n    def two(self):\n{body}"
        );

        let output = index_artifact(&summary, text.as_bytes()).unwrap();

        assert!(output.chunks[0]
            .heading_path
            .as_deref()
            .unwrap()
            .contains("first"));
        assert!(output
            .chunks
            .iter()
            .any(|chunk| chunk.heading_path.as_deref().is_some_and(|path| path.starts_with("Index > one"))));
        assert_covers_all_lines(&output.chunks, &text);
    }

    #[test]
    fn typescript_exports_and_arrow_functions_are_named() {
        let summary = artifact_summary(ArtifactType::CodeFile, Some("TypeScript"));
        let text = "export function createStore() {\n  return new Map();\n}\n\nexport const readStore = () => {\n  return 1;\n};\n";

        let output = index_artifact(&summary, text.as_bytes()).unwrap();

        assert_eq!(
            output.chunks[0].heading_path.as_deref(),
            Some("createStore, readStore")
        );
        assert_covers_all_lines(&output.chunks, text);
    }

    #[test]
    fn identical_code_produces_identical_chunk_hashes() {
        let summary = artifact_summary(ArtifactType::CodeFile, Some("Rust"));
        let text = "fn stable() {}\n";
        let first = index_artifact(&summary, text.as_bytes()).unwrap();
        let second = index_artifact(&summary, text.as_bytes()).unwrap();

        assert_eq!(first.chunks[0].content_hash, second.chunks[0].content_hash);
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

    /// Chunks must be contiguous and cover the file, so citations never point
    /// at lines that no chunk owns.
    fn assert_covers_all_lines(chunks: &[Chunk], text: &str) {
        let total = text.lines().count() as i64;
        assert_eq!(chunks.first().unwrap().start_line, Some(1));
        assert_eq!(chunks.last().unwrap().end_line, Some(total));
        for pair in chunks.windows(2) {
            assert_eq!(pair[1].start_line, pair[0].end_line.map(|line| line + 1));
        }
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
