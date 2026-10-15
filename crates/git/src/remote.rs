//! Fetching from a remote repository over `https` (roadmap F4 phase 2a,
//! V0.2.20).
//!
//! A remote link is kept as a **bare, partial clone** on the server: no work
//! tree is ever checked out, blobs over the size the engine indexes are left
//! on the host, and the rest of the server reads it like any local repository
//! (see [`crate::GitRepo::open_bare`]). Everything that touches the network is
//! here, and follows the rules of the security review (section 8):
//!
//! - git is started with a **cleared environment**, so the server's secrets
//!   (`REPOMEMO_JWT_SECRET`, `REPOMEMO_SECRET_KEY`, ...) never reach it;
//! - only the protocol of the link is allowed (`protocol.allow=never`, then
//!   `https` or `ssh` alone), redirects are not followed, and every object is
//!   checked (`transfer.fsckObjects`);
//! - over `ssh` the deploy key is written to a private file that exists only
//!   while the command runs, the host's key is the one pinned on the link and
//!   nothing else (`StrictHostKeyChecking=yes`, an empty `known_hosts` apart
//!   from it), agents and passwords are off, and the key's contents never
//!   appear on a command line;
//! - a token travels through `GIT_ASKPASS` (this program, run again, see
//!   [`run_as_askpass_if_requested`]) and the environment of that one git
//!   process, never in the address or in the arguments;
//! - nothing is checked out, no hook runs, submodules are not fetched;
//! - a git that is older than the patched releases is refused.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use tokio::process::Command;

/// Set on the git process so that this program, when git runs it as the
/// credential helper, prints the credential instead of starting the server.
const ASKPASS_MARKER: &str = "REPOMEMO_GIT_ASKPASS";
const ASKPASS_USERNAME: &str = "REPOMEMO_GIT_USERNAME";
const ASKPASS_SECRET: &str = "REPOMEMO_GIT_SECRET";

/// Blobs larger than this are left on the host (and are never indexed: the
/// engine skips files over a mebibyte).
pub const PARTIAL_CLONE_BLOB_LIMIT: &str = "1m";

/// How long one network operation may take before it is stopped.
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(60);
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// A token for one repository, with the account name that goes with it.
#[derive(Clone)]
pub struct RemoteCredential {
    username: String,
    secret: String,
}

impl RemoteCredential {
    /// `username` is what hosts expect next to a token (`x-access-token` for
    /// GitHub apps, `oauth2` for GitLab, any name for personal tokens).
    pub fn token(username: Option<&str>, secret: &str) -> Self {
        Self {
            username: username
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .unwrap_or("x-access-token")
                .to_owned(),
            secret: secret.to_owned(),
        }
    }
}

impl std::fmt::Debug for RemoteCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteCredential")
            .field("username", &self.username)
            .field("secret", &"<hidden>")
            .finish()
    }
}

/// How to sign in to an `ssh` repository: a deploy key and the one host key
/// the connection may end at.
#[derive(Clone)]
pub struct SshAuth {
    private_key: String,
    /// One or more complete `known_hosts` lines (see [`known_hosts_lines`]).
    known_hosts: String,
}

impl SshAuth {
    pub fn new(private_key: &str, known_hosts: String) -> Self {
        Self {
            private_key: private_key.to_owned(),
            known_hosts,
        }
    }
}

impl std::fmt::Debug for SshAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SshAuth")
            .field("private_key", &"<hidden>")
            .field("known_hosts", &self.known_hosts)
            .finish()
    }
}

/// Where a remote repository is and how to sign in to it.
#[derive(Debug, Clone)]
pub struct RemoteTarget {
    /// An address that already passed the host policy, with no credentials.
    pub url: String,
    /// A token, for an `https` address.
    pub credential: Option<RemoteCredential>,
    /// A deploy key and pinned host key, for an `ssh` address.
    pub ssh: Option<SshAuth>,
}

