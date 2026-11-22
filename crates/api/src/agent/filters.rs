//! Which files a request means (V0.9.25). "The Word documents", "all pdfs in the Runbooks folder", "images named wordle" say a
//! kind of file, a folder and a name. A kind is matched by the file's extension and never by a piece of its name, so "word"
//! finds .docx files and not `wordle.png` or `passwordPolicy.ts`. The same filter serves finding files and, from V0.9.26, the
//! sets of files the other jobs work on.

use repomemo_domain::ArtifactSummary;

use super::words_of;

/// A kind of file: the words people use for it and the extensions it has.
pub(crate) struct FileKind {
    pub label: &'static str,
    pub plural: &'static str,
    words: &'static [&'static str],
    pub extensions: &'static [&'static str],
}

pub(crate) const KINDS: &[FileKind] = &[
    FileKind {
        label: "Word document",
        plural: "Word documents",
        words: &["word", "docx"],
        extensions: &[".doc", ".docx", ".dotx"],
    },
    FileKind {
        label: "spreadsheet",
        plural: "spreadsheets",
        words: &["excel", "spreadsheet", "xlsx", "xls", "csv"],
        extensions: &[".xls", ".xlsx", ".xlsm", ".csv"],
    },
    FileKind {
        label: "presentation",
        plural: "presentations",
        words: &["powerpoint", "presentation", "slide", "deck", "pptx", "ppt"],
        extensions: &[".ppt", ".pptx"],
    },
    FileKind {
        label: "PDF",
        plural: "PDFs",
        words: &["pdf"],
        extensions: &[".pdf"],
    },
    FileKind {
        label: "image",
        plural: "images",
        words: &[
            "image",
            "picture",
            "screenshot",
            "photo",
            "png",
            "jpg",
            "jpeg",
            "gif",
            "svg",
        ],
        extensions: &[".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg", ".bmp"],
    },
    FileKind {
        label: "Markdown file",
        plural: "Markdown files",
        words: &["markdown", "md"],
        extensions: &[".md", ".mdx"],
    },
    FileKind {
        label: "text file",
        plural: "text files",
        words: &["txt"],
        extensions: &[".txt"],
    },
    FileKind {
        label: "code file",
        plural: "code files",
        words: &["code", "source", "script"],
        extensions: &[
            ".ts", ".tsx", ".js", ".jsx", ".mjs", ".py", ".rs", ".go", ".java", ".cs", ".php", ".rb", ".c", ".cpp",
            ".h", ".sh", ".sql",
        ],
    },
    FileKind {
        label: "JSON file",
        plural: "JSON files",
        words: &["json"],
        extensions: &[".json"],
    },
    FileKind {
        label: "YAML file",
        plural: "YAML files",
        words: &["yaml", "yml"],
        extensions: &[".yaml", ".yml"],
    },
    FileKind {
        label: "email",
        plural: "emails",
        words: &["email", "mail", "outlook"],
        extensions: &[".msg", ".eml"],
    },
    FileKind {
        label: "OneNote file",
        plural: "OneNote files",
        words: &["onenote"],
        extensions: &[".one"],
    },
];

/// Words that say nothing about which files: they are dropped before a request is read.
const NOISE: &[&str] = &[
    "all",
    "only",
    "just",
    "every",
    "each",
    "the",
    "a",
    "an",
    "my",
    "our",
    "this",
    "that",
    "these",
    "those",
    "of",
    "in",
    "on",
    "from",
    "within",
    "at",
    "workspace",
    "repo",
    "repository",
    "project",
    "here",
    "there",
    "document",
    "documents",
    "doc",
    "docs",
    "file",
    "files",
    "type",
    "types",
    "kind",
    "kinds",
    "format",
    "formats",
    "extension",
    "extensions",
    "any",
    "some",
    "please",
    "me",
    "and",
    "or",
    "with",
    "about",
    "related",
    "inside",
    "under",
    "to",
];

/// Words that introduce a folder.
const FOLDER_WORDS: &[&str] = &["folder", "directory", "folders"];
const FOLDER_PREPOSITIONS: &[&str] = &["in", "from", "inside", "under", "within"];

/// The files a request means.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct FileFilter {
    /// Kinds of file (by index into [`KINDS`]); a file matches if it is any of them.
    pub kinds: Vec<usize>,
    /// Words the name or path must all contain.
    pub name: String,
    /// The folder the files are in, as the request names it.
    pub folder: Option<String>,
}

