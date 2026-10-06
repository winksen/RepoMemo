//! Workspace Health: deterministic checks over what is already stored —
//! file versions, content hashes, indexed symbols, lifecycle states, index
//! failures and embeddings. Nothing here calls an AI provider, and every
//! finding carries the facts it was raised from.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use anyhow::Result;
use repomemo_domain::{
    ArtifactSummary, ArtifactType, HealthAction, HealthDetector, HealthDetectorStats,
    HealthEvidence, HealthFile, HealthFinding, HealthSeverity, WorkspaceHealth,
};
use repomemo_storage::ChunkMention;

use crate::knowledge_map::similarity_edges;
use crate::RepoMemoCore;

/// Full-text lookups one health check may run, across all detectors.
const MAX_MENTION_QUERIES: usize = 200;
const MENTIONS_PER_QUERY: i64 = 200;
const EVIDENCE_PER_FINDING: usize = 4;
const EXCERPT_CHARS: usize = 220;
/// Shorter symbol names are too likely to be ordinary words.
const MIN_SYMBOL_LEN: usize = 4;
/// Shorter file names (`a.md`) match too much prose to be trusted.
const MIN_FILENAME_LEN: usize = 5;
/// Below this many embedded files there is no distribution to judge
/// "unconnected" against.
const UNCONNECTED_MIN_FILES: usize = 8;
/// Pairwise similarity is quadratic; past this the check is skipped.
const UNCONNECTED_MAX_FILES: usize = 2_000;
/// When more than this share of files is unconnected, the workspace is just
/// varied, and listing them would be noise rather than a signal.
const UNCONNECTED_MAX_SHARE: f64 = 0.15;

impl RepoMemoCore {
    pub async fn workspace_health(&self, workspace_id: &str) -> Result<WorkspaceHealth> {
        let artifacts = self.storage.list_artifacts(workspace_id).await?;
        let lifecycle = self.storage.workspace_lifecycle_states(workspace_id).await?;
        let retired = artifacts
            .iter()
            .filter(|artifact| {
                lifecycle
                    .get(&artifact.id)
                    .is_some_and(|(status, _)| matches!(status.as_str(), "outdated" | "superseded"))
            })
            .map(|artifact| artifact.id.clone())
            .collect::<HashSet<_>>();
        let live = artifacts
            .iter()
            .filter(|artifact| !retired.contains(&artifact.id))
            .collect::<Vec<_>>();
        let by_id = artifacts
            .iter()
            .map(|artifact| (artifact.id.as_str(), artifact))
            .collect::<HashMap<_, _>>();
        let versions = version_groups(&artifacts);

        let mut budget = MAX_MENTION_QUERIES;
        let mut findings = Vec::new();
        findings.extend(older_versions(&versions, &retired));
        findings.extend(duplicates(&live));
        findings.extend(
            self.removed_symbols(workspace_id, &versions, &retired, &by_id, &mut budget)
                .await?,
        );
        findings.extend(
            self.outdated_references(workspace_id, &artifacts, &lifecycle, &retired, &by_id, &mut budget)
                .await?,
        );
        findings.extend(self.index_failures(workspace_id, &retired, &by_id).await?);
        let similarity_available = match self.unconnected(workspace_id, &live).await? {
            Some(unconnected) => {
                findings.extend(unconnected);
                true
            }
            None => false,
        };

        let actions = self.storage.list_health_actions(workspace_id).await?;
        let handled = actions
            .iter()
            .map(|(fingerprint, _, _)| fingerprint.as_str())
            .collect::<HashSet<_>>();
        findings.retain(|finding| !handled.contains(finding.fingerprint.as_str()));
        findings.sort_by(|a, b| {
            a.severity
                .cmp(&b.severity)
                .then_with(|| detector_rank(a.detector).cmp(&detector_rank(b.detector)))
                .then_with(|| a.title.cmp(&b.title))
        });

        let detectors = HealthDetector::ALL
            .iter()
            .map(|&detector| {
                let recorded = actions
                    .iter()
                    .filter(|(_, name, _)| name == detector.as_str());
                let dismissed_count = recorded
                    .clone()
                    .filter(|(_, _, action)| action == HealthAction::Dismiss.as_str())
                    .count() as i64;
                HealthDetectorStats {
                    detector,
                    open_count: findings.iter().filter(|f| f.detector == detector).count() as i64,
                    acted_count: recorded.count() as i64 - dismissed_count,
                    dismissed_count,
                }
            })
            .collect();

        Ok(WorkspaceHealth {
            findings,
            detectors,
            checked_file_count: live.len() as i64,
            similarity_available,
        })
    }