/// The `known_hosts` lines that pin a host to the key (or keys) the person
/// pasted: either the key as `ssh-keyscan` prints it (`host ssh-ed25519
/// AAAA...`), a `known_hosts` line, or just `ssh-ed25519 AAAA...`. The host
/// and port come from the link, never from the pasted text, so a line cannot
/// pin another host.
pub fn known_hosts_lines(host: &str, port: u16, pasted: &str) -> Result<String> {
    let pattern = match (host.contains(':'), port) {
        (true, 22) => host.to_owned(),
        (true, port) => format!("[{host}]:{port}"),
        (false, 22) => host.to_owned(),
        (false, port) => format!("[{host}]:{port}"),
    };
    let mut lines = Vec::new();
    for line in pasted.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('#')) {
        let tokens = line.split_whitespace().collect::<Vec<_>>();
        if tokens.first().is_some_and(|token| token.starts_with('@')) {
            repomemo_domain::bail_invalid!("A certificate authority or revoked key cannot be used as the host key.");
        }
        let Some(at) = tokens.iter().position(|token| {
            token.starts_with("ssh-") || token.starts_with("ecdsa-sha2-") || token.starts_with("sk-")
        }) else {
            repomemo_domain::bail_invalid!(
                "That is not a host key. Paste the line printed by: ssh-keyscan -t ed25519 {host}"
            );
        };
        let Some(key) = tokens.get(at + 1) else {
            repomemo_domain::bail_invalid!("The host key is missing its key data.");
        };
        let valid = key.len() >= 20
            && key.len() <= 4096
            && key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '='));
        if !valid {
            repomemo_domain::bail_invalid!("That host key is not valid.");
        }
        lines.push(format!("{pattern} {} {key}", tokens[at]));
    }
    if lines.is_empty() {
        repomemo_domain::bail_invalid!(
            "Enter the host's public key, as printed by: ssh-keyscan -t ed25519 {host}"
        );
    }
    if lines.len() > 4 {
        repomemo_domain::bail_invalid!("Give at most four host keys.");
    }
    Ok(lines.join("\n") + "\n")
}

/// Files written for one `ssh` command, removed when it is over.
pub struct SshFiles {
    directory: Option<PathBuf>,
}

impl Drop for SshFiles {
    fn drop(&mut self) {
        if let Some(directory) = self.directory.take() {
            let _ = std::fs::remove_dir_all(directory);
        }
    }
}

static SSH_FILE_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

impl SshFiles {
    fn none() -> Self {
        Self { directory: None }
    }

    /// A private folder with the key and the pinned host keys in it.
    fn write(auth: &SshAuth) -> Result<(Self, PathBuf, PathBuf)> {
        let unique = format!(
            "repomemo-ssh-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default(),
            SSH_FILE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        let directory = std::env::temp_dir().join(unique);
        std::fs::create_dir(&directory).context("a private folder for the deploy key could not be created")?;
        let files = Self {
            directory: Some(directory.clone()),
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))?;
        }
        let key_path = directory.join("id");
        let mut key = auth.private_key.trim().to_owned();
        key.push('\n');
        write_private(&key_path, key.as_bytes())?;
        let hosts_path = directory.join("known_hosts");
        write_private(&hosts_path, auth.known_hosts.as_bytes())?;
        Ok((files, key_path, hosts_path))
    }
}

fn write_private(path: &Path, content: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).context("the deploy key could not be written")?;
    file.write_all(content)?;
    Ok(())
}

/// A path as `ssh -o` options and `-i` take it: forward slashes, and quoted.
fn ssh_path(path: &Path) -> String {
    format!("\"{}\"", path.to_string_lossy().replace('\\', "/"))
}

/// Where a branch of the remote points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteHead {
    pub branch: String,
    pub commit: String,
}

