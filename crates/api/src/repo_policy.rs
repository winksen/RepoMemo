//! Where repository links may point.
//!
//! A repository link is a folder on the server, so whoever may link one can
//! make the server read any git checkout its account can see. The operator can
//! narrow that to a few folders (the server's `REPOMEMO_REPO_ROOTS`). Network
//! paths (Windows UNC shares such as `\\host\share`) are refused unless one of
//! those folders is itself on a share: merely looking at a UNC path makes
//! Windows authenticate to the remote host, which would hand the server
//! account's credentials to whoever named it.

use std::path::{Component, Path, PathBuf, Prefix};

use anyhow::{bail, Context, Result};

#[derive(Debug, Clone, Default)]
pub struct RepoLinkPolicy {
    /// Folders a repository must sit inside, canonical. Empty: any local folder.
    allowed_roots: Vec<PathBuf>,
}

impl RepoLinkPolicy {
    /// Any local folder the server can read (the default).
    pub fn unrestricted() -> Self {
        Self::default()
    }

    /// Only repositories inside these folders. Each must exist.
    pub fn with_allowed_roots(roots: Vec<PathBuf>) -> Result<Self> {
        let mut allowed_roots = Vec::new();
        for root in roots {
            let canonical = std::fs::canonicalize(&root).with_context(|| {
                format!("the repository root {} does not exist or cannot be read", root.display())
            })?;
            allowed_roots.push(canonical);
        }
        Ok(Self { allowed_roots })
    }

    pub fn allowed_roots(&self) -> &[PathBuf] {
        &self.allowed_roots
    }

    pub fn is_restricted(&self) -> bool {
        !self.allowed_roots.is_empty()
    }

    /// Checks a link before the filesystem is touched (so nothing outside
    /// the allowed folders is even probed), and again once symbolic links are
    /// resolved.
    pub fn check(&self, path: &Path) -> Result<()> {
        if is_network_path(path) && !self.allowed_roots.iter().any(|root| is_network_path(root)) {
            bail!(
                "Repository links must be folders on the server's own disks; network shares such as {} are not allowed.",
                path.display()
            );
        }
        if self.allowed_roots.is_empty() {
            return Ok(());
        }
        if !path.is_absolute() {
            bail!("Enter the full path of the repository folder, such as {}.", self.example());
        }
        let lexical = normalize_lexically(path);
        if !self.contains(&lexical) {
            return Err(self.outside_error());
        }
        // Symbolic links inside an allowed folder may point anywhere.
        if let Ok(canonical) = std::fs::canonicalize(path) {
            if !self.contains(&canonical) {
                return Err(self.outside_error());
            }
        }
        Ok(())
    }

    fn contains(&self, path: &Path) -> bool {
        let candidate = comparable(path);
        self.allowed_roots.iter().any(|root| {
            let root = comparable(root);
            candidate == root || candidate.starts_with(&format!("{}/", root.trim_end_matches('/')))
        })
    }

    fn example(&self) -> String {
        self.allowed_roots
            .first()
            .map(|root| comparable_display(root))
            .unwrap_or_default()
    }

    fn outside_error(&self) -> anyhow::Error {
        anyhow::anyhow!(
            "This server only links repositories inside: {}.",
            self.allowed_roots
                .iter()
                .map(|root| comparable_display(root))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

/// Whether a path names a network location (a UNC share on Windows).
pub fn is_network_path(path: &Path) -> bool {
    let text = path.to_string_lossy();
    if cfg!(windows) && (text.starts_with("//") || text.starts_with("\\\\")) && !text.starts_with("\\\\?\\") {
        return true;
    }
    matches!(
        path.components().next(),
        Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::UNC(..) | Prefix::VerbatimUNC(..))
    )
}

/// Resolves `.` and `..` without touching the filesystem.
fn normalize_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

/// A path as text that compares equal across spellings of the same folder:
/// forward slashes, no `\\?\` prefix, and case-insensitive on Windows.
fn comparable(path: &Path) -> String {
    let text = comparable_display(path);
    if cfg!(windows) {
        text.to_lowercase()
    } else {
        text
    }
}

fn comparable_display(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    let text = if let Some(rest) = text.strip_prefix("//?/UNC/") {
        format!("//{rest}")
    } else if let Some(rest) = text.strip_prefix("//?/") {
        rest.to_owned()
    } else {
        text
    };
    if text.len() > 1 {
        text.trim_end_matches('/').to_owned()
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unrestricted_policy_accepts_local_folders_but_not_shares() {
        let policy = RepoLinkPolicy::unrestricted();
        assert!(policy.check(Path::new("relative/folder")).is_ok());
        assert!(policy.check(&std::env::temp_dir()).is_ok());
        if cfg!(windows) {
            assert!(policy.check(Path::new(r"\\attacker\share\repo")).is_err());
            assert!(policy.check(Path::new("//attacker/share/repo")).is_err());
        }
    }

    #[test]
    fn restricted_policy_keeps_links_inside_the_roots() {
        let base = std::env::temp_dir().join(format!("repomemo-roots-{}", uuid_like()));
        let allowed = base.join("allowed");
        let other = base.join("other");
        std::fs::create_dir_all(allowed.join("repo")).unwrap();
        std::fs::create_dir_all(&other).unwrap();
        let policy = RepoLinkPolicy::with_allowed_roots(vec![allowed.clone()]).unwrap();

        assert!(policy.check(&allowed.join("repo")).is_ok());
        assert!(policy.check(&allowed).is_ok());
        assert!(policy.check(&other).is_err());
        assert!(policy.check(&allowed.join("..").join("other")).is_err());
        assert!(policy.check(Path::new("allowed/repo")).is_err(), "relative links are refused");
        // A sibling whose name starts like the root is outside it.
        assert!(policy.check(&base.join("allowed-not")).is_err());
        assert!(RepoLinkPolicy::with_allowed_roots(vec![base.join("missing")]).is_err());
        let _ = std::fs::remove_dir_all(base);
    }

    fn uuid_like() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }
}
