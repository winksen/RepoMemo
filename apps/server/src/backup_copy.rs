//! A second copy of each backup, somewhere else (roadmap O6b).
//!
//! A backup in the data folder dies with the disk. `REPOMEMO_BACKUP_COPY_DIR`
//! names another folder, such as a mounted network share, and after each
//! backup the server copies it there and **checks the copy** before calling it
//! done.
//!
//! * The copy is written under `<name>.partial` and renamed when it is whole,
//!   so a listing of the destination never shows half a backup.
//! * Every file is hashed (SHA-256) as it is read, written, and read again
//!   from the destination; the hashes are written to `SHA256SUMS` in the
//!   standard format, so `sha256sum -c SHA256SUMS` repeats the check later.
//! * The totals (files and bytes) must equal the ones in the backup's
//!   manifest.
//! * Backups share unchanged files with each other through hard links. The
//!   copy keeps that: a file whose hash is the same as in the previous copy is
//!   linked to it instead of sent again (a share that cannot link gets a
//!   normal copy).

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use ring::digest::{Context as Digest, SHA256};

pub(crate) const SUMS_FILE: &str = "SHA256SUMS";
const MANIFEST_FILE: &str = "manifest.json";
const PARTIAL_SUFFIX: &str = ".partial";
const BACKUP_PREFIX: &str = "backup-";

/// What one copy did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CopyReport {
    pub files: u64,
    pub bytes: u64,
    /// Files linked to the previous copy instead of sent again.
    pub linked: u64,
}

/// The files of a backup folder (everything but the manifest) as
/// forward-slash relative paths, sorted.
fn backup_files(folder: &Path) -> Result<Vec<String>> {
    let mut found = Vec::new();
    let mut pending = vec![PathBuf::new()];
    while let Some(relative) = pending.pop() {
        for entry in std::fs::read_dir(folder.join(&relative))
            .with_context(|| format!("{} could not be read", folder.join(&relative).display()))?
        {
            let entry = entry?;
            let child = relative.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                pending.push(child);
            } else if child != Path::new(MANIFEST_FILE) && child != Path::new(SUMS_FILE) {
                found.push(child.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    found.sort();
    Ok(found)
}

/// How many files a backup folder holds (the manifest excepted) and their
/// total size: what its manifest records, and what a copy must add up to.
pub(crate) fn folder_totals(folder: &Path) -> Result<(u64, u64)> {
    let mut bytes = 0;
    let files = backup_files(folder)?;
    for relative in &files {
        bytes += std::fs::metadata(folder.join(relative))?.len();
    }
    Ok((files.len() as u64, bytes))
}

/// SHA-256 (hex) and size of a file, read in pieces.
fn hash_file(path: &Path) -> Result<(String, u64)> {
    let mut input = BufReader::new(File::open(path).with_context(|| format!("{} could not be read", path.display()))?);
    let mut digest = Digest::new(&SHA256);
    let mut buffer = vec![0u8; 256 * 1024];
    let mut size = 0_u64;
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
        size += read as u64;
    }
    Ok((hex::encode(digest.finish().as_ref()), size))
}

/// Copies `from` to `to` and returns the SHA-256 of what was read.
fn copy_hashing(from: &Path, to: &Path) -> Result<(String, u64)> {
    let mut input = BufReader::new(File::open(from).with_context(|| format!("{} could not be read", from.display()))?);
    let mut output =
        BufWriter::new(File::create(to).with_context(|| format!("{} could not be written", to.display()))?);
    let mut digest = Digest::new(&SHA256);
    let mut buffer = vec![0u8; 256 * 1024];
    let mut size = 0_u64;
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
        output.write_all(&buffer[..read])?;
        size += read as u64;
    }
    output.flush()?;
    output.get_ref().sync_all().ok();
    Ok((hex::encode(digest.finish().as_ref()), size))
}

fn parse_sums(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|line| {
            let (hash, path) = line.split_once("  ")?;
            (hash.len() == 64).then(|| (path.to_owned(), hash.to_owned()))
        })
        .collect()
}

