//! Earlier versions of an uploaded file: their text, and what changed between
//! two of them. The versions themselves are kept by the storage engine when a
//! new one is uploaded; this reads them back.

use std::path::Path;

use anyhow::{Context, Result};
use repomemo_domain::{
    ArtifactSummary, ArtifactType, ArtifactVersion, ArtifactVersionText, DiffKind, VersionComparison, VersionDiffLine,
};
use repomemo_ingestion::{extract_document_text, is_document};

use crate::RepoMemoCore;

/// Characters of one version's text that are returned.
const VERSION_TEXT_CHARS: usize = 400_000;
/// Lines of each version that are compared.
const DIFF_MAX_LINES: usize = 4_000;
/// Cells of the comparison table; beyond it a changed middle is shown whole.
const DIFF_MAX_CELLS: usize = 4_000_000;
/// Unchanged lines kept around a change.
const DIFF_CONTEXT: usize = 3;

impl RepoMemoCore {
    pub async fn list_artifact_versions(&self, artifact_id: &str) -> Result<Vec<ArtifactVersion>> {
        self.storage.list_artifact_versions(artifact_id).await
    }

    /// Restores an earlier version of a file as a new version with that content (V0.3.8). The caller has
    /// already checked that the person may change the file; the file is indexed again by the caller.
    pub async fn restore_artifact_version(
        &self,
        artifact_id: &str,
        version: i64,
        restored_by: Option<&str>,
    ) -> Result<ArtifactSummary> {
        self.storage
            .restore_artifact_version(artifact_id, version, restored_by)
            .await
    }

    pub async fn artifact_version_text(&self, artifact_id: &str, version: i64) -> Result<ArtifactVersionText> {
        let (text, note) = self.version_text(artifact_id, version).await?;
        let (text, truncated) = match text {
            Some(text) if text.chars().count() > VERSION_TEXT_CHARS => {
                (Some(text.chars().take(VERSION_TEXT_CHARS).collect()), true)
            }
            other => (other, false),
        };
        Ok(ArtifactVersionText {
            version,
            text,
            truncated,
            note,
        })
    }

    pub async fn compare_artifact_versions(&self, artifact_id: &str, from: i64, to: i64) -> Result<VersionComparison> {
        let (older, newer) = if from <= to { (from, to) } else { (to, from) };
        let (old_text, old_note) = self.version_text(artifact_id, older).await?;
        let (new_text, new_note) = self.version_text(artifact_id, newer).await?;
        let (Some(old_text), Some(new_text)) = (old_text, new_text) else {
            return Ok(VersionComparison {
                from_version: older,
                to_version: newer,
                lines: Vec::new(),
                added: 0,
                removed: 0,
                truncated: false,
                note: old_note.or(new_note),
            });
        };
        let mut comparison = compare_texts(&old_text, &new_text);
        comparison.from_version = older;
        comparison.to_version = newer;
        if older == newer || (comparison.added == 0 && comparison.removed == 0 && !comparison.truncated) {
            comparison.note = Some("These two versions have the same text.".to_owned());
        }
        Ok(comparison)
    }

    /// The text of a version and, when it has none, why.
    async fn version_text(&self, artifact_id: &str, version: i64) -> Result<(Option<String>, Option<String>)> {
        let summary = self.storage.get_artifact_summary(artifact_id).await?;
        let versions = self.storage.list_artifact_versions(artifact_id).await?;
        let Some(found) = versions.into_iter().find(|candidate| candidate.version == version) else {
            repomemo_domain::bail_not_found!("That version of this file was not found.");
        };
        if matches!(summary.artifact_type, ArtifactType::Image) {
            return Ok((None, Some("Images have no text to show.".to_owned())));
        }
        let bytes = self.storage.read_blob(&found.content_hash).await.with_context(|| {
            format!(
                "The stored content of version {version} could not be read. It may have been removed with the server's old files."
            )
        })?;
        let path = summary.path.clone();
        if is_document(Path::new(&path)) {
            let extracted = tokio::task::spawn_blocking(move || extract_document_text(Path::new(&path), &bytes))
                .await
                .context("the document reading task failed")?;
            return Ok(match extracted {
                Ok(Some(text)) if !text.trim().is_empty() => (Some(text), None),
                _ => (
                    None,
                    Some("No text could be read from this version of the document.".to_owned()),
                ),
            });
        }
        if bytes.contains(&0) {
            return Ok((None, Some("This version is not a text file.".to_owned())));
        }
        Ok((Some(String::from_utf8_lossy(&bytes).into_owned()), None))
    }
}

