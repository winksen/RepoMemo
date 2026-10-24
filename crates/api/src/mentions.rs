//! What issues and pull requests mention (roadmap F17d, V0.2.30).
//!
//! The text of a tracker item names files (`src/ledger.rs`, or `ledger.rs`
//! when only one file has that name), symbols (in backticks, such as
//! `post_refund()` or `Ledger::post_refund`) and commits (a hash of seven
//! characters or more, plus the commits of a pull request itself). Each is
//! resolved to the file artifact it concerns, so a file's page can list the
//! items that name it, and an item's page the files it names. Nothing is
//! guessed beyond that: a name that matches several files, or a symbol with
//! more than three definitions, is left alone.
//!
//! Since V0.3.21 an item also names other items: `#12` is the issue or pull
//! request of that number in the item's own repository, `owner/repo#12` one
//! of another connected repository, `!12` a GitLab merge request, and the
//! address of an item (as the tracker shows it) is the item. These are kept as
//! mentions of kind `item`, so an issue lists the pull requests that name it.

use std::collections::{BTreeMap, HashMap};

use anyhow::Result;
use repomemo_domain::{ArtifactMention, ArtifactMentions};
use repomemo_storage::{MentionWrite, WorkspaceFileIndex};
use serde_json::Value;

use crate::RepoMemoCore;

/// Most mentions kept for one item.
const MAX_MENTIONS: usize = 50;
/// Items looked at when a repository changes.
const MAX_ITEMS_PER_REFRESH: i64 = 5_000;
/// A symbol defined in more places than this is too common to link.
const MAX_SYMBOL_DEFINITIONS: usize = 3;
/// The longest address or repository name a reference is read from: a text made of one endless "word" costs
/// no more than a text of short ones (V0.3.26).
const MAX_REFERENCE_CHARS: usize = 300;

/// Files, symbols and commits of a workspace, ready to look names up in.
#[derive(Debug, Default)]
pub(crate) struct MentionIndex {
    /// Every path, and every suffix of it that starts after a `/`, to the
    /// files (artifact id, path) it could mean.
    by_suffix: HashMap<String, Vec<(String, String)>>,
    symbols: HashMap<String, Vec<String>>,
    /// The commit that last changed a file, to the files.
    commits: BTreeMap<String, Vec<String>>,
}

/// The issues and pull requests of a workspace, ready to look references up in.
#[derive(Debug, Default)]
pub(crate) struct ItemIndex {
    /// (provider, repository in lower case, number) to the items with that number: an issue and a GitLab merge
    /// request can share one.
    by_number: HashMap<(String, String, i64), Vec<(String, bool)>>,
    /// The address the tracker shows an item at, without a trailing `/`, to the item.
    by_url: HashMap<String, String>,
    count: usize,
}

/// What an item's metadata says about where it lives.
struct ItemPlace {
    provider: String,
    repository: String,
}

fn item_place(metadata: &Value) -> ItemPlace {
    let text = |key: &str| {
        metadata
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_lowercase()
    };
    ItemPlace {
        provider: text("provider"),
        repository: text("repository"),
    }
}

impl ItemIndex {
    pub(crate) fn new(items: &[(String, Value)]) -> Self {
        let mut index = Self::default();
        for (artifact_id, metadata) in items {
            let place = item_place(metadata);
            let origin = metadata.get("origin").and_then(Value::as_str).unwrap_or_default();
            let is_pull = origin.ends_with("pull_request") || origin.ends_with("merge_request");
            if let Some(number) = metadata.get("number").and_then(Value::as_i64) {
                if !place.provider.is_empty() && !place.repository.is_empty() {
                    index
                        .by_number
                        .entry((place.provider, place.repository, number))
                        .or_default()
                        .push((artifact_id.clone(), is_pull));
                }
            }
            let url = metadata.get("url").and_then(Value::as_str).unwrap_or_default();
            if !url.is_empty() {
                index
                    .by_url
                    .entry(url.trim_end_matches('/').to_lowercase())
                    .or_insert_with(|| artifact_id.clone());
            }
            index.count += 1;
        }
        index
    }

