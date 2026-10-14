//! Building a plain-text message that cannot carry anything its writer did
//! not mean: addresses are checked, no header value may hold a line break or
//! any other control character, and non-ASCII text is encoded (RFC 2047 in
//! headers, base64 in the body), so a crafted display name or subject cannot
//! add a header or a recipient.

use base64::{engine::general_purpose::STANDARD, Engine};

const MAX_ADDRESS_LEN: usize = 254;
const MAX_SUBJECT_CHARS: usize = 200;
const MAX_NAME_CHARS: usize = 120;
const MAX_BODY_BYTES: usize = 200 * 1024;
/// Raw bytes per RFC 2047 encoded word: 45 become 60 base64 characters, which
/// keeps `=?UTF-8?B?…?=` within the 75 characters a word may have.
const ENCODED_WORD_BYTES: usize = 45;

/// A message to build.
#[derive(Debug, Clone, Copy)]
pub struct Outgoing<'a> {
    pub from_address: &'a str,
    pub from_name: Option<&'a str>,
    pub to: &'a str,
    pub subject: &'a str,
    pub body: &'a str,
}

/// Why a message was not built. Always the sender's input, so never retried.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposeError(pub String);

impl std::fmt::Display for ComposeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ComposeError {}

fn invalid(message: &str) -> ComposeError {
    ComposeError(message.to_owned())
}

/// An address this server will put in an envelope: ASCII, one `@`, a
/// plausible local part and a dotted domain. Quoted local parts, comments,
/// address literals and internationalised addresses are not accepted.
pub fn valid_address(address: &str) -> bool {
    if address.is_empty() || address.len() > MAX_ADDRESS_LEN || !address.is_ascii() {
        return false;
    }
    let Some((local, domain)) = address.split_once('@') else {
        return false;
    };
    if local.is_empty() || local.len() > 64 || domain.contains('@') {
        return false;
    }
    let local_ok = local
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "!#$%&'*+/=?^_`{|}~.-".contains(c))
        && !local.starts_with('.')
        && !local.ends_with('.')
        && !local.contains("..");
    let labels = domain.split('.').collect::<Vec<_>>();
    let domain_ok = labels.len() >= 2
        && labels.iter().all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        });
    local_ok && domain_ok
}

fn has_control_characters(text: &str) -> bool {
    text.chars().any(|c| c.is_control())
}