/// Compares two texts line by line. The result has only the changed lines and
/// a few unchanged ones around each, with gaps in between.
pub(crate) fn compare_texts(old_text: &str, new_text: &str) -> VersionComparison {
    let mut old_lines = old_text.lines().collect::<Vec<_>>();
    let mut new_lines = new_text.lines().collect::<Vec<_>>();
    let truncated = old_lines.len() > DIFF_MAX_LINES || new_lines.len() > DIFF_MAX_LINES;
    old_lines.truncate(DIFF_MAX_LINES);
    new_lines.truncate(DIFF_MAX_LINES);

    let operations = diff_lines(&old_lines, &new_lines);
    let added = operations
        .iter()
        .filter(|(kind, _, _)| *kind == DiffKind::Added)
        .count() as i64;
    let removed = operations
        .iter()
        .filter(|(kind, _, _)| *kind == DiffKind::Removed)
        .count() as i64;

    let mut lines = Vec::new();
    let mut index = 0;
    while index < operations.len() {
        if operations[index].0 != DiffKind::Same {
            lines.push(line_of(&operations[index], &old_lines, &new_lines));
            index += 1;
            continue;
        }
        let start = index;
        while index < operations.len() && operations[index].0 == DiffKind::Same {
            index += 1;
        }
        let run = &operations[start..index];
        let leading = if start == 0 { 0 } else { DIFF_CONTEXT };
        let trailing = if index == operations.len() { 0 } else { DIFF_CONTEXT };
        if run.len() <= leading + trailing + 1 {
            for operation in run {
                lines.push(line_of(operation, &old_lines, &new_lines));
            }
            continue;
        }
        for operation in &run[..leading] {
            lines.push(line_of(operation, &old_lines, &new_lines));
        }
        let hidden = run.len() - leading - trailing;
        lines.push(VersionDiffLine {
            kind: DiffKind::Gap,
            text: format!("{hidden} unchanged {}", if hidden == 1 { "line" } else { "lines" }),
            old_line: None,
            new_line: None,
        });
        for operation in &run[run.len() - trailing..] {
            lines.push(line_of(operation, &old_lines, &new_lines));
        }
    }
    VersionComparison {
        from_version: 0,
        to_version: 0,
        lines,
        added,
        removed,
        truncated,
        note: None,
    }
}

fn line_of(operation: &(DiffKind, Option<usize>, Option<usize>), old: &[&str], new: &[&str]) -> VersionDiffLine {
    let (kind, old_index, new_index) = operation;
    let text = match (kind, old_index, new_index) {
        (DiffKind::Added, _, Some(index)) => new[*index],
        (_, Some(index), _) => old[*index],
        _ => "",
    };
    VersionDiffLine {
        kind: *kind,
        text: text.to_owned(),
        old_line: old_index.map(|index| index as i64 + 1),
        new_line: new_index.map(|index| index as i64 + 1),
    }
}

