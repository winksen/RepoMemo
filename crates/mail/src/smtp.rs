//! A small SMTP submission client: TLS from the first byte (port 465) or
//! upgraded with `STARTTLS` (port 587), never plain text, with the server's
//! certificate checked against the public roots plus any the operator adds.
//!
//! Nothing here waits without a limit: connecting, every reply and the whole
//! delivery have timeouts, so a mail server that stalls costs one background
//! task for a short while, never a request.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use base64::{engine::general_purpose::STANDARD, Engine};
use rustls::pki_types::{CertificateDer, ServerName};
use rustls::{ClientConfig, RootCertStore};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_rustls::TlsConnector;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REPLY_TIMEOUT: Duration = Duration::from_secs(20);
const DELIVERY_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_LINE_BYTES: usize = 4096;
const MAX_REPLY_LINES: usize = 64;

/// How the connection is secured. There is no plain-text choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Security {
    /// TLS from the first byte (usually port 465).
    Tls,
    /// Plain greeting, then `STARTTLS` before anything else (usually port 587).
    StartTls,
}

impl Security {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "tls" => Some(Self::Tls),
            "starttls" => Some(Self::StartTls),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tls => "tls",
            Self::StartTls => "starttls",
        }
    }
}

/// The server to hand mail to.
#[derive(Clone)]
pub struct SmtpSettings {
    pub host: String,
    pub port: u16,
    pub security: Security,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl std::fmt::Debug for SmtpSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SmtpSettings")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("security", &self.security)
            .field("username", &self.username)
            .field("password", &self.password.as_ref().map(|_| "…"))
            .finish()
    }
}

/// Which addresses the server may be reached at, and which certificates it may
/// be trusted by. The server's address is looked up here and checked before
/// anything is sent: link-local and cloud-metadata addresses are never allowed
/// (see `repomemo_ai::is_forbidden_address`), and names must resolve to public
/// addresses unless the operator lists the host.
#[derive(Debug, Clone, Default)]
pub struct OutboundPolicy {
    /// Hosts allowed to resolve to non-public addresses: an exact name or
    /// address, a suffix starting with a dot, or `*` for any host.
    pub allowed_hosts: Vec<String>,
    /// Certificates (DER) trusted in addition to the public roots, for a mail
    /// server run by the operator's own authority.
    pub extra_roots: Vec<Vec<u8>>,
}

impl OutboundPolicy {
    fn lists(&self, host: &str) -> bool {
        self.allowed_hosts.iter().any(|entry| {
            let entry = entry.trim().to_ascii_lowercase();
            if entry == "*" {
                return true;
            }
            match entry.strip_prefix('.') {
                Some(suffix) => host == suffix || host.ends_with(&format!(".{suffix}")),
                None => host == entry,
            }
        })
    }
}

/// Why a delivery did not happen, and whether trying again can help.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendError {
    /// The server or the network may say yes later (4xx replies, timeouts,
    /// connection problems). A refusal of the message or of the settings is
    /// permanent.
    pub temporary: bool,
    pub message: String,
}

impl std::fmt::Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SendError {}

fn permanent(message: impl Into<String>) -> SendError {
    SendError {
        temporary: false,
        message: message.into(),
    }
}

fn temporary(message: impl Into<String>) -> SendError {
    SendError {
        temporary: true,
        message: message.into(),
    }
}

/// Refuses a host that must never be called, before any lookup.
pub fn check_host(host: &str, policy: &OutboundPolicy) -> Result<(), SendError> {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty()
        || host
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || "/@#?\\".contains(c))
    {
        return Err(permanent("The mail server name is not valid."));
    }
    if repomemo_ai::is_metadata_host(&host) {
        return Err(permanent(
            "The mail server points at a cloud metadata service, which is not allowed.",
        ));
    }
    if let Ok(address) = host.trim_matches(['[', ']']).parse::<IpAddr>() {
        if repomemo_ai::is_forbidden_address(&address) {
            return Err(permanent(format!(
                "The mail server is a link-local or reserved address ({address}), which is not allowed."
            )));
        }
        if repomemo_ai::is_non_public_address(&address) && !policy.lists(&host) {
            return Err(permanent(format!(
                "The mail server is a private address ({address}). Ask the server operator to list it in REPOMEMO_SMTP_ALLOWED_HOSTS."
            )));
        }
    }
    Ok(())
}