    /// Documents that still mention a symbol the latest version of a code
    /// file removed. Only symbols that no longer exist anywhere in the
    /// workspace count, so code that merely moved is not reported.
    async fn removed_symbols(
        &self,
        workspace_id: &str,
        versions: &[Vec<&ArtifactSummary>],
        retired: &HashSet<String>,
        by_id: &HashMap<&str, &ArtifactSummary>,
        budget: &mut usize,
    ) -> Result<Vec<HealthFinding>> {
        let mut symbols: HashMap<String, HashSet<String>> = HashMap::new();
        for (artifact_id, name) in self.storage.workspace_symbol_names(workspace_id).await? {
            symbols.entry(artifact_id).or_default().insert(name);
        }
        // Symbol name -> files in use that define it.
        let mut defined_in: HashMap<&str, Vec<&str>> = HashMap::new();
        for (artifact_id, names) in symbols.iter().filter(|(id, _)| !retired.contains(*id)) {
            for name in names {
                defined_in.entry(name.as_str()).or_default().push(artifact_id.as_str());
            }
        }
        let empty = HashSet::new();

        // (document, code file) -> removed names with the passages naming them.
        let mut mentions: BTreeMap<(String, String), Vec<(String, ChunkMention, usize)>> = BTreeMap::new();
        for group in versions {
            let [.., previous, latest] = group.as_slice() else {
                continue;
            };
            if retired.contains(&latest.id) || latest.indexed_at.is_none() || previous.indexed_at.is_none() {
                continue;
            }
            let before = symbols.get(&previous.id).unwrap_or(&empty);
            let after = symbols.get(&latest.id).unwrap_or(&empty);
            let group_ids = group.iter().map(|artifact| artifact.id.as_str()).collect::<HashSet<_>>();
            let defined_elsewhere = |name: &str| {
                defined_in
                    .get(name)
                    .is_some_and(|ids| ids.iter().any(|id| !group_ids.contains(id)))
            };
            let removed = before
                .iter()
                .filter(|name| !after.contains(*name) && !defined_elsewhere(name))
                .filter(|name| is_identifier(name) && name.chars().count() >= MIN_SYMBOL_LEN)
                .collect::<BTreeSet<_>>();
            for name in removed {
                if *budget == 0 {
                    break;
                }
                *budget -= 1;
                let Ok(found) = self
                    .storage
                    .chunks_matching(workspace_id, &fts_phrase(name), MENTIONS_PER_QUERY)
                    .await
                else {
                    continue;
                };
                for mention in found {
                    let Some(document) = by_id.get(mention.artifact_id.as_str()) else {
                        continue;
                    };
                    if retired.contains(&document.id)
                        || group_ids.contains(document.id.as_str())
                        || matches!(document.artifact_type, ArtifactType::CodeFile | ArtifactType::Image)
                    {
                        continue;
                    }
                    if let Some(at) = find_identifier(&mention.text, name) {
                        mentions
                            .entry((document.id.clone(), latest.id.clone()))
                            .or_default()
                            .push((name.clone(), mention, at));
                    }
                }
            }
        }

        let mut findings = Vec::new();
        for ((document_id, code_id), found) in mentions {
            let (Some(document), Some(code)) = (by_id.get(document_id.as_str()), by_id.get(code_id.as_str())) else {
                continue;
            };
            let names = found.iter().map(|(name, _, _)| name.as_str()).collect::<BTreeSet<_>>();
            let listed = names.iter().map(|name| format!("`{name}`")).collect::<Vec<_>>().join(", ");
            let them = if names.len() == 1 { "it" } else { "them" };
            findings.push(HealthFinding {
                fingerprint: format!(
                    "{}:{}:{}:{}",
                    HealthDetector::RemovedSymbolMentioned.as_str(),
                    document.id,
                    code.id,
                    names.iter().copied().collect::<Vec<_>>().join(",")
                ),
                detector: HealthDetector::RemovedSymbolMentioned,
                severity: HealthSeverity::Warning,
                title: format!("{} mentions code removed from {}", document.title, code.path),
                detail: format!(
                    "The latest version of {} no longer defines {listed}, and nothing else in the workspace does. This document still refers to {them}, so it may describe code that is gone.",
                    code.path
                ),
                files: vec![health_file(document)],
                evidence: found
                    .iter()
                    .take(EVIDENCE_PER_FINDING)
                    .map(|(_, mention, at)| evidence(document, mention, *at))
                    .collect(),
                keep_artifact_id: None,
                actions: vec![HealthAction::NeedsReview, HealthAction::CreateTask, HealthAction::Dismiss],
            });
        }
        Ok(findings)
    }