    /// With fewer than two items nothing can refer to another.
    fn is_trivial(&self) -> bool {
        self.count < 2
    }

    /// The one item a reference means, if the tracker numbers make it certain. `#` is an issue or, on GitHub, a
    /// pull request too; `!` is a GitLab merge request.
    fn resolve(&self, providers: &[&str], repository: &str, sign: char, number: i64) -> Option<&str> {
        let mut found = Vec::new();
        for provider in providers {
            let key = ((*provider).to_owned(), repository.to_lowercase(), number);
            for (artifact_id, is_pull) in self.by_number.get(&key).map(Vec::as_slice).unwrap_or_default() {
                let fits = match sign {
                    '!' => *provider == "gitlab" && *is_pull,
                    _ => *provider != "gitlab" || !*is_pull,
                };
                if fits {
                    found.push(artifact_id.as_str());
                }
            }
        }
        match found.as_slice() {
            [only] => Some(*only),
            _ => None,
        }
    }
}

/// The other items a text refers to (V0.3.21), as mentions of kind `item`. `from` is the item the text belongs to,
/// never a reference to itself; its provider and repository say what a bare `#12` or `!12` means.
pub(crate) fn find_references(items: &ItemIndex, from: &str, place_of_from: &Value, text: &str) -> Vec<MentionWrite> {
    let place = item_place(place_of_from);
    let mut found: Vec<MentionWrite> = Vec::new();
    let mut push = |artifact_id: &str, label: String| {
        if artifact_id != from && found.len() < MAX_MENTIONS {
            let mention = MentionWrite {
                to_artifact_id: artifact_id.to_owned(),
                kind: "item".to_owned(),
                label,
            };
            if !found.contains(&mention) {
                found.push(mention);
            }
        }
    };

    for (position, sign) in text.char_indices().filter(|(_, c)| matches!(*c, '#' | '!')) {
        let digits: String = text[position + 1..].chars().take_while(char::is_ascii_digit).collect();
        if digits.is_empty() || digits.len() > 9 {
            continue;
        }
        // `#12abc` and `#123_` are not references.
        if text[position + 1 + digits.len()..]
            .chars()
            .next()
            .is_some_and(|c| c.is_alphanumeric() || c == '_')
        {
            continue;
        }
        let Ok(number) = digits.parse::<i64>() else { continue };
        let before = &text[..position];
        let token_start = before
            .char_indices()
            .rev()
            .take_while(|(_, c)| c.is_ascii_alphanumeric() || matches!(*c, '_' | '.' | '/' | '-'))
            .last()
            .map_or(position, |(start, _)| start);
        let token = before[token_start..].trim_matches(|c| matches!(c, '.' | '-' | '/'));
        if (token_start < position && token.is_empty()) || token.len() > MAX_REFERENCE_CHARS {
            continue;
        }
        if token.is_empty() {
            // A bare reference: this item's own repository.
            if place.repository.is_empty() {
                continue;
            }
            if let Some(target) = items.resolve(&[place.provider.as_str()], &place.repository, sign, number) {
                push(target, format!("{sign}{number}"));
            }
        } else if token.contains('/') {
            // `owner/repo#12`, also written after a host (`github.com/owner/repo#12`).
            let mut candidates = vec![token];
            if let Some((first, rest)) = token.split_once('/') {
                if first.contains('.') {
                    candidates.push(rest);
                }
            }
            for repository in candidates {
                if let Some(target) = items.resolve(&["github", "gitlab"], repository, sign, number) {
                    push(target, format!("{repository}{sign}{number}"));
                    break;
                }
            }
        }
    }

    // The address of an item, as the tracker shows it. Each address is read once and the search goes on after
    // it, and one is read to 300 characters at most, so a text of "http://" over and over is read in a single
    // pass and not once for every one of them (V0.3.26).
    let mut from = 0;
    while let Some(offset) = text[from..].find("http") {
        let start = from + offset;
        let rest = &text[start..];
        if !(rest.starts_with("http://") || rest.starts_with("https://")) {
            from = start + 4;
            continue;
        }
        let end = rest
            .char_indices()
            .find(|(at, c)| {
                *at >= MAX_REFERENCE_CHARS
                    || c.is_whitespace()
                    || matches!(*c, ')' | '>' | ']' | '"' | '\'' | '<' | '`')
            })
            .map_or(rest.len(), |(at, _)| at);
        from = start + end.max(4);
        let address = rest[..end].trim_end_matches(['.', ',', ';', ':', '!', '?']);
        let address = address.split(['#', '?']).next().unwrap_or(address);
        if let Some(target) = items.by_url.get(&address.trim_end_matches('/').to_lowercase()) {
            push(target, address.to_owned());
        }
    }
    found
}