fn kind_of(word: &str) -> Option<usize> {
    let word = word.trim_start_matches(['.', '*']).to_lowercase();
    let singular = word.trim_end_matches('s');
    KINDS
        .iter()
        .position(|kind| kind.words.iter().any(|known| *known == word || *known == singular))
}

impl FileFilter {
    /// Whether the filter says anything about kind or folder, beyond a name.
    pub(crate) fn is_structured(&self) -> bool {
        !self.kinds.is_empty() || self.folder.is_some()
    }

    /// Whether a file's extension is one of the kinds, or there is no kind named.
    pub(crate) fn keeps_kind(&self, file: &ArtifactSummary) -> bool {
        if self.kinds.is_empty() {
            return true;
        }
        let path = file.path.to_lowercase();
        let title = file.title.to_lowercase();
        self.kinds.iter().any(|&index| {
            KINDS[index]
                .extensions
                .iter()
                .any(|ext| path.ends_with(ext) || title.ends_with(ext))
        })
    }

    /// Whether every word of the name is in the file's title or path.
    pub(crate) fn keeps_name(&self, file: &ArtifactSummary) -> bool {
        let title = file.title.to_lowercase();
        let path = file.path.to_lowercase();
        self.name
            .to_lowercase()
            .split_whitespace()
            .all(|term| title.contains(term) || path.contains(term))
    }

    /// The filter in words, as a heading for what was found ("Word documents in the folder Runbooks named like “login”").
    pub(crate) fn describe(&self) -> String {
        let mut text = match self.kinds.as_slice() {
            [] => "files".to_owned(),
            kinds => kinds
                .iter()
                .map(|&index| KINDS[index].plural)
                .collect::<Vec<_>>()
                .join(" and "),
        };
        if let Some(folder) = &self.folder {
            text.push_str(&format!(" in the folder {folder}"));
        }
        if !self.name.is_empty() {
            text.push_str(&format!(" named like “{}”", self.name));
        }
        text
    }
}

/// The words that say something: those that are not noise.
fn significant(words: &[&str]) -> Vec<String> {
    words
        .iter()
        .filter(|token| !NOISE.contains(&token.to_lowercase().as_str()))
        .map(|token| (*token).to_owned())
        .collect()
}

