//! Where remote repository links may point.
//!
//! A remote link makes the server fetch from an address a workspace
//! administrator typed, so the same care applies as for any outbound
//! connection (roadmap S6a, decision 11): the operator names the hosts
//! (`REPOMEMO_GIT_ALLOWED_HOSTS`, and a server with none has remote links
//! turned off), only `https` and `ssh` are spoken, and an address cannot carry
//! a credential, an option for git or a local path. The check runs before any
//! network call and is the same for every refusal.
//!
//! git resolves host names itself when it connects, so this check is advisory
//! for names that resolve to a private address: an egress rule or a proxy on
//! the server is what enforces it. Addresses written as literals are checked
//! here, and link-local and cloud-metadata ones are never allowed.

use std::net::IpAddr;

use anyhow::Result;

/// The longest address accepted.
const MAX_ADDRESS_CHARS: usize = 2_048;

/// How a remote repository is reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteScheme {
    Https,
    Ssh,
}

impl RemoteScheme {
    pub fn label(self) -> &'static str {
        match self {
            Self::Https => "https",
            Self::Ssh => "ssh",
        }
    }

    fn default_port(self) -> u16 {
        match self {
            Self::Https => 443,
            Self::Ssh => 22,
        }
    }
}

/// An address that passed the policy, in a normal form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteRepository {
    pub scheme: RemoteScheme,
    /// Lower case, without brackets for an IPv6 literal.
    pub host: String,
    pub port: Option<u16>,
    /// The account part of an `ssh` address (`git@…`), if any. Never a secret.
    pub user: Option<String>,
    /// The repository's path on the host, without a leading or trailing `/`.
    pub path: String,
}

impl RemoteRepository {
    /// The address as git is given it: no credentials, ever.
    pub fn url(&self) -> String {
        let host = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        let port = self.port.map(|port| format!(":{port}")).unwrap_or_default();
        match self.scheme {
            RemoteScheme::Https => format!("https://{host}{port}/{}", self.path),
            RemoteScheme::Ssh => {
                let user = self.user.as_deref().unwrap_or("git");
                format!("ssh://{user}@{host}{port}/{}", self.path)
            }
        }
    }
}

/// The hosts remote links may use. Without any, remote links are off.
#[derive(Debug, Clone, Default)]
pub struct RemoteGitPolicy {
    /// Lower case entries: `host`, `host:port`, `*.suffix` or `*`.
    allowed: Vec<String>,
}

impl RemoteGitPolicy {
    /// Remote links turned off: what a server has until its operator names hosts.
    pub fn disabled() -> Self {
        Self::default()
    }

    /// Only these hosts. An empty list is the same as [`RemoteGitPolicy::disabled`].
    pub fn with_allowed_hosts(hosts: Vec<String>) -> Self {
        Self {
            allowed: hosts
                .into_iter()
                .map(|host| host.trim().to_ascii_lowercase())
                .filter(|host| !host.is_empty())
                .collect(),
        }
    }

    pub fn is_enabled(&self) -> bool {
        !self.allowed.is_empty()
    }

    pub fn allowed_hosts(&self) -> &[String] {
        &self.allowed
    }

    /// Checks an address typed in a link before anything is contacted.
    pub fn check(&self, raw: &str) -> Result<RemoteRepository> {
        if !self.is_enabled() {
            repomemo_domain::bail_invalid!("Remote repository links are turned off on this server. Its operator turns them on by setting REPOMEMO_GIT_ALLOWED_HOSTS to the hosts they may use.");
        }
        let repository = parse(raw)?;
        if let Ok(address) = repository.host.parse::<IpAddr>() {
            if repomemo_ai::is_forbidden_address(&address) {
                repomemo_domain::bail_invalid!("This address is never allowed as a repository host.");
            }
        } else if repomemo_ai::is_metadata_host(&repository.host) {
            repomemo_domain::bail_invalid!("This address is never allowed as a repository host.");
        }
        if !self.allows(&repository) {
            repomemo_domain::bail_invalid!(
                "This server only links remote repositories on: {}.",
                self.allowed.join(", ")
            );
        }
        Ok(repository)
    }