impl MentionIndex {
    pub(crate) fn new(index: WorkspaceFileIndex) -> Self {
        let mut by_suffix: HashMap<String, Vec<(String, String)>> = HashMap::new();
        let mut commits: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (artifact_id, path, commit) in index.files {
            let mut rest = path.as_str();
            loop {
                by_suffix
                    .entry(rest.to_owned())
                    .or_default()
                    .push((artifact_id.clone(), path.clone()));
                match rest.split_once('/') {
                    Some((_, tail)) if !tail.is_empty() => rest = tail,
                    _ => break,
                }
            }
            if let Some(commit) = commit.filter(|commit| !commit.is_empty()) {
                commits.entry(commit).or_default().push(artifact_id);
            }
        }
        Self {
            by_suffix,
            symbols: index.symbols,
            commits,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.by_suffix.is_empty()
    }
}

fn is_commit_word(word: &str) -> bool {
    (7..=40).contains(&word.len())
        && word.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        && word.chars().any(|c| c.is_ascii_digit())
}

/// The files, symbols and commits a text names. `own_commits` are full commit
/// ids the item itself carries (a pull request's).
pub(crate) fn find_mentions(index: &MentionIndex, text: &str, own_commits: &[String]) -> Vec<MentionWrite> {
    let mut found: Vec<MentionWrite> = Vec::new();
    let push = |found: &mut Vec<MentionWrite>, mention: MentionWrite| {
        if found.len() < MAX_MENTIONS && !found.contains(&mention) {
            found.push(mention);
        }
    };

    for word in text.split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '/' | '-'))) {
        // An item that names fifty things names no more: a text of a million words, or one commit that
        // touched a hundred thousand files, is not walked to the end.
        if found.len() >= MAX_MENTIONS {
            break;
        }
        let word = word.strip_prefix("./").unwrap_or(word);
        let word = word.trim_matches(|c| c == '.' || c == '-').trim_start_matches('/');
        if word.len() < 3 {
            continue;
        }
        if word.contains('.') || word.contains('/') {
            // A path, or a file name; only an unambiguous one counts.
            if let Some(candidates) = index.by_suffix.get(word) {
                if let [(artifact, path)] = candidates.as_slice() {
                    push(
                        &mut found,
                        MentionWrite {
                            to_artifact_id: artifact.clone(),
                            kind: "file".to_owned(),
                            label: path.clone(),
                        },
                    );
                }
            }
        }
        if is_commit_word(word) {
            for (commit, artifacts) in index.commits.range(word.to_owned()..) {
                if !commit.starts_with(word) {
                    break;
                }
                for artifact in artifacts {
                    if found.len() >= MAX_MENTIONS {
                        break;
                    }
                    push(
                        &mut found,
                        MentionWrite {
                            to_artifact_id: artifact.clone(),
                            kind: "commit".to_owned(),
                            label: commit.clone(),
                        },
                    );
                }
            }
        }
    }

    // Symbols are only taken from backticks, where people mean code.
    for (position, span) in text.split('`').enumerate() {
        if position % 2 == 0 {
            continue;
        }
        let name = span.trim().trim_end_matches("()");
        let name = name.rsplit("::").next().unwrap_or(name);
        let name = name.rsplit('.').next().unwrap_or(name);
        if let Some(artifacts) = index.symbols.get(name) {
            if artifacts.len() <= MAX_SYMBOL_DEFINITIONS {
                for artifact in artifacts {
                    push(
                        &mut found,
                        MentionWrite {
                            to_artifact_id: artifact.clone(),
                            kind: "symbol".to_owned(),
                            label: name.to_owned(),
                        },
                    );
                }
            }
        }
    }

    for commit in own_commits {
        if let Some(artifacts) = index.commits.get(commit) {
            for artifact in artifacts {
                push(
                    &mut found,
                    MentionWrite {
                        to_artifact_id: artifact.clone(),
                        kind: "commit".to_owned(),
                        label: commit.clone(),
                    },
                );
            }
        }
    }
    found
}

