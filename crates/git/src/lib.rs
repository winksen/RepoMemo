//! Read-only access to git repositories through the `git` command line.
//!
//! RepoMemo indexes what a repository has committed, not its working tree, so
//! everything here reads objects (commits, trees, blobs) and never touches the
//! index, the work tree or refs. The `git` CLI is used instead of a library so
//! that the user's own git setup (credential helpers, `safe.directory`, long
//! paths on Windows) applies unchanged.
//!
//! A server runs git on folders that other people chose, so it calls
//! [`harden`] once: git then ignores the system and global configuration,
//! allows no transport protocol and never recurses into submodules (review
//! L8). Only the folders given to `harden` are exempt from git's
//! `safe.directory` ownership check, which would otherwise have come from the
//! global configuration.

pub mod remote;

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::OnceLock;

use anyhow::{anyhow, bail, Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

/// The commit that last changed a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastChange {
    pub sha: String,
    pub author_name: String,
    /// The author date in strict ISO 8601.
    pub authored_at: String,
}

/// A file tracked in a commit's tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeFile {
    /// Path relative to the repository root, always with `/` separators.
    pub path: String,
    /// Object id of the file content. Equal ids mean equal content.
    pub blob_sha: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitInfo {
    pub sha: String,
    pub summary: String,
    pub author_name: String,
    /// Committer date in strict ISO 8601.
    pub committed_at: String,
}

#[derive(Debug, Clone)]
pub struct GitRepo {
    root: PathBuf,
    /// A bare, partial clone of a remote repository (see [`remote`]): blobs
    /// over the limit are not here, and sizes are read without fetching them.
    partial: bool,
}