/// If git started this program as its credential helper, answers it and
/// returns true: `main` must then exit at once. Call it first thing in `main`.
pub fn run_as_askpass_if_requested() -> bool {
    if std::env::var_os(ASKPASS_MARKER).is_none() {
        return false;
    }
    let prompt = std::env::args().nth(1).unwrap_or_default().to_ascii_lowercase();
    let answer = if prompt.contains("username") {
        std::env::var(ASKPASS_USERNAME).unwrap_or_default()
    } else {
        std::env::var(ASKPASS_SECRET).unwrap_or_default()
    };
    println!("{answer}");
    true
}

/// The oldest git releases that carry the fix for CVE-2025-48384 (a carriage
/// return in a submodule path, exploited on clone), one per maintained line.
pub fn is_patched_git(version: &str) -> bool {
    let numbers = version
        .split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .take(3)
        .map(|part| part.parse::<u32>().unwrap_or(0))
        .collect::<Vec<_>>();
    let [major, minor, patch] = match numbers.as_slice() {
        [major, minor, patch] => [*major, *minor, *patch],
        [major, minor] => [*major, *minor, 0],
        _ => return false,
    };
    if major != 2 {
        return major > 2;
    }
    match minor {
        0..=42 => false,
        43 => patch >= 7,
        44 => patch >= 4,
        45 => patch >= 4,
        46 => patch >= 4,
        47 => patch >= 3,
        48 => patch >= 2,
        49 => patch >= 1,
        _ => minor > 50 || patch >= 1,
    }
}

static GIT_VERSION: OnceLock<String> = OnceLock::new();

/// Refuses to go on with a git that is not patched (once per process).
pub async fn require_patched_git() -> Result<()> {
    if let Some(version) = GIT_VERSION.get() {
        return check_version(version);
    }
    let output = Command::new("git")
        .arg("--version")
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                repomemo_domain::not_found!("git was not found. Install git and make sure it is on the PATH.")
            } else {
                anyhow!("git could not be started: {error}")
            }
        })?;
    let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let version = text
        .split_whitespace()
        .find(|part| part.chars().next().is_some_and(|c| c.is_ascii_digit()))
        .unwrap_or_default()
        .to_owned();
    let _ = GIT_VERSION.set(version.clone());
    check_version(&version)
}

fn check_version(version: &str) -> Result<()> {
    if is_patched_git(version) {
        Ok(())
    } else {
        repomemo_domain::bail_invalid!(
            "The server's git ({}) is older than the releases that fix CVE-2025-48384. Remote repositories need git 2.50.1 or later (or a patched 2.43.7, 2.44.4, 2.45.4, 2.46.4, 2.47.3, 2.48.2 or 2.49.1); its operator updates git first.",
            if version.is_empty() { "unknown version" } else { version }
        )
    }
}