impl RepoMemoCore {
    /// Recomputes what every tracker item of a workspace mentions. Cheap when
    /// the workspace has none.
    pub async fn refresh_workspace_mentions(&self, workspace_id: &str) -> Result<usize> {
        let items = self
            .storage
            .list_connector_item_metadata(workspace_id, MAX_ITEMS_PER_REFRESH)
            .await?;
        if items.is_empty() {
            return Ok(0);
        }
        let index = MentionIndex::new(self.storage.workspace_file_index(workspace_id).await?);
        let item_index = ItemIndex::new(&items);
        let mut stored = 0;
        for (artifact_id, metadata) in items {
            let mentions = if index.is_empty() && item_index.is_trivial() {
                Vec::new()
            } else {
                let bytes = self.storage.read_artifact_blob(&artifact_id).await?;
                let text = String::from_utf8_lossy(&bytes);
                let commits = metadata
                    .get("commits")
                    .and_then(|value| value.as_array())
                    .map(|commits| {
                        commits
                            .iter()
                            .filter_map(|commit| commit.as_str().map(str::to_owned))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let mut mentions = if index.is_empty() {
                    Vec::new()
                } else {
                    find_mentions(&index, &text, &commits)
                };
                if !item_index.is_trivial() {
                    mentions.extend(find_references(&item_index, &artifact_id, &metadata, &text));
                }
                mentions
            };
            stored += self
                .storage
                .replace_mentions(workspace_id, &artifact_id, &mentions)
                .await?;
        }
        Ok(stored)
    }

    /// The items that name an artifact, and what the artifact names.
    pub async fn artifact_mentions(&self, artifact_id: &str) -> Result<ArtifactMentions> {
        let convert = |rows: Vec<repomemo_storage::MentionRow>| {
            rows.into_iter()
                .map(|row| ArtifactMention {
                    artifact_id: row.artifact_id,
                    title: row.title,
                    artifact_type: row.artifact_type,
                    path: row.path,
                    kind: row.kind,
                    label: row.label,
                })
                .collect::<Vec<_>>()
        };
        Ok(ArtifactMentions {
            mentioned_in: convert(self.storage.artifact_mentions(artifact_id, true).await?),
            mentions: convert(self.storage.artifact_mentions(artifact_id, false).await?),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index() -> MentionIndex {
        MentionIndex::new(WorkspaceFileIndex {
            files: vec![
                (
                    "a1".into(),
                    "src/ledger.rs".into(),
                    Some("0123456789abcdef0123456789abcdef01234567".into()),
                ),
                ("a2".into(), "src/web/mod.rs".into(), None),
                ("a3".into(), "src/db/mod.rs".into(), None),
                (
                    "a4".into(),
                    "docs/guide.md".into(),
                    Some("89abcdef0123456789abcdef0123456789abcdef".into()),
                ),
            ],
            symbols: HashMap::from([
                ("post_refund".to_owned(), vec!["a1".to_owned()]),
                (
                    "common".to_owned(),
                    vec!["a1".into(), "a2".into(), "a3".into(), "a4".into()],
                ),
            ]),
        })
    }

    fn labels(mentions: &[MentionWrite]) -> Vec<(String, String, String)> {
        mentions
            .iter()
            .map(|m| (m.kind.clone(), m.label.clone(), m.to_artifact_id.clone()))
            .collect()
    }

    #[test]
    fn files_are_named_by_path_or_by_a_name_only_one_file_has() {
        let found = find_mentions(
            &index(),
            "The bug is in src/ledger.rs and also guide.md. See mod.rs, and ./src/web/mod.rs (not web/x.rs).",
            &[],
        );
        let files = labels(&found);
        assert!(files.contains(&("file".into(), "src/ledger.rs".into(), "a1".into())));
        assert!(files.contains(&("file".into(), "docs/guide.md".into(), "a4".into())));
        assert!(files.contains(&("file".into(), "src/web/mod.rs".into(), "a2".into())));
        // `mod.rs` alone is two files: not guessed.
        assert!(!files.iter().any(|(_, label, _)| label == "src/db/mod.rs"));
        assert_eq!(found.len(), 3);
    }

    #[test]
    fn symbols_come_from_backticks_and_rare_ones_only() {
        let found = find_mentions(
            &index(),
            "`Ledger::post_refund()` posts twice, post_refund without ticks is ignored. `common` is everywhere.",
            &[],
        );
        assert_eq!(
            labels(&found),
            vec![("symbol".into(), "post_refund".into(), "a1".into())]
        );
    }

    #[test]
    fn commits_resolve_to_the_files_they_last_changed() {
        let found = find_mentions(
            &index(),
            "Fixed in 0123456 (and deadbeef is nothing, added is a word). Merged 89abcdef0123456789abcdef0123456789abcdef.",
            &["0123456789abcdef0123456789abcdef01234567".to_owned()],
        );
        let commits = labels(&found);
        assert_eq!(
            commits,
            vec![
                (
                    "commit".into(),
                    "0123456789abcdef0123456789abcdef01234567".into(),
                    "a1".into()
                ),
                (
                    "commit".into(),
                    "89abcdef0123456789abcdef0123456789abcdef".into(),
                    "a4".into()
                ),
            ]
        );
    }

    fn item(id: &str, provider: &str, repository: &str, number: i64, pull: bool, url: &str) -> (String, Value) {
        let origin = match (provider, pull) {
            ("gitlab", true) => "gitlab_merge_request",
            ("gitlab", false) => "gitlab_issue",
            (_, true) => "github_pull_request",
            (_, false) => "github_issue",
        };
        (
            id.to_owned(),
            serde_json::json!({"provider": provider, "repository": repository, "number": number, "origin": origin, "url": url}),
        )
    }

    fn items() -> (Vec<(String, Value)>, ItemIndex) {
        let list = vec![
            item(
                "i12",
                "github",
                "acme/app",
                12,
                false,
                "https://github.com/acme/app/issues/12",
            ),
            item(
                "p13",
                "github",
                "acme/app",
                13,
                true,
                "https://github.com/acme/app/pull/13",
            ),
            item(
                "o4",
                "github",
                "acme/other",
                4,
                false,
                "https://github.com/acme/other/issues/4",
            ),
            item(
                "g5",
                "gitlab",
                "grp/proj",
                5,
                false,
                "https://gitlab.example/grp/proj/-/issues/5",
            ),
            item(
                "m5",
                "gitlab",
                "grp/proj",
                5,
                true,
                "https://gitlab.example/grp/proj/-/merge_requests/5",
            ),
        ];
        let index = ItemIndex::new(&list);
        (list, index)
    }

    fn refs(list: &[(String, Value)], index: &ItemIndex, from: &str, text: &str) -> Vec<(String, String)> {
        let metadata = &list.iter().find(|(id, _)| id == from).unwrap().1;
        find_references(index, from, metadata, text)
            .into_iter()
            .map(|m| (m.to_artifact_id, m.label))
            .collect()
    }

    #[test]
    fn a_bare_number_is_an_item_of_the_same_repository_and_never_the_item_itself() {
        let (list, index) = items();
        // On GitHub an issue and a pull request share the number space.
        assert_eq!(
            refs(
                &list,
                &index,
                "p13",
                "Pull request #13 in acme/app. Fixes #12, see also (#12) and #99."
            ),
            vec![("i12".to_owned(), "#12".to_owned())]
        );
        assert_eq!(
            refs(&list, &index, "i12", "Fixed by #13."),
            vec![("p13".to_owned(), "#13".to_owned())]
        );
        // The number of an item of another repository is not guessed.
        assert!(refs(&list, &index, "p13", "Compare #4.").is_empty());
    }

    #[test]
    fn another_repository_is_named_in_front_of_the_number() {
        let (list, index) = items();
        // After a host the same repository is meant, and it is one mention, not two; a name without an owner is not guessed.
        assert_eq!(
            refs(
                &list,
                &index,
                "p13",
                "Blocked by acme/other#4 and github.com/acme/other#4, not other#4."
            ),
            vec![("o4".to_owned(), "acme/other#4".to_owned())]
        );
    }

    #[test]
    fn on_gitlab_a_hash_is_an_issue_and_a_bang_is_a_merge_request() {
        let (list, index) = items();
        assert_eq!(
            refs(&list, &index, "m5", "Closes #5 and !5."),
            vec![("g5".to_owned(), "#5".to_owned())]
        );
        assert_eq!(
            refs(&list, &index, "g5", "Fixed in !5."),
            vec![("m5".to_owned(), "!5".to_owned())]
        );
        // A bang means nothing on GitHub.
        assert!(refs(&list, &index, "p13", "Wow!12").is_empty());
        assert!(refs(&list, &index, "i12", "!13").is_empty());
    }

    #[test]
    fn the_address_of_an_item_is_a_reference_and_prose_is_not() {
        let (list, index) = items();
        assert_eq!(
            refs(
                &list,
                &index,
                "p13",
                "See https://github.com/acme/app/issues/12#issuecomment-1, and (https://github.com/acme/other/issues/4)."
            ),
            vec![
                ("i12".to_owned(), "https://github.com/acme/app/issues/12".to_owned()),
                ("o4".to_owned(), "https://github.com/acme/other/issues/4".to_owned()),
            ]
        );
        assert!(refs(
            &list,
            &index,
            "p13",
            "# 12 steps\nColour #123456 and #12abc, page#12, version 1.2#3, #, ## and !!"
        )
        .is_empty());
        assert!(find_references(&ItemIndex::default(), "x", &Value::Null, "#1 acme/app#2").is_empty());
    }

    #[test]
    fn an_empty_index_and_plain_prose_find_nothing() {
        assert!(find_mentions(&MentionIndex::default(), "src/ledger.rs `x`", &[]).is_empty());
        assert!(find_mentions(&index(), "Nothing to see here, version 1.2.3 and 2026-01-02.", &[]).is_empty());
    }

    #[test]
    fn a_text_of_endless_addresses_or_repeated_words_is_read_in_one_pass_and_stops_when_the_item_is_full() {
        // Text from a tracker is written by whoever can comment there (V0.3.26).
        let (list, index) = items();
        let started = std::time::Instant::now();
        let endless = "http://".repeat(300_000);
        assert!(refs(&list, &index, "p13", &endless).is_empty());
        let long_name = format!("{}#12 #12", "a/".repeat(500_000));
        assert_eq!(
            refs(&list, &index, "p13", &long_name),
            vec![("i12".to_owned(), "#12".to_owned())],
            "a name too long to be a repository is skipped, a bare reference after it is not"
        );
        let hashes = "#1".repeat(500_000);
        assert!(refs(&list, &index, "p13", &hashes).is_empty());
        // An address is read to 300 characters, so one longer than that names nothing, even if it starts like an item.
        let cut = format!("https://github.com/acme/app/issues/12{}", "x".repeat(400));
        assert!(refs(&list, &index, "p13", &cut).is_empty());
        assert!(
            started.elapsed() < std::time::Duration::from_secs(20),
            "{:?}",
            started.elapsed()
        );

        // One commit that last changed a hundred thousand files, named a hundred thousand times, fills the item
        // with fifty mentions and the rest of the text is not walked.
        let files: Vec<(String, String, Option<String>)> = (0..100_000)
            .map(|number| {
                (
                    format!("a{number}"),
                    format!("src/file{number}.rs"),
                    Some("0123456789abcdef0123456789abcdef01234567".to_owned()),
                )
            })
            .collect();
        let index = MentionIndex::new(WorkspaceFileIndex {
            files,
            symbols: HashMap::new(),
        });
        let started = std::time::Instant::now();
        let found = find_mentions(&index, &"0123456 ".repeat(100_000), &[]);
        assert_eq!(found.len(), MAX_MENTIONS);
        assert!(
            started.elapsed() < std::time::Duration::from_secs(20),
            "{:?}",
            started.elapsed()
        );
    }
}
