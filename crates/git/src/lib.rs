//! Read-only access to git repositories through the `git` command line.
//!
//! RepoMemo indexes what a repository has committed, not its working tree, so
//! everything here reads objects (commits, trees, blobs) and never touches the
//! index, the work tree or refs. The `git` CLI is used instead of a library so
//! that the user's own git setup (credential helpers, `safe.directory`, long
//! paths on Windows) applies unchanged.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{anyhow, bail, Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

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
}

impl GitRepo {
    /// Opens the repository containing `path`. Fails with a readable message
    /// when git is not installed or the path is not inside a repository.
    pub async fn open(path: &Path) -> Result<Self> {
        if !path.is_dir() {
            bail!("The folder {} does not exist.", path.display());
        }
        let output = git_command(path)
            .args(["rev-parse", "--show-toplevel"])
            .output()
            .await
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    anyhow!("git was not found. Install git and make sure it is on the PATH.")
                } else {
                    anyhow!("git could not be started: {error}")
                }
            })?;
        if !output.status.success() {
            let detail = stderr_text(&output.stderr);
            if detail.contains("dubious ownership") {
                bail!(
                    "git refuses to read {} because it belongs to another user account than the server's. On the server, run: git config --global --add safe.directory \"{}\"",
                    path.display(),
                    path.display().to_string().replace('\\', "/")
                );
            }
            bail!("{} is not inside a git repository: {detail}", path.display());
        }
        let root = String::from_utf8(output.stdout)
            .context("git returned a non UTF-8 repository path")?
            .trim()
            .to_owned();
        if root.is_empty() {
            bail!("{} is a bare repository; choose a checkout instead.", path.display());
        }
        Ok(Self {
            root: PathBuf::from(root),
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
                Some(branch) => bail!("The branch {branch} does not exist in this repository."),
                None => bail!("This repository has no commits yet."),
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
        let output = git_command(&self.root)
            .args(["ls-tree", "-r", "-z", "-l", "--full-tree", commit])
            .output()
            .await?;
        if !output.status.success() {
            bail!("git ls-tree failed: {}", stderr_text(&output.stderr));
        }
        parse_ls_tree(&output.stdout)
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
        self.stdin
            .write_all(format!("{blob_sha}\n").as_bytes())
            .await?;
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
        .stdin(Stdio::null());
    #[cfg(windows)]
    {
        // CREATE_NO_WINDOW: no console window flashes up from the desktop app.
        command.creation_flags(0x0800_0000);
    }
    command
}

fn validate_branch_name(branch: &str) -> Result<()> {
    let invalid = branch.is_empty()
        || branch.starts_with('-')
        || branch.contains("..")
        || branch
            .chars()
            .any(|value| value.is_whitespace() || value.is_control() || "~^:?*[\\".contains(value));
    if invalid {
        bail!("{branch:?} is not a valid branch name.");
    }
    Ok(())
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
    fn ls_tree_output_keeps_regular_files_only() {
        let output = b"100644 blob aaaa       12\tsrc/main.rs\0120000 blob bbbb        6\tlink\0160000 commit cccc       -\tvendor/sub\0100755 blob dddd      300\tscripts/run.sh\0";
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
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
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

        let files = repo.list_files(&head).await.unwrap();
        let paths = files.iter().map(|file| file.path.as_str()).collect::<Vec<_>>();
        assert_eq!(paths, vec!["README.md", "docs/guide.md"]);

        let mut reader = repo.blob_reader().await.unwrap();
        assert_eq!(reader.read(&files[0].blob_sha).await.unwrap(), b"# Hello\n");
        assert_eq!(reader.read(&files[1].blob_sha).await.unwrap(), b"Guide text\n");

        let _ = std::fs::remove_dir_all(dir);
    }
}