fn valid_name(name: &str) -> bool {
    name.starts_with(BACKUP_PREFIX)
        && name.len() <= 64
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

/// Complete copies in a destination folder, newest first: those with a
/// manifest and their checksums.
fn complete_copies(root: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut names = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| valid_name(name))
        .filter(|name| root.join(name).join(MANIFEST_FILE).is_file() && root.join(name).join(SUMS_FILE).is_file())
        .collect::<Vec<_>>();
    names.sort_by(|left, right| right.cmp(left));
    names
}

fn rename_with_retries(from: &Path, to: &Path) -> std::io::Result<()> {
    let mut attempt = 0;
    loop {
        match std::fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(error) if attempt < 25 => {
                tracing::debug!(error = %error, attempt, "A copied folder could not be renamed yet");
                attempt += 1;
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            Err(error) => return Err(error),
        }
    }
}

/// Copies the backup folder `source` into `destination_root` and checks the
/// copy. `expected` is the manifest's file count and size (`None` for a backup
/// made before manifests carried them).
pub(crate) fn copy_backup(source: &Path, destination_root: &Path, expected: Option<(u64, u64)>) -> Result<CopyReport> {
    let name = source
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| valid_name(name))
        .context("that is not a backup folder")?;
    std::fs::create_dir_all(destination_root)
        .with_context(|| format!("{} could not be created", destination_root.display()))?;
    let destination_root = destination_root
        .canonicalize()
        .unwrap_or_else(|_| destination_root.to_path_buf());
    if let Ok(source) = source.canonicalize() {
        if destination_root.starts_with(&source) || source.starts_with(&destination_root) {
            bail!("The folder for second copies must be somewhere else than the backups themselves.");
        }
    }
    let final_folder = destination_root.join(&name);
    if final_folder.exists() {
        // Already there: trust it only after checking it again, else send it afresh.
        match verify_copy(&final_folder) {
            Ok(report) => return Ok(report),
            Err(error) => {
                tracing::warn!(backup = %name, error = %format!("{error:#}"), "A second copy did not check out and is sent again");
                std::fs::remove_dir_all(&final_folder)
                    .with_context(|| format!("{} could not be replaced", final_folder.display()))?;
            }
        }
    }
    let partial = destination_root.join(format!("{name}{PARTIAL_SUFFIX}"));
    if partial.exists() {
        std::fs::remove_dir_all(&partial).with_context(|| format!("{} could not be cleared", partial.display()))?;
    }
    std::fs::create_dir_all(&partial)?;
    let result = copy_files_into(source, &destination_root, &partial, &name, expected);
    match result {
        Ok(report) => {
            rename_with_retries(&partial, &final_folder)
                .with_context(|| format!("the copy could not be put in place at {}", final_folder.display()))?;
            Ok(report)
        }
        Err(error) => {
            let _ = std::fs::remove_dir_all(&partial);
            Err(error)
        }
    }
}

