//! Secrets kept from cloud models (V0.9.94). Before text goes to a model whose server is outside the network, what looks like a secret is
//! replaced by a marker, or the request is refused when the organization chose that. The patterns are few and specific, so that what
//! they remove is a secret and an ordinary sentence is not: a private key, the keys and tokens of the common services, a JSON web
//! token, a password in a configuration line and the password of a connection string. A value that is plainly a placeholder
//! (`${PASSWORD}`, `<password>`, `changeme`) is left alone. Nothing here reads or keeps the secrets it finds, only how many.

use std::sync::OnceLock;

use regex::{Captures, Regex};

/// What stands in the place of a secret.
pub const MARKER: &str = "[secret removed]";

/// A pattern, and which part of a match is the secret (0 for all of it).
struct Rule {
    pattern: Regex,
    secret_group: usize,
    /// Whether a value that looks like a placeholder is left alone.
    skip_placeholders: bool,
}

fn rule(pattern: &str, secret_group: usize, skip_placeholders: bool) -> Rule {
    Rule {
        pattern: Regex::new(pattern).expect("a secret pattern is a valid expression"),
        secret_group,
        skip_placeholders,
    }
}

fn rules() -> &'static [Rule] {
    static RULES: OnceLock<Vec<Rule>> = OnceLock::new();
    RULES.get_or_init(|| {
        vec![
            // A private key block, or the start of one whose end was cut off.
            rule(
                r"(?s)-----BEGIN (?:RSA |EC |DSA |OPENSSH |ENCRYPTED |PGP )?PRIVATE KEY(?: BLOCK)?-----.*?(?:-----END (?:RSA |EC |DSA |OPENSSH |ENCRYPTED |PGP )?PRIVATE KEY(?: BLOCK)?-----|\z)",
                0,
                false,
            ),
            // Amazon access key ids, and the secret key of an assignment that names it.
            rule(r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b", 0, false),
            rule(r#"(?i)aws_secret_access_key\s*[=:]\s*["']?([A-Za-z0-9/+=]{40})"#, 1, false),
            // GitHub, Slack, Stripe, Google and the common "sk-" keys.
            rule(r"\bgh[pousr]_[A-Za-z0-9]{36,}\b", 0, false),
            rule(r"\bgithub_pat_[A-Za-z0-9_]{22,}\b", 0, false),
            rule(r"\bxox[abprs]-[A-Za-z0-9-]{10,}\b", 0, false),
            rule(r"\b[sr]k_(?:live|test)_[A-Za-z0-9]{16,}\b", 0, false),
            rule(r"\bAIza[0-9A-Za-z_-]{35}\b", 0, false),
            rule(r"\bsk-[A-Za-z0-9_-]{20,}", 0, false),
            // A JSON web token: three long base64url parts, the first of which starts a JSON header.
            rule(r"\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}", 0, false),
            // The password in a connection string.
            rule(
                r"\b(?:postgres(?:ql)?|mysql|mariadb|mongodb(?:\+srv)?|redis|rediss|amqps?|mssql|sqlserver)://[^\s:@/]+:([^\s@/]{3,})@",
                1,
                true,
            ),
            // A password, secret or key in a configuration line: only the value goes.
            rule(
                r#"(?i)(?:password|passwd|pwd|secret|api[_-]?key|access[_-]?token|auth[_-]?token|client[_-]?secret|private[_-]?key)["']?\s*[=:]\s*["']?([^\s"'#;,]{6,})"#,
                1,
                true,
            ),
        ]
    })
}

/// Whether a value that follows a name such as `password:` is plainly not a secret.
fn is_placeholder(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if value.starts_with(['$', '<', '{', '%', '*', '(', '[']) || value.contains("${") || value.contains("{{") {
        return true;
    }
    if matches!(
        lower.as_str(),
        "changeme"
            | "change-me"
            | "example"
            | "your-password"
            | "yourpassword"
            | "password"
            | "secret"
            | "none"
            | "null"
            | "true"
            | "false"
            | "redacted"
            | "placeholder"
            | "undefined"
    ) {
        return true;
    }
    // "xxxxxx" and "******": one character repeated.
    let mut characters = value.chars();
    let first = characters.next();
    if first.is_some() && characters.all(|character| Some(character) == first) {
        return true;
    }
    // A plain word is a sentence's word, not a password: a secret has a digit or a symbol in it, or is long.
    let plain = value.chars().all(|character| character.is_ascii_alphabetic());
    plain && value.len() < 12
}

/// The text with every secret replaced by [`MARKER`], and how many were replaced. Never returns the secrets.
pub fn mask_secrets(text: &str) -> (String, usize) {
    let mut masked = text.to_owned();
    let mut count = 0;
    for rule in rules() {
        let replaced = rule.pattern.replace_all(&masked, |found: &Captures| {
            let whole = found.get(0).expect("a match has a whole");
            let Some(secret) = (if rule.secret_group == 0 {
                Some(whole)
            } else {
                found.get(rule.secret_group)
            }) else {
                return whole.as_str().to_owned();
            };
            if rule.skip_placeholders && is_placeholder(secret.as_str()) {
                return whole.as_str().to_owned();
            }
            count += 1;
            let start = secret.start() - whole.start();
            let end = secret.end() - whole.start();
            format!("{}{MARKER}{}", &whole.as_str()[..start], &whole.as_str()[end..])
        });
        masked = replaced.into_owned();
    }
    (masked, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn masked(text: &str) -> (String, usize) {
        mask_secrets(text)
    }

    #[test]
    fn the_secrets_of_the_common_services_are_removed() {
        let cases = [
            "key AKIAIOSFODNN7EXAMPLE here",
            "token ghp_0123456789abcdefghijklmnopqrstuvwxyzAB used",
            "github_pat_11ABCDEFG0123456789_abcdefghijklmnopqrstuvwxyz0123456789",
            "slack xoxb-1234567890-abcdefghijkl",
            "stripe sk_live_abcdefghijklmnop1234",
            "google AIzaSyA1234567890abcdefghijklmnopqrstuv",
            "openai sk-proj-abcdefghijklmnopqrstuvwxyz0123456789",
            "jwt eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dBjftJeZ4CVPmB92K27uhbUJU1p1r_wW1gFWFOEjXk",
            "aws_secret_access_key = wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
        ];
        for text in cases {
            let (out, count) = masked(text);
            assert_eq!(count, 1, "{text} -> {out}");
            assert!(out.contains(MARKER), "{out}");
        }
        let (out, _) = masked("key AKIAIOSFODNN7EXAMPLE here");
        assert_eq!(out, "key [secret removed] here");
    }

    #[test]
    fn a_private_key_goes_whole_and_a_cut_one_goes_to_the_end() {
        let key = "-----BEGIN RSA PRIVATE KEY-----\nMIIEowIBAAKCAQEA7\nabc\n-----END RSA PRIVATE KEY-----";
        let (out, count) = masked(&format!("before\n{key}\nafter"));
        assert_eq!((count, out.as_str()), (1, "before\n[secret removed]\nafter"));
        let (cut, count) = masked("-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXk");
        assert_eq!((count, cut.as_str()), (1, MARKER));
        // A public key is not a secret.
        assert_eq!(masked("-----BEGIN PUBLIC KEY-----\nabc\n-----END PUBLIC KEY-----").1, 0);
    }

    #[test]
    fn only_the_value_of_a_password_in_configuration_goes() {
        let (out, count) = masked("DB_PASSWORD=hunter2hunter\nHOST=db.internal");
        assert_eq!(
            (count, out.as_str()),
            (1, "DB_PASSWORD=[secret removed]\nHOST=db.internal")
        );
        let (out, _) = masked("password: \"s3cr3t-Value!\"");
        assert!(
            out.starts_with("password: \"[secret removed]") || out.contains(MARKER),
            "{out}"
        );
        let (out, count) = masked("postgres://app:s3cretpw@db.internal:5432/main");
        assert_eq!(
            (count, out.as_str()),
            (1, "postgres://app:[secret removed]@db.internal:5432/main")
        );
        let (out, _) = masked("api_key = abcd1234efgh5678");
        assert_eq!(out, "api_key = [secret removed]");
    }

    #[test]
    fn ordinary_text_and_placeholders_are_left_alone() {
        for text in [
            "Reset your password: choose a strong one.",
            "The password policy requires a password: stronger",
            "password: ${DB_PASSWORD}",
            "password=<your password>",
            "password: changeme",
            "secret: ********",
            "postgres://app:${PASSWORD}@db.internal/main",
            "api_key: {{ api_key }}",
            "The ghp_ prefix marks a GitHub token, as does github_pat_.",
            "An sk-key is short here: sk-abc",
            "eyJ is how a JSON web token starts",
            "AKIA is the prefix of an access key id",
            "Use the token field. token: x",
        ] {
            assert_eq!(masked(text), (text.to_owned(), 0), "{text}");
        }
    }

    #[test]
    fn several_secrets_are_counted_and_none_is_returned() {
        let (out, count) =
            masked("a AKIAIOSFODNN7EXAMPLE b ghp_0123456789abcdefghijklmnopqrstuvwxyzAB c password=abcdef123456");
        assert_eq!(count, 3);
        for secret in ["AKIAIOSFODNN7EXAMPLE", "ghp_0123456789", "abcdef123456"] {
            assert!(!out.contains(secret), "{out}");
        }
    }
}
