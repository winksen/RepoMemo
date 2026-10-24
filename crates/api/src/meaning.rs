//! What the knowledge map and Workspace Health read from every file's passages
//! and vectors, kept between requests and redone only for the files that were
//! written since (roadmap P11).
//!
//! Three things are expensive on a large workspace: reading every vector to
//! find where each file sits in meaning space (its *centroid*), the passage
//! counts per file, and comparing every pair of files. All three are kept per
//! file with a *stamp*: the journal number of the file's last write in storage
//! (`StorageEngine::vector_stamps`), its index time and the embedding model. A
//! file whose stamp is unchanged is not read again, and a pair of files whose
//! stamps are both unchanged is not compared again, so one edited file costs
//! its own passages and one row of comparisons, not the workspace.
//!
//! The caches are in memory and are never the source of truth: a restart, a
//! new embedding model or a workspace that is too large to keep simply makes
//! the next request do the whole work, as it always did.

use std::collections::{hash_map::DefaultHasher, HashMap};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

use anyhow::Result;
use repomemo_domain::{ArtifactSummary, KnowledgeEdge, KnowledgeEdgeKind};

use crate::RepoMemoCore;

/// Centroids kept across workspaces, in numbers (4 bytes each): 64 MiB. Past
/// it the work is redone on each request, as before.
const MAX_CACHED_FLOATS: usize = 16 * 1024 * 1024;
/// When this share of the files is stale, one pass over the workspace is
/// cheaper than many small reads.
const BULK_READ_SHARE: usize = 3;

/// Which comparison of files a cached matrix belongs to: the map and the
/// health check compare different sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Comparison {
    Map,
    Health,
}

#[derive(Clone)]
struct FileMeaning {
    stamp: String,
    passages: i64,
    embedded: i64,
    centroid: Option<Arc<Vec<f32>>>,
}

#[derive(Default)]
struct WorkspaceMeaning {
    model: Option<String>,
    files: HashMap<String, FileMeaning>,
    map_pairs: Option<PairSimilarity>,
    health_pairs: Option<PairSimilarity>,
}

#[derive(Clone, Default)]
pub(crate) struct MeaningCache {
    workspaces: Arc<Mutex<HashMap<String, WorkspaceMeaning>>>,
}

impl std::fmt::Debug for MeaningCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MeaningCache").finish_non_exhaustive()
    }
}

impl MeaningCache {
    /// Forgets everything, so the next request does the whole work.
    #[cfg(test)]
    pub(crate) fn clear(&self) {
        if let Ok(mut workspaces) = self.workspaces.lock() {
            workspaces.clear();
        }
    }
}

/// What the map and health read for the files of one workspace, as of now.
pub(crate) struct FileMeanings {
    /// `(passages, passages with a vector from the model)` per file.
    pub counts: HashMap<String, (i64, i64)>,
    /// The direction of each embedded file, scaled to length one.
    pub centroids: HashMap<String, Arc<Vec<f32>>>,
    /// Changes whenever what the file's centroid is made of does.
    pub stamps: HashMap<String, String>,
    /// Files read from the database this time (the rest came from memory).
    #[cfg_attr(not(test), allow(dead_code))]
    pub refreshed: usize,
}

impl FileMeanings {
    /// Gives each repository item the meaning of its files together (the
    /// normalised mean of their vectors and its own), and a stamp that changes
    /// when any of them does.
    pub(crate) fn fold_repositories(&mut self, folded_into: &HashMap<String, String>) {
        let mut members: HashMap<&str, Vec<&str>> = HashMap::new();
        for (file_id, item_id) in folded_into {
            members.entry(item_id.as_str()).or_default().push(file_id.as_str());
        }
        let mut combined = Vec::new();
        for (item_id, mut files) in members {
            files.sort_unstable();
            let vectors = files
                .iter()
                .copied()
                .chain(std::iter::once(item_id))
                .filter_map(|id| self.centroids.get(id).map(|vector| vector.as_ref()))
                .collect::<Vec<_>>();
            let Some(vector) = mean_unit_vector(&vectors) else {
                continue;
            };
            let mut hasher = DefaultHasher::new();
            for id in files.iter().copied().chain(std::iter::once(item_id)) {
                id.hash(&mut hasher);
                self.stamps.get(id).hash(&mut hasher);
            }
            combined.push((
                item_id.to_owned(),
                Arc::new(vector),
                format!("folded:{:x}", hasher.finish()),
            ));
        }
        for (item_id, vector, stamp) in combined {
            self.centroids.insert(item_id.clone(), vector);
            self.stamps.insert(item_id, stamp);
        }
    }

