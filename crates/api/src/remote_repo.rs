//! Remote repository links (roadmap F4 phase 2, V0.2.20 and V0.2.21).
//!
//! A remote link is an `https` address (a token optional, for a private
//! repository) or an `ssh` address (a deploy key and the host's key, both
//! required) that a workspace administrator typed. The server checks the
//! address against `REPOMEMO_GIT_ALLOWED_HOSTS` before any network call, keeps
//! the token or key sealed in the credential store, and keeps a bare, partial
//! clone of the branch under its data folder. The sync engine then reads that
//! clone like any local repository, so the files, renames, overview and
//! indexing are the same; only getting the commits is different
//! (`refresh_remote_clone`).

use std::path::PathBuf;

use anyhow::Result;
use repomemo_domain::{RepoAccessCheck, RepoCommit, RepoSettings, RepoSource, SourceType};
use repomemo_git::remote::{self, RemoteCredential, RemoteTarget, SshAuth};
use repomemo_git::GitRepo;

use crate::remote_policy::{RemoteRepository, RemoteScheme};
use crate::repo_sync::{normalize_settings, RemoteLink, RepoSourceMetadata};
use crate::RepoMemoCore;

/// The folder under the data folder that holds the server's copies.
const CLONES_DIR: &str = "remote-repos";

/// Whether a link names a repository on a host rather than a folder.
pub fn looks_remote(link: &str) -> bool {
    let lower = link.trim().trim_matches('"').trim().to_ascii_lowercase();
    ["http://", "https://", "ssh://", "git://"]
        .iter()
        .any(|prefix| lower.starts_with(prefix))
        || (lower.contains('@') && lower.contains(':') && !lower.contains("://") && !lower.contains('\\'))
}

/// A token typed with a link, for a private repository.
#[derive(Clone)]
pub struct RemoteToken {
    pub username: Option<String>,
    secret: String,
}

impl RemoteToken {
    pub fn new(username: Option<String>, secret: String) -> Option<Self> {
        let secret = secret.trim().to_owned();
        (!secret.is_empty()).then_some(Self { username, secret })
    }
}

impl std::fmt::Debug for RemoteToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteToken")
            .field("username", &self.username)
            .field("secret", &"<hidden>")
            .finish()
    }
}

/// A deploy key (a private key in PEM or OpenSSH form) with the public key of
/// the host it is used against.
#[derive(Clone)]
pub struct RemoteDeployKey {
    private_key: String,
    /// What was pasted: the host's key as `ssh-keyscan` prints it.
    pub host_key: String,
}

impl RemoteDeployKey {
    pub fn new(private_key: String, host_key: String) -> Option<Self> {
        let private_key = private_key.trim().to_owned();
        (!private_key.is_empty()).then_some(Self {
            private_key,
            host_key: host_key.trim().to_owned(),
        })
    }
}

impl std::fmt::Debug for RemoteDeployKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteDeployKey")
            .field("private_key", &"<hidden>")
            .field("host_key", &self.host_key)
            .finish()
    }
}

/// What was typed with a link to sign in to it.
#[derive(Debug, Clone, Default)]
pub struct RemoteAuth {
    pub token: Option<RemoteToken>,
    pub deploy_key: Option<RemoteDeployKey>,
}

/// The name a remote repository gets unless another is chosen: its last path
/// segment without `.git`.
fn name_from_path(path: &str) -> String {
    let last = path.rsplit('/').next().unwrap_or(path);
    let name = last.strip_suffix(".git").unwrap_or(last);
    if name.is_empty() {
        "Repository".to_owned()
    } else {
        name.to_owned()
    }
}

/// The `known_hosts` lines for a link's host from what was pasted.
fn pinned_host_keys(repository: &RemoteRepository, pasted: &str) -> Result<String> {
    remote::known_hosts_lines(&repository.host, repository.port.unwrap_or(22), pasted)
}

impl RepoMemoCore {
    /// The server's copy of a remote repository.
    pub(crate) fn remote_clone_dir(&self, source_id: &str) -> PathBuf {
        self.storage
            .data_dir()
            .join(CLONES_DIR)
            .join(format!("{source_id}.git"))
    }

    /// Where to fetch a linked repository from and how to sign in. The address
    /// goes through the host policy every time, so narrowing
    /// `REPOMEMO_GIT_ALLOWED_HOSTS` also stops syncs of older links.
    pub(crate) async fn remote_target(&self, source_id: &str, link: &RemoteLink) -> Result<RemoteTarget> {
        let repository = self.remote_git_policy.check(&link.url)?;
        let stored = self.storage.open_repo_credential(source_id).await?;
        match repository.scheme {
            RemoteScheme::Https => Ok(RemoteTarget {
                url: repository.url(),
                credential: stored
                    .filter(|secret| secret.kind == "token")
                    .map(|secret| RemoteCredential::token(secret.username.as_deref(), secret.expose())),
                ssh: None,
            }),
            RemoteScheme::Ssh => {
                let Some(key) = stored.filter(|secret| secret.kind == "deploy_key") else {
                    repomemo_domain::bail_invalid!(
                        "This repository has no deploy key (or it could not be read). Enter it again in its settings."
                    );
                };
                let Some(host_key) = link.host_key.as_deref() else {
                    repomemo_domain::bail_invalid!("This repository has no pinned host key. Enter it in its settings.");
                };
                Ok(RemoteTarget {
                    url: repository.url(),
                    credential: None,
                    ssh: Some(SshAuth::new(key.expose(), pinned_host_keys(&repository, host_key)?)),
                })
            }
        }
    }

