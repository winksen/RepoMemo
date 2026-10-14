//! The knowledge map: indexing and embedding progress, coverage per file
//! type, and a graph of how files relate. Everything is computed from data
//! that already exists (chunks, their embeddings, memory card citations).
//!
//! A git repository is drawn as one node, the evidence item that stands for
//! it, not as one node per file: its size is the content of all its files,
//! its meaning is the average of theirs, and a memory card citing any of its
//! files links to it. The progress and coverage figures still count every
//! file, since each one is indexed.

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use repomemo_domain::{
    ArtifactType, IndexState, KnowledgeCoverage, KnowledgeEdge, KnowledgeEdgeKind, KnowledgeMap, KnowledgeNode,
    KnowledgeNodeKind, KnowledgePipeline,
};

use crate::meaning::Comparison;
use crate::RepoMemoCore;

/// Files drawn in the graph. Past this the graph is unreadable and the
/// pairwise similarity grows quadratically, so the smallest files are left out.
const MAX_FILE_NODES: usize = 250;

impl RepoMemoCore {
    pub async fn knowledge_map(&self, workspace_id: &str) -> Result<KnowledgeMap> {
        let artifacts = self.storage.list_artifacts(workspace_id).await?;
        let failed = self
            .storage
            .list_index_failures(workspace_id)
            .await?
            .into_iter()
            .map(|failure| failure.artifact_id)
            .collect::<HashSet<_>>();
        let embedding_model = self
            .embedding_provider_for_workspace(workspace_id)
            .await?
            .map(|settings| settings.embedding_model_name().to_owned());
        // Passage counts and meaning per file, read only for the files
        // written since the last request (see `meaning.rs`).
        let mut meanings = self
            .file_meanings(workspace_id, embedding_model.as_deref(), &artifacts)
            .await?;
        let state_of = |artifact: &repomemo_domain::ArtifactSummary| {
            if artifact.indexed_at.is_some() {
                IndexState::Indexed
            } else if failed.contains(&artifact.id) {
                IndexState::Failed
            } else {
                IndexState::Pending
            }
        };
        let counts_of = |artifact_id: &str| meanings.counts.get(artifact_id).copied().unwrap_or((0, 0));

        let repo_file_sources = self.storage.repo_artifact_sources(workspace_id).await?;
        let folded_into = folded_repository_files(&artifacts, &repo_file_sources);
        // Passages and embedded passages of a graph node, its folded files included.
        let mut node_counts: HashMap<String, (i64, i64)> = HashMap::new();
        for artifact in &artifacts {
            let node_id = folded_into.get(&artifact.id).unwrap_or(&artifact.id);
            let (passages, embedded) = counts_of(&artifact.id);
            let entry = node_counts.entry(node_id.clone()).or_default();
            entry.0 += passages;
            entry.1 += embedded;
        }
        let node_counts_of = |artifact_id: &str| node_counts.get(artifact_id).copied().unwrap_or((0, 0));

        let mut pipeline = KnowledgePipeline {
            file_count: artifacts.len() as i64,
            indexed_count: 0,
            pending_count: 0,
            failed_count: 0,
            passage_count: 0,
            embedded_count: embedding_model.as_ref().map(|_| 0),
            embedding_model: embedding_model.clone(),
        };
        let mut coverage: Vec<KnowledgeCoverage> = Vec::new();
        for artifact in &artifacts {
            let state = state_of(artifact);
            let (passages, embedded) = counts_of(&artifact.id);
            match state {
                IndexState::Indexed => pipeline.indexed_count += 1,
                IndexState::Pending => pipeline.pending_count += 1,
                IndexState::Failed => pipeline.failed_count += 1,
            }
            pipeline.passage_count += passages;
            if let Some(total) = pipeline.embedded_count.as_mut() {
                *total += embedded;
            }
            let label = if repo_file_sources.contains_key(&artifact.id) {
                "Repository files".to_owned()
            } else {
                kind_label(artifact)
            };
            let row = match coverage.iter_mut().position(|row| row.label == label) {
                Some(index) => &mut coverage[index],
                None => {
                    coverage.push(KnowledgeCoverage {
                        label,
                        file_count: 0,
                        indexed_count: 0,
                        pending_count: 0,
                        failed_count: 0,
                        passage_count: 0,
                        embedded_count: 0,
                    });
                    coverage.last_mut().expect("just pushed")
                }
            };
            row.file_count += 1;
            match state {
                IndexState::Indexed => row.indexed_count += 1,
                IndexState::Pending => row.pending_count += 1,
                IndexState::Failed => row.failed_count += 1,
            }
            row.passage_count += passages;
            row.embedded_count += embedded;
        }
        coverage.sort_by_key(|row| std::cmp::Reverse(row.file_count));

        // The graph keeps the files with the most content. Files of a
        // repository are not drawn on their own: their repository is.
        let mut ranked = artifacts
            .iter()
            .filter(|artifact| !folded_into.contains_key(&artifact.id))
            .collect::<Vec<_>>();
        ranked.sort_by(|a, b| {
            node_counts_of(&b.id)
                .0
                .cmp(&node_counts_of(&a.id).0)
                .then_with(|| a.title.cmp(&b.title))
        });
        let hidden_file_count = ranked.len().saturating_sub(MAX_FILE_NODES) as i64;
        ranked.truncate(MAX_FILE_NODES);
        let mut nodes = ranked
            .iter()
            .map(|artifact| {
                let (passage_count, embedded_count) = node_counts_of(&artifact.id);
                KnowledgeNode {
                    id: artifact.id.clone(),
                    kind: KnowledgeNodeKind::File,
                    title: artifact.title.clone(),
                    path: Some(artifact.path.clone()),
                    artifact_type: Some(artifact.artifact_type.clone()),
                    state: Some(state_of(artifact)),
                    passage_count,
                    embedded_count,
                }
            })
            .collect::<Vec<_>>();
        let visible = nodes.iter().map(|node| node.id.clone()).collect::<HashSet<_>>();

        let mut edges = Vec::new();
        let mut similarity_available = false;
        if embedding_model.is_some() {
            meanings.fold_repositories(&folded_into);
            let files = meanings.files(nodes.iter().map(|node| node.id.as_str()));
            similarity_available = !files.is_empty();
            // Only the pairs that involve a file written since last time are compared.
            edges.extend(self.similarity_edges_cached(workspace_id, Comparison::Map, &files).0);
        }

        let mut cards: HashMap<String, String> = HashMap::new();
        let mut card_links = HashSet::new();
        for (card_id, title, artifact_id) in self.storage.memory_card_artifact_links(workspace_id).await? {
            // A card citing a repository's file links to the repository.
            let artifact_id = folded_into.get(&artifact_id).cloned().unwrap_or(artifact_id);
            if !visible.contains(&artifact_id) || !card_links.insert((card_id.clone(), artifact_id.clone())) {
                continue;
            }
            cards.entry(card_id.clone()).or_insert(title);
            edges.push(KnowledgeEdge {
                source: card_id,
                target: artifact_id,
                kind: KnowledgeEdgeKind::Cites,
                weight: 1.0,
            });
        }
        let mut cards = cards.into_iter().collect::<Vec<_>>();
        cards.sort_by(|a, b| a.1.cmp(&b.1));
        nodes.extend(cards.into_iter().map(|(id, title)| KnowledgeNode {
            id,
            kind: KnowledgeNodeKind::Memory,
            title,
            path: None,
            artifact_type: None,
            state: None,
            passage_count: 0,
            embedded_count: 0,
        }));

        Ok(KnowledgeMap {
            pipeline,
            coverage,
            nodes,
            edges,
            hidden_file_count,
            similarity_available,
        })
    }
}

