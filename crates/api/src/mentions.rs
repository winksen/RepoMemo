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

use std::collections::{BTreeMap, HashMap};

use anyhow::Result;
use repomemo_domain::{ArtifactMention, ArtifactMentions};
use repomemo_storage::{MentionWrite, WorkspaceFileIndex};

use crate::RepoMemoCore;

/// Most mentions kept for one item.
const MAX_MENTIONS: usize = 50;
/// Items looked at when a repository changes.
const MAX_ITEMS_PER_REFRESH: i64 = 5_000;
/// A symbol defined in more places than this is too common to link.
const MAX_SYMBOL_DEFINITIONS: usize = 3;

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
        let mut stored = 0;
        for (artifact_id, metadata) in items {
            let mentions = if index.is_empty() {
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
                find_mentions(&index, &text, &commits)
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
                ("a1".into(), "src/ledger.rs".into(), Some("0123456789abcdef0123456789abcdef01234567".into())),
                ("a2".into(), "src/web/mod.rs".into(), None),
                ("a3".into(), "src/db/mod.rs".into(), None),
                ("a4".into(), "docs/guide.md".into(), Some("89abcdef0123456789abcdef0123456789abcdef".into())),
            ],
            symbols: HashMap::from([
                ("post_refund".to_owned(), vec!["a1".to_owned()]),
                ("common".to_owned(), vec!["a1".into(), "a2".into(), "a3".into(), "a4".into()]),
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
        assert_eq!(labels(&found), vec![("symbol".into(), "post_refund".into(), "a1".into())]);
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
                ("commit".into(), "0123456789abcdef0123456789abcdef01234567".into(), "a1".into()),
                ("commit".into(), "89abcdef0123456789abcdef0123456789abcdef".into(), "a4".into()),
            ]
        );
    }

    #[test]
    fn an_empty_index_and_plain_prose_find_nothing() {
        assert!(find_mentions(&MentionIndex::default(), "src/ledger.rs `x`", &[]).is_empty());
        assert!(find_mentions(&index(), "Nothing to see here, version 1.2.3 and 2026-01-02.", &[]).is_empty());
    }
}