    /// Files in use that name a file marked outdated or superseded. A name
    /// still carried by a file in use is skipped: the mention most likely
    /// means that one.
    async fn outdated_references(
        &self,
        workspace_id: &str,
        artifacts: &[ArtifactSummary],
        lifecycle: &HashMap<String, (String, Option<String>)>,
        retired: &HashSet<String>,
        by_id: &HashMap<&str, &ArtifactSummary>,
        budget: &mut usize,
    ) -> Result<Vec<HealthFinding>> {
        let live_names = artifacts
            .iter()
            .filter(|artifact| !retired.contains(&artifact.id))
            .map(|artifact| file_name(&artifact.path).to_lowercase())
            .collect::<HashSet<_>>();
        let mut targets: BTreeMap<String, Vec<&ArtifactSummary>> = BTreeMap::new();
        for artifact in artifacts.iter().filter(|artifact| retired.contains(&artifact.id)) {
            let name = file_name(&artifact.path);
            let key = name.to_lowercase();
            if name.chars().count() < MIN_FILENAME_LEN || !name.contains('.') || live_names.contains(&key) {
                continue;
            }
            targets.entry(name.to_owned()).or_default().push(artifact);
        }

        let mut referrers: BTreeMap<String, Vec<(&ArtifactSummary, ChunkMention, usize)>> = BTreeMap::new();
        for (name, retired_files) in &targets {
            if *budget == 0 {
                break;
            }
            *budget -= 1;
            let Ok(found) = self
                .storage
                .chunks_matching(workspace_id, &fts_phrase(name), MENTIONS_PER_QUERY)
                .await
            else {
                continue;
            };
            for mention in found {
                if retired.contains(&mention.artifact_id) || !by_id.contains_key(mention.artifact_id.as_str()) {
                    continue;
                }
                if let Some(at) = find_file_name(&mention.text, name) {
                    for target in retired_files {
                        referrers
                            .entry(mention.artifact_id.clone())
                            .or_default()
                            .push((target, mention.clone(), at));
                    }
                }
            }
        }

        let mut findings = Vec::new();
        for (document_id, found) in referrers {
            let Some(document) = by_id.get(document_id.as_str()) else {
                continue;
            };
            let mut target_ids = found.iter().map(|(target, _, _)| target.id.as_str()).collect::<Vec<_>>();
            target_ids.sort_unstable();
            target_ids.dedup();
            let reasons = target_ids
                .iter()
                .filter_map(|id| by_id.get(id))
                .map(|target| {
                    let (status, replacement) = lifecycle.get(&target.id).cloned().unwrap_or_default();
                    let replaced = replacement
                        .as_deref()
                        .and_then(|id| by_id.get(id))
                        .map(|next| format!(", replaced by {}", next.title))
                        .unwrap_or_default();
                    format!("{} is marked {status}{replaced}", file_name(&target.path))
                })
                .collect::<Vec<_>>()
                .join("; ");
            let mut seen = HashSet::new();
            findings.push(HealthFinding {
                fingerprint: format!(
                    "{}:{}:{}",
                    HealthDetector::OutdatedEvidenceReferenced.as_str(),
                    document.id,
                    target_ids.join(",")
                ),
                detector: HealthDetector::OutdatedEvidenceReferenced,
                severity: HealthSeverity::Warning,
                title: format!("{} points to outdated evidence", document.title),
                detail: format!("{reasons}. Review this file so it sends readers to current evidence."),
                files: vec![health_file(document)],
                evidence: found
                    .iter()
                    .filter(|(_, mention, _)| seen.insert(mention.chunk_id.clone()))
                    .take(EVIDENCE_PER_FINDING)
                    .map(|(_, mention, at)| evidence(document, mention, *at))
                    .collect(),
                keep_artifact_id: None,
                actions: vec![HealthAction::NeedsReview, HealthAction::CreateTask, HealthAction::Dismiss],
            });
        }
        Ok(findings)
    }

