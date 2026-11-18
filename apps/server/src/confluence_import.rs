//! Confluence migration (V0.8.12): a space exported as **HTML** (Space settings, Export space, HTML), sent as its files.
//!
//! The export is a folder of `Title_12345.html` pages, an `index.html` that lists them as a tree, an `attachments/` folder and
//! some style and script files. Each page is turned into Markdown (headings, paragraphs, lists, links, images, code, quotes and
//! simple tables), placed in folders that follow the page tree, given a line that says where it came from, and then goes through the
//! same dry run, link mapping and ledger as a directory import, so links between pages keep working and a second send makes nothing
//! twice. What does not translate is reported, not hidden: macros that did not become plain content, and the fact that an export
//! carries **no page restrictions**. Everyone who can read the workspace will read every page, so committing needs the caller's
//! explicit `acknowledge_restrictions`. Only the HTML export is understood; the XML export and the cloud site export are not.

use std::collections::{BTreeMap, HashMap};

use crate::directory_import::{run_import, DirectoryImportReport, ImportFile};
use crate::*;

const MAX_FILES: usize = 500;

#[derive(Debug, Deserialize)]
pub(crate) struct ConfluenceImportRequest {
    source_key: String,
    /// The space's key, written on each page as where it came from.
    #[serde(default)]
    space_key: String,
    #[serde(default = "yes")]
    dry_run: bool,
    #[serde(default)]
    skip_invalid: bool,
    /// The caller's statement that the export holds no restrictions to keep and that every page may be read by everyone who can
    /// read this workspace. Required to commit.
    #[serde(default)]
    acknowledge_restrictions: bool,
    files: Vec<ImportFile>,
}

fn yes() -> bool {
    true
}

fn decode_entities(text: &str) -> String {
    let mut out = text
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'");
    out = out.replace("&amp;", "&");
    out
}

/// A tag read from HTML: its name (lower case, with `/` for a closing tag), and its attributes.
struct Tag {
    name: String,
    attributes: HashMap<String, String>,
}

fn parse_tag(inner: &str) -> Tag {
    let inner = inner.trim().trim_end_matches('/').trim();
    let (name, rest) = inner.split_once(char::is_whitespace).unwrap_or((inner, ""));
    let mut attributes = HashMap::new();
    let mut remaining = rest.trim();
    while let Some(eq) = remaining.find('=') {
        let key = remaining[..eq].trim().to_ascii_lowercase();
        let after = remaining[eq + 1..].trim_start();
        let (value, next) = match after.chars().next() {
            Some(quote @ ('"' | '\'')) => match after[1..].find(quote) {
                Some(close) => (after[1..1 + close].to_owned(), after[close + 2..].trim_start()),
                None => (after[1..].to_owned(), ""),
            },
            _ => {
                let end = after.find(char::is_whitespace).unwrap_or(after.len());
                (after[..end].to_owned(), after[end..].trim_start())
            }
        };
        attributes.insert(
            key.rsplit(char::is_whitespace).next().unwrap_or(&key).to_owned(),
            decode_entities(&value),
        );
        remaining = next;
    }
    Tag {
        name: name.to_ascii_lowercase(),
        attributes,
    }
}

/// Splits HTML into text and tags. Comments are dropped; the content of `script`, `style` and `head` is skipped except `title`.
fn tokens(html: &str) -> Vec<(bool, String)> {
    let mut out = Vec::new();
    let mut rest = html;
    while !rest.is_empty() {
        match rest.find('<') {
            None => {
                out.push((false, rest.to_owned()));
                break;
            }
            Some(at) => {
                if at > 0 {
                    out.push((false, rest[..at].to_owned()));
                }
                rest = &rest[at..];
                if let Some(comment) = rest.strip_prefix("<!--") {
                    rest = comment.find("-->").map(|end| &comment[end + 3..]).unwrap_or("");
                    continue;
                }
                match rest.find('>') {
                    Some(close) => {
                        out.push((true, rest[1..close].to_owned()));
                        rest = &rest[close + 1..];
                    }
                    None => break,
                }
            }
        }
    }
    out
}

/// One page converted.
pub(crate) struct Converted {
    pub(crate) title: String,
    pub(crate) markdown: String,
    /// The macros that were not plain content, by name.
    pub(crate) macros: Vec<String>,
}