/// The kind of file a person would name: uploaded documents all share the
/// `file` type, so their extension tells PDF from Word or Excel.
fn kind_label(artifact: &repomemo_domain::ArtifactSummary) -> String {
    use repomemo_domain::ArtifactType;
    let label = match artifact.artifact_type {
        ArtifactType::MarkdownDoc => "Markdown",
        ArtifactType::CodeFile => "Code",
        ArtifactType::Note => "Notes",
        ArtifactType::Repository => "Repositories",
        ArtifactType::Image => "Images",
        ArtifactType::ApiSpec => "API specs",
        ArtifactType::Runbook => "Runbooks",
        ArtifactType::Decision => "Decisions",
        ArtifactType::Incident => "Incidents",
        ArtifactType::Issue => "Issues",
        ArtifactType::Pr => "Pull requests",
        ArtifactType::File => {
            let extension = artifact
                .path
                .rsplit_once('.')
                .map(|(_, extension)| extension.to_ascii_lowercase())
                .unwrap_or_default();
            match extension.as_str() {
                "pdf" => "PDF",
                "doc" | "docx" | "odt" | "rtf" => "Word",
                "xls" | "xlsx" | "csv" | "ods" => "Excel",
                "ppt" | "pptx" | "odp" => "PowerPoint",
                "one" => "OneNote",
                "msg" | "eml" => "Email",
                "txt" | "log" => "Text",
                "json" | "toml" | "yaml" | "yml" => "Config",
                _ => "Other files",
            }
        }
    };
    label.to_owned()
}

