//! Encryption at rest for secrets kept in the database, such as AI provider
//! API keys.
//!
//! Values are sealed with AES-256-GCM. The key comes from the operator
//! (`StorageConfig::master_key`, which the server reads from
//! `REPOMEMO_SECRET_KEY`) or, when none is given, from `secret.key` in the data
//! directory, created on first use. Keeping the key out of the database means a
//! copy of the database file alone (a backup, an export, a stray attachment)
//! does not reveal the secrets.
//!
//! Each sealed value is bound to a context string (for example the provider id
//! it belongs to), so a sealed value copied onto another row does not open.

use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM, NONCE_LEN};
use ring::hkdf::{Salt, HKDF_SHA256};
use ring::rand::{SecureRandom, SystemRandom};

/// Marks a sealed value and the format version used to seal it.
const SEALED_PREFIX: &str = "enc:v1:";
/// Created in the data directory when no master key is configured.
pub const KEY_FILE_NAME: &str = "secret.key";
/// Shortest master key accepted from the operator.
pub const MIN_MASTER_KEY_CHARS: usize = 32;
const KEY_FILE_BYTES: usize = 32;
const HKDF_SALT: &[u8] = b"repomemo/secrets/v1";
const HKDF_INFO: &[u8] = b"aes-256-gcm";

#[derive(Clone)]
pub(crate) struct SecretBox {
    key: Arc<LessSafeKey>,
    rng: SystemRandom,
}

impl std::fmt::Debug for SecretBox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretBox").finish_non_exhaustive()
    }
}

impl SecretBox {
    /// Loads the sealing key: derived from `master_key` when given, otherwise
    /// from the key file in `data_dir`, which is created when missing.
    pub(crate) fn load(data_dir: &Path, master_key: Option<&str>) -> Result<Self> {
        let material = match master_key.map(str::trim).filter(|value| !value.is_empty()) {
            Some(master_key) => {
                if master_key.chars().count() < MIN_MASTER_KEY_CHARS {
                    bail!("The secret key must contain at least {MIN_MASTER_KEY_CHARS} characters.");
                }
                master_key.as_bytes().to_vec()
            }
            None => load_or_create_key_file(&data_dir.join(KEY_FILE_NAME))?,
        };
        Ok(Self::from_material(&material))
    }

    fn from_material(material: &[u8]) -> Self {
        let prk = Salt::new(HKDF_SHA256, HKDF_SALT).extract(material);
        let okm = prk
            .expand(&[HKDF_INFO], &AES_256_GCM)
            .expect("AES-256-GCM key length is a valid HKDF output length");
        Self {
            key: Arc::new(LessSafeKey::new(UnboundKey::from(okm))),
            rng: SystemRandom::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn is_sealed(value: &str) -> bool {
        value.starts_with(SEALED_PREFIX)
    }

    /// Encrypts `plaintext` for storage, bound to `context`.
    pub(crate) fn seal(&self, plaintext: &str, context: &str) -> Result<String> {
        let mut nonce_bytes = [0_u8; NONCE_LEN];
        self.rng
            .fill(&mut nonce_bytes)
            .map_err(|_| anyhow!("the system random generator failed"))?;
        let mut sealed = plaintext.as_bytes().to_vec();
        self.key
            .seal_in_place_append_tag(
                Nonce::assume_unique_for_key(nonce_bytes),
                Aad::from(context.as_bytes()),
                &mut sealed,
            )
            .map_err(|_| anyhow!("a secret could not be encrypted"))?;
        let mut payload = Vec::with_capacity(NONCE_LEN + sealed.len());
        payload.extend_from_slice(&nonce_bytes);
        payload.extend_from_slice(&sealed);
        Ok(format!("{SEALED_PREFIX}{}", STANDARD.encode(payload)))
    }

    /// Decrypts a value produced by [`SecretBox::seal`] with the same context.
    pub(crate) fn open(&self, sealed: &str, context: &str) -> Result<String> {
        let encoded = sealed
            .strip_prefix(SEALED_PREFIX)
            .context("the stored secret has an unknown format")?;
        let mut payload = STANDARD
            .decode(encoded)
            .context("the stored secret is not valid base64")?;
        if payload.len() < NONCE_LEN {
            bail!("the stored secret is truncated");
        }
        let mut nonce_bytes = [0_u8; NONCE_LEN];
        nonce_bytes.copy_from_slice(&payload[..NONCE_LEN]);
        let plaintext = self
            .key
            .open_in_place(
                Nonce::assume_unique_for_key(nonce_bytes),
                Aad::from(context.as_bytes()),
                &mut payload[NONCE_LEN..],
            )
            .map_err(|_| {
                anyhow!("the stored secret could not be decrypted; the secret key may have changed")
            })?;
        String::from_utf8(plaintext.to_vec()).context("the stored secret is not UTF-8 text")
    }
}

fn load_or_create_key_file(path: &Path) -> Result<Vec<u8>> {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let key = STANDARD
                .decode(text.trim())
                .with_context(|| format!("{} is not a valid key file", path.display()))?;
            if key.len() < KEY_FILE_BYTES {
                bail!("{} holds a key that is too short", path.display());
            }
            Ok(key)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => create_key_file(path),
        Err(error) => {
            Err(error).with_context(|| format!("the key file {} could not be read", path.display()))
        }
    }
}

fn create_key_file(path: &Path) -> Result<Vec<u8>> {
    let mut key = vec![0_u8; KEY_FILE_BYTES];
    SystemRandom::new()
        .fill(&mut key)
        .map_err(|_| anyhow!("the system random generator failed"))?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(path) {
        Ok(mut file) => {
            use std::io::Write;
            file.write_all(STANDARD.encode(&key).as_bytes())
                .and_then(|_| file.sync_all())
                .with_context(|| format!("the key file {} could not be written", path.display()))?;
            Ok(key)
        }
        // Another process created it first: use that key.
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => load_or_create_key_file(path),
        Err(error) => {
            Err(error).with_context(|| format!("the key file {} could not be created", path.display()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("repomemo-secrets-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn sealed_values_round_trip_and_stay_bound_to_their_context() {
        let dir = temp_dir();
        let secrets = SecretBox::load(&dir, None).unwrap();
        let sealed = secrets.seal("sk-or-v1-secret", "provider:a").unwrap();
        assert!(SecretBox::is_sealed(&sealed));
        assert!(!sealed.contains("sk-or-v1-secret"));
        assert_eq!(secrets.open(&sealed, "provider:a").unwrap(), "sk-or-v1-secret");
        assert!(secrets.open(&sealed, "provider:b").is_err());
        // Two seals of the same value differ (fresh nonce each time).
        assert_ne!(sealed, secrets.seal("sk-or-v1-secret", "provider:a").unwrap());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_key_file_is_reused_and_a_master_key_takes_precedence() {
        let dir = temp_dir();
        let first = SecretBox::load(&dir, None).unwrap();
        let sealed = first.seal("value", "ctx").unwrap();
        let reloaded = SecretBox::load(&dir, None).unwrap();
        assert_eq!(reloaded.open(&sealed, "ctx").unwrap(), "value");

        let master = SecretBox::load(&dir, Some("an-operator-supplied-master-key-of-enough-length")).unwrap();
        assert!(master.open(&sealed, "ctx").is_err());
        assert!(SecretBox::load(&dir, Some("too-short")).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