    async fn index_failures(
        &self,
        workspace_id: &str,
        retired: &HashSet<String>,
        by_id: &HashMap<&str, &ArtifactSummary>,
    ) -> Result<Vec<HealthFinding>> {
        let failures = self.storage.list_index_failures(workspace_id).await?;
        Ok(failures
            .into_iter()
            .filter(|failure| !retired.contains(&failure.artifact_id))
            .filter_map(|failure| {
                let artifact = by_id.get(failure.artifact_id.as_str())?;
                Some(HealthFinding {
                    fingerprint: format!("{}:{}", HealthDetector::IndexFailed.as_str(), artifact.id),
                    detector: HealthDetector::IndexFailed,
                    severity: HealthSeverity::Warning,
                    title: format!("{} could not be indexed", artifact.title),
                    detail: format!(
                        "Indexing stopped after {} attempt{}: {} Search, answers and the knowledge map cannot see this file until it is fixed or uploaded again.",
                        failure.attempts,
                        if failure.attempts == 1 { "" } else { "s" },
                        sentence(&failure.message)
                    ),
                    files: vec![health_file(artifact)],
                    evidence: Vec::new(),
                    keep_artifact_id: None,
                    actions: vec![HealthAction::CreateTask, HealthAction::MarkOutdated, HealthAction::Dismiss],
                })
            })
            .collect())
    }

    /// `None` when the workspace has no embedding provider.
    async fn unconnected(
        &self,
        workspace_id: &str,
        live: &[&ArtifactSummary],
    ) -> Result<Option<Vec<HealthFinding>>> {
        let Some(model) = self
            .embedding_provider_for_workspace(workspace_id)
            .await?
            .map(|settings| settings.embedding_model_name().to_owned())
        else {
            return Ok(None);
        };
        let centroids = self.storage.artifact_embedding_centroids(workspace_id, &model).await?;
        let files = live
            .iter()
            .filter_map(|artifact| {
                centroids
                    .get(&artifact.id)
                    .map(|vector| (artifact.id.as_str(), vector.as_slice()))
            })
            .collect::<Vec<_>>();
        if files.len() < UNCONNECTED_MIN_FILES || files.len() > UNCONNECTED_MAX_FILES {
            return Ok(Some(Vec::new()));
        }

        let mut connected = HashSet::new();
        for edge in similarity_edges(&files) {
            connected.insert(edge.source);
            connected.insert(edge.target);
        }
        for (_, _, artifact_id) in self.storage.memory_card_artifact_links(workspace_id).await? {
            connected.insert(artifact_id);
        }
        let lonely = live
            .iter()
            .filter(|artifact| centroids.contains_key(&artifact.id) && !connected.contains(&artifact.id))
            .collect::<Vec<_>>();
        if lonely.len() as f64 > files.len() as f64 * UNCONNECTED_MAX_SHARE {
            return Ok(Some(Vec::new()));
        }

        Ok(Some(
            lonely
                .into_iter()
                .map(|artifact| HealthFinding {
                    fingerprint: format!("{}:{}", HealthDetector::Unconnected.as_str(), artifact.id),
                    detector: HealthDetector::Unconnected,
                    severity: HealthSeverity::Info,
                    title: format!("{} is not connected to other files", artifact.title),
                    detail: "Its content is not close in meaning to any other file here, and no memory card cites it. A one-off file can still be valuable; check that it belongs in this workspace.".to_owned(),
                    files: vec![health_file(artifact)],
                    evidence: Vec::new(),
                    keep_artifact_id: None,
                    actions: vec![HealthAction::CreateTask, HealthAction::Dismiss],
                })
                .collect(),
        ))
    }
}

/// Uploads of the same file (same source and path), oldest first. Groups
/// with a single version are left out.
fn version_groups(artifacts: &[ArtifactSummary]) -> Vec<Vec<&ArtifactSummary>> {
    let mut groups: BTreeMap<(&str, &str), Vec<&ArtifactSummary>> = BTreeMap::new();
    for artifact in artifacts {
        groups
            .entry((artifact.source_id.as_str(), artifact.path.as_str()))
            .or_default()
            .push(artifact);
    }
    groups
        .into_values()
        .filter(|group| group.len() > 1)
        .map(|mut group| {
            group.sort_by(|a, b| a.created_at.cmp(&b.created_at).then_with(|| a.id.cmp(&b.id)));
            group
        })
        .collect()
}