    fn allows(&self, repository: &RemoteRepository) -> bool {
        let port = repository.port.unwrap_or_else(|| repository.scheme.default_port());
        self.allowed.iter().any(|entry| {
            if entry == "*" {
                return true;
            }
            let (host, entry_port) = split_entry(entry);
            if entry_port.is_some_and(|wanted| wanted != port) {
                return false;
            }
            match host.strip_prefix("*.") {
                Some(suffix) => {
                    repository.host.len() > suffix.len() + 1 && repository.host.ends_with(&format!(".{suffix}"))
                }
                None => repository.host == host,
            }
        })
    }
}

/// An allowed-host entry as its host and the port it is limited to, if any:
/// `host`, `host:port`, `[v6]` and `[v6]:port`.
fn split_entry(entry: &str) -> (String, Option<u16>) {
    if let Some(rest) = entry.strip_prefix('[') {
        if let Some((host, after)) = rest.split_once(']') {
            return (host.to_owned(), after.strip_prefix(':').and_then(|port| port.parse().ok()));
        }
    }
    if entry.matches(':').count() == 1 {
        if let Some((host, port)) = entry.split_once(':') {
            if let Ok(port) = port.parse::<u16>() {
                return (host.to_owned(), Some(port));
            }
        }
    }
    (entry.to_owned(), None)
}

fn refusal(reason: &str) -> anyhow::Error {
    repomemo_domain::invalid!("{reason}")
}

/// Splits and validates an address without looking at the allowed hosts.
fn parse(raw: &str) -> Result<RemoteRepository> {
    let text = raw.trim();
    if text.is_empty() {
        return Err(refusal("Enter the address of the repository."));
    }
    if text.chars().count() > MAX_ADDRESS_CHARS || text.chars().any(|c| c.is_control() || c.is_whitespace() || c == '\\') {
        return Err(refusal("That is not a repository address."));
    }
    // git's own transports (`ext::`, `fd::`, helpers) and options.
    let head = text.split('/').next().unwrap_or_default();
    let transport_marker = head.find("::").is_some_and(|at| !head[..at].contains('['));
    if transport_marker || text.starts_with('-') {
        return Err(refusal("Only https and ssh addresses can be linked."));
    }
    let (scheme, authority_and_path) = match text.split_once("://") {
        Some((scheme, rest)) => match scheme.to_ascii_lowercase().as_str() {
            "https" => (RemoteScheme::Https, rest.to_owned()),
            "ssh" => (RemoteScheme::Ssh, rest.to_owned()),
            _ => return Err(refusal("Only https and ssh addresses can be linked.")),
        },
        None => {
            // `git@host:owner/repo.git`, the form hosts show for ssh.
            let Some((before, after)) = text.split_once(':') else {
                return Err(refusal("Only https and ssh addresses can be linked."));
            };
            if before.contains('/') || after.is_empty() {
                // A local path, or nothing after the colon.
                return Err(refusal("Only https and ssh addresses can be linked."));
            }
            (RemoteScheme::Ssh, format!("{before}/{}", after.trim_start_matches('/')))
        }
    };
    let (authority, path) = authority_and_path
        .split_once('/')
        .ok_or_else(|| refusal("The address needs the repository's path after the host."))?;
    let (user, host_port) = match authority.rsplit_once('@') {
        Some((user, host_port)) => (Some(user), host_port),
        None => (None, authority),
    };
    if let Some(user) = user {
        // A credential does not belong in an address: it goes in the link's
        // credential, which is stored encrypted and never shown again.
        if scheme == RemoteScheme::Https || user.contains(':') || user.is_empty() {
            return Err(refusal(
                "Leave credentials out of the address: a token or key is entered separately, and is stored encrypted.",
            ));
        }
        if !user.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')) || user.starts_with('-') {
            return Err(refusal("That is not a valid account name for the address."));
        }
    }
    let (host, port) = split_host_port(host_port)?;
    validate_host(&host)?;
    let path = path.trim_matches('/');
    if path.is_empty()
        || path.split('/').any(|segment| segment.is_empty() || segment == "." || segment == "..")
        || path.contains(['?', '#'])
        || !path
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/' | '~' | '%' | '+' | '@'))
    {
        return Err(refusal("That is not a repository path."));
    }
    Ok(RemoteRepository {
        scheme,
        host,
        port,
        user: user.map(str::to_owned),
        path: path.to_owned(),
    })
}