/// Converts a page's HTML to Markdown. `link_for` maps an `href` to the target to write, or none to keep the `href` as it is.
pub(crate) fn html_to_markdown(html: &str, link_for: &dyn Fn(&str) -> Option<String>) -> Converted {
    let mut out = String::new();
    let mut title = String::new();
    let mut macros = Vec::new();
    let mut skip = 0_usize;
    let mut in_title = false;
    let mut in_pre = false;
    let mut lists: Vec<(bool, usize)> = Vec::new();
    let mut link: Option<(String, usize)> = None;
    let mut first_heading = String::new();
    let mut in_first_heading = false;
    for (is_tag, token) in tokens(html) {
        if !is_tag {
            if in_title {
                title.push_str(&decode_entities(&token));
                continue;
            }
            if skip > 0 {
                continue;
            }
            let text = decode_entities(&token);
            if in_pre {
                out.push_str(&text);
            } else {
                let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
                if !collapsed.is_empty() {
                    if text.starts_with(char::is_whitespace) && !out.ends_with(['\n', ' ']) && !out.is_empty() {
                        out.push(' ');
                    }
                    out.push_str(&collapsed);
                    if text.ends_with(char::is_whitespace) {
                        out.push(' ');
                    }
                    if in_first_heading {
                        first_heading.push_str(&collapsed);
                    }
                }
            }
            continue;
        }
        let closing = token.starts_with('/');
        let tag = parse_tag(token.trim_start_matches('/'));
        let name = tag.name.as_str();
        // Macros that did not become plain content.
        if !closing && (name == "ac:structured-macro" || name == "ac:macro") {
            macros.push(
                tag.attributes
                    .get("ac:name")
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_owned()),
            );
        }
        if !closing {
            if let Some(class) = tag.attributes.get("class") {
                for word in class.split_whitespace() {
                    if let Some(found) = word.strip_prefix("conf-macro") {
                        if !found.is_empty() && !found.starts_with("-output") && !found.starts_with("output") {
                            macros.push(found.trim_start_matches('-').to_owned());
                        }
                    }
                }
            }
        }
        match (name, closing) {
            ("script" | "style" | "head", false) => skip += 1,
            ("script" | "style" | "head", true) => skip = skip.saturating_sub(1),
            ("title", false) => in_title = true,
            ("title", true) => in_title = false,
            _ if skip > 0 => {}
            ("h1" | "h2" | "h3" | "h4" | "h5" | "h6", false) => {
                let level = name[1..].parse::<usize>().unwrap_or(1);
                out.push_str(&format!("\n\n{} ", "#".repeat(level)));
                if first_heading.is_empty() {
                    in_first_heading = true;
                }
            }
            ("h1" | "h2" | "h3" | "h4" | "h5" | "h6", true) => {
                in_first_heading = false;
                out.push_str("\n\n");
            }
            ("p" | "div" | "blockquote", false) => out.push_str(if name == "blockquote" { "\n\n> " } else { "\n\n" }),
            ("p" | "div" | "blockquote", true) => out.push_str("\n\n"),
            ("br", _) => out.push('\n'),
            ("hr", _) => out.push_str("\n\n---\n\n"),
            ("ul", false) => lists.push((false, 0)),
            ("ol", false) => lists.push((true, 0)),
            ("ul" | "ol", true) => {
                lists.pop();
                out.push('\n');
            }
            ("li", false) => {
                let depth = lists.len().saturating_sub(1);
                let marker = match lists.last_mut() {
                    Some((true, count)) => {
                        *count += 1;
                        format!("{count}.")
                    }
                    _ => "-".to_owned(),
                };
                out.push_str(&format!("\n{}{marker} ", "  ".repeat(depth)));
            }
            ("strong" | "b", _) => out.push_str("**"),
            ("em" | "i", _) => out.push('*'),
            ("code", _) if !in_pre => out.push('`'),
            ("pre", false) => {
                in_pre = true;
                out.push_str("\n\n```\n");
            }
            ("pre", true) => {
                in_pre = false;
                out.push_str("\n```\n\n");
            }
            ("tr", false) => out.push_str("\n| "),
            ("th" | "td", true) => out.push_str(" | "),
            ("a", false) => {
                let href = tag.attributes.get("href").cloned().unwrap_or_default();
                link = Some((href, out.len()));
                out.push('[');
            }
            ("a", true) => {
                if let Some((href, start)) = link.take() {
                    let label = out[start + 1..].trim().to_owned();
                    if href.is_empty() || href.starts_with('#') && label.is_empty() {
                        out.truncate(start);
                        out.push_str(&label);
                    } else {
                        let target = link_for(&href).unwrap_or(href);
                        out.truncate(start);
                        out.push_str(&format!("[{label}](<{target}>)"));
                    }
                }
            }
            ("img", false) => {
                let src = tag.attributes.get("src").cloned().unwrap_or_default();
                let alt = tag.attributes.get("alt").cloned().unwrap_or_default();
                let target = link_for(&src).unwrap_or(src);
                out.push_str(&format!("![{alt}](<{target}>)"));
            }
            _ => {}
        }
    }
    let markdown = {
        let mut lines: Vec<String> = Vec::new();
        for line in out.lines() {
            let line = line.trim_end().to_owned();
            if line.is_empty() && lines.last().is_some_and(String::is_empty) {
                continue;
            }
            lines.push(line);
        }
        lines.join("\n").trim().to_owned()
    };
    // A title is read as "Space : Page"; the page is what follows the last separator.
    let title = decode_entities(title.trim());
    let title = title.rsplit(" : ").next().unwrap_or(&title).trim().to_owned();
    let title = if title.is_empty() { first_heading } else { title };
    Converted {
        title,
        markdown,
        macros,
    }
}

