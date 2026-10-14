//! Deliveries against a scripted SMTP server that speaks TLS with a test
//! certificate, so the real client code runs end to end.

use std::sync::{Arc, Mutex};

use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

use crate::test_certificates::{CA_PEM, SERVER_CERT_PEM, SERVER_KEY_PEM};
use crate::{compose, send, OutboundPolicy, Outgoing, Security, SmtpSettings};

/// What the scripted server should do.
#[derive(Clone)]
struct Script {
    /// Offer STARTTLS (when the connection is not TLS already).
    offer_starttls: bool,
    /// Code answered to `RCPT TO` instead of 250.
    rcpt_code: u16,
    /// Mechanisms advertised after TLS.
    auth: &'static str,
    /// The user and password that sign in.
    credentials: (&'static str, &'static str),
}

impl Default for Script {
    fn default() -> Self {
        Self {
            offer_starttls: true,
            rcpt_code: 250,
            auth: "PLAIN LOGIN",
            credentials: ("memo", "app-password-1"),
        }
    }
}

#[derive(Default, Debug)]
struct Seen {
    commands: Vec<String>,
    messages: Vec<String>,
    /// Whether AUTH was tried before TLS.
    auth_in_clear: bool,
}

fn acceptor() -> TlsAcceptor {
    let certificates = CertificateDer::pem_slice_iter(SERVER_CERT_PEM.as_bytes())
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let key = PrivateKeyDer::from_pem_slice(SERVER_KEY_PEM.as_bytes()).unwrap();
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(certificates, key)
        .unwrap();
    TlsAcceptor::from(Arc::new(config))
}

fn trusted() -> OutboundPolicy {
    OutboundPolicy {
        allowed_hosts: vec!["127.0.0.1".to_owned()],
        extra_roots: CertificateDer::pem_slice_iter(CA_PEM.as_bytes())
            .map(|certificate| certificate.unwrap().to_vec())
            .collect(),
    }
}

async fn serve_session<S: AsyncRead + AsyncWrite + Unpin>(
    stream: S,
    script: &Script,
    seen: &Arc<Mutex<Seen>>,
    in_tls: bool,
) -> Option<S> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let mut signed_in_user = false;
    loop {
        line.clear();
        if reader.read_line(&mut line).await.ok()? == 0 {
            return None;
        }
        let command = line.trim_end().to_owned();
        let upper = command.to_ascii_uppercase();
        seen.lock().unwrap().commands.push(if upper.starts_with("AUTH") {
            "AUTH …".to_owned()
        } else {
            command.clone()
        });
        let writer = reader.get_mut();
        if upper.starts_with("EHLO") {
            let mut reply = String::from("250-test.example\r\n");
            if script.offer_starttls && !in_tls {
                reply.push_str("250-STARTTLS\r\n");
            }
            if in_tls {
                reply.push_str(&format!("250-AUTH {}\r\n", script.auth));
            }
            reply.push_str("250 SIZE 10000000\r\n");
            writer.write_all(reply.as_bytes()).await.ok()?;
        } else if upper == "STARTTLS" {
            writer.write_all(b"220 ready to start TLS\r\n").await.ok()?;
            return Some(reader.into_inner());
        } else if upper.starts_with("AUTH PLAIN") {
            if !in_tls {
                seen.lock().unwrap().auth_in_clear = true;
            }
            let token = command.split_whitespace().nth(2).unwrap_or("");
            let expected = {
                use base64::{engine::general_purpose::STANDARD, Engine};
                STANDARD.encode(format!("\0{}\0{}", script.credentials.0, script.credentials.1))
            };
            if token == expected {
                signed_in_user = true;
                writer.write_all(b"235 welcome\r\n").await.ok()?;
            } else {
                writer.write_all(b"535 5.7.8 credentials not accepted\r\n").await.ok()?;
            }
        } else if upper.starts_with("MAIL FROM:") {
            if script.credentials.0.is_empty() || signed_in_user {
                writer.write_all(b"250 sender ok\r\n").await.ok()?;
            } else {
                writer.write_all(b"530 5.7.0 authentication required\r\n").await.ok()?;
            }
        } else if upper.starts_with("RCPT TO:") {
            writer
                .write_all(format!("{} recipient\r\n", script.rcpt_code).as_bytes())
                .await
                .ok()?;
        } else if upper == "DATA" {
            writer.write_all(b"354 go ahead\r\n").await.ok()?;
            let mut message = String::new();
            loop {
                line.clear();
                if reader.read_line(&mut line).await.ok()? == 0 {
                    return None;
                }
                if line == ".\r\n" {
                    break;
                }
                message.push_str(&line);
            }
            seen.lock().unwrap().messages.push(message);
            reader.get_mut().write_all(b"250 queued as 1234\r\n").await.ok()?;
        } else if upper == "QUIT" {
            writer.write_all(b"221 bye\r\n").await.ok()?;
            return None;
        } else {
            writer.write_all(b"502 not implemented\r\n").await.ok()?;
        }
    }
}

/// Starts the server; `implicit_tls` speaks TLS from the first byte.
async fn start_server(script: Script, implicit_tls: bool) -> (u16, Arc<Mutex<Seen>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(Seen::default()));
    let recorded = seen.clone();
    let acceptor = acceptor();
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            let (script, seen, acceptor) = (script.clone(), recorded.clone(), acceptor.clone());
            tokio::spawn(async move {
                if implicit_tls {
                    let Ok(mut tls) = acceptor.accept(stream).await else {
                        return;
                    };
                    let _ = tls.write_all(b"220 test.example ESMTP\r\n").await;
                    serve_session(tls, &script, &seen, true).await;
                } else {
                    let mut stream = stream;
                    let _ = stream.write_all(b"220 test.example ESMTP\r\n").await;
                    // The STARTTLS exchange ends the plain session and starts a TLS one.
                    if let Some(stream) = serve_session(stream, &script, &seen, false).await {
                        if let Ok(tls) = acceptor.accept(stream).await {
                            serve_session(tls, &script, &seen, true).await;
                        }
                    }
                }
            });
        }
    });
    (port, seen)
}