    /// The comparison inputs for these files: those that have a centroid.
    pub(crate) fn files<'a>(&'a self, ids: impl IntoIterator<Item = &'a str>) -> Vec<PairFile<'a>> {
        ids.into_iter()
            .filter_map(|id| {
                Some(PairFile {
                    id,
                    stamp: self.stamps.get(id)?.as_str(),
                    vector: self.centroids.get(id)?.as_slice(),
                })
            })
            .collect()
    }
}

impl RepoMemoCore {
    /// Passage counts and centroids of the given files for `model`, read from
    /// the database only for files written since they were last read.
    pub(crate) async fn file_meanings(
        &self,
        workspace_id: &str,
        model: Option<&str>,
        artifacts: &[ArtifactSummary],
    ) -> Result<FileMeanings> {
        let journal = self.storage.vector_stamps(workspace_id);
        let stamps = artifacts
            .iter()
            .map(|artifact| {
                (
                    artifact.id.clone(),
                    format!(
                        "{}|{}|{}|{}",
                        model.unwrap_or(""),
                        journal.of(&artifact.id),
                        artifact.indexed_at.as_deref().unwrap_or(""),
                        artifact.updated_at
                    ),
                )
            })
            .collect::<HashMap<_, _>>();

        // Which files need reading, decided under the lock; the reads happen outside it.
        let stale = {
            let mut workspaces = self
                .meaning
                .workspaces
                .lock()
                .map_err(|_| anyhow::anyhow!("The meaning cache is unavailable."))?;
            let cached = workspaces.entry(workspace_id.to_owned()).or_default();
            if cached.model.as_deref() != model {
                *cached = WorkspaceMeaning {
                    model: model.map(str::to_owned),
                    ..WorkspaceMeaning::default()
                };
            }
            cached.files.retain(|id, _| stamps.contains_key(id));
            artifacts
                .iter()
                .filter(|artifact| {
                    cached
                        .files
                        .get(&artifact.id)
                        .is_none_or(|file| file.stamp != stamps[&artifact.id])
                })
                .map(|artifact| artifact.id.clone())
                .collect::<Vec<_>>()
        };

        let mut fresh: HashMap<String, FileMeaning> = HashMap::new();
        if !stale.is_empty() {
            let bulk = stale.len() * BULK_READ_SHARE >= artifacts.len();
            let counts = if bulk {
                self.storage.chunk_counts_by_artifact(workspace_id, model).await?
            } else {
                self.storage.chunk_counts_for_artifacts(model, &stale).await?
            };
            let mut centroids = match model {
                Some(model) if bulk => self.storage.artifact_embedding_centroids(workspace_id, model).await?,
                Some(model) => self.storage.artifact_embedding_centroids_for(model, &stale).await?,
                None => HashMap::new(),
            };
            for id in &stale {
                let (passages, embedded) = counts.get(id).copied().unwrap_or((0, 0));
                fresh.insert(
                    id.clone(),
                    FileMeaning {
                        stamp: stamps[id].clone(),
                        passages,
                        embedded,
                        centroid: centroids.remove(id).map(Arc::new),
                    },
                );
            }
        }
        let refreshed = fresh.len();

        let mut workspaces = self
            .meaning
            .workspaces
            .lock()
            .map_err(|_| anyhow::anyhow!("The meaning cache is unavailable."))?;
        let cached = workspaces.entry(workspace_id.to_owned()).or_default();
        // Another request may have reset it for another model meanwhile: then
        // what was read is still right for this one, and just not kept.
        let keep = cached.model.as_deref() == model;
        if keep {
            cached
                .files
                .extend(fresh.iter().map(|(id, file)| (id.clone(), file.clone())));
        }
        let mut meanings = FileMeanings {
            counts: HashMap::new(),
            centroids: HashMap::new(),
            stamps: HashMap::new(),
            refreshed,
        };
        for artifact in artifacts {
            let Some(file) = fresh.get(&artifact.id).or_else(|| cached.files.get(&artifact.id)) else {
                continue;
            };
            meanings
                .counts
                .insert(artifact.id.clone(), (file.passages, file.embedded));
            if let Some(centroid) = &file.centroid {
                meanings.centroids.insert(artifact.id.clone(), centroid.clone());
                meanings.stamps.insert(artifact.id.clone(), file.stamp.clone());
            }
        }
        // Past the budget the centroids are not kept; counts always are.
        let kept = cached
            .files
            .values()
            .filter_map(|file| file.centroid.as_ref().map(|centroid| centroid.len()))
            .sum::<usize>();
        if kept > MAX_CACHED_FLOATS {
            for file in cached.files.values_mut() {
                file.centroid = None;
            }
            cached.map_pairs = None;
            cached.health_pairs = None;
        }
        Ok(meanings)
    }