async fn resolve(host: &str, port: u16, policy: &OutboundPolicy) -> Result<Vec<SocketAddr>, SendError> {
    let found = tokio::net::lookup_host((host, port))
        .await
        .map_err(|error| temporary(format!("The mail server name could not be looked up: {error}")))?
        .collect::<Vec<_>>();
    if found.is_empty() {
        return Err(temporary("The mail server name has no address."));
    }
    let listed = policy.lists(host);
    let usable = found
        .into_iter()
        .filter(|address| !repomemo_ai::is_forbidden_address(&address.ip()))
        .filter(|address| listed || !repomemo_ai::is_non_public_address(&address.ip()))
        .collect::<Vec<_>>();
    if usable.is_empty() {
        return Err(permanent(format!(
            "{host} resolves only to addresses mail is not sent to (private, link-local or reserved). Ask the server operator to list it in REPOMEMO_SMTP_ALLOWED_HOSTS."
        )));
    }
    Ok(usable)
}

fn tls_connector(policy: &OutboundPolicy) -> Result<TlsConnector, SendError> {
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    for der in &policy.extra_roots {
        roots
            .add(CertificateDer::from(der.clone()))
            .map_err(|error| permanent(format!("A certificate in REPOMEMO_SMTP_CA_FILE was refused: {error}")))?;
    }
    let config = ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .map_err(|error| permanent(format!("TLS could not be set up: {error}")))?
        .with_root_certificates(roots)
        .with_no_client_auth();
    Ok(TlsConnector::from(Arc::new(config)))
}

/// One server reply: a three-digit code and its text lines.
#[derive(Debug)]
struct Reply {
    code: u16,
    lines: Vec<String>,
}

impl Reply {
    fn advertises(&self, keyword: &str) -> bool {
        self.lines.iter().skip(1).any(|line| {
            let line = line.to_ascii_uppercase();
            line == keyword || line.starts_with(&format!("{keyword} ")) || line.starts_with(&format!("{keyword}="))
        })
    }

    fn mechanisms(&self) -> Vec<String> {
        self.lines
            .iter()
            .skip(1)
            .filter_map(|line| {
                let upper = line.to_ascii_uppercase();
                upper
                    .strip_prefix("AUTH ")
                    .or_else(|| upper.strip_prefix("AUTH="))
                    .map(str::to_owned)
            })
            .flat_map(|list| list.split_whitespace().map(str::to_owned).collect::<Vec<_>>())
            .collect()
    }

    fn text(&self) -> String {
        self.lines.first().cloned().unwrap_or_default()
    }
}

/// A connection with its read buffer, so replies are read line by line and
/// nothing read past a reply is lost (or, before STARTTLS, smuggled in).
struct Wire<S> {
    stream: S,
    buffer: Vec<u8>,
}

impl<S: AsyncRead + AsyncWrite + Unpin> Wire<S> {
    fn new(stream: S) -> Self {
        Self {
            stream,
            buffer: Vec::new(),
        }
    }

    async fn line(&mut self) -> Result<String, SendError> {
        loop {
            if let Some(end) = self.buffer.iter().position(|byte| *byte == b'\n') {
                let line = self.buffer.drain(..=end).collect::<Vec<_>>();
                let text = String::from_utf8_lossy(&line);
                return Ok(text.trim_end_matches(['\r', '\n']).to_owned());
            }
            if self.buffer.len() > MAX_LINE_BYTES {
                return Err(temporary("The mail server sent a reply line that is too long."));
            }
            let mut chunk = [0_u8; 1024];
            let read = timeout(REPLY_TIMEOUT, self.stream.read(&mut chunk))
                .await
                .map_err(|_| temporary("The mail server did not answer in time."))?
                .map_err(|error| temporary(format!("The connection to the mail server failed: {error}")))?;
            if read == 0 {
                return Err(temporary("The mail server closed the connection."));
            }
            self.buffer.extend_from_slice(&chunk[..read]);
        }
    }