fn settings(port: u16, security: Security) -> SmtpSettings {
    SmtpSettings {
        host: "127.0.0.1".to_owned(),
        port,
        security,
        username: Some("memo".to_owned()),
        password: Some("app-password-1".to_owned()),
    }
}

fn message() -> Vec<u8> {
    compose(&Outgoing {
        from_address: "memo@example.com",
        from_name: Some("RepoMemo"),
        to: "ada@example.com",
        subject: "Your invitation",
        body: "Open the link.\n.This line starts with a dot.\nBye",
    })
    .unwrap()
}

#[tokio::test]
async fn a_message_is_delivered_over_tls_from_the_first_byte() {
    let (port, seen) = start_server(Script::default(), true).await;
    send(
        &settings(port, Security::Tls),
        &trusted(),
        "memo@example.com",
        "ada@example.com",
        &message(),
    )
    .await
    .unwrap();
    let seen = seen.lock().unwrap();
    assert_eq!(seen.messages.len(), 1);
    assert!(seen.messages[0].contains("Subject: Your invitation"));
    assert!(
        seen.commands.iter().any(|c| c == "MAIL FROM:<memo@example.com>"),
        "{:?}",
        seen.commands
    );
    assert!(seen.commands.iter().any(|c| c == "RCPT TO:<ada@example.com>"));
    assert!(seen.commands.iter().any(|c| c == "EHLO example.com"));
    assert!(!seen.auth_in_clear);
}

#[tokio::test]
async fn starttls_upgrades_before_signing_in_and_sending() {
    let (port, seen) = start_server(Script::default(), false).await;
    send(
        &settings(port, Security::StartTls),
        &trusted(),
        "memo@example.com",
        "ada@example.com",
        &message(),
    )
    .await
    .unwrap();
    let seen = seen.lock().unwrap();
    assert_eq!(seen.messages.len(), 1);
    assert!(!seen.auth_in_clear, "the password never travels before TLS");
    let position = |wanted: &str| seen.commands.iter().position(|command| command == wanted).unwrap();
    assert!(position("STARTTLS") < position("AUTH …"));
    assert!(position("AUTH …") < position("MAIL FROM:<memo@example.com>"));
    // The dot-stuffed line arrives as written once the server undoes the stuffing: here still doubled.
    assert!(seen.messages[0].contains("Auto-Submitted"));
}

#[tokio::test]
async fn a_server_that_cannot_do_tls_is_refused_and_nothing_is_sent_in_clear() {
    let (port, seen) = start_server(
        Script {
            offer_starttls: false,
            ..Script::default()
        },
        false,
    )
    .await;
    let error = send(
        &settings(port, Security::StartTls),
        &trusted(),
        "memo@example.com",
        "ada@example.com",
        &message(),
    )
    .await
    .unwrap_err();
    assert!(!error.temporary);
    assert!(error.message.contains("STARTTLS"), "{error}");
    let seen = seen.lock().unwrap();
    assert!(seen.messages.is_empty() && !seen.auth_in_clear);
}

#[tokio::test]
async fn an_untrusted_certificate_is_not_accepted() {
    let (port, seen) = start_server(Script::default(), true).await;
    let policy = OutboundPolicy {
        extra_roots: Vec::new(),
        ..trusted()
    };
    let error = send(
        &settings(port, Security::Tls),
        &policy,
        "memo@example.com",
        "ada@example.com",
        &message(),
    )
    .await
    .unwrap_err();
    assert!(error.message.contains("certificate"), "{error}");
    assert!(seen.lock().unwrap().commands.is_empty());
}

#[tokio::test]
async fn temporary_and_permanent_refusals_are_told_apart() {
    for (code, temporary) in [(451_u16, true), (452, true), (550, false), (553, false)] {
        let (port, _) = start_server(
            Script {
                rcpt_code: code,
                ..Script::default()
            },
            true,
        )
        .await;
        let error = send(
            &settings(port, Security::Tls),
            &trusted(),
            "memo@example.com",
            "ada@example.com",
            &message(),
        )
        .await
        .unwrap_err();
        assert_eq!(error.temporary, temporary, "{code}: {error}");
        assert!(error.message.contains(&code.to_string()));
    }
}

#[tokio::test]
async fn wrong_credentials_are_a_permanent_failure_and_the_password_is_not_in_the_message() {
    let (port, _) = start_server(Script::default(), true).await;
    let mut wrong = settings(port, Security::Tls);
    wrong.password = Some("not-the-password".to_owned());
    let error = send(&wrong, &trusted(), "memo@example.com", "ada@example.com", &message())
        .await
        .unwrap_err();
    assert!(!error.temporary, "{error}");
    assert!(!error.message.contains("not-the-password"));
}

#[tokio::test]
async fn an_address_the_operator_did_not_list_is_not_called() {
    let (port, seen) = start_server(Script::default(), true).await;
    let policy = OutboundPolicy {
        allowed_hosts: Vec::new(),
        ..trusted()
    };
    let error = send(
        &settings(port, Security::Tls),
        &policy,
        "memo@example.com",
        "ada@example.com",
        &message(),
    )
    .await
    .unwrap_err();
    assert!(
        !error.temporary && error.message.contains("REPOMEMO_SMTP_ALLOWED_HOSTS"),
        "{error}"
    );
    assert!(seen.lock().unwrap().commands.is_empty(), "no connection was made");
}