/// An edit script: `(kind, index in old, index in new)` for every line.
fn diff_lines(old: &[&str], new: &[&str]) -> Vec<(DiffKind, Option<usize>, Option<usize>)> {
    let mut prefix = 0;
    while prefix < old.len() && prefix < new.len() && old[prefix] == new[prefix] {
        prefix += 1;
    }
    let mut suffix = 0;
    while suffix < old.len() - prefix
        && suffix < new.len() - prefix
        && old[old.len() - 1 - suffix] == new[new.len() - 1 - suffix]
    {
        suffix += 1;
    }
    let old_middle = &old[prefix..old.len() - suffix];
    let new_middle = &new[prefix..new.len() - suffix];

    let mut operations = Vec::with_capacity(old.len().max(new.len()));
    for index in 0..prefix {
        operations.push((DiffKind::Same, Some(index), Some(index)));
    }
    let (rows, columns) = (old_middle.len(), new_middle.len());
    if rows * columns > DIFF_MAX_CELLS {
        // Too tangled to align cheaply: show the middle as replaced.
        for index in 0..rows {
            operations.push((DiffKind::Removed, Some(prefix + index), None));
        }
        for index in 0..columns {
            operations.push((DiffKind::Added, None, Some(prefix + index)));
        }
    } else {
        // Longest common subsequence of the middles.
        let width = columns + 1;
        let mut table = vec![0_u32; (rows + 1) * width];
        for row in (0..rows).rev() {
            for column in (0..columns).rev() {
                table[row * width + column] = if old_middle[row] == new_middle[column] {
                    table[(row + 1) * width + column + 1] + 1
                } else {
                    table[(row + 1) * width + column].max(table[row * width + column + 1])
                };
            }
        }
        let (mut row, mut column) = (0, 0);
        while row < rows && column < columns {
            if old_middle[row] == new_middle[column] {
                operations.push((DiffKind::Same, Some(prefix + row), Some(prefix + column)));
                row += 1;
                column += 1;
            } else if table[(row + 1) * width + column] >= table[row * width + column + 1] {
                operations.push((DiffKind::Removed, Some(prefix + row), None));
                row += 1;
            } else {
                operations.push((DiffKind::Added, None, Some(prefix + column)));
                column += 1;
            }
        }
        while row < rows {
            operations.push((DiffKind::Removed, Some(prefix + row), None));
            row += 1;
        }
        while column < columns {
            operations.push((DiffKind::Added, None, Some(prefix + column)));
            column += 1;
        }
    }
    for index in 0..suffix {
        operations.push((
            DiffKind::Same,
            Some(old.len() - suffix + index),
            Some(new.len() - suffix + index),
        ));
    }
    operations
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(comparison: &VersionComparison) -> Vec<(DiffKind, String)> {
        comparison
            .lines
            .iter()
            .map(|line| (line.kind, line.text.clone()))
            .collect()
    }

    #[test]
    fn a_changed_line_is_one_removal_and_one_addition() {
        let comparison = compare_texts("one\ntwo\nthree\n", "one\n2\nthree\n");
        assert_eq!((comparison.added, comparison.removed), (1, 1));
        assert_eq!(
            kinds(&comparison),
            [
                (DiffKind::Same, "one".to_owned()),
                (DiffKind::Removed, "two".to_owned()),
                (DiffKind::Added, "2".to_owned()),
                (DiffKind::Same, "three".to_owned()),
            ]
        );
        let removed = &comparison.lines[1];
        assert_eq!((removed.old_line, removed.new_line), (Some(2), None));
        let added = &comparison.lines[2];
        assert_eq!((added.old_line, added.new_line), (None, Some(2)));
    }

    #[test]
    fn lines_added_and_removed_around_unchanged_ones_keep_their_numbers() {
        let comparison = compare_texts("a\nb\nc\nd\n", "x\nb\nc\nd\ne\n");
        assert_eq!((comparison.added, comparison.removed), (2, 1));
        let last = comparison.lines.last().unwrap();
        assert_eq!(
            (last.kind, last.text.as_str(), last.new_line),
            (DiffKind::Added, "e", Some(5))
        );
        let same = comparison.lines.iter().find(|line| line.text == "d").unwrap();
        assert_eq!((same.old_line, same.new_line), (Some(4), Some(4)));
    }

    #[test]
    fn long_unchanged_stretches_become_a_gap_with_context_kept() {
        let old = (1..=40)
            .map(|line| format!("line {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let new = old.replace("line 20", "line twenty");
        let comparison = compare_texts(&old, &new);
        assert_eq!((comparison.added, comparison.removed), (1, 1));
        let texts = kinds(&comparison);
        assert_eq!(texts.iter().filter(|(kind, _)| *kind == DiffKind::Gap).count(), 2);
        // Three lines of context on each side of the change, and nothing from far away.
        assert!(texts.iter().any(|(_, text)| text == "line 17"));
        assert!(texts.iter().any(|(_, text)| text == "line 23"));
        assert!(!texts.iter().any(|(_, text)| text == "line 5" || text == "line 35"));
        assert!(texts.first().map(|(kind, _)| *kind) == Some(DiffKind::Gap));
    }

    #[test]
    fn identical_texts_have_no_changes() {
        let comparison = compare_texts("same\ntext\n", "same\ntext\n");
        assert_eq!((comparison.added, comparison.removed), (0, 0));
        assert!(comparison
            .lines
            .iter()
            .all(|line| line.kind != DiffKind::Added && line.kind != DiffKind::Removed));
    }

    #[test]
    fn very_long_files_are_compared_by_their_first_lines() {
        let old = (0..DIFF_MAX_LINES + 50)
            .map(|line| format!("old {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let new = (0..DIFF_MAX_LINES + 50)
            .map(|line| format!("new {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let comparison = compare_texts(&old, &new);
        assert!(comparison.truncated);
        // Too different to align: shown as replaced rather than computed.
        assert_eq!(comparison.removed, DIFF_MAX_LINES as i64);
        assert_eq!(comparison.added, DIFF_MAX_LINES as i64);
    }
}