    async fn reply(&mut self) -> Result<Reply, SendError> {
        let mut lines = Vec::new();
        let mut code = None;
        loop {
            let line = self.line().await?;
            let digits = line
                .get(..3)
                .filter(|digits| digits.bytes().all(|byte| byte.is_ascii_digit()));
            let Some(digits) = digits else {
                return Err(temporary("The mail server sent a reply that is not SMTP."));
            };
            let parsed = digits.parse::<u16>().unwrap_or(0);
            if code.is_some_and(|expected| expected != parsed) {
                return Err(temporary("The mail server sent an inconsistent reply."));
            }
            code = Some(parsed);
            let more = line.as_bytes().get(3) == Some(&b'-');
            lines.push(line.get(4..).unwrap_or("").to_owned());
            if lines.len() > MAX_REPLY_LINES {
                return Err(temporary("The mail server sent too long a reply."));
            }
            if !more {
                return Ok(Reply { code: parsed, lines });
            }
        }
    }

    async fn write(&mut self, bytes: &[u8]) -> Result<(), SendError> {
        timeout(REPLY_TIMEOUT, async {
            self.stream.write_all(bytes).await?;
            self.stream.flush().await
        })
        .await
        .map_err(|_| temporary("The mail server did not accept data in time."))?
        .map_err(|error| temporary(format!("The connection to the mail server failed: {error}")))
    }

    async fn command(&mut self, text: &str) -> Result<Reply, SendError> {
        self.write(format!("{text}\r\n").as_bytes()).await?;
        self.reply().await
    }
}

/// Turns an unexpected reply into the error it deserves: 4xx may pass, 5xx will not.
fn unexpected(step: &str, reply: &Reply) -> SendError {
    let message = format!("The mail server refused {step}: {} {}", reply.code, reply.text());
    if (500..600).contains(&reply.code) {
        permanent(message)
    } else {
        temporary(message)
    }
}

fn expect(step: &str, reply: Reply, accepted: &[u16]) -> Result<Reply, SendError> {
    if accepted.contains(&reply.code) {
        Ok(reply)
    } else {
        Err(unexpected(step, &reply))
    }
}

/// `.` at the start of a line is doubled so the data cannot end early.
fn dot_stuffed(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + 16);
    let mut at_line_start = true;
    for &byte in data {
        if at_line_start && byte == b'.' {
            out.push(b'.');
        }
        out.push(byte);
        at_line_start = byte == b'\n';
    }
    out
}

fn identity(from: &str) -> String {
    from.rsplit('@')
        .next()
        .filter(|domain| !domain.is_empty())
        .unwrap_or("localhost")
        .to_owned()
}

async fn authenticate<S: AsyncRead + AsyncWrite + Unpin>(
    wire: &mut Wire<S>,
    settings: &SmtpSettings,
    capabilities: &Reply,
) -> Result<(), SendError> {
    let (Some(username), Some(password)) = (settings.username.as_deref(), settings.password.as_deref()) else {
        return Ok(());
    };
    if username.is_empty() {
        return Ok(());
    }
    let mechanisms = capabilities.mechanisms();
    if mechanisms.iter().any(|name| name == "PLAIN") {
        let token = STANDARD.encode(format!("\0{username}\0{password}"));
        let reply = wire.command(&format!("AUTH PLAIN {token}")).await?;
        expect("the sign-in", reply, &[235])?;
    } else if mechanisms.iter().any(|name| name == "LOGIN") {
        expect("the sign-in", wire.command("AUTH LOGIN").await?, &[334])?;
        let reply = wire.command(&STANDARD.encode(username)).await?;
        expect("the sign-in", reply, &[334])?;
        let reply = wire.command(&STANDARD.encode(password)).await?;
        expect("the sign-in", reply, &[235])?;
    } else {
        return Err(permanent(
            "The mail server offers no sign-in method this server supports (PLAIN or LOGIN).",
        ));
    }
    Ok(())
}