fn split_host_port(text: &str) -> Result<(String, Option<u16>)> {
    if let Some(rest) = text.strip_prefix('[') {
        // [v6]:port
        let (host, after) = rest
            .split_once(']')
            .ok_or_else(|| refusal("That is not a valid host."))?;
        let port = match after.strip_prefix(':') {
            Some(port) => Some(parse_port(port)?),
            None if after.is_empty() => None,
            None => return Err(refusal("That is not a valid host.")),
        };
        return Ok((host.to_ascii_lowercase(), port));
    }
    match text.rsplit_once(':') {
        Some((host, port)) => Ok((host.to_ascii_lowercase(), Some(parse_port(port)?))),
        None => Ok((text.to_ascii_lowercase(), None)),
    }
}

fn parse_port(text: &str) -> Result<u16> {
    text.parse::<u16>()
        .ok()
        .filter(|port| *port > 0)
        .ok_or_else(|| refusal("The port is a number from 1 to 65535."))
}

fn validate_host(host: &str) -> Result<()> {
    if host.parse::<IpAddr>().is_ok() {
        return Ok(());
    }
    let valid = !host.is_empty()
        && host.len() <= 253
        && !host.starts_with('-')
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        });
    if valid {
        Ok(())
    } else {
        Err(refusal("That is not a valid host."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(hosts: &[&str]) -> RemoteGitPolicy {
        RemoteGitPolicy::with_allowed_hosts(hosts.iter().map(|host| (*host).to_owned()).collect())
    }

    #[test]
    fn remote_links_are_off_until_hosts_are_named() {
        let off = RemoteGitPolicy::disabled();
        assert!(!off.is_enabled());
        let error = off.check("https://github.com/acme/app.git").unwrap_err().to_string();
        assert!(error.contains("REPOMEMO_GIT_ALLOWED_HOSTS"), "{error}");
        assert!(!RemoteGitPolicy::with_allowed_hosts(vec![" ".into()]).is_enabled());
    }

    #[test]
    fn only_https_and_ssh_to_allowed_hosts_pass_in_a_normal_form() {
        let policy = policy(&["github.com", "*.corp.example", "git.internal:2222"]);
        let https = policy.check("https://GitHub.com/acme/app.git/").unwrap();
        assert_eq!(https.url(), "https://github.com/acme/app.git");
        let scp = policy.check("git@github.com:acme/app.git").unwrap();
        assert_eq!((scp.scheme, scp.url()), (RemoteScheme::Ssh, "ssh://git@github.com/acme/app.git".to_owned()));
        assert!(policy.check("ssh://deploy@code.corp.example:22/team/app").is_ok());
        assert!(policy.check("https://git.internal:2222/team/app").is_ok());
        for refused in [
            "https://git.internal/team/app",
            "https://corp.example/team/app",
            "https://evilgithub.com/acme/app",
            "https://github.com.evil.example/acme/app",
            "http://github.com/acme/app",
            "git://github.com/acme/app",
            "file:///srv/repos/app",
            "/srv/repos/app",
            "C:\\repos\\app",
            "ext::sh -c touch /tmp/x",
            "--upload-pack=touch /tmp/x",
            "-oProxyCommand=x@github.com:acme/app",
            "https://user:token@github.com/acme/app",
            "https://token@github.com/acme/app",
            "https://github.com/acme/../secret",
            "https://github.com/",
            "https://github.com",
            "https://github.com/acme/app?x=1",
            "https://git hub.com/acme/app",
            "https://-github.com/acme/app",
        ] {
            assert!(policy.check(refused).is_err(), "{refused} should be refused");
        }
    }

    #[test]
    fn metadata_and_link_local_addresses_are_never_allowed_and_private_ones_need_naming() {
        let any = policy(&["*"]);
        assert!(any.check("https://169.254.169.254/latest/meta-data").is_err());
        assert!(any.check("https://metadata.google.internal/computeMetadata").is_err());
        assert!(any.check("https://[fe80::1]/x/y").is_err());
        assert!(any.check("https://git.example.com/acme/app").is_ok());
        let named = policy(&["10.1.2.3"]);
        assert!(named.check("https://10.1.2.3/acme/app").is_ok());
        assert!(named.check("https://10.1.2.4/acme/app").is_err());
        assert!(policy(&["[2001:db8::1]"]).check("https://[2001:db8::1]:8443/a/b").is_ok());
        assert!(policy(&["[2001:db8::1]:443"]).check("https://[2001:db8::1]:8443/a/b").is_err());
        assert!(policy(&["[2001:db8::1]:443"]).check("https://[2001:db8::1]/a/b").is_ok());
    }
}