    /// Brings the server's copy of a branch up to date (cloning it the first
    /// time) and opens it.
    pub(crate) async fn refresh_remote_clone(
        &self,
        source: &repomemo_domain::Source,
        link: &RemoteLink,
        branch: Option<&str>,
    ) -> Result<GitRepo> {
        let target = self.remote_target(&source.id, link).await?;
        let head = remote::remote_head(&target, branch).await?;
        let directory = self.remote_clone_dir(&source.id);
        if directory.is_dir() {
            remote::fetch_branch(&target, &directory, &head.branch).await?;
        } else {
            remote::clone_bare(&target, &directory, &head.branch).await?;
        }
        GitRepo::open_bare(&directory).await
    }

    /// The target for a link that has not been stored yet, from what was typed.
    fn target_from_input(repository: &RemoteRepository, auth: &RemoteAuth) -> Result<RemoteTarget> {
        match repository.scheme {
            RemoteScheme::Https => Ok(RemoteTarget {
                url: repository.url(),
                credential: auth
                    .token
                    .as_ref()
                    .map(|token| RemoteCredential::token(token.username.as_deref(), &token.secret)),
                ssh: None,
            }),
            RemoteScheme::Ssh => {
                let Some(key) = &auth.deploy_key else {
                    repomemo_domain::bail_invalid!(
                        "An ssh address needs a deploy key and the host's public key (ssh-keyscan -t ed25519 {}).",
                        repository.host
                    );
                };
                Ok(RemoteTarget {
                    url: repository.url(),
                    credential: None,
                    ssh: Some(SshAuth::new(&key.private_key, pinned_host_keys(repository, &key.host_key)?)),
                })
            }
        }
    }

    /// Looks at a remote link before it is connected: whether the host knows
    /// the repository and the token or key can read it, and which commit the
    /// branch is at. Nothing is stored and nothing is cloned.
    pub async fn check_remote_repo_link(
        &self,
        workspace_id: &str,
        link: &str,
        settings: RepoSettings,
        auth: RemoteAuth,
    ) -> Result<RepoAccessCheck> {
        if !self.storage.workspace_exists(workspace_id).await? {
            repomemo_domain::bail_not_found!("Workspace was not found.");
        }
        let settings = normalize_settings(settings)?;
        let repository = self.remote_git_policy.check(link)?;
        let target = Self::target_from_input(&repository, &auth)?;
        let url = target.url.clone();
        let head = remote::remote_head(&target, settings.branch.as_deref()).await?;
        let already_connected = self
            .storage
            .list_sources_of_type(Some(workspace_id), SourceType::GitRepo)
            .await?
            .iter()
            .any(|source| source.root_uri.as_deref() == Some(url.as_str()));
        Ok(RepoAccessCheck {
            name: name_from_path(&repository.path),
            root_path: url,
            remote: true,
            commit: RepoCommit {
                sha: head.commit,
                summary: String::new(),
                author_name: String::new(),
                committed_at: String::new(),
                branch: Some(head.branch),
            },
            tracked_files: 0,
            indexable_files: 0,
            already_connected,
        })
    }

    /// Connects a remote repository to a workspace. The host is asked once, so
    /// a wrong address, branch, token or key fails now and not on the first
    /// sync; nothing is cloned until a sync runs.
    pub async fn connect_remote_repo(
        &self,
        workspace_id: &str,
        link: &str,
        name: Option<String>,
        settings: RepoSettings,
        auth: RemoteAuth,
        actor_user_id: Option<&str>,
    ) -> Result<RepoSource> {
        let check = self
            .check_remote_repo_link(workspace_id, link, settings.clone(), auth.clone())
            .await?;
        let settings = normalize_settings(settings)?;
        let name = name
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or(check.name);
        let metadata = RepoSourceMetadata {
            settings,
            remote: Some(RemoteLink {
                url: check.root_path.clone(),
                host_key: auth.deploy_key.as_ref().map(|key| key.host_key.clone()),
            }),
            ..Default::default()
        };
        let source = self
            .storage
            .create_repo_source(workspace_id, &name, &check.root_path, &metadata.to_value())
            .await?;
        let stored = if let Some(key) = &auth.deploy_key {
            Some(
                self.storage
                    .set_repo_credential(&source.id, "deploy_key", None, &key.private_key, actor_user_id)
                    .await,
            )
        } else if let Some(token) = &auth.token {
            Some(
                self.storage
                    .set_repo_credential(&source.id, "token", token.username.as_deref(), &token.secret, actor_user_id)
                    .await,
            )
        } else {
            None
        };
        if let Some(Err(error)) = stored {
            // A link without the credential it was given would only fail later.
            let _ = self.storage.delete_source(&source.id).await;
            return Err(error);
        }
        self.repo_source(&source.id).await
    }