    /// Links each file to its closest neighbours by meaning, comparing only
    /// the pairs that involve a file written since the last comparison of the
    /// same kind. Returns the edges and how many pairs were compared.
    pub(crate) fn similarity_edges_cached(
        &self,
        workspace_id: &str,
        comparison: Comparison,
        files: &[PairFile<'_>],
    ) -> (Vec<KnowledgeEdge>, usize) {
        let previous = self.meaning.workspaces.lock().ok().and_then(|mut workspaces| {
            let cached = workspaces.get_mut(workspace_id)?;
            match comparison {
                Comparison::Map => cached.map_pairs.take(),
                Comparison::Health => cached.health_pairs.take(),
            }
        });
        let (pairs, compared) = PairSimilarity::build(previous.as_ref(), files);
        let edges = edges_from_similarity(&pairs);
        if let Ok(mut workspaces) = self.meaning.workspaces.lock() {
            if let Some(cached) = workspaces.get_mut(workspace_id) {
                match comparison {
                    Comparison::Map => cached.map_pairs = Some(pairs),
                    Comparison::Health => cached.health_pairs = Some(pairs),
                }
            }
        }
        (edges, compared)
    }
}

/// A file as the comparison sees it.
pub(crate) struct PairFile<'a> {
    pub id: &'a str,
    /// Equal stamps mean equal vectors: the comparison is not redone.
    pub stamp: &'a str,
    pub vector: &'a [f32],
}

/// The cosine similarity of every pair of a set of files (vectors of length
/// one), with the stamps they were compared at.
pub(crate) struct PairSimilarity {
    ids: Vec<String>,
    stamps: Vec<String>,
    values: Vec<f32>,
}