/// A git process for the network: a cleared environment, one protocol, and
/// the credential, if any, reachable through this program's askpass mode (a
/// token) or in files that live as long as the returned guard (a deploy key).
fn remote_command(target: &RemoteTarget) -> Result<(Command, SshFiles)> {
    let mut command = Command::new("git");
    command.env_clear();
    // The few things a process needs to run at all.
    for name in [
        "PATH",
        "SystemRoot",
        "SYSTEMROOT",
        "windir",
        "TEMP",
        "TMP",
        "TMPDIR",
        "LANG",
        "LC_ALL",
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
    ] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    command
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", null_device())
        .env("GIT_ATTR_NOSYSTEM", "1")
        .env("GIT_ALLOW_PROTOCOL", if target.ssh.is_some() { "ssh" } else { "https" })
        .env("GIT_NO_LAZY_FETCH", "1")
        .args([
            "-c",
            "protocol.allow=never",
            "-c",
            if target.ssh.is_some() {
                "protocol.ssh.allow=always"
            } else {
                "protocol.https.allow=always"
            },
            "-c",
            "http.followRedirects=false",
            "-c",
            "transfer.fsckObjects=true",
            "-c",
            "fetch.fsckObjects=true",
            "-c",
            "submodule.recurse=false",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "credential.helper=",
            "-c",
            "core.askPass=",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(credential) = &target.credential {
        let program = std::env::current_exe().context("the server's own path could not be found")?;
        command
            .env("GIT_ASKPASS", program)
            .env(ASKPASS_MARKER, "1")
            .env(ASKPASS_USERNAME, &credential.username)
            .env(ASKPASS_SECRET, &credential.secret);
    }
    let mut files = SshFiles::none();
    if let Some(auth) = &target.ssh {
        let (written, key_path, hosts_path) = SshFiles::write(auth)?;
        files = written;
        // Only the pinned key may end the connection; nothing else on this
        // machine (agent, config, other known hosts, password prompts) counts.
        let ssh = format!(
            "ssh -i {key} -o IdentitiesOnly=yes -o BatchMode=yes -o StrictHostKeyChecking=yes \
             -o UserKnownHostsFile={hosts} -o GlobalKnownHostsFile={null} -o IdentityAgent=none \
             -o ForwardAgent=no -o ClearAllForwardings=yes -o PasswordAuthentication=no \
             -o KbdInteractiveAuthentication=no -o ConnectTimeout=30 -F {null}",
            key = ssh_path(&key_path),
            hosts = ssh_path(&hosts_path),
            null = "/dev/null",
        );
        command.env("GIT_SSH_COMMAND", ssh).env("GIT_SSH_VARIANT", "ssh");
    }
    #[cfg(windows)]
    {
        command.creation_flags(0x0800_0000);
    }
    Ok((command, files))
}

fn null_device() -> &'static str {
    if cfg!(windows) {
        "NUL"
    } else {
        "/dev/null"
    }
}

/// Runs a prepared command with a time limit and returns its output, or a
/// message that never contains the token.
async fn run(mut command: Command, limit: Duration, what: &str, target: &RemoteTarget) -> Result<std::process::Output> {
    let child = command.spawn().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            repomemo_domain::not_found!("git was not found. Install git and make sure it is on the PATH.")
        } else {
            anyhow!("git could not be started: {error}")
        }
    })?;
    let output = match tokio::time::timeout(limit, child.wait_with_output()).await {
        Ok(result) => result.context("git did not finish")?,
        Err(_) => {
            repomemo_domain::bail_invalid!(
                "{what} did not finish within {} minutes and was stopped.",
                limit.as_secs() / 60
            );
        }
    };
    if !output.status.success() {
        // What git said, with the token taken out, is the person's to read.
        repomemo_domain::bail_invalid!("{what} failed: {}", explain(&output.stderr, target));
    }
    Ok(output)
}

/// What git said, in plain words, with the token and the address's secret
/// parts removed.
fn explain(stderr: &[u8], target: &RemoteTarget) -> String {
    let mut text = String::from_utf8_lossy(stderr).trim().to_owned();
    if let Some(credential) = &target.credential {
        if !credential.secret.is_empty() {
            text = text.replace(&credential.secret, "<hidden>");
        }
    }
    let lower = text.to_ascii_lowercase();
    let hint = if lower.contains("authentication failed")
        || lower.contains("could not read username")
        || lower.contains("could not read password")
        || lower.contains("returned error: 401")
        || lower.contains("returned error: 403")
    {
        Some("The host refused the token, or the repository needs one: check that it is valid, not expired, and can read this repository.")
    } else if lower.contains("host key verification failed") || lower.contains("remote host identification has changed") {
        Some("The host's key is not the one pinned on this link. Check it with the host's operator and paste the right one.")
    } else if lower.contains("permission denied (publickey") || lower.contains("permission denied, please try again") {
        Some("The host refused the deploy key: check that it is added to the repository (or the account) and has read access.")
    } else if lower.contains("not found") || lower.contains("returned error: 404") {
        Some("The host does not know this repository, or the token cannot see it.")
    } else if lower.contains("redirect") {
        Some("The host redirected the request, which is not followed: use the repository's current address.")
    } else if lower.contains("could not resolve host") {
        Some("The host name could not be resolved from the server.")
    } else if lower.contains("ssl") || lower.contains("certificate") {
        Some("The host's certificate was not accepted by the server.")
    } else {
        None
    };
    let text = text.lines().take(6).collect::<Vec<_>>().join(" ");
    match hint {
        Some(hint) => format!("{hint} (git said: {text})"),
        None => {
            if text.is_empty() {
                "no details".to_owned()
            } else {
                text
            }
        }
    }
}