/// `file artifact id -> repository item id` for every file of a repository
/// that has its evidence item. Views that relate files to each other (the map,
/// health checks) draw a repository as that one item, not as its files.
pub(crate) fn folded_repository_files(
    artifacts: &[repomemo_domain::ArtifactSummary],
    repo_file_sources: &HashMap<String, String>,
) -> HashMap<String, String> {
    let repository_items = artifacts
        .iter()
        .filter(|artifact| artifact.artifact_type == ArtifactType::Repository)
        .map(|artifact| (artifact.source_id.as_str(), artifact.id.as_str()))
        .collect::<HashMap<_, _>>();
    repo_file_sources
        .iter()
        .filter_map(|(file_id, source_id)| {
            repository_items
                .get(source_id.as_str())
                .map(|item_id| (file_id.clone(), (*item_id).to_owned()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::meaning::{edges_from_similarity, FileMeanings, PairFile, PairSimilarity};
    use std::collections::HashMap;
    use std::sync::Arc;

    fn similarity_edges(files: &[(&str, &[f32])]) -> Vec<repomemo_domain::KnowledgeEdge> {
        let files = files
            .iter()
            .map(|(id, vector)| PairFile { id, stamp: "", vector })
            .collect::<Vec<_>>();
        edges_from_similarity(&PairSimilarity::build(None, &files).0)
    }

    #[test]
    fn a_repository_means_what_its_files_mean_together() {
        let stamped = |id: &str| (id.to_owned(), "1".to_owned());
        let mut meanings = FileMeanings {
            counts: HashMap::new(),
            centroids: HashMap::from([
                ("file-a".to_owned(), Arc::new(vec![1.0_f32, 0.0])),
                ("file-b".to_owned(), Arc::new(vec![0.0_f32, 1.0])),
                ("other".to_owned(), Arc::new(vec![1.0_f32, 0.0])),
            ]),
            stamps: HashMap::from([stamped("file-a"), stamped("file-b"), stamped("other")]),
            refreshed: 0,
        };
        let folded = HashMap::from([
            ("file-a".to_owned(), "repo".to_owned()),
            ("file-b".to_owned(), "repo".to_owned()),
        ]);
        meanings.fold_repositories(&folded);
        let repo = &meanings.centroids["repo"];
        assert!((repo[0] - repo[1]).abs() < 1e-6);
        assert!((repo[0] * repo[0] + repo[1] * repo[1] - 1.0).abs() < 1e-6);
        assert_eq!(*meanings.centroids["other"], vec![1.0, 0.0]);
        // The repository's stamp follows its files': a changed file changes it.
        let before = meanings.stamps["repo"].clone();
        meanings.stamps.insert("file-b".to_owned(), "2".to_owned());
        meanings.fold_repositories(&folded);
        assert_ne!(meanings.stamps["repo"], before);
    }
    #[test]
    fn small_workspaces_link_only_clearly_similar_files() {
        let deploy = [1.0_f32, 0.0];
        let release = [0.995_f32, 0.0998];
        let retry = [0.0_f32, 1.0];
        let edges = similarity_edges(&[("deploy", &deploy), ("release", &release), ("retry", &retry)]);
        assert_eq!(edges.len(), 1);
        assert_eq!(
            (edges[0].source.as_str(), edges[0].target.as_str()),
            ("deploy", "release")
        );
    }

    #[test]
    fn each_file_keeps_at_most_its_closest_neighbours() {
        // Twelve near-identical files and one outlier: 66 pairs, so the
        // adaptive threshold applies and no file gets more than three links
        // of its own.
        let close = (0..12)
            .map(|index| {
                let angle = index as f32 * 0.01;
                [angle.cos(), angle.sin()]
            })
            .collect::<Vec<_>>();
        let outlier = [0.0_f32, -1.0];
        let mut files = close
            .iter()
            .enumerate()
            .map(|(index, vector)| {
                (
                    ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l"][index],
                    vector.as_slice(),
                )
            })
            .collect::<Vec<_>>();
        files.push(("outlier", &outlier));
        let edges = similarity_edges(&files);
        assert!(edges
            .iter()
            .all(|edge| edge.source != "outlier" && edge.target != "outlier"));
        assert!(edges.len() <= 12 * 3);
        assert!(!edges.is_empty());
    }
}