    /// Replaces the token of a remote repository.
    pub async fn set_repo_token(&self, source_id: &str, token: RemoteToken, actor_user_id: Option<&str>) -> Result<()> {
        let source = self.repo_source_row(source_id).await?;
        let metadata = RepoSourceMetadata::of(&source);
        let Some(remote) = metadata.remote else {
            repomemo_domain::bail_invalid!("Only a remote repository has a token.");
        };
        let repository = self.remote_git_policy.check(&remote.url)?;
        if repository.scheme != RemoteScheme::Https {
            repomemo_domain::bail_invalid!("An ssh repository uses a deploy key, not a token.");
        }
        // The new token has to work before it replaces the old one.
        let target = RemoteTarget {
            url: repository.url(),
            credential: Some(RemoteCredential::token(token.username.as_deref(), &token.secret)),
            ssh: None,
        };
        remote::remote_head(&target, metadata.settings.branch.as_deref()).await?;
        self.storage
            .set_repo_credential(source_id, "token", token.username.as_deref(), &token.secret, actor_user_id)
            .await?;
        Ok(())
    }

    /// Replaces the deploy key of an `ssh` repository, and its pinned host key
    /// when one is given. The new key has to work before it replaces the old.
    pub async fn set_repo_deploy_key(
        &self,
        source_id: &str,
        key: RemoteDeployKey,
        actor_user_id: Option<&str>,
    ) -> Result<()> {
        let source = self.repo_source_row(source_id).await?;
        let mut metadata = RepoSourceMetadata::of(&source);
        let Some(mut remote) = metadata.remote.clone() else {
            repomemo_domain::bail_invalid!("Only a remote repository has a deploy key.");
        };
        let repository = self.remote_git_policy.check(&remote.url)?;
        if repository.scheme != RemoteScheme::Ssh {
            repomemo_domain::bail_invalid!("An https repository uses a token, not a deploy key.");
        }
        let host_key = if key.host_key.is_empty() {
            remote.host_key.clone().unwrap_or_default()
        } else {
            key.host_key.clone()
        };
        let auth = RemoteAuth {
            token: None,
            deploy_key: Some(RemoteDeployKey {
                private_key: key.private_key.clone(),
                host_key: host_key.clone(),
            }),
        };
        let target = Self::target_from_input(&repository, &auth)?;
        remote::remote_head(&target, metadata.settings.branch.as_deref()).await?;
        self.storage
            .set_repo_credential(source_id, "deploy_key", None, &key.private_key, actor_user_id)
            .await?;
        if remote.host_key.as_deref() != Some(host_key.as_str()) {
            remote.host_key = Some(host_key);
            metadata.remote = Some(remote);
            self.storage
                .update_source(source_id, &source.name, &source.status, &metadata.to_value(), false)
                .await?;
        }
        Ok(())
    }

    /// Removes the token of a remote repository (it is then fetched without
    /// signing in).
    pub async fn clear_repo_token(&self, source_id: &str) -> Result<bool> {
        let source = self.repo_source_row(source_id).await?;
        let Some(remote) = RepoSourceMetadata::of(&source).remote else {
            repomemo_domain::bail_invalid!("Only a remote repository has a token.");
        };
        if self.remote_git_policy.check(&remote.url)?.scheme != RemoteScheme::Https {
            repomemo_domain::bail_invalid!("An ssh repository needs its deploy key: replace it instead of removing it.");
        }
        self.storage.clear_repo_credential(source_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_of_hosts_are_told_from_folders() {
        for remote in [
            "https://github.com/acme/app.git",
            "HTTPS://github.com/acme/app",
            "ssh://git@github.com/acme/app",
            "git@github.com:acme/app.git",
            "git://github.com/acme/app",
        ] {
            assert!(looks_remote(remote), "{remote}");
        }
        for local in ["C:\\code\\app", "/srv/code/app", "\"C:\\Users\\me\\app\"", "file:///srv/app", "../app"] {
            assert!(!looks_remote(local), "{local}");
        }
        assert_eq!(name_from_path("acme/app.git"), "app");
        assert_eq!(name_from_path("group/sub/service"), "service");
    }

    #[test]
    fn secrets_are_hidden_in_debug_output_and_empty_ones_are_none() {
        let token = RemoteToken::new(Some("oauth2".to_owned()), " glpat-secret ".to_owned()).unwrap();
        assert!(!format!("{token:?}").contains("glpat-secret"));
        assert!(RemoteToken::new(None, "   ".to_owned()).is_none());
        let key = RemoteDeployKey::new("-----BEGIN KEY-----\nPRIVATEBITS".to_owned(), "ssh-ed25519 AAAA".to_owned()).unwrap();
        assert!(!format!("{key:?}").contains("PRIVATEBITS"));
        assert!(RemoteDeployKey::new("  ".to_owned(), String::new()).is_none());
    }
}