fn copy_files_into(
    source: &Path,
    destination_root: &Path,
    partial: &Path,
    name: &str,
    expected: Option<(u64, u64)>,
) -> Result<CopyReport> {
    // What the previous copy holds, for linking.
    let previous = complete_copies(destination_root)
        .into_iter()
        .find(|previous| previous.as_str() < name)
        .map(|previous| {
            let folder = destination_root.join(&previous);
            let sums = std::fs::read_to_string(folder.join(SUMS_FILE))
                .map(|text| parse_sums(&text))
                .unwrap_or_default();
            (folder, sums)
        });

    let mut report = CopyReport::default();
    let mut sums = BTreeMap::new();
    for relative in backup_files(source)? {
        let from = source.join(&relative);
        let to = partial.join(&relative);
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let candidate = previous.as_ref().and_then(|(folder, known)| {
            let hash = known.get(&relative)?;
            Some((folder.join(&relative), hash))
        });
        let mut linked = false;
        let (hash, size) = match candidate {
            Some((earlier, known_hash)) => {
                let (hash, size) = hash_file(&from)?;
                if &hash == known_hash && std::fs::hard_link(&earlier, &to).is_ok() {
                    linked = true;
                    (hash, size)
                } else {
                    let (copied_hash, copied_size) = copy_hashing(&from, &to)?;
                    if copied_hash != hash {
                        bail!("{relative} changed while it was being copied");
                    }
                    (copied_hash, copied_size)
                }
            }
            None => copy_hashing(&from, &to)?,
        };
        if !linked {
            // Read back from the destination, not from what was written.
            let (written_hash, written_size) = hash_file(&to)?;
            if written_hash != hash || written_size != size {
                bail!("{relative} does not match its source after being copied");
            }
        } else {
            report.linked += 1;
        }
        report.files += 1;
        report.bytes += size;
        sums.insert(relative, hash);
    }
    if let Some((files, bytes)) = expected {
        if files != report.files || bytes != report.bytes {
            bail!(
                "The copy holds {} files and {} bytes, but the backup's manifest says {} and {}.",
                report.files,
                report.bytes,
                files,
                bytes
            );
        }
    }
    let mut text = String::new();
    for (path, hash) in &sums {
        text.push_str(&format!("{hash}  {path}\n"));
    }
    std::fs::write(partial.join(SUMS_FILE), text)?;
    std::fs::copy(source.join(MANIFEST_FILE), partial.join(MANIFEST_FILE))
        .context("the manifest could not be copied")?;
    Ok(report)
}

/// Checks a copy again against its `SHA256SUMS`: every listed file present
/// with its hash, and nothing else in the folder.
pub(crate) fn verify_copy(folder: &Path) -> Result<CopyReport> {
    let sums = parse_sums(
        &std::fs::read_to_string(folder.join(SUMS_FILE))
            .with_context(|| format!("{} has no {SUMS_FILE}", folder.display()))?,
    );
    let present = backup_files(folder)?;
    if present.len() != sums.len() || present.iter().any(|path| !sums.contains_key(path)) {
        bail!("the files in the copy are not the ones its checksums list");
    }
    let mut report = CopyReport::default();
    for (path, hash) in &sums {
        let (found, size) = hash_file(&folder.join(path))?;
        if &found != hash {
            bail!("{path} does not match its checksum");
        }
        report.files += 1;
        report.bytes += size;
    }
    Ok(report)
}

