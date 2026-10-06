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

use crate::RepoMemoCore;

/// Files drawn in the graph. Past this the graph is unreadable and the
/// pairwise similarity grows quadratically, so the smallest files are left out.
const MAX_FILE_NODES: usize = 250;
/// Similar-content links kept per file: its closest neighbours only, so the
/// graph shows structure instead of a hairball.
const NEIGHBOURS_PER_FILE: usize = 3;
/// With few files there is no distribution to judge "unusually similar"
/// against, so a fixed cosine floor is used instead.
const SMALL_WORKSPACE_PAIRS: usize = 10;
const SMALL_WORKSPACE_MIN_SIMILARITY: f64 = 0.5;

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
        let counts = self
            .storage
            .chunk_counts_by_artifact(workspace_id, embedding_model.as_deref())
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
        let counts_of = |artifact_id: &str| counts.get(artifact_id).copied().unwrap_or((0, 0));

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
        coverage.sort_by(|a, b| b.file_count.cmp(&a.file_count));

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
        if let Some(model) = embedding_model.as_deref() {
            let mut centroids = self
                .storage
                .artifact_embedding_centroids(workspace_id, model)
                .await?;
            fold_repository_centroids(&mut centroids, &folded_into);
            let files = nodes
                .iter()
                .filter_map(|node| centroids.get(&node.id).map(|vector| (node.id.as_str(), vector.as_slice())))
                .collect::<Vec<_>>();
            similarity_available = !files.is_empty();
            edges.extend(similarity_edges(&files));
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

/// Gives each repository item the meaning of its files together: the
/// normalised mean of their vectors and its own.
pub(crate) fn fold_repository_centroids(
    centroids: &mut HashMap<String, Vec<f32>>,
    folded_into: &HashMap<String, String>,
) {
    let mut members: HashMap<&str, Vec<&str>> = HashMap::new();
    for (file_id, item_id) in folded_into {
        members.entry(item_id.as_str()).or_default().push(file_id.as_str());
    }
    let mut combined = Vec::new();
    for (item_id, files) in members {
        let vectors = files
            .into_iter()
            .chain(std::iter::once(item_id))
            .filter_map(|id| centroids.get(id))
            .collect::<Vec<_>>();
        if let Some(vector) = mean_unit_vector(&vectors) {
            combined.push((item_id.to_owned(), vector));
        }
    }
    centroids.extend(combined);
}

/// The normalised mean of unit vectors of the same length, if any.
fn mean_unit_vector(vectors: &[&Vec<f32>]) -> Option<Vec<f32>> {
    let length = vectors.first()?.len();
    let mut sum = vec![0.0_f32; length];
    for vector in vectors.iter().filter(|vector| vector.len() == length) {
        for (total, value) in sum.iter_mut().zip(vector.iter()) {
            *total += value;
        }
    }
    let norm = sum.iter().map(|value| value * value).sum::<f32>().sqrt();
    (norm > 0.0).then(|| sum.into_iter().map(|value| value / norm).collect())
}

/// Links each file to its closest neighbours by meaning. Only pairs that are
/// unusually similar for this workspace count: at least the mean plus a
/// quarter standard deviation of all pairs, which adapts to embedding models
/// whose cosines sit in different ranges. The bar never rises above the 75th
/// percentile, so a workspace about one topic still shows its closest pairs.
/// Vectors must be unit length.
pub(crate) fn similarity_edges(files: &[(&str, &[f32])]) -> Vec<KnowledgeEdge> {
    let count = files.len();
    if count < 2 {
        return Vec::new();
    }
    let mut similarity = vec![vec![0.0_f64; count]; count];
    let mut all = Vec::with_capacity(count * (count - 1) / 2);
    for i in 0..count {
        for j in (i + 1)..count {
            let (left, right) = (files[i].1, files[j].1);
            let value = if left.len() == right.len() {
                left.iter().zip(right).map(|(a, b)| f64::from(*a) * f64::from(*b)).sum()
            } else {
                0.0
            };
            similarity[i][j] = value;
            similarity[j][i] = value;
            all.push(value);
        }
    }
    let threshold = if all.len() < SMALL_WORKSPACE_PAIRS {
        SMALL_WORKSPACE_MIN_SIMILARITY
    } else {
        let mean = all.iter().sum::<f64>() / all.len() as f64;
        let variance = all.iter().map(|value| (value - mean).powi(2)).sum::<f64>() / all.len() as f64;
        let mut sorted = all.clone();
        sorted.sort_by(f64::total_cmp);
        let upper_quartile = sorted[sorted.len() * 3 / 4];
        (mean + variance.sqrt() / 4.0).min(upper_quartile)
    };

    let mut kept = HashSet::new();
    let mut edges = Vec::new();
    for i in 0..count {
        let mut neighbours = (0..count)
            .filter(|&j| j != i && similarity[i][j] > 0.0 && similarity[i][j] >= threshold)
            .collect::<Vec<_>>();
        neighbours.sort_by(|&a, &b| similarity[i][b].total_cmp(&similarity[i][a]));
        for j in neighbours.into_iter().take(NEIGHBOURS_PER_FILE) {
            if kept.insert((i.min(j), i.max(j))) {
                edges.push(KnowledgeEdge {
                    source: files[i.min(j)].0.to_owned(),
                    target: files[i.max(j)].0.to_owned(),
                    kind: KnowledgeEdgeKind::Similar,
                    weight: similarity[i][j],
                });
            }
        }
    }
    edges
}

#[cfg(test)]
mod tests {
    use super::{fold_repository_centroids, similarity_edges};
    use std::collections::HashMap;

    #[test]
    fn a_repository_means_what_its_files_mean_together() {
        let mut centroids = HashMap::from([
            ("file-a".to_owned(), vec![1.0_f32, 0.0]),
            ("file-b".to_owned(), vec![0.0_f32, 1.0]),
            ("other".to_owned(), vec![1.0_f32, 0.0]),
        ]);
        let folded = HashMap::from([
            ("file-a".to_owned(), "repo".to_owned()),
            ("file-b".to_owned(), "repo".to_owned()),
        ]);
        fold_repository_centroids(&mut centroids, &folded);
        let repo = &centroids["repo"];
        assert!((repo[0] - repo[1]).abs() < 1e-6);
        assert!((repo[0] * repo[0] + repo[1] * repo[1] - 1.0).abs() < 1e-6);
        assert_eq!(centroids["other"], vec![1.0, 0.0]);
    }

    #[test]
    fn small_workspaces_link_only_clearly_similar_files() {
        let deploy = [1.0_f32, 0.0];
        let release = [0.995_f32, 0.0998];
        let retry = [0.0_f32, 1.0];
        let edges = similarity_edges(&[("deploy", &deploy), ("release", &release), ("retry", &retry)]);
        assert_eq!(edges.len(), 1);
        assert_eq!((edges[0].source.as_str(), edges[0].target.as_str()), ("deploy", "release"));
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
        let mut files = close.iter().enumerate().map(|(index, vector)| (["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l"][index], vector.as_slice())).collect::<Vec<_>>();
        files.push(("outlier", &outlier));
        let edges = similarity_edges(&files);
        assert!(edges.iter().all(|edge| edge.source != "outlier" && edge.target != "outlier"));
        assert!(edges.len() <= 12 * 3);
        assert!(!edges.is_empty());
    }
}