/// Where a branch (or the default branch, when `branch` is `None`) points on
/// the remote, asked of the host without fetching anything.
pub async fn remote_head(target: &RemoteTarget, branch: Option<&str>) -> Result<RemoteHead> {
    require_patched_git().await?;
    if let Some(branch) = branch {
        validate_branch(branch)?;
    }
    let (mut command, _files) = remote_command(target)?;
    command.args(["ls-remote", "--symref", "--"]).arg(&target.url);
    match branch {
        Some(branch) => {
            command.arg(format!("refs/heads/{branch}"));
        }
        None => {
            command.arg("HEAD");
        }
    }
    let output = run(command, LOOKUP_TIMEOUT, "Looking the repository up", target).await?;
    parse_ls_remote(&String::from_utf8_lossy(&output.stdout), branch)
}

fn parse_ls_remote(text: &str, branch: Option<&str>) -> Result<RemoteHead> {
    let mut symref_branch = None;
    let mut commit = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("ref: ") {
            // "ref: refs/heads/main\tHEAD"
            if let Some((target, name)) = rest.split_once('\t') {
                if name.trim() == "HEAD" {
                    symref_branch = target.trim().strip_prefix("refs/heads/").map(str::to_owned);
                }
            }
            continue;
        }
        let Some((sha, name)) = line.split_once('\t') else {
            continue;
        };
        let name = name.trim();
        let matches = match branch {
            Some(branch) => name == format!("refs/heads/{branch}"),
            None => name == "HEAD",
        };
        if matches && sha.len() >= 40 && sha.chars().all(|c| c.is_ascii_hexdigit()) {
            commit = Some(sha.to_owned());
        }
    }
    match (branch, commit) {
        (Some(branch), Some(commit)) => Ok(RemoteHead {
            branch: branch.to_owned(),
            commit,
        }),
        (None, Some(commit)) => Ok(RemoteHead {
            branch: symref_branch.unwrap_or_else(|| "HEAD".to_owned()),
            commit,
        }),
        (Some(branch), None) => {
            repomemo_domain::bail_invalid!("The branch {branch} does not exist in this repository.")
        }
        (None, None) => repomemo_domain::bail_invalid!("This repository has no commits yet."),
    }
}

/// Makes a bare, partial clone of one branch in `directory` (which must not
/// exist yet).
pub async fn clone_bare(target: &RemoteTarget, directory: &Path, branch: &str) -> Result<()> {
    require_patched_git().await?;
    validate_branch(branch)?;
    if let Some(parent) = directory.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .context("the folder for remote repositories could not be created")?;
    }
    let (mut command, _files) = remote_command(target)?;
    command
        .args([
            "clone",
            "--bare",
            "--no-tags",
            "--single-branch",
            "--filter=blob:limit=1m",
            "--template=",
            "--branch",
            branch,
            "--",
        ])
        .arg(&target.url)
        .arg(directory);
    match run(command, TRANSFER_TIMEOUT, "Cloning the repository", target).await {
        Ok(_) => Ok(()),
        Err(error) => {
            // Whatever a failed clone left is not a repository.
            let _ = tokio::fs::remove_dir_all(directory).await;
            Err(error)
        }
    }
}