/// Deletes the oldest copies beyond `keep` and unfinished ones; returns how
/// many complete copies were deleted.
pub(crate) fn prune_copies(root: &Path, keep: usize) -> usize {
    let mut deleted = 0;
    for old in complete_copies(root).into_iter().skip(keep.max(1)) {
        if std::fs::remove_dir_all(root.join(&old)).is_ok() {
            deleted += 1;
        } else {
            tracing::warn!(copy = %old, "An old second copy could not be deleted");
        }
    }
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().ends_with(PARTIAL_SUFFIX) {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }
    deleted
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "repomemo-copy-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn backup(root: &Path, name: &str, files: &[(&str, &[u8])]) -> PathBuf {
        let folder = root.join(name);
        for (path, bytes) in files {
            let target = folder.join(path);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::write(target, bytes).unwrap();
        }
        std::fs::write(folder.join(MANIFEST_FILE), b"{}").unwrap();
        folder
    }

    #[test]
    fn a_copy_is_whole_checked_and_listed_with_its_checksums() {
        let (backups, copies) = (folder("src"), folder("dst"));
        let first = backup(
            &backups,
            "backup-20261001-000000",
            &[
                ("repomemo.sqlite", b"database one"),
                ("blobs/ab/same", b"unchanged"),
                ("blobs/ab/empty", b""),
            ],
        );
        let report = copy_backup(&first, &copies, Some((3, 21))).unwrap();
        assert_eq!((report.files, report.bytes, report.linked), (3, 21, 0));
        let done = copies.join("backup-20261001-000000");
        assert_eq!(std::fs::read(done.join("blobs/ab/same")).unwrap(), b"unchanged");
        assert!(done.join(MANIFEST_FILE).is_file());
        let sums = std::fs::read_to_string(done.join(SUMS_FILE)).unwrap();
        assert_eq!(sums.lines().count(), 3);
        assert!(sums.contains("  blobs/ab/same"), "{sums}");
        assert_eq!(verify_copy(&done).unwrap().files, 3);
        assert!(!copies.join("backup-20261001-000000.partial").exists());

        // Copying again changes nothing.
        assert_eq!(copy_backup(&first, &copies, None).unwrap().files, 3);

        // A file changed behind the copy's back is found.
        std::fs::write(done.join("blobs/ab/same"), b"tampered!").unwrap();
        assert!(verify_copy(&done).unwrap_err().to_string().contains("blobs/ab/same"));
        for path in [backups, copies] {
            let _ = std::fs::remove_dir_all(path);
        }
    }

    #[test]
    fn a_copy_that_does_not_add_up_to_the_manifest_is_refused_and_leaves_nothing() {
        let (backups, copies) = (folder("short-src"), folder("short-dst"));
        let first = backup(&backups, "backup-20261001-000000", &[("repomemo.sqlite", b"database")]);
        let error = copy_backup(&first, &copies, Some((2, 99))).unwrap_err();
        assert!(error.to_string().contains("manifest says 2 and 99"), "{error}");
        assert!(!copies.join("backup-20261001-000000").exists());
        assert!(!copies.join("backup-20261001-000000.partial").exists());
        for path in [backups, copies] {
            let _ = std::fs::remove_dir_all(path);
        }
    }

    #[test]
    fn unchanged_files_are_linked_to_the_previous_copy_and_changed_ones_are_not() {
        let (backups, copies) = (folder("link-src"), folder("link-dst"));
        let first = backup(
            &backups,
            "backup-20261001-000000",
            &[("repomemo.sqlite", b"database one"), ("blobs/ab/same", b"unchanged")],
        );
        let second = backup(
            &backups,
            "backup-20261002-000000",
            &[
                ("repomemo.sqlite", b"database two"),
                ("blobs/ab/same", b"unchanged"),
                ("blobs/cd/new", b"new file"),
            ],
        );
        copy_backup(&first, &copies, None).unwrap();
        let report = copy_backup(&second, &copies, Some((3, 12 + 9 + 8))).unwrap();
        assert_eq!((report.files, report.linked), (3, 1));
        let done = copies.join("backup-20261002-000000");
        assert_eq!(std::fs::read(done.join("repomemo.sqlite")).unwrap(), b"database two");
        assert_eq!(verify_copy(&done).unwrap().files, 3);
        assert_eq!(verify_copy(&copies.join("backup-20261001-000000")).unwrap().files, 2);
        for path in [backups, copies] {
            let _ = std::fs::remove_dir_all(path);
        }
    }

    #[test]
    fn a_destination_inside_the_backups_is_refused_and_old_copies_are_pruned() {
        let backups = folder("inside");
        let first = backup(&backups, "backup-20261001-000000", &[("repomemo.sqlite", b"database")]);
        assert!(copy_backup(&first, &first.join("copies"), None).is_err());
        assert!(copy_backup(&first, &backups, None).is_err());

        let (source, copies) = (folder("prune-src"), folder("prune-dst"));
        for day in 1..=4 {
            let name = format!("backup-2026100{day}-000000");
            let made = backup(
                &source,
                &name,
                &[("repomemo.sqlite", format!("database {day}").as_bytes())],
            );
            copy_backup(&made, &copies, None).unwrap();
        }
        std::fs::create_dir_all(copies.join("backup-20261005-000000.partial")).unwrap();
        assert_eq!(prune_copies(&copies, 2), 2);
        let mut left = complete_copies(&copies);
        left.sort();
        assert_eq!(left, ["backup-20261003-000000", "backup-20261004-000000"]);
        assert!(!copies.join("backup-20261005-000000.partial").exists());
        for path in [backups, source, copies] {
            let _ = std::fs::remove_dir_all(path);
        }
    }
}