fn older_versions(versions: &[Vec<&ArtifactSummary>], retired: &HashSet<String>) -> Vec<HealthFinding> {
    versions
        .iter()
        .filter_map(|group| {
            let live = group
                .iter()
                .copied()
                .filter(|artifact| !retired.contains(&artifact.id))
                .collect::<Vec<_>>();
            let (latest, older) = live.split_last()?;
            if older.is_empty() {
                return None;
            }
            let mut ids = older.iter().map(|artifact| artifact.id.as_str()).collect::<Vec<_>>();
            ids.sort_unstable();
            Some(HealthFinding {
                fingerprint: format!(
                    "{}:{}:{}",
                    HealthDetector::OlderVersionActive.as_str(),
                    latest.id,
                    ids.join(",")
                ),
                detector: HealthDetector::OlderVersionActive,
                severity: HealthSeverity::Warning,
                title: format!("Older versions of {} are still in use", latest.path),
                detail: format!(
                    "{} earlier upload{} of this file {} still active next to the latest one, so readers cannot tell which copy is current. Superseding keeps them inspectable.",
                    older.len(),
                    if older.len() == 1 { "" } else { "s" },
                    if older.len() == 1 { "is" } else { "are" }
                ),
                files: live.iter().map(|artifact| health_file(artifact)).collect(),
                evidence: Vec::new(),
                keep_artifact_id: Some(latest.id.clone()),
                actions: vec![HealthAction::Supersede, HealthAction::Dismiss],
            })
        })
        .collect()
}

fn duplicates(live: &[&ArtifactSummary]) -> Vec<HealthFinding> {
    let mut groups: BTreeMap<&str, Vec<&ArtifactSummary>> = BTreeMap::new();
    for artifact in live {
        groups.entry(artifact.content_hash.as_str()).or_default().push(artifact);
    }
    groups
        .into_iter()
        .filter(|(_, group)| group.len() > 1)
        .map(|(hash, mut group)| {
            group.sort_by(|a, b| a.created_at.cmp(&b.created_at).then_with(|| a.id.cmp(&b.id)));
            let mut ids = group.iter().map(|artifact| artifact.id.as_str()).collect::<Vec<_>>();
            ids.sort_unstable();
            HealthFinding {
                fingerprint: format!(
                    "{}:{}:{}",
                    HealthDetector::DuplicateContent.as_str(),
                    hash,
                    ids.join(",")
                ),
                detector: HealthDetector::DuplicateContent,
                severity: HealthSeverity::Warning,
                title: format!("{} files have identical content", group.len()),
                detail: "These files are byte-for-byte the same. Keep one and mark the others superseded, so the workspace has a single copy to cite and maintain.".to_owned(),
                files: group.iter().map(|artifact| health_file(artifact)).collect(),
                evidence: Vec::new(),
                keep_artifact_id: Some(group[0].id.clone()),
                actions: vec![HealthAction::Supersede, HealthAction::Dismiss],
            }
        })
        .collect()
}

fn detector_rank(detector: HealthDetector) -> usize {
    HealthDetector::ALL
        .iter()
        .position(|candidate| *candidate == detector)
        .unwrap_or(usize::MAX)
}

fn health_file(artifact: &ArtifactSummary) -> HealthFile {
    HealthFile {
        artifact_id: artifact.id.clone(),
        title: artifact.title.clone(),
        path: artifact.path.clone(),
        created_at: artifact.created_at.clone(),
    }
}

fn evidence(document: &ArtifactSummary, mention: &ChunkMention, at: usize) -> HealthEvidence {
    HealthEvidence {
        artifact_id: document.id.clone(),
        title: document.title.clone(),
        start_line: mention.start_line,
        end_line: mention.end_line,
        excerpt: excerpt(&mention.text, at),
    }
}

/// A full-text query matching `term` as a phrase.
fn fts_phrase(term: &str) -> String {
    format!("\"{}\"", term.replace('"', "\"\""))
}

fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn sentence(text: &str) -> String {
    let text = text.trim();
    if text.ends_with(['.', '!', '?']) {
        text.to_owned()
    } else {
        format!("{text}.")
    }
}

fn is_identifier_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn is_identifier(name: &str) -> bool {
    name.chars().any(char::is_alphabetic) && name.chars().all(is_identifier_char)
}

/// Names unlikely to appear as plain words: snake_case, camelCase or with
/// digits. Others only count when written like code.
fn is_distinctive(name: &str) -> bool {
    name.contains('_')
        || name.chars().any(|c| c.is_ascii_digit())
        || name.chars().skip(1).any(char::is_uppercase)
}

/// Byte offset of the first mention of `name` as a whole identifier that
/// is either distinctive or written like code (`name`, .name, name().
fn find_identifier(text: &str, name: &str) -> Option<usize> {
    text.match_indices(name).map(|(at, _)| at).find(|&at| {
        let before = text[..at].chars().next_back();
        let after = text[at + name.len()..].chars().next();
        if before.is_some_and(is_identifier_char) || after.is_some_and(is_identifier_char) {
            return false;
        }
        is_distinctive(name)
            || matches!(before, Some('`' | '.' | ':'))
            || matches!(after, Some('(' | '`'))
    })
}

/// Byte offset of the first mention of a file name, ignoring case, that is
/// not part of a longer name (`old-deploy.md` does not mention `deploy.md`).
fn find_file_name(text: &str, name: &str) -> Option<usize> {
    let lower_text = text.to_lowercase();
    let lower_name = name.to_lowercase();
    // Lowercasing can change byte lengths outside ASCII; only trust offsets
    // when it did not.
    if lower_text.len() != text.len() {
        return text.find(name);
    }
    lower_text.match_indices(&lower_name).map(|(at, _)| at).find(|&at| {
        let before = lower_text[..at].chars().next_back();
        let after = lower_text[at + lower_name.len()..].chars().next();
        let joins = |c: char| is_identifier_char(c) || c == '-';
        !before.is_some_and(|c| joins(c) || c == '.') && !after.is_some_and(joins)
    })
}

/// About `EXCERPT_CHARS` of text centred on byte offset `at`, on one line.
fn excerpt(text: &str, at: usize) -> String {
    let half = EXCERPT_CHARS / 2;
    let start = text[..at]
        .char_indices()
        .rev()
        .nth(half)
        .map(|(index, _)| index)
        .unwrap_or(0);
    let end = text[at..]
        .char_indices()
        .nth(half)
        .map(|(index, _)| at + index)
        .unwrap_or(text.len());
    let body = text[start..end].split_whitespace().collect::<Vec<_>>().join(" ");
    format!(
        "{}{body}{}",
        if start > 0 { "…" } else { "" },
        if end < text.len() { "…" } else { "" }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_must_be_whole_and_look_like_code() {
        assert!(find_identifier("Call parse_config before start.", "parse_config").is_some());
        assert!(find_identifier("Call reparse_config instead.", "parse_config").is_none());
        assert!(find_identifier("We render the page.", "render").is_none());
        assert!(find_identifier("Then call `render` again.", "render").is_some());
        assert!(find_identifier("Then call render() again.", "render").is_some());
        assert!(find_identifier("Use the ConfigLoader.", "ConfigLoader").is_some());
    }

    #[test]
    fn file_names_must_not_be_part_of_longer_names() {
        assert!(find_file_name("See docs/Deploy.md for steps.", "deploy.md").is_some());
        assert!(find_file_name("See old-deploy.md for steps.", "deploy.md").is_none());
        assert!(find_file_name("See deploy.mdx for steps.", "deploy.md").is_none());
        assert!(find_file_name("Read deploy.md.", "deploy.md").is_some());
    }

    #[test]
    fn excerpts_stay_on_char_boundaries() {
        let text = format!("{}needle{}", "é".repeat(300), "ü".repeat(300));
        let at = text.find("needle").unwrap();
        let cut = excerpt(&text, at);
        assert!(cut.contains("needle"));
        assert!(cut.starts_with('…') && cut.ends_with('…'));
    }

    #[test]
    fn phrases_escape_quotes() {
        assert_eq!(fts_phrase("say \"hi\""), "\"say \"\"hi\"\"\"");
    }
}