/// Brings one branch of an existing clone up to date, discarding what the
/// local branch had (it only ever mirrors the remote's).
pub async fn fetch_branch(target: &RemoteTarget, directory: &Path, branch: &str) -> Result<()> {
    require_patched_git().await?;
    validate_branch(branch)?;
    let (mut command, _files) = remote_command(target)?;
    command
        .arg("-C")
        .arg(directory)
        .arg("-c")
        .arg(format!("remote.origin.url={}", target.url))
        .args([
            "fetch",
            "--no-tags",
            "--force",
            "--no-recurse-submodules",
            "--",
            "origin",
        ])
        .arg(format!("+refs/heads/{branch}:refs/heads/{branch}"));
    run(command, TRANSFER_TIMEOUT, "Fetching the repository", target).await?;
    // Follow a changed default branch when no branch was chosen.
    let mut head = Command::new("git");
    head.arg("-C")
        .arg(directory)
        .args(["symbolic-ref", "HEAD", &format!("refs/heads/{branch}")])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let _ = head.status().await;
    Ok(())
}

fn validate_branch(branch: &str) -> Result<()> {
    let invalid = branch.is_empty()
        || branch.starts_with('-')
        || branch.contains("..")
        || branch.contains("@{")
        || branch.ends_with('/')
        || branch.ends_with(".lock")
        || branch
            .chars()
            .any(|value| value.is_whitespace() || value.is_control() || "~^:?*[\\".contains(value));
    if invalid {
        repomemo_domain::bail_invalid!("{branch:?} is not a valid branch name.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_patched_git_releases_are_accepted() {
        for patched in [
            "2.50.1",
            "2.50.1.windows.1",
            "2.51.0",
            "2.51.0.windows.2",
            "2.60.0",
            "3.0.0",
            "2.43.7",
            "2.49.1",
            "2.48.2",
            "2.47.3",
            "2.46.4",
            "2.45.4",
            "2.44.4",
        ] {
            assert!(is_patched_git(patched), "{patched}");
        }
        for unpatched in [
            "2.50.0",
            "2.49.0",
            "2.48.1",
            "2.43.0",
            "2.43.6",
            "2.39.5",
            "1.9.0",
            "",
            "garbage",
        ] {
            assert!(!is_patched_git(unpatched), "{unpatched}");
        }
    }

    #[test]
    fn ls_remote_answers_are_read_for_a_named_branch_and_for_the_default() {
        let sha = "0123456789abcdef0123456789abcdef01234567";
        let other = "89abcdef0123456789abcdef0123456789abcdef";
        let default = format!("ref: refs/heads/trunk\tHEAD\n{sha}\tHEAD\n");
        assert_eq!(
            parse_ls_remote(&default, None).unwrap(),
            RemoteHead {
                branch: "trunk".to_owned(),
                commit: sha.to_owned()
            }
        );
        let named = format!("{other}\trefs/heads/release/foo\n{sha}\trefs/heads/foo\n");
        assert_eq!(parse_ls_remote(&named, Some("foo")).unwrap().commit, sha);
        assert!(parse_ls_remote(&named, Some("missing")).is_err());
        assert!(parse_ls_remote("", None).is_err());
    }

    #[test]
    fn host_keys_are_pinned_to_the_host_and_port_of_the_link() {
        let key = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl";
        assert_eq!(
            known_hosts_lines("github.com", 22, key).unwrap(),
            format!("github.com {key}\n")
        );
        // ssh-keyscan output and comments name the host being scanned: ignored.
        let scanned = format!("# github.com:22 SSH-2.0-x\nother.example {key} comment\n");
        assert_eq!(
            known_hosts_lines("git.corp.example", 2222, &scanned).unwrap(),
            format!("[git.corp.example]:2222 {key}\n")
        );
        assert_eq!(known_hosts_lines("::1", 22, key).unwrap(), format!("::1 {key}\n"));
        for refused in [
            "",
            "github.com",
            "@cert-authority * ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl",
            "ssh-ed25519",
            "ssh-ed25519 not base64 !!",
            "ssh-ed25519 short",
        ] {
            assert!(known_hosts_lines("github.com", 22, refused).is_err(), "{refused:?}");
        }
    }

    #[test]
    fn a_deploy_key_lives_in_a_private_file_and_never_on_the_command_line() {
        let key = "-----BEGIN OPENSSH PRIVATE KEY-----\nSECRETKEYMATERIAL\n-----END OPENSSH PRIVATE KEY-----";
        let target = RemoteTarget {
            url: "ssh://git@github.com/acme/app.git".to_owned(),
            credential: None,
            ssh: Some(SshAuth::new(
                key,
                "github.com ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl\n".to_owned(),
            )),
        };
        let directory;
        {
            let (command, files) = remote_command(&target).unwrap();
            directory = files.directory.clone().unwrap();
            let std_command = command.as_std();
            let environment = std_command
                .get_envs()
                .filter_map(|(name, value)| Some((name.to_string_lossy().into_owned(), value?.to_string_lossy().into_owned())))
                .collect::<std::collections::HashMap<_, _>>();
            let ssh = environment.get("GIT_SSH_COMMAND").unwrap();
            assert!(!ssh.contains("SECRETKEYMATERIAL"));
            assert!(ssh.contains("StrictHostKeyChecking=yes") && ssh.contains("IdentitiesOnly=yes"));
            assert!(ssh.contains("BatchMode=yes") && ssh.contains("PasswordAuthentication=no"));
            assert_eq!(environment.get("GIT_ALLOW_PROTOCOL").map(String::as_str), Some("ssh"));
            let arguments = std_command
                .get_args()
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect::<Vec<_>>();
            assert!(arguments.iter().any(|argument| argument == "protocol.ssh.allow=always"));
            assert!(!arguments.iter().any(|argument| argument == "protocol.https.allow=always"));
            let written = std::fs::read_to_string(directory.join("id")).unwrap();
            assert!(written.contains("SECRETKEYMATERIAL") && written.ends_with("-----\n"));
            assert!(directory.join("known_hosts").is_file());
            assert!(!format!("{target:?}").contains("SECRETKEYMATERIAL"));
        }
        assert!(!directory.exists(), "the files are removed when the command is over");
    }

    #[test]
    fn a_credential_stays_out_of_arguments_and_messages() {
        let target = RemoteTarget {
            url: "https://github.com/acme/app.git".to_owned(),
            credential: Some(RemoteCredential::token(Some("x-access-token"), "ghp_supersecret")),
            ssh: None,
        };
        let (command, _files) = remote_command(&target).unwrap();
        let std_command = command.as_std();
        let arguments = std_command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(!arguments.iter().any(|argument| argument.contains("ghp_supersecret")));
        assert!(arguments.iter().any(|argument| argument == "protocol.https.allow=always"));
        assert!(arguments.iter().any(|argument| argument == "http.followRedirects=false"));
        let environment = std_command
            .get_envs()
            .filter_map(|(key, value)| Some((key.to_string_lossy().into_owned(), value?.to_string_lossy().into_owned())))
            .collect::<std::collections::HashMap<_, _>>();
        assert!(!environment.contains_key("REPOMEMO_JWT_SECRET"));
        assert_eq!(environment.get("GIT_TERMINAL_PROMPT").map(String::as_str), Some("0"));
        assert!(!format!("{:?}", target.credential).contains("ghp_supersecret"));

        let said = explain(
            b"fatal: Authentication failed for 'https://x-access-token:ghp_supersecret@github.com/acme/app.git/'",
            &target,
        );
        assert!(!said.contains("ghp_supersecret"), "{said}");
        assert!(said.contains("refused the token"), "{said}");
    }
}