/// Reads a request's subject as a filter. When the subject says the files are *named*, *called* or *titled* something, or quotes a
/// name, every word is part of the name and no word is taken for a kind.
pub(crate) fn parse_filter(subject: &str) -> FileFilter {
    let lower = subject.to_lowercase();
    let says_named = [" named ", " called ", " titled "]
        .iter()
        .any(|word| lower.contains(word))
        || lower.starts_with("named ")
        || subject.contains(['"', '“', '`']);
    if says_named {
        let name = subject
            .rsplit_once(" named ")
            .or_else(|| subject.rsplit_once(" called "))
            .or_else(|| subject.rsplit_once(" titled "))
            .map_or(subject, |(_, name)| name);
        return FileFilter {
            kinds: Vec::new(),
            name: name
                .trim_matches(|c: char| c.is_whitespace() || matches!(c, '"' | '“' | '”' | '`' | '\'' | '.'))
                .to_owned(),
            folder: None,
        };
    }
    let tokens = subject
        .split(|c: char| c.is_whitespace() || c == ',')
        .map(|token| token.trim_matches(|c: char| matches!(c, '?' | '!' | ':' | ';')))
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    // "in the Runbooks folder", "from folder Runbooks": the folder and the words that name it.
    let mut folder = None;
    let mut used = vec![false; tokens.len()];
    if let Some(at) = tokens
        .iter()
        .position(|token| FOLDER_WORDS.contains(&token.to_lowercase().as_str()))
    {
        let before = tokens[..at]
            .iter()
            .rposition(|token| FOLDER_PREPOSITIONS.contains(&token.to_lowercase().as_str()));
        // "in the Runbooks folder" names it before the word; "in folder Runbooks" and "folder Runbooks" after it.
        let between = before.map_or_else(Vec::new, |prep| significant(&tokens[prep + 1..at]));
        let (start, end, named) = if between.is_empty() {
            (before.unwrap_or(at), tokens.len() - 1, significant(&tokens[at + 1..]))
        } else {
            (before.unwrap_or(at), at, between)
        };
        if !named.is_empty() {
            folder = Some(named.join(" "));
            for flag in &mut used[start..=end] {
                *flag = true;
            }
        }
    }
    let mut kinds = Vec::new();
    let mut name = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if used[index] || NOISE.contains(&token.to_lowercase().as_str()) {
            continue;
        }
        match kind_of(token) {
            Some(kind) => {
                if !kinds.contains(&kind) {
                    kinds.push(kind);
                }
            }
            None => name.push(*token),
        }
    }
    // Words of a request that only ask for files ("find", "list") are not part of a name.
    let name = words_of(&name.join(" ")).join(" ");
    FileFilter { kinds, name, folder }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str) -> ArtifactSummary {
        ArtifactSummary {
            id: format!("id-{path}"),
            workspace_id: "w".to_owned(),
            source_id: "s".to_owned(),
            source_name: "src".to_owned(),
            artifact_type: repomemo_domain::ArtifactType::File,
            title: path.rsplit('/').next().unwrap().to_owned(),
            path: path.to_owned(),
            content_hash: String::new(),
            mime_type: None,
            language: None,
            size_bytes: 1,
            created_at: String::new(),
            updated_at: String::new(),
            indexed_at: None,
        }
    }

    fn labels(filter: &FileFilter) -> Vec<&'static str> {
        filter.kinds.iter().map(|&index| KINDS[index].label).collect()
    }

    #[test]
    fn a_kind_word_is_a_kind_and_never_part_of_a_name() {
        for subject in [
            "Word",
            "the word documents",
            "all word docs in the workspace",
            "only the Word documents",
            "docx files",
        ] {
            let filter = parse_filter(subject);
            assert_eq!(labels(&filter), ["Word document"], "{subject}");
            assert_eq!(filter.name, "", "{subject}");
            assert_eq!(filter.folder, None, "{subject}");
        }
        assert_eq!(labels(&parse_filter("pdfs and spreadsheets")), ["PDF", "spreadsheet"]);
        assert_eq!(labels(&parse_filter(".pdf")), ["PDF"]);
    }

    /// The bug that listed wordle.png and passwordPolicy.ts for "the word documents".
    #[test]
    fn a_kind_keeps_only_files_of_that_extension() {
        let filter = parse_filter("the word documents");
        for path in [
            "wordle (2).png",
            "src/lib/passwordPolicy.ts",
            "src/components/PasswordInputField.tsx",
            "notes.md",
        ] {
            assert!(!filter.keeps_kind(&file(path)), "{path}");
        }
        for path in ["CV Amine Tabia FR.docx", "Etude de Cas HANTICK.DOCX", "old/report.doc"] {
            assert!(filter.keeps_kind(&file(path)), "{path}");
        }
        assert!(
            parse_filter("login").keeps_kind(&file("anything.bin")),
            "no kind named keeps every file"
        );
    }

    #[test]
    fn a_name_and_a_folder_are_read_beside_the_kind() {
        let filter = parse_filter("pdfs about invoices in the Finance folder");
        assert_eq!(labels(&filter), ["PDF"]);
        assert_eq!(filter.name, "invoices");
        assert_eq!(filter.folder.as_deref(), Some("Finance"));
        assert_eq!(
            parse_filter("files in folder Runbooks").folder.as_deref(),
            Some("Runbooks")
        );
        assert_eq!(
            parse_filter("images from the Design / Assets folder").folder.as_deref(),
            Some("Design / Assets")
        );
        let plain = parse_filter("login");
        assert!(!plain.is_structured() && plain.name == "login");
    }

    #[test]
    fn a_named_file_is_a_name_even_when_it_contains_a_kind_word() {
        let filter = parse_filter("the file named Word counter");
        assert!(filter.kinds.is_empty());
        assert_eq!(filter.name, "Word counter");
        let quoted = parse_filter("\"excel tips\"");
        assert!(quoted.kinds.is_empty() && quoted.name == "excel tips");
    }

    #[test]
    fn the_filter_describes_itself_for_the_reply() {
        assert_eq!(parse_filter("word documents").describe(), "Word documents");
        assert_eq!(
            parse_filter("pdfs about invoices in the Finance folder").describe(),
            "PDFs in the folder Finance named like “invoices”"
        );
        assert_eq!(parse_filter("login").describe(), "files named like “login”");
        assert_eq!(parse_filter("pdfs and images").describe(), "PDFs and images");
    }
}