/// The pages of the index as a tree: for each page file, the titles of the pages above it.
pub(crate) fn page_tree(index_html: &str) -> BTreeMap<String, Vec<String>> {
    let mut tree = BTreeMap::new();
    let mut depth = 0_usize;
    let mut ancestors: Vec<String> = Vec::new();
    let mut current: Option<(String, String)> = None;
    for (is_tag, token) in tokens(index_html) {
        if !is_tag {
            if let Some((_, text)) = current.as_mut() {
                text.push_str(&decode_entities(&token));
            }
            continue;
        }
        let closing = token.starts_with('/');
        let tag = parse_tag(token.trim_start_matches('/'));
        match (tag.name.as_str(), closing) {
            ("ul", false) => depth += 1,
            ("ul", true) => {
                depth = depth.saturating_sub(1);
                ancestors.truncate(depth.saturating_sub(1));
            }
            ("a", false) => {
                if let Some(href) = tag.attributes.get("href") {
                    if href.ends_with(".html") && !href.contains("://") {
                        current = Some((href.clone(), String::new()));
                    }
                }
            }
            ("a", true) => {
                if let Some((href, text)) = current.take() {
                    let title = text.trim().to_owned();
                    let level = depth.saturating_sub(1);
                    ancestors.truncate(level);
                    tree.insert(href.clone(), ancestors.clone());
                    ancestors.push(title);
                }
            }
            _ => {}
        }
    }
    tree
}