async fn transaction<S: AsyncRead + AsyncWrite + Unpin>(
    wire: &mut Wire<S>,
    from: &str,
    to: &str,
    data: &[u8],
) -> Result<(), SendError> {
    expect(
        "the sender",
        wire.command(&format!("MAIL FROM:<{from}>")).await?,
        &[250],
    )?;
    expect(
        "the recipient",
        wire.command(&format!("RCPT TO:<{to}>")).await?,
        &[250, 251],
    )?;
    expect("the message", wire.command("DATA").await?, &[354])?;
    let mut body = dot_stuffed(data);
    if !body.ends_with(b"\r\n") {
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(b".\r\n");
    wire.write(&body).await?;
    expect("the message", wire.reply().await?, &[250])?;
    Ok(())
}

async fn handshake(
    connector: &TlsConnector,
    host: &str,
    stream: TcpStream,
) -> Result<tokio_rustls::client::TlsStream<TcpStream>, SendError> {
    let name = ServerName::try_from(host.trim_matches(['[', ']']).to_owned())
        .map_err(|_| permanent("The mail server name is not valid for TLS."))?;
    timeout(CONNECT_TIMEOUT, connector.connect(name, stream))
        .await
        .map_err(|_| temporary("The TLS handshake with the mail server timed out."))?
        .map_err(|error| temporary(format!("The mail server's TLS certificate was not accepted: {error}")))
}

/// Hands one message to the server. `data` is a composed message (see
/// `compose`); `from` and `to` are the envelope addresses.
pub async fn send(
    settings: &SmtpSettings,
    policy: &OutboundPolicy,
    from: &str,
    to: &str,
    data: &[u8],
) -> Result<(), SendError> {
    match timeout(DELIVERY_TIMEOUT, deliver(settings, policy, from, to, data)).await {
        Ok(result) => result,
        Err(_) => Err(temporary("Handing the message to the mail server took too long.")),
    }
}

async fn deliver(
    settings: &SmtpSettings,
    policy: &OutboundPolicy,
    from: &str,
    to: &str,
    data: &[u8],
) -> Result<(), SendError> {
    let host = settings.host.trim().trim_end_matches('.').to_ascii_lowercase();
    check_host(&host, policy)?;
    let connector = tls_connector(policy)?;
    let addresses = resolve(host.trim_matches(['[', ']']), settings.port, policy).await?;
    let mut last_error = None;
    let mut connected = None;
    for address in addresses {
        match timeout(CONNECT_TIMEOUT, TcpStream::connect(address)).await {
            Ok(Ok(stream)) => {
                connected = Some(stream);
                break;
            }
            Ok(Err(error)) => last_error = Some(format!("{error}")),
            Err(_) => last_error = Some("the connection timed out".to_owned()),
        }
    }
    let Some(tcp) = connected else {
        return Err(temporary(format!(
            "The mail server could not be reached: {}",
            last_error.unwrap_or_else(|| "no address answered".to_owned())
        )));
    };
    let name = identity(from);

    match settings.security {
        Security::Tls => {
            let tls = handshake(&connector, &host, tcp).await?;
            let mut wire = Wire::new(tls);
            expect("the connection", wire.reply().await?, &[220])?;
            let capabilities = expect("EHLO", wire.command(&format!("EHLO {name}")).await?, &[250])?;
            authenticate(&mut wire, settings, &capabilities).await?;
            transaction(&mut wire, from, to, data).await?;
            let _ = wire.command("QUIT").await;
        }
        Security::StartTls => {
            let mut plain = Wire::new(tcp);
            expect("the connection", plain.reply().await?, &[220])?;
            let capabilities = expect("EHLO", plain.command(&format!("EHLO {name}")).await?, &[250])?;
            if !capabilities.advertises("STARTTLS") {
                return Err(permanent(
                    "The mail server does not offer STARTTLS, and mail is only sent over TLS. Use port 465 with TLS, or another server.",
                ));
            }
            expect("STARTTLS", plain.command("STARTTLS").await?, &[220])?;
            // Anything the server sent after its answer would be read as part
            // of the encrypted session: refuse it.
            if !plain.buffer.is_empty() {
                return Err(permanent("The mail server sent data before the TLS handshake."));
            }
            let tls = handshake(&connector, &host, plain.stream).await?;
            let mut wire = Wire::new(tls);
            let capabilities = expect("EHLO", wire.command(&format!("EHLO {name}")).await?, &[250])?;
            authenticate(&mut wire, settings, &capabilities).await?;
            transaction(&mut wire, from, to, data).await?;
            let _ = wire.command("QUIT").await;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_starting_with_a_dot_is_doubled() {
        assert_eq!(dot_stuffed(b"a\r\n.b\r\n..c\r\n"), b"a\r\n..b\r\n...c\r\n");
        assert_eq!(dot_stuffed(b".start"), b"..start");
        assert_eq!(dot_stuffed(b"no dots here"), b"no dots here");
    }

    #[test]
    fn hosts_that_must_never_be_called_are_refused_before_any_lookup() {
        let policy = OutboundPolicy::default();
        for host in [
            "169.254.169.254",
            "metadata.google.internal",
            "[fe80::1]",
            "0.0.0.0",
            "",
            "smtp.example.com/evil",
            "user@smtp.example.com",
        ] {
            assert!(check_host(host, &policy).is_err(), "{host:?}");
        }
        // Private and loopback addresses need the operator's say.
        for host in ["127.0.0.1", "10.0.0.5", "192.168.1.20", "[::1]"] {
            let error = check_host(host, &policy).unwrap_err();
            assert!(
                !error.temporary && error.message.contains("REPOMEMO_SMTP_ALLOWED_HOSTS"),
                "{host}"
            );
        }
        let listed = OutboundPolicy {
            allowed_hosts: vec!["127.0.0.1".to_owned(), ".corp.example".to_owned()],
            ..OutboundPolicy::default()
        };
        assert!(check_host("127.0.0.1", &listed).is_ok());
        assert!(check_host("10.0.0.5", &listed).is_err());
        assert!(check_host("smtp.example.com", &policy).is_ok());
        assert!(listed.lists("mail.corp.example") && listed.lists("corp.example"));
        assert!(!listed.lists("evilcorp.example"));
        // A listed host still cannot be a metadata address.
        let everything = OutboundPolicy {
            allowed_hosts: vec!["*".to_owned()],
            ..OutboundPolicy::default()
        };
        assert!(check_host("10.0.0.5", &everything).is_ok());
        assert!(check_host("169.254.169.254", &everything).is_err());
        assert!(check_host("metadata.google.internal", &everything).is_err());
    }

    #[test]
    fn replies_are_classified_by_their_first_digit() {
        let reply = |code| Reply {
            code,
            lines: vec!["text".to_owned()],
        };
        assert!(unexpected("the recipient", &reply(450)).temporary);
        assert!(unexpected("the recipient", &reply(452)).temporary);
        assert!(!unexpected("the recipient", &reply(550)).temporary);
        assert!(!unexpected("the sign-in", &reply(535)).temporary);
        assert!(unexpected("the connection", &reply(421)).temporary);
        let capabilities = Reply {
            code: 250,
            lines: vec![
                "host".to_owned(),
                "STARTTLS".to_owned(),
                "AUTH PLAIN LOGIN".to_owned(),
                "SIZE 10000".to_owned(),
            ],
        };
        assert!(capabilities.advertises("STARTTLS") && capabilities.advertises("SIZE"));
        assert!(!capabilities.advertises("PIPELINING"));
        assert_eq!(capabilities.mechanisms(), ["PLAIN", "LOGIN"]);
    }

    #[test]
    fn the_password_is_not_printed() {
        let settings = SmtpSettings {
            host: "smtp.example.com".to_owned(),
            port: 587,
            security: Security::StartTls,
            username: Some("memo".to_owned()),
            password: Some("hunter2-hunter2".to_owned()),
        };
        assert!(!format!("{settings:?}").contains("hunter2"));
    }
}