impl GitRepo {
    /// Opens the repository containing `path`. Fails with a readable message
    /// when git is not installed or the path is not inside a repository.
    pub async fn open(path: &Path) -> Result<Self> {
        if !path.is_dir() {
            repomemo_domain::bail_invalid!("The folder {} does not exist.", path.display());
        }
        let output = git_command(path)
            .args(["rev-parse", "--show-toplevel"])
            .output()
            .await
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    repomemo_domain::not_found!("git was not found. Install git and make sure it is on the PATH.")
                } else {
                    anyhow!("git could not be started: {error}")
                }
            })?;
        if !output.status.success() {
            let detail = stderr_text(&output.stderr);
            if detail.contains("dubious ownership") {
                if HARDENING.get().is_some() {
                    repomemo_domain::bail_invalid!(
                        "git refuses to read {} because it belongs to another user account than the server's. Ask the server operator to list its folder in REPOMEMO_GIT_SAFE_DIRECTORIES, or to give the server's account ownership of it.",
                        path.display()
                    );
                }
                repomemo_domain::bail_invalid!(
                    "git refuses to read {} because it belongs to another user account than the server's. On the server, run: git config --global --add safe.directory \"{}\"",
                    path.display(),
                    path.display().to_string().replace('\\', "/")
                );
            }
            repomemo_domain::bail_invalid!("{} is not inside a git repository: {detail}", path.display());
        }
        let root = String::from_utf8(output.stdout)
            .context("git returned a non UTF-8 repository path")?
            .trim()
            .to_owned();
        if root.is_empty() {
            repomemo_domain::bail_invalid!("{} is a bare repository; choose a checkout instead.", path.display());
        }
        Ok(Self {
            root: PathBuf::from(root),
            partial: false,
        })
    }

    /// Opens the bare, partial clone the server keeps for a remote link. It is
    /// read like any other repository, but never reaches the network: the
    /// objects it needs are there, and sizes of the ones that are not are
    /// reported as unknown.
    pub async fn open_bare(path: &Path) -> Result<Self> {
        if !path.is_dir() {
            repomemo_domain::bail_not_found!("The server's copy of this repository is not there yet.");
        }
        let output = git_command(path)
            .args(["rev-parse", "--is-bare-repository"])
            .output()
            .await
            .context("git could not be started")?;
        if !output.status.success() || String::from_utf8_lossy(&output.stdout).trim() != "true" {
            bail!(
                "the server's copy of this repository is not usable: {}",
                stderr_text(&output.stderr)
            );
        }
        Ok(Self {
            root: path.to_path_buf(),
            partial: true,
        })
    }

    /// The repository root as git reports it (forward slashes on Windows).
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The branch HEAD points at, or `None` for a detached HEAD.
    pub async fn current_branch(&self) -> Result<Option<String>> {
        let output = git_command(&self.root)
            .args(["symbolic-ref", "--quiet", "--short", "HEAD"])
            .output()
            .await?;
        if !output.status.success() {
            return Ok(None);
        }
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        Ok((!branch.is_empty()).then_some(branch))
    }

    /// Resolves a local branch name, or HEAD when `branch` is `None`, to a
    /// commit id.
    pub async fn resolve_commit(&self, branch: Option<&str>) -> Result<String> {
        let revision = match branch {
            Some(branch) => {
                validate_branch_name(branch)?;
                format!("refs/heads/{branch}^{{commit}}")
            }
            None => "HEAD^{commit}".to_owned(),
        };
        let output = self
            .run(&["rev-parse", "--verify", "--quiet", "--end-of-options", &revision])
            .await;
        match output {
            Ok(sha) if !sha.trim().is_empty() => Ok(sha.trim().to_owned()),
            _ => match branch {
                Some(branch) => {
                    repomemo_domain::bail_invalid!("The branch {branch} does not exist in this repository.")
                }
                None => repomemo_domain::bail_invalid!("This repository has no commits yet."),
            },
        }
    }

    pub async fn commit_info(&self, commit: &str) -> Result<CommitInfo> {
        let output = self
            .run(&["log", "-1", "--format=%H%x00%s%x00%an%x00%cI", commit, "--"])
            .await?;
        let mut parts = output.trim_end_matches('\n').split('\0');
        let mut next = || parts.next().unwrap_or_default().to_owned();
        Ok(CommitInfo {
            sha: next(),
            summary: next(),
            author_name: next(),
            committed_at: next(),
        })
    }

    /// The latest `limit` commits reachable from `commit`, newest first.
    pub async fn recent_commits(&self, commit: &str, limit: usize) -> Result<Vec<CommitInfo>> {
        let output = self
            .run(&[
                "log",
                &format!("--max-count={limit}"),
                "--format=%H%x00%s%x00%an%x00%cI%x1e",
                commit,
                "--",
            ])
            .await?;
        Ok(output
            .split('\u{1e}')
            .map(|record| record.trim_matches('\n'))
            .filter(|record| !record.is_empty())
            .map(|record| {
                let mut parts = record.split('\0');
                let mut next = || parts.next().unwrap_or_default().to_owned();
                CommitInfo {
                    sha: next(),
                    summary: next(),
                    author_name: next(),
                    committed_at: next(),
                }
            })
            .collect())
    }

    /// Every regular file in the commit's tree. Symbolic links and submodules
    /// are left out because they have no content of their own to index.
    pub async fn list_files(&self, commit: &str) -> Result<Vec<TreeFile>> {
        if self.partial {
            return self.list_files_partial(commit).await;
        }
        let output = git_command(&self.root)
            .args(["ls-tree", "-r", "-z", "-l", "--full-tree", commit])
            .output()
            .await?;
        if !output.status.success() {
            bail!("git ls-tree failed: {}", stderr_text(&output.stderr));
        }
        parse_ls_tree(&output.stdout)
    }

    /// The same for a partial clone: names first, then sizes from the object
    /// store with no fetching. A blob that was left on the host has an unknown
    /// size, reported as the largest possible so the sync skips it as too big.
    async fn list_files_partial(&self, commit: &str) -> Result<Vec<TreeFile>> {
        let output = git_command(&self.root)
            .args(["ls-tree", "-r", "-z", "--full-tree", commit])
            .output()
            .await?;
        if !output.status.success() {
            bail!("git ls-tree failed: {}", stderr_text(&output.stderr));
        }
        let mut entries = Vec::new();
        for record in output.stdout.split(|byte| *byte == 0).filter(|record| !record.is_empty()) {
            let record = std::str::from_utf8(record).context("git returned a non UTF-8 path")?;
            // "<mode> <type> <object>\t<path>"
            let (meta, path) = record
                .split_once('\t')
                .ok_or_else(|| anyhow!("unexpected git ls-tree line: {record}"))?;
            let fields = meta.split_whitespace().collect::<Vec<_>>();
            let [mode, kind, object] = fields.as_slice() else {
                bail!("unexpected git ls-tree line: {record}");
            };
            if *kind == "blob" && matches!(*mode, "100644" | "100755") {
                entries.push((path.to_owned(), (*object).to_owned()));
            }
        }
        if entries.is_empty() {
            return Ok(Vec::new());
        }
        let mut child = git_command(&self.root)
            .args(["cat-file", "--batch-check"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .context("failed to start git cat-file")?;
        let mut stdin = child.stdin.take().context("git cat-file has no stdin")?;
        let objects = entries.iter().map(|(_, object)| object.clone()).collect::<Vec<_>>();
        let writer = tokio::spawn(async move {
            for object in objects {
                if stdin.write_all(format!("{object}\n").as_bytes()).await.is_err() {
                    break;
                }
            }
            let _ = stdin.shutdown().await;
        });
        let mut reader = BufReader::new(child.stdout.take().context("git cat-file has no stdout")?);
        let mut files = Vec::with_capacity(entries.len());
        for (path, object) in entries {
            let mut line = String::new();
            reader.read_line(&mut line).await?;
            // "<sha> <type> <size>" or "<sha> missing"
            let size = match line.split_whitespace().collect::<Vec<_>>().as_slice() {
                [_, "blob", size] => size.parse().unwrap_or(u64::MAX),
                _ => u64::MAX,
            };
            files.push(TreeFile {
                path,
                blob_sha: object,
                size_bytes: size,
            });
        }
        let _ = writer.await;
        let _ = child.wait().await;
        Ok(files)
    }

    /// Who last changed each file, from one `git log` pass over the latest
    /// `max_commits` commits reachable from `commit`: the newest commit that
    /// lists a path wins. Files untouched in that window are not in the map.
    /// Renames count as a change of the new path (no similarity detection).
    pub async fn last_changes(
        &self,
        commit: &str,
        max_commits: usize,
    ) -> Result<std::collections::HashMap<String, LastChange>> {
        if commit.is_empty() || !commit.chars().all(|value| value.is_ascii_hexdigit()) {
            bail!("{commit:?} is not a commit id.");
        }
        let output = git_command(&self.root)
            .args([
                "log",
                "--name-only",
                "--no-renames",
                "--no-color",
                &format!("--max-count={max_commits}"),
                "--format=%x1e%H%x1f%an%x1f%aI",
                commit,
                "--",
            ])
            .output()
            .await?;
        if !output.status.success() {
            bail!("git log failed: {}", stderr_text(&output.stderr));
        }
        Ok(parse_last_changes(&String::from_utf8_lossy(&output.stdout)))
    }

    /// The content of `path` as it was at `commit`, without reading the work
    /// tree. Fails with a plain message when the commit or the file is not
    /// there (a force-push can drop commits, and a partial clone does not hold
    /// blobs over its size limit).
    pub async fn file_at(&self, commit: &str, path: &str, max_bytes: u64) -> Result<Vec<u8>> {
        if commit.len() < 7 || commit.len() > 64 || !commit.chars().all(|value| value.is_ascii_hexdigit()) {
            repomemo_domain::bail_invalid!("{commit:?} is not a commit id.");
        }
        if path.is_empty() || path.contains(['\0', '\n', '\r']) {
            repomemo_domain::bail_invalid!("That is not a file path.");
        }
        let spec = format!("{commit}:{path}");
        let size = git_command(&self.root).args(["cat-file", "-s", &spec]).output().await?;
        if !size.status.success() {
            let detail = stderr_text(&size.stderr);
            if detail.contains("exists on disk") || detail.contains("does not exist") || detail.contains("not in") {
                repomemo_domain::bail_not_found!("This file did not exist at that commit.");
            }
            repomemo_domain::bail_not_found!(
                "That commit is not in the server's copy of the repository any more (it may have been force-pushed away)."
            );
        }
        let size = String::from_utf8_lossy(&size.stdout).trim().parse::<u64>().unwrap_or(u64::MAX);
        if size > max_bytes {
            repomemo_domain::bail_invalid!("This file is too large to show at a commit.");
        }
        let content = git_command(&self.root).args(["cat-file", "blob", &spec]).output().await?;
        if !content.status.success() {
            repomemo_domain::bail_not_found!(
                "The content of this file at that commit is not held by the server (it was too large to copy)."
            );
        }
        Ok(content.stdout)
    }

    /// Files renamed between two commits, as `(old path, new path)`, found by
    /// content similarity (`git diff -M`): a file renamed *and* edited counts
    /// while about half of it is unchanged, which comparing blob ids cannot see.
    pub async fn renamed_files(&self, from: &str, to: &str) -> Result<Vec<(String, String)>> {
        for revision in [from, to] {
            if revision.is_empty() || !revision.chars().all(|value| value.is_ascii_hexdigit()) {
                bail!("{revision:?} is not a commit id.");
            }
        }
        let output = git_command(&self.root)
            .args([
                "diff",
                "--name-status",
                "-z",
                "-M50%",
                "--diff-filter=R",
                from,
                to,
                "--",
            ])
            .output()
            .await?;
        if !output.status.success() {
            bail!("git diff failed: {}", stderr_text(&output.stderr));
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let mut parts = text.split('\0').filter(|part| !part.is_empty());
        let mut renames = Vec::new();
        while let Some(status) = parts.next() {
            let (Some(old), Some(new)) = (parts.next(), parts.next()) else {
                break;
            };
            if status.starts_with('R') {
                renames.push((old.to_owned(), new.to_owned()));
            }
        }
        Ok(renames)
    }

    /// Starts a reader that streams blob contents from one long-lived
    /// `git cat-file` process, which is much faster than one process per file.
    pub async fn blob_reader(&self) -> Result<BlobReader> {
        let mut child = git_command(&self.root)
            .args(["cat-file", "--batch"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .context("failed to start git cat-file")?;
        let stdin = child.stdin.take().context("git cat-file has no stdin")?;
        let stdout = child.stdout.take().context("git cat-file has no stdout")?;
        Ok(BlobReader {
            _child: child,
            stdin,
            stdout: BufReader::new(stdout),
        })
    }

    async fn run(&self, args: &[&str]) -> Result<String> {
        let output = git_command(&self.root).args(args).output().await?;
        if !output.status.success() {
            bail!("git {} failed: {}", args[0], stderr_text(&output.stderr));
        }
        String::from_utf8(output.stdout).context("git returned non UTF-8 output")
    }
}

pub struct BlobReader {
    _child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl BlobReader {
    pub async fn read(&mut self, blob_sha: &str) -> Result<Vec<u8>> {
        if !blob_sha.chars().all(|value| value.is_ascii_hexdigit()) {
            bail!("invalid object id {blob_sha}");
        }
        self.stdin.write_all(format!("{blob_sha}\n").as_bytes()).await?;
        self.stdin.flush().await?;

        // Header: "<sha> <type> <size>\n", or "<sha> missing\n".
        let mut header = String::new();
        self.stdout.read_line(&mut header).await?;
        let fields = header.split_whitespace().collect::<Vec<_>>();
        let [_, kind, size] = fields.as_slice() else {
            bail!("object {blob_sha} is missing from the repository");
        };
        if *kind != "blob" {
            bail!("object {blob_sha} is a {kind}, not a file");
        }
        let size: usize = size.parse().context("git reported an invalid object size")?;
        let mut content = vec![0; size];
        self.stdout.read_exact(&mut content).await?;
        // Each object is followed by a newline.
        let mut newline = [0_u8; 1];
        self.stdout.read_exact(&mut newline).await?;
        Ok(content)
    }
}

/// What a server turns on with [`harden`].
#[derive(Debug, Clone, Default)]
pub struct Hardening {
    /// Folders git may read although another account owns them. Each is given
    /// to git as `safe.directory`, and with `/*` appended for the folders
    /// below it (git 2.46 and later).
    pub safe_directories: Vec<String>,
}

static HARDENING: OnceLock<Hardening> = OnceLock::new();

/// Isolates git from the account's own configuration for this process (see the
/// module notes). Returns false when it was already called.
pub fn harden(hardening: Hardening) -> bool {
    HARDENING.set(hardening).is_ok()
}

fn git_command(directory: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(directory)
        // Never prompt, never take optional locks (the user may be working in
        // the repository at the same time) and never start an fsmonitor.
        .args(["-c", "core.fsmonitor=false", "-c", "core.quotepath=off"])
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        // Reading a partial clone never goes to the network on its own.
        .env("GIT_NO_LAZY_FETCH", "1")
        .stdin(Stdio::null());
    if let Some(hardening) = HARDENING.get() {
        // Only reading local objects is ever needed: no transport, no
        // submodule recursion, and configuration only from the repository
        // itself (and from here).
        command
            .args(["-c", "protocol.allow=never", "-c", "submodule.recurse=false"])
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", null_device())
            .env("GIT_ATTR_NOSYSTEM", "1");
        for directory in &hardening.safe_directories {
            let directory = directory.replace('\\', "/");
            command.arg("-c").arg(format!("safe.directory={directory}"));
            command
                .arg("-c")
                .arg(format!("safe.directory={}/*", directory.trim_end_matches('/')));
        }
    }
    #[cfg(windows)]
    {
        // CREATE_NO_WINDOW: no console window flashes up from the desktop app.
        command.creation_flags(0x0800_0000);
    }
    command
}

fn null_device() -> &'static str {
    if cfg!(windows) {
        "NUL"
    } else {
        "/dev/null"
    }
}

fn validate_branch_name(branch: &str) -> Result<()> {
    let invalid = branch.is_empty()
        || branch.starts_with('-')
        || branch.contains("..")
        || branch
            .chars()
            .any(|value| value.is_whitespace() || value.is_control() || "~^:?*[\\".contains(value));
    if invalid {
        repomemo_domain::bail_invalid!("{branch:?} is not a valid branch name.");
    }
    Ok(())
}

/// Reads the output of the `last_changes` log: records start at `\x1e` with
/// `sha \x1f author \x1f date`, followed by the paths the commit touched.
/// The log is newest first, so the first record to list a path is its last
/// change.
fn parse_last_changes(output: &str) -> std::collections::HashMap<String, LastChange> {
    let mut changes = std::collections::HashMap::new();
    for record in output.split('\u{1e}').filter(|record| !record.trim().is_empty()) {
        let mut lines = record.lines();
        let Some(header) = lines.next() else {
            continue;
        };
        let mut parts = header.split('\u{1f}');
        let (Some(sha), Some(author), Some(date)) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        for path in lines.map(str::trim_end).filter(|line| !line.is_empty()) {
            changes.entry(path.to_owned()).or_insert_with(|| LastChange {
                sha: sha.to_owned(),
                author_name: author.to_owned(),
                authored_at: date.to_owned(),
            });
        }
    }
    changes
}

fn parse_ls_tree(output: &[u8]) -> Result<Vec<TreeFile>> {
    let mut files = Vec::new();
    for record in output.split(|byte| *byte == 0) {
        if record.is_empty() {
            continue;
        }
        let record = std::str::from_utf8(record).context("git returned a non UTF-8 path")?;
        // "<mode> <type> <object> <size>\t<path>"; the size is space-padded.
        let (meta, path) = record
            .split_once('\t')
            .ok_or_else(|| anyhow!("unexpected git ls-tree line: {record}"))?;
        let fields = meta.split_whitespace().collect::<Vec<_>>();
        let [mode, kind, object, size] = fields.as_slice() else {
            bail!("unexpected git ls-tree line: {record}");
        };
        if *kind != "blob" || !matches!(*mode, "100644" | "100755") {
            continue;
        }
        files.push(TreeFile {
            path: path.to_owned(),
            blob_sha: (*object).to_owned(),
            size_bytes: size.parse().unwrap_or(0),
        });
    }
    Ok(files)
}

fn stderr_text(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr).trim().to_owned();
    if text.is_empty() {
        "no details".to_owned()
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn git(dir: &Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=Test", "-c", "user.email=test@example.com"])
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    }

    #[test]
    fn the_newest_commit_that_lists_a_path_is_its_last_change() {
        let log = "\u{1e}ccc\u{1f}Ana\u{1f}2026-03-03T10:00:00+00:00\n\nsrc/a.rs\nREADME.md\n\
                   \u{1e}bbb\u{1f}Bo Li\u{1f}2026-02-02T10:00:00+00:00\n\nsrc/a.rs\nsrc/b.rs\n\
                   \u{1e}aaa\u{1f}Cy\u{1f}2026-01-01T10:00:00+00:00\n\nsrc/b.rs\ndocs/d.md\n";
        let changes = parse_last_changes(log);
        assert_eq!(changes.len(), 4);
        assert_eq!(changes["src/a.rs"].sha, "ccc");
        assert_eq!(changes["src/a.rs"].author_name, "Ana");
        assert_eq!(changes["src/b.rs"].author_name, "Bo Li");
        assert_eq!(changes["docs/d.md"].authored_at, "2026-01-01T10:00:00+00:00");
        assert!(parse_last_changes("").is_empty());
    }

    #[test]
    fn ls_tree_output_keeps_regular_files_only() {
        let output = b"100644 blob aaaa       12\tsrc/main.rs\x00120000 blob bbbb        6\tlink\x00160000 commit cccc       -\tvendor/sub\x00100755 blob dddd      300\tscripts/run.sh\0";
        let files = parse_ls_tree(output).unwrap();
        assert_eq!(
            files,
            vec![
                TreeFile {
                    path: "src/main.rs".to_owned(),
                    blob_sha: "aaaa".to_owned(),
                    size_bytes: 12
                },
                TreeFile {
                    path: "scripts/run.sh".to_owned(),
                    blob_sha: "dddd".to_owned(),
                    size_bytes: 300
                },
            ]
        );
    }

    #[tokio::test]
    async fn a_file_renamed_and_edited_is_still_found_as_a_rename() {
        let dir = std::env::temp_dir().join(format!(
            "repomemo-git-rename-{}",
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        git(&dir, &["init", "--quiet", "--initial-branch=main"]);
        let body = (0..40)
            .map(|line| format!("line {line} of the guide\n"))
            .collect::<String>();
        std::fs::write(dir.join("guide.md"), &body).unwrap();
        std::fs::write(dir.join("other.md"), "unrelated\n").unwrap();
        git(&dir, &["add", "."]);
        git(&dir, &["commit", "--quiet", "-m", "first"]);
        let repo = GitRepo::open(&dir).await.unwrap();
        let first = repo.resolve_commit(None).await.unwrap();

        git(&dir, &["mv", "guide.md", "handbook.md"]);
        std::fs::write(dir.join("handbook.md"), format!("{body}one more line\n")).unwrap();
        std::fs::remove_file(dir.join("other.md")).unwrap();
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "--quiet", "-m", "rename and edit"]);
        let second = repo.resolve_commit(None).await.unwrap();

        let renames = repo.renamed_files(&first, &second).await.unwrap();
        assert_eq!(renames, vec![("guide.md".to_owned(), "handbook.md".to_owned())]);
        assert!(repo.renamed_files("--oops", &second).await.is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn hardened_commands_ignore_the_account_configuration() {
        // Process-wide, so this is the only test that calls it.
        assert!(harden(Hardening {
            safe_directories: vec!["C:\\repos".to_owned()]
        }));
        let command = git_command(Path::new("."));
        let std_command = command.as_std();
        let args = std_command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        for expected in [
            "protocol.allow=never",
            "submodule.recurse=false",
            "safe.directory=C:/repos",
            "safe.directory=C:/repos/*",
        ] {
            assert!(
                args.iter().any(|arg| arg == expected),
                "{expected} missing from {args:?}"
            );
        }
        let envs = std_command
            .get_envs()
            .filter_map(|(key, value)| {
                Some((
                    key.to_string_lossy().into_owned(),
                    value?.to_string_lossy().into_owned(),
                ))
            })
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(envs.get("GIT_CONFIG_NOSYSTEM").map(String::as_str), Some("1"));
        assert_eq!(envs.get("GIT_CONFIG_GLOBAL").map(String::as_str), Some(null_device()));
    }

    #[tokio::test]
    async fn a_bare_partial_clone_is_read_like_any_repository() {
        let base = std::env::temp_dir().join(format!(
            "repomemo-git-partial-{}",
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        let source = base.join("source");
        let clone = base.join("clone.git");
        std::fs::create_dir_all(&source).unwrap();
        git(&source, &["init", "--quiet", "--initial-branch=main"]);
        git(&source, &["config", "uploadpack.allowFilter", "true"]);
        git(&source, &["config", "uploadpack.allowAnySHA1InWant", "true"]);
        std::fs::write(source.join("small.md"), "# Small\n").unwrap();
        std::fs::write(source.join("big.txt"), "x".repeat(4096)).unwrap();
        git(&source, &["add", "."]);
        git(&source, &["commit", "--quiet", "-m", "Initial commit"]);

        // A blobless-over-a-limit clone, the shape the server keeps.
        let status = std::process::Command::new("git")
            .args(["clone", "--quiet", "--bare", "--no-local", "--filter=blob:limit=100", "--"])
            .arg(format!("file://{}", source.to_string_lossy().replace('\\', "/")))
            .arg(&clone)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        if !status.success() {
            // A git too old for partial clones: nothing to check here.
            let _ = std::fs::remove_dir_all(base);
            return;
        }

        assert!(GitRepo::open(&clone).await.is_err(), "a bare repository is not a checkout");
        let repo = GitRepo::open_bare(&clone).await.unwrap();
        assert_eq!(repo.current_branch().await.unwrap().as_deref(), Some("main"));
        let head = repo.resolve_commit(Some("main")).await.unwrap();
        let files = repo.list_files(&head).await.unwrap();
        let paths = files.iter().map(|file| file.path.as_str()).collect::<Vec<_>>();
        assert_eq!(paths, vec!["big.txt", "small.md"]);
        let small = files.iter().find(|file| file.path == "small.md").unwrap();
        assert_eq!(small.size_bytes, 8);
        let mut reader = repo.blob_reader().await.unwrap();
        assert_eq!(reader.read(&small.blob_sha).await.unwrap(), b"# Small\n");
        assert!(GitRepo::open_bare(&base.join("missing.git")).await.is_err());
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn branch_names_that_could_be_options_are_rejected() {
        assert!(validate_branch_name("main").is_ok());
        assert!(validate_branch_name("feature/repo-sync").is_ok());
        assert!(validate_branch_name("--upload-pack=x").is_err());
        assert!(validate_branch_name("a..b").is_err());
        assert!(validate_branch_name("").is_err());
    }

    #[tokio::test]
    async fn reads_committed_files_and_their_content() {
        let dir = std::env::temp_dir().join(format!(
            "repomemo-git-test-{}",
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(dir.join("docs")).unwrap();
        git(&dir, &["init", "--quiet", "--initial-branch=main"]);
        std::fs::write(dir.join("README.md"), "# Hello\n").unwrap();
        std::fs::write(dir.join("docs/guide.md"), "Guide text\n").unwrap();
        std::fs::write(dir.join("untracked.txt"), "not committed").unwrap();
        git(&dir, &["add", "README.md", "docs/guide.md"]);
        git(&dir, &["commit", "--quiet", "-m", "Initial commit"]);

        let repo = GitRepo::open(&dir.join("docs")).await.unwrap();
        assert_eq!(repo.current_branch().await.unwrap().as_deref(), Some("main"));
        let head = repo.resolve_commit(None).await.unwrap();
        assert_eq!(repo.resolve_commit(Some("main")).await.unwrap(), head);
        assert!(repo.resolve_commit(Some("missing")).await.is_err());

        let info = repo.commit_info(&head).await.unwrap();
        assert_eq!(info.sha, head);
        assert_eq!(info.summary, "Initial commit");
        assert_eq!(repo.recent_commits(&head, 5).await.unwrap(), vec![info]);
        assert_eq!(repo.file_at(&head, "README.md", 1024).await.unwrap(), b"# Hello\n");
        assert!(repo.file_at(&head, "missing.md", 1024).await.is_err());
        assert!(repo.file_at(&head, "README.md", 3).await.is_err(), "over the size limit");
        assert!(repo.file_at("deadbeefdeadbeef", "README.md", 1024).await.is_err());
        assert!(repo.file_at("--oops", "README.md", 1024).await.is_err());
        let last = repo.last_changes(&head, 100).await.unwrap();
        assert_eq!(last["README.md"].sha, head);
        assert_eq!(last["docs/guide.md"].author_name, "Test");
        assert!(!last.contains_key("untracked.txt"));
        assert!(repo.last_changes("--oops", 10).await.is_err());

        let files = repo.list_files(&head).await.unwrap();
        let paths = files.iter().map(|file| file.path.as_str()).collect::<Vec<_>>();
        assert_eq!(paths, vec!["README.md", "docs/guide.md"]);

        let mut reader = repo.blob_reader().await.unwrap();
        assert_eq!(reader.read(&files[0].blob_sha).await.unwrap(), b"# Hello\n");
        assert_eq!(reader.read(&files[1].blob_sha).await.unwrap(), b"Guide text\n");

        let _ = std::fs::remove_dir_all(dir);
    }
}