impl PairSimilarity {
    /// Compares every pair, except those both of whose files `previous`
    /// already compared with the same stamps. Returns the pairs compared.
    pub(crate) fn build(previous: Option<&PairSimilarity>, files: &[PairFile<'_>]) -> (Self, usize) {
        let count = files.len();
        let earlier = previous.map(|previous| {
            let positions = previous
                .ids
                .iter()
                .enumerate()
                .map(|(position, id)| (id.as_str(), position))
                .collect::<HashMap<_, _>>();
            let reusable = files
                .iter()
                .map(|file| {
                    positions
                        .get(file.id)
                        .copied()
                        .filter(|&position| previous.stamps[position] == file.stamp)
                })
                .collect::<Vec<_>>();
            (previous, reusable)
        });
        let mut values = vec![0.0_f32; count * count];
        let mut compared = 0;
        for i in 0..count {
            for j in (i + 1)..count {
                let known = earlier.as_ref().and_then(|(previous, reusable)| {
                    let (a, b) = (reusable[i]?, reusable[j]?);
                    Some(previous.values[a * previous.ids.len() + b])
                });
                let value = known.unwrap_or_else(|| {
                    compared += 1;
                    let (left, right) = (files[i].vector, files[j].vector);
                    if left.len() == right.len() {
                        left.iter().zip(right).map(|(a, b)| a * b).sum()
                    } else {
                        0.0
                    }
                });
                values[i * count + j] = value;
                values[j * count + i] = value;
            }
        }
        (
            Self {
                ids: files.iter().map(|file| file.id.to_owned()).collect(),
                stamps: files.iter().map(|file| file.stamp.to_owned()).collect(),
                values,
            },
            compared,
        )
    }

    fn at(&self, i: usize, j: usize) -> f64 {
        f64::from(self.values[i * self.ids.len() + j])
    }
}

/// With few files there is no distribution to judge "unusually similar"
/// against, so a fixed cosine floor is used instead.
const SMALL_WORKSPACE_PAIRS: usize = 10;
const SMALL_WORKSPACE_MIN_SIMILARITY: f64 = 0.5;
/// Similar-content links kept per file: its closest neighbours only, so the
/// graph shows structure instead of a hairball.
const NEIGHBOURS_PER_FILE: usize = 3;

/// Links each file to its closest neighbours by meaning. Only pairs that are
/// unusually similar for this workspace count: at least the mean plus a
/// quarter standard deviation of all pairs, which adapts to embedding models
/// whose cosines sit in different ranges. The bar never rises above the 75th
/// percentile, so a workspace about one topic still shows its closest pairs.
pub(crate) fn edges_from_similarity(pairs: &PairSimilarity) -> Vec<KnowledgeEdge> {
    let count = pairs.ids.len();
    if count < 2 {
        return Vec::new();
    }
    let mut all = Vec::with_capacity(count * (count - 1) / 2);
    for i in 0..count {
        for j in (i + 1)..count {
            all.push(pairs.at(i, j));
        }
    }
    let threshold = if all.len() < SMALL_WORKSPACE_PAIRS {
        SMALL_WORKSPACE_MIN_SIMILARITY
    } else {
        let mean = all.iter().sum::<f64>() / all.len() as f64;
        let variance = all.iter().map(|value| (value - mean).powi(2)).sum::<f64>() / all.len() as f64;
        let quartile_at = all.len() * 3 / 4;
        let (_, upper_quartile, _) = all.select_nth_unstable_by(quartile_at, f64::total_cmp);
        (mean + variance.sqrt() / 4.0).min(*upper_quartile)
    };

    let mut kept = std::collections::HashSet::new();
    let mut edges = Vec::new();
    for i in 0..count {
        let mut neighbours = (0..count)
            .filter(|&j| j != i && pairs.at(i, j) > 0.0 && pairs.at(i, j) >= threshold)
            .collect::<Vec<_>>();
        neighbours.sort_by(|&a, &b| pairs.at(i, b).total_cmp(&pairs.at(i, a)));
        for j in neighbours.into_iter().take(NEIGHBOURS_PER_FILE) {
            if kept.insert((i.min(j), i.max(j))) {
                edges.push(KnowledgeEdge {
                    source: pairs.ids[i.min(j)].clone(),
                    target: pairs.ids[i.max(j)].clone(),
                    kind: KnowledgeEdgeKind::Similar,
                    weight: pairs.at(i, j),
                });
            }
        }
    }
    edges
}

/// The normalised mean of unit vectors of the same length, if any.
pub(crate) fn mean_unit_vector(vectors: &[&Vec<f32>]) -> Option<Vec<f32>> {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(angle: f32) -> Vec<f32> {
        vec![angle.cos(), angle.sin()]
    }

    fn file<'a>(id: &'a str, stamp: &'a str, vector: &'a [f32]) -> PairFile<'a> {
        PairFile { id, stamp, vector }
    }

    #[test]
    fn only_the_pairs_of_a_changed_file_are_compared_again() {
        let vectors = (0..6).map(|index| unit(index as f32 * 0.3)).collect::<Vec<_>>();
        let ids = ["a", "b", "c", "d", "e", "f"];
        let files = |stamps: &[&'static str]| {
            ids.iter()
                .zip(stamps)
                .zip(&vectors)
                .map(|((id, stamp), vector)| file(id, stamp, vector))
                .collect::<Vec<_>>()
        };
        let (first, compared) = PairSimilarity::build(None, &files(&["1"; 6]));
        assert_eq!(compared, 15, "every pair the first time");

        let (same, compared) = PairSimilarity::build(Some(&first), &files(&["1"; 6]));
        assert_eq!(compared, 0, "nothing changed");
        assert_eq!(same.values, first.values);

        // One file written: five pairs, not fifteen.
        let (changed, compared) = PairSimilarity::build(Some(&first), &files(&["1", "1", "2", "1", "1", "1"]));
        assert_eq!(compared, 5);
        assert_eq!(changed.values, first.values, "the vector was not really different");
    }

    #[test]
    fn a_new_file_is_compared_with_the_others_and_a_removed_one_leaves() {
        let vectors = [unit(0.0), unit(0.4), unit(0.8)];
        let (three, _) = PairSimilarity::build(
            None,
            &[
                file("a", "1", &vectors[0]),
                file("b", "1", &vectors[1]),
                file("c", "1", &vectors[2]),
            ],
        );
        let (two, compared) = PairSimilarity::build(
            Some(&three),
            &[file("c", "1", &vectors[2]), file("a", "1", &vectors[0])],
        );
        assert_eq!(compared, 0, "c and a were compared before, in another order");
        assert!((two.at(0, 1) - three.at(2, 0)).abs() < 1e-9);
        let (four, compared) = PairSimilarity::build(
            Some(&three),
            &[
                file("a", "1", &vectors[0]),
                file("b", "1", &vectors[1]),
                file("c", "1", &vectors[2]),
                file("d", "1", &vectors[0]),
            ],
        );
        assert_eq!(compared, 3, "d against a, b and c");
        assert_eq!(four.ids.len(), 4);
    }

    #[test]
    fn reused_pairs_give_the_edges_a_full_comparison_gives() {
        let vectors = (0..40)
            .map(|index| unit((index % 9) as f32 * 0.17 + index as f32 * 0.004))
            .collect::<Vec<_>>();
        let ids = (0..40).map(|index| format!("f{index}")).collect::<Vec<_>>();
        let stamps = |changed: Option<usize>| {
            (0..40)
                .map(|index| if Some(index) == changed { "2" } else { "1" })
                .collect::<Vec<_>>()
        };
        let build = |previous: Option<&PairSimilarity>, stamps: &[&str]| {
            let files = ids
                .iter()
                .zip(stamps)
                .zip(&vectors)
                .map(|((id, stamp), vector)| file(id, stamp, vector))
                .collect::<Vec<_>>();
            PairSimilarity::build(previous, &files)
        };
        let (cold, _) = build(None, &stamps(None));
        let (warm, compared) = build(Some(&cold), &stamps(Some(7)));
        assert_eq!(compared, 39);
        let describe = |pairs: &PairSimilarity| {
            edges_from_similarity(pairs)
                .into_iter()
                .map(|edge| (edge.source, edge.target))
                .collect::<Vec<_>>()
        };
        assert_eq!(describe(&warm), describe(&cold));
        assert!(!describe(&cold).is_empty());
    }
}