fn safe_segment(text: &str) -> String {
    let cleaned: String = text
        .chars()
        .map(|c| {
            if c.is_control() || "/\\:*?\"<>|".contains(c) {
                '-'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_start_matches('.').trim();
    let cleaned: String = cleaned.chars().take(80).collect();
    if cleaned.is_empty() {
        "Untitled".to_owned()
    } else {
        cleaned
    }
}

/// The page id in `Title_12345.html`, when there is one.
fn page_id(file: &str) -> Option<String> {
    let stem = file.rsplit('/').next()?.strip_suffix(".html")?;
    let digits = stem.rsplit('_').next()?;
    (!digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())).then(|| digits.to_owned())
}

/// The path from a directory to a file, both inside the import.
fn relative_path(from_dir: &str, to: &str) -> String {
    let from: Vec<&str> = from_dir.split('/').filter(|part| !part.is_empty()).collect();
    let target: Vec<&str> = to.split('/').collect();
    let common = from.iter().zip(&target).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<String> = vec!["..".to_owned(); from.len() - common];
    parts.extend(target[common..].iter().map(|part| part.replace(' ', "%20")));
    parts.join("/")
}

/// What conversion made of an export.
pub(crate) struct Conversion {
    pub(crate) files: Vec<ImportFile>,
    pub(crate) notes: Vec<String>,
}

pub(crate) fn convert_export(files: Vec<ImportFile>, space_key: &str) -> Result<Conversion, ApiError> {
    if files.is_empty() || files.len() > MAX_FILES {
        return Err(ApiError::bad_request(format!(
            "Send between 1 and {MAX_FILES} files from the export."
        )));
    }
    let normalize = |path: &str| path.trim().trim_start_matches("./").replace('\\', "/");
    let index = files
        .iter()
        .find(|file| {
            normalize(&file.path).rsplit('/').next() == Some("index.html")
                && !normalize(&file.path).contains("attachments/")
        })
        .and_then(|file| file.text.clone());
    let Some(index) = index else {
        return Err(ApiError::bad_request(
            "The export has no index.html. Send the files of a space exported as HTML (Space settings, Export space, HTML).",
        ));
    };
    let tree = page_tree(&index);
    let mut notes = Vec::new();
    let mut ignored = 0;
    // First: every page's destination, so a link to another page can be written before it is converted.
    let mut destinations: HashMap<String, String> = HashMap::new();
    let mut pages: Vec<(String, ImportFile)> = Vec::new();
    let mut attachments: Vec<ImportFile> = Vec::new();
    for file in files {
        let path = normalize(&file.path);
        let name = path.rsplit('/').next().unwrap_or(&path).to_owned();
        if name == "index.html"
            || path.starts_with("styles/")
            || path.starts_with("scripts/")
            || path.starts_with("images/")
            || path.ends_with(".css")
            || path.ends_with(".js")
        {
            ignored += 1;
        } else if name.ends_with(".html") {
            pages.push((name, file));
        } else if path.starts_with("attachments/") || path.contains("/attachments/") {
            attachments.push(ImportFile {
                path: path.clone(),
                ..file
            });
        } else {
            ignored += 1;
        }
    }
    let mut titles: HashMap<String, String> = HashMap::new();
    for (name, file) in &pages {
        let html = file.text.clone().unwrap_or_default();
        let converted = html_to_markdown(&html, &|_| None);
        titles.insert(name.clone(), converted.title);
    }
    let mut used: HashMap<String, usize> = HashMap::new();
    for (name, _) in &pages {
        let title = titles.get(name).cloned().unwrap_or_default();
        let folders = tree
            .get(name)
            .map(|ancestors| ancestors.iter().map(|title| safe_segment(title)).collect::<Vec<_>>())
            .unwrap_or_default();
        let base = format!(
            "{}{}",
            folders.iter().map(|folder| format!("{folder}/")).collect::<String>(),
            safe_segment(&title)
        );
        // Two pages with one title under one parent keep their own file by their id.
        let count = used.entry(base.to_ascii_lowercase()).or_insert(0);
        *count += 1;
        let path = if *count > 1 {
            format!("{base} ({}).md", page_id(name).unwrap_or_else(|| count.to_string()))
        } else {
            format!("{base}.md")
        };
        destinations.insert(name.clone(), path);
    }
    let attachment_paths: std::collections::HashSet<String> =
        attachments.iter().map(|file| file.path.clone()).collect();
    let mut macros: BTreeMap<String, usize> = BTreeMap::new();
    let mut converted_files = Vec::new();
    for (name, file) in pages {
        let Some(destination) = destinations.get(&name).cloned() else {
            continue;
        };
        let from_dir = destination
            .rsplit_once('/')
            .map(|(dir, _)| dir.to_owned())
            .unwrap_or_default();
        let link_for = |href: &str| -> Option<String> {
            if href.contains("://") || href.starts_with("mailto:") || href.starts_with('#') {
                return None;
            }
            let (target, fragment) = match href.split_once('#') {
                Some((target, fragment)) => (target, format!("#{fragment}")),
                None => (href, String::new()),
            };
            let target = target.trim_start_matches("./");
            let file_name = target.rsplit('/').next().unwrap_or(target);
            if let Some(path) = destinations.get(file_name) {
                return Some(format!("{}{fragment}", relative_path(&from_dir, path)));
            }
            if attachment_paths.contains(target) {
                return Some(relative_path(&from_dir, target));
            }
            None
        };
        let converted = html_to_markdown(&file.text.unwrap_or_default(), &link_for);
        for found in &converted.macros {
            *macros.entry(found.clone()).or_insert(0) += 1;
        }
        let id = page_id(&name).map(|id| format!(", page {id}")).unwrap_or_default();
        let space = if space_key.trim().is_empty() {
            "a Confluence space".to_owned()
        } else {
            format!("Confluence space {}", space_key.trim())
        };
        let text = format!(
            "# {}\n\n> Imported from {space}{id}. The original is the source of record.\n\n{}\n",
            converted.title, converted.markdown
        );
        converted_files.push(ImportFile {
            path: destination,
            text: Some(text),
            base64: None,
        });
    }
    converted_files.extend(attachments);
    if !macros.is_empty() {
        notes.push(format!(
            "Macros that did not become plain content: {}.",
            macros
                .iter()
                .map(|(name, count)| format!("{name} ×{count}"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if ignored > 0 {
        notes.push(format!(
            "{ignored} style, script, icon or index files were not imported."
        ));
    }
    notes.push(
        "An HTML export carries no page restrictions: every page will be readable by everyone who can read this workspace. Confirm with acknowledge_restrictions to commit."
            .to_owned(),
    );
    Ok(Conversion {
        files: converted_files,
        notes,
    })
}

pub(crate) async fn import_confluence(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<ConfluenceImportRequest>,
) -> Result<(StatusCode, Json<DirectoryImportReport>), ApiError> {
    require_workspace_write(&state, &subject, &workspace_id).await?;
    let source_key = crate::directory_import::check_key(&request.source_key)?;
    let conversion = convert_export(request.files, &request.space_key)?;
    if !request.dry_run && !request.acknowledge_restrictions {
        return Err(ApiError::conflict(
            "A Confluence export carries no page restrictions, so every page would be readable by everyone who can read this workspace. Send acknowledge_restrictions: true to import it that way, or run a dry run first.",
        ));
    }
    let (status, Json(mut report)) = run_import(
        &subject,
        &state,
        &workspace_id,
        source_key,
        request.dry_run,
        request.skip_invalid,
        conversion.files,
    )
    .await?;
    report.notes = conversion.notes;
    Ok((status, Json(report)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_becomes_markdown_and_its_links_are_mapped() {
        let html = r#"<html><head><title>Docs : Release notes</title><style>p{}</style></head><body>
            <h1>Release notes</h1><p>See <a href="Setup_2.html#install">the setup</a> &amp; <strong>read</strong> <em>this</em>.</p>
            <ul><li>one</li><li>two</li></ul><ol><li>first</li></ol>
            <pre>let x = 1;</pre><img src="attachments/1/pic.png" alt="pic"/>
            <div class="conf-macro output-block" data-macro-name="jira">x</div><ac:structured-macro ac:name="toc"></ac:structured-macro></body></html>"#;
        let converted = html_to_markdown(html, &|href| {
            (href == "Setup_2.html#install").then(|| "Setup.md#install".to_owned())
        });
        assert_eq!(converted.title, "Release notes");
        for expected in [
            "# Release notes",
            "[the setup](<Setup.md#install>)",
            "& **read** *this*.",
            "- one",
            "- two",
            "1. first",
            "```\nlet x = 1;\n```",
            "![pic](<attachments/1/pic.png>)",
        ] {
            assert!(
                converted.markdown.contains(expected),
                "{expected}\n{}",
                converted.markdown
            );
        }
        assert!(!converted.markdown.contains("p{}"), "styles are dropped");
        assert!(converted.macros.contains(&"toc".to_owned()), "{:?}", converted.macros);
    }

    #[test]
    fn the_index_gives_each_page_its_parents() {
        let index = r#"<ul><li><a href="Home_1.html">Home</a><ul><li><a href="Guide_2.html">Guide</a><ul><li><a href="Deep_3.html">Deep</a></li></ul></li><li><a href="Other_4.html">Other</a></li></ul></li></ul>"#;
        let tree = page_tree(index);
        assert_eq!(tree["Home_1.html"], Vec::<String>::new());
        assert_eq!(tree["Guide_2.html"], vec!["Home".to_owned()]);
        assert_eq!(tree["Deep_3.html"], vec!["Home".to_owned(), "Guide".to_owned()]);
        assert_eq!(tree["Other_4.html"], vec!["Home".to_owned()]);
    }

    #[test]
    fn names_ids_and_relative_paths() {
        assert_eq!(safe_segment("A/B: C?"), "A-B- C-");
        assert_eq!(safe_segment("..."), "Untitled");
        assert_eq!(page_id("Release_notes_12345.html").as_deref(), Some("12345"));
        assert_eq!(page_id("index.html"), None);
        assert_eq!(relative_path("Home/Guide", "Home/Other.md"), "../Other.md");
        assert_eq!(relative_path("", "Home/My page.md"), "Home/My%20page.md");
        assert_eq!(relative_path("Home", "attachments/1/a.png"), "../attachments/1/a.png");
    }
}