/// `text` as it may appear in an unstructured header: as is when it is plain
/// ASCII, otherwise as folded RFC 2047 encoded words.
fn header_text(text: &str) -> String {
    if text.is_ascii() {
        return text.to_owned();
    }
    let mut words = Vec::new();
    let mut current = String::new();
    for character in text.chars() {
        if current.len() + character.len_utf8() > ENCODED_WORD_BYTES {
            words.push(std::mem::take(&mut current));
        }
        current.push(character);
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
        .iter()
        .map(|word| format!("=?UTF-8?B?{}?=", STANDARD.encode(word.as_bytes())))
        .collect::<Vec<_>>()
        .join("\r\n ")
}

/// A display name for the `From` header: a quoted string, or encoded words
/// when it is not ASCII.
fn display_name(name: &str) -> String {
    if name.is_ascii() {
        let escaped = name.replace('\\', "\\\\").replace('"', "\\\"");
        format!("\"{escaped}\"")
    } else {
        header_text(name)
    }
}

/// The message as bytes with CRLF line ends, ready for the DATA command.
pub fn compose(message: &Outgoing<'_>) -> Result<Vec<u8>, ComposeError> {
    if !valid_address(message.from_address) {
        return Err(invalid("The sender address is not a valid email address."));
    }
    if !valid_address(message.to) {
        return Err(invalid("The recipient is not a valid email address."));
    }
    if message.subject.trim().is_empty() || message.subject.chars().count() > MAX_SUBJECT_CHARS {
        return Err(invalid("A subject is 1 to 200 characters."));
    }
    if has_control_characters(message.subject) {
        return Err(invalid("A subject cannot contain line breaks or control characters."));
    }
    if let Some(name) = message.from_name {
        if name.chars().count() > MAX_NAME_CHARS || has_control_characters(name) {
            return Err(invalid(
                "The sender name is at most 120 characters, without line breaks.",
            ));
        }
    }
    if message.body.len() > MAX_BODY_BYTES {
        return Err(invalid("The message is longer than 200 KiB."));
    }

    let domain = message.from_address.rsplit('@').next().unwrap_or("localhost");
    let from = match message.from_name.map(str::trim).filter(|name| !name.is_empty()) {
        Some(name) => format!("{} <{}>", display_name(name), message.from_address),
        None => message.from_address.to_owned(),
    };
    let mut text = String::new();
    for line in message.body.replace("\r\n", "\n").replace('\r', "\n").split('\n') {
        text.push_str(line);
        text.push_str("\r\n");
    }
    let encoded_body = STANDARD
        .encode(text.as_bytes())
        .as_bytes()
        .chunks(76)
        .map(|line| String::from_utf8_lossy(line).into_owned())
        .collect::<Vec<_>>()
        .join("\r\n");

    let headers = [
        format!("Date: {}", chrono::Utc::now().to_rfc2822()),
        format!("From: {from}"),
        format!("To: {}", message.to),
        format!("Subject: {}", header_text(message.subject.trim())),
        format!("Message-ID: <{}@{domain}>", uuid::Uuid::new_v4()),
        "MIME-Version: 1.0".to_owned(),
        "Content-Type: text/plain; charset=utf-8".to_owned(),
        "Content-Transfer-Encoding: base64".to_owned(),
        // Tells auto-responders not to answer.
        "Auto-Submitted: auto-generated".to_owned(),
        "X-Mailer: RepoMemo".to_owned(),
    ];
    let mut bytes = headers.join("\r\n").into_bytes();
    bytes.extend_from_slice(b"\r\n\r\n");
    bytes.extend_from_slice(encoded_body.as_bytes());
    bytes.extend_from_slice(b"\r\n");
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message<'a>(to: &'a str, subject: &'a str, name: Option<&'a str>) -> Outgoing<'a> {
        Outgoing {
            from_address: "memo@example.com",
            from_name: name,
            to,
            subject,
            body: "Hello\nWorld",
        }
    }

    fn text(bytes: Vec<u8>) -> String {
        String::from_utf8(bytes).unwrap()
    }

    #[test]
    fn addresses_are_checked_before_they_reach_an_envelope() {
        for good in ["ada@example.com", "a.b+tag@sub.example.co.uk", "o'brien@example.org"] {
            assert!(valid_address(good), "{good}");
        }
        for bad in [
            "",
            "ada",
            "ada@",
            "@example.com",
            "ada@localhost",
            "a b@example.com",
            "ada@example.com\r\nBcc: boss@example.com",
            "ada@example.com>",
            "<ada@example.com>",
            "\"ada\"@example.com",
            "ada@@example.com",
            "ada@exa_mple.com",
            "ada@-example.com",
            "adá@example.com",
            ".ada@example.com",
            "ada..b@example.com",
            "ada@[127.0.0.1]",
        ] {
            assert!(!valid_address(bad), "{bad:?}");
        }
    }

    #[test]
    fn a_line_break_in_any_field_is_refused_rather_than_made_a_header() {
        let crafted = "Hello\r\nBcc: boss@example.com";
        assert!(compose(&message("ada@example.com", crafted, None)).is_err());
        assert!(compose(&message("ada@example.com", "Hi", Some(crafted))).is_err());
        assert!(compose(&message("ada@example.com\r\nBcc: boss@example.com", "Hi", None)).is_err());
        assert!(compose(&message("ada@example.com", "Hi\nthere", None)).is_err());
        assert!(compose(&message("ada@example.com", "  ", None)).is_err());
        let sent = text(compose(&message("ada@example.com", "Hi", Some("Ada \"the\" Countess"))).unwrap());
        assert!(!sent.contains("Bcc:"));
        assert!(
            sent.contains("From: \"Ada \\\"the\\\" Countess\" <memo@example.com>"),
            "{sent}"
        );
    }

    #[test]
    fn headers_are_complete_and_the_body_is_base64_with_crlf_lines() {
        let sent = text(compose(&message("ada@example.com", "Your invitation", Some("RepoMemo"))).unwrap());
        let (headers, body) = sent.split_once("\r\n\r\n").unwrap();
        for expected in [
            "From: \"RepoMemo\" <memo@example.com>",
            "To: ada@example.com",
            "Subject: Your invitation",
            "MIME-Version: 1.0",
            "Content-Type: text/plain; charset=utf-8",
            "Content-Transfer-Encoding: base64",
            "Auto-Submitted: auto-generated",
        ] {
            assert!(headers.contains(expected), "{expected} in {headers}");
        }
        assert!(headers.contains("Message-ID: <") && headers.contains("@example.com>"));
        let decoded = STANDARD.decode(body.replace("\r\n", "")).unwrap();
        assert_eq!(String::from_utf8(decoded).unwrap(), "Hello\r\nWorld\r\n");
        assert!(!sent.replace("\r\n", "").contains('\n'), "only CRLF line ends");
    }

    #[test]
    fn non_ascii_text_is_encoded_in_headers_and_split_into_short_words() {
        let long = "Réunion de revue des décisions d'architecture et des écarts constatés cette semaine ✓";
        let sent = text(compose(&message("ada@example.com", long, Some("Équipe Mémoire"))).unwrap());
        assert!(sent.is_ascii());
        let subject = sent
            .split("\r\nSubject: ")
            .nth(1)
            .unwrap()
            .split("\r\nMessage-ID")
            .next()
            .unwrap();
        assert!(subject.contains("\r\n "), "folded into several words");
        for word in subject.split("\r\n ") {
            assert!(word.starts_with("=?UTF-8?B?") && word.ends_with("?="));
            assert!(word.len() <= 75, "{word}");
        }
        let decoded = subject
            .split("\r\n ")
            .map(|word| {
                let inner = word.trim_start_matches("=?UTF-8?B?").trim_end_matches("?=");
                String::from_utf8(STANDARD.decode(inner).unwrap()).unwrap()
            })
            .collect::<String>();
        assert_eq!(decoded, long);
    }
}
