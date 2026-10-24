//! Encryption of backups (roadmap O6a, decision 14).
//!
//! A backup holds the database, the stored files and the key to the secrets
//! inside the database, so a copy of it is as sensitive as the data folder.
//! When `REPOMEMO_BACKUP_KEY` is set, every file of a backup is sealed with
//! AES-256-GCM before it is written, and only the manifest stays readable.
//!
//! * **The key** is derived from the operator's passphrase with PBKDF2-HMAC-
//!   SHA-256 and a random salt kept in the manifest. The passphrase itself is
//!   never stored anywhere in a backup. The manifest also keeps a check value
//!   (an HMAC of a fixed text under the derived key), so a wrong passphrase is
//!   recognised before anything is read or written.
//! * **Files are sealed in chunks** of 64 KiB, so a large file never sits in
//!   memory. Each chunk has its own nonce (a random eight bytes per file and a
//!   counter) and is bound to the file's header and to whether it is the last
//!   one, so a chunk cannot be moved, repeated or dropped from the end without
//!   the file failing to open.
//! * **A passphrase can be changed** (V0.3.24): new backups use `REPOMEMO_BACKUP_KEY`, backups made with the one
//!   before stay readable while it is kept as `REPOMEMO_BACKUP_KEY_PREVIOUS`, and a backup can be sealed again
//!   with the new one (`BackupCipher::reseal_file`, a chunk at a time).
//! * **Backups keep sharing unchanged files.** A new backup made with the same
//!   passphrase reuses the salt of the one before it, so a stored file that has
//!   not changed seals to the same bytes and can still be hard-linked instead
//!   of copied again.

use std::fs::File;
use std::io::{BufWriter, Read, Write};
use std::num::NonZeroU32;
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM, NONCE_LEN};
use ring::rand::{SecureRandom, SystemRandom};
use ring::{hmac, pbkdf2};
use serde::{Deserialize, Serialize};

/// Shortest passphrase accepted.
pub(crate) const MIN_KEY_CHARS: usize = 16;
/// The suffix of a sealed file.
pub(crate) const SEALED_SUFFIX: &str = ".enc";
const ITERATIONS: u32 = 600_000;
const SALT_BYTES: usize = 16;
const CHUNK_BYTES: usize = 64 * 1024;
const TAG_BYTES: usize = 16;
const MAGIC: &[u8; 4] = b"RMBK";
const FORMAT_VERSION: u8 = 1;
const PREFIX_BYTES: usize = NONCE_LEN - 4;
const HEADER_BYTES: usize = MAGIC.len() + 1 + PREFIX_BYTES;
const CHECK_TEXT: &[u8] = b"repomemo backup key check v1";

/// What a backup's manifest says about how it was sealed. Nothing in it can
/// open the backup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BackupEncryption {
    pub cipher: String,
    pub kdf: String,
    pub iterations: u32,
    /// Hex.
    pub salt: String,
    /// Hex: an HMAC of a fixed text under the derived key.
    pub key_check: String,
    pub chunk_bytes: u32,
}

/// Why a passphrase could not be used.
#[derive(Debug)]
pub(crate) enum KeyError {
    /// It is not the passphrase the backup was made with.
    Mismatch,
    Unusable(String),
}

impl std::fmt::Display for KeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mismatch => f.write_str(
                "This backup was made with a different key than REPOMEMO_BACKUP_KEY (or REPOMEMO_BACKUP_KEY_PREVIOUS), so nothing was restored.",
            ),
            Self::Unusable(reason) => f.write_str(reason),
        }
    }
}

impl std::error::Error for KeyError {}

pub(crate) struct BackupCipher {
    key: LessSafeKey,
    salt: [u8; SALT_BYTES],
    key_check: String,
}

impl std::fmt::Debug for BackupCipher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BackupCipher").finish_non_exhaustive()
    }
}

/// Checks a passphrase the operator configured.
pub(crate) fn validate_passphrase(passphrase: &str) -> Result<(), String> {
    if passphrase.chars().count() < MIN_KEY_CHARS {
        return Err(format!(
            "REPOMEMO_BACKUP_KEY must contain at least {MIN_KEY_CHARS} characters."
        ));
    }
    Ok(())
}

impl BackupCipher {
    fn derive(passphrase: &str, salt: [u8; SALT_BYTES], iterations: u32) -> Result<Self, KeyError> {
        validate_passphrase(passphrase).map_err(KeyError::Unusable)?;
        let iterations = NonZeroU32::new(iterations)
            .ok_or_else(|| KeyError::Unusable("The backup's key settings are invalid.".to_owned()))?;
        let mut material = [0u8; 32];
        pbkdf2::derive(
            pbkdf2::PBKDF2_HMAC_SHA256,
            iterations,
            &salt,
            passphrase.as_bytes(),
            &mut material,
        );
        let check = hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, &material), CHECK_TEXT);
        let key = UnboundKey::new(&AES_256_GCM, &material)
            .map_err(|_| KeyError::Unusable("The backup key could not be set up.".to_owned()))?;
        Ok(Self {
            key: LessSafeKey::new(key),
            salt,
            key_check: hex::encode(check.as_ref()),
        })
    }

    /// A cipher with a new random salt.
    pub(crate) fn create(passphrase: &str) -> Result<Self, KeyError> {
        let mut salt = [0u8; SALT_BYTES];
        SystemRandom::new()
            .fill(&mut salt)
            .map_err(|_| KeyError::Unusable("The system could not provide random bytes.".to_owned()))?;
        Self::derive(passphrase, salt, ITERATIONS)
    }

    /// The cipher a backup was made with, or [`KeyError::Mismatch`] when the
    /// passphrase is not the one it was made with.
    pub(crate) fn open(passphrase: &str, description: &BackupEncryption) -> Result<Self, KeyError> {
        if description.cipher != "aes-256-gcm" || description.kdf != "pbkdf2-hmac-sha256" {
            return Err(KeyError::Unusable(
                "This backup was sealed in a way this server does not know.".to_owned(),
            ));
        }
        if description.chunk_bytes as usize != CHUNK_BYTES {
            return Err(KeyError::Unusable(
                "This backup uses a chunk size this server does not know.".to_owned(),
            ));
        }
        let salt: [u8; SALT_BYTES] = hex::decode(&description.salt)
            .ok()
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or_else(|| KeyError::Unusable("The backup's key settings are invalid.".to_owned()))?;
        let cipher = Self::derive(passphrase, salt, description.iterations)?;
        if cipher.key_check != description.key_check {
            return Err(KeyError::Mismatch);
        }
        Ok(cipher)
    }

    pub(crate) fn description(&self) -> BackupEncryption {
        BackupEncryption {
            cipher: "aes-256-gcm".to_owned(),
            kdf: "pbkdf2-hmac-sha256".to_owned(),
            iterations: ITERATIONS,
            salt: hex::encode(self.salt),
            key_check: self.key_check.clone(),
            chunk_bytes: CHUNK_BYTES as u32,
        }
    }

    /// The size a file of `plain` bytes has once sealed.
    pub(crate) fn sealed_len(plain: u64) -> u64 {
        let chunks = plain.div_ceil(CHUNK_BYTES as u64).max(1);
        HEADER_BYTES as u64 + plain + chunks * TAG_BYTES as u64
    }

    /// Seals `from` into `to`; returns the size of the plain file.
    pub(crate) fn seal_file(&self, from: &Path, to: &Path) -> Result<u64> {
        let result = self.seal_file_inner(from, to);
        if result.is_err() {
            let _ = std::fs::remove_file(to);
        }
        result
    }

    fn seal_file_inner(&self, from: &Path, to: &Path) -> Result<u64> {
        let mut input = File::open(from).with_context(|| format!("{} could not be read", from.display()))?;
        let mut output =
            BufWriter::new(File::create(to).with_context(|| format!("{} could not be written", to.display()))?);
        let (header, prefix) = new_header()?;
        output.write_all(&header)?;

        let mut plain_bytes = 0_u64;
        let mut counter = 0_u32;
        let mut current = read_up_to(&mut input, CHUNK_BYTES)?;
        loop {
            let next = read_up_to(&mut input, CHUNK_BYTES)?;
            let last = next.is_empty();
            plain_bytes += current.len() as u64;
            let mut chunk = current;
            self.key
                .seal_in_place_append_tag(chunk_nonce(&prefix, counter), chunk_aad(&header, last), &mut chunk)
                .map_err(|_| anyhow!("a chunk could not be sealed"))?;
            output.write_all(&chunk)?;
            if last {
                break;
            }
            current = next;
            counter = counter.checked_add(1).context("the file is too large to seal")?;
        }
        output.flush()?;
        Ok(plain_bytes)
    }

    /// Opens `from` into `to`. A wrong key, a changed byte, a missing end and
    /// a chunk out of place all fail.
    pub(crate) fn open_file(&self, from: &Path, to: &Path) -> Result<()> {
        let result = self.open_file_inner(from, to);
        if result.is_err() {
            let _ = std::fs::remove_file(to);
        }
        result
    }

    fn open_file_inner(&self, from: &Path, to: &Path) -> Result<()> {
        let mut output =
            BufWriter::new(File::create(to).with_context(|| format!("{} could not be written", to.display()))?);
        self.read_chunks(from, |plain, _, _| Ok(output.write_all(plain)?))?;
        output.flush()?;
        Ok(())
    }

    /// Opens `from`, which this cipher sealed, and seals it again with `new` into `to`, a chunk at a time, so
    /// the plain text is never whole in memory and never on the disk (V0.3.24). The new file has a header of its
    /// own; a wrong key, a changed byte or a missing end fails as it does when opening.
    pub(crate) fn reseal_file(&self, new: &BackupCipher, from: &Path, to: &Path) -> Result<()> {
        let result = self.reseal_file_inner(new, from, to);
        if result.is_err() {
            let _ = std::fs::remove_file(to);
        }
        result
    }

    fn reseal_file_inner(&self, new: &BackupCipher, from: &Path, to: &Path) -> Result<()> {
        let mut output =
            BufWriter::new(File::create(to).with_context(|| format!("{} could not be written", to.display()))?);
        let (header, prefix) = new_header()?;
        output.write_all(&header)?;
        self.read_chunks(from, |plain, counter, last| {
            let mut chunk = plain.to_vec();
            new.key
                .seal_in_place_append_tag(chunk_nonce(&prefix, counter), chunk_aad(&header, last), &mut chunk)
                .map_err(|_| anyhow!("a chunk could not be sealed"))?;
            Ok(output.write_all(&chunk)?)
        })?;
        output.flush()?;
        Ok(())
    }

    /// Opens the sealed file `from` a chunk at a time and hands each plain chunk, its number and whether it is the
    /// last to `each`. A wrong key, a changed byte, a missing end and a chunk out of place all fail.
    fn read_chunks(&self, from: &Path, mut each: impl FnMut(&[u8], u32, bool) -> Result<()>) -> Result<()> {
        let name = from
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut input = File::open(from).with_context(|| format!("{name} could not be read"))?;
        let mut header = [0u8; HEADER_BYTES];
        input
            .read_exact(&mut header)
            .with_context(|| format!("{name} is too short to be a sealed file"))?;
        if &header[..MAGIC.len()] != MAGIC || header[MAGIC.len()] != FORMAT_VERSION {
            bail!("{name} is not a sealed backup file this server can read");
        }
        let prefix: [u8; PREFIX_BYTES] = header[MAGIC.len() + 1..].try_into().expect("fixed length");

        let mut counter = 0_u32;
        let mut current = read_up_to(&mut input, CHUNK_BYTES + TAG_BYTES)?;
        loop {
            if current.len() < TAG_BYTES {
                bail!("{name} ends before its end marker, so it is incomplete");
            }
            let next = read_up_to(&mut input, CHUNK_BYTES + TAG_BYTES)?;
            let last = next.is_empty();
            let plain = self
                .key
                .open_in_place(chunk_nonce(&prefix, counter), chunk_aad(&header, last), &mut current)
                .map_err(|_| {
                    anyhow!("{name} does not open: it was changed, is incomplete or was sealed with another key")
                })?;
            each(plain, counter, last)?;
            if last {
                break;
            }
            current = next;
            counter = counter.checked_add(1).context("the file is too large to open")?;
        }
        Ok(())
    }
}

/// A fresh header for a sealed file: the format and a random prefix for the chunks' nonces.
fn new_header() -> Result<([u8; HEADER_BYTES], [u8; PREFIX_BYTES])> {
    let mut header = [0u8; HEADER_BYTES];
    header[..MAGIC.len()].copy_from_slice(MAGIC);
    header[MAGIC.len()] = FORMAT_VERSION;
    let mut prefix = [0u8; PREFIX_BYTES];
    SystemRandom::new()
        .fill(&mut prefix)
        .map_err(|_| anyhow!("the system could not provide random bytes"))?;
    header[MAGIC.len() + 1..].copy_from_slice(&prefix);
    Ok((header, prefix))
}

fn chunk_nonce(prefix: &[u8; PREFIX_BYTES], counter: u32) -> Nonce {
    let mut bytes = [0u8; NONCE_LEN];
    bytes[..PREFIX_BYTES].copy_from_slice(prefix);
    bytes[PREFIX_BYTES..].copy_from_slice(&counter.to_be_bytes());
    Nonce::assume_unique_for_key(bytes)
}

/// The header, and whether this is the file's last chunk.
fn chunk_aad(header: &[u8; HEADER_BYTES], last: bool) -> Aad<[u8; HEADER_BYTES + 1]> {
    let mut bytes = [0u8; HEADER_BYTES + 1];
    bytes[..HEADER_BYTES].copy_from_slice(header);
    bytes[HEADER_BYTES] = u8::from(last);
    Aad::from(bytes)
}

/// Up to `limit` bytes, fewer only at the end of the input.
fn read_up_to(input: &mut impl Read, limit: usize) -> std::io::Result<Vec<u8>> {
    let mut buffer = vec![0u8; limit];
    let mut filled = 0;
    while filled < limit {
        let read = input.read(&mut buffer[filled..])?;
        if read == 0 {
            break;
        }
        filled += read;
    }
    buffer.truncate(filled);
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PASSPHRASE: &str = "a long enough passphrase for tests";

    fn folder(label: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "repomemo-backup-crypto-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn cipher() -> BackupCipher {
        BackupCipher::create(PASSPHRASE).unwrap()
    }

    #[test]
    fn files_of_every_size_round_trip_and_have_the_size_that_is_predicted() {
        let dir = folder("sizes");
        let cipher = cipher();
        for size in [
            0usize,
            1,
            100,
            CHUNK_BYTES - 1,
            CHUNK_BYTES,
            CHUNK_BYTES + 1,
            CHUNK_BYTES * 3,
            CHUNK_BYTES * 3 + 17,
        ] {
            let plain: Vec<u8> = (0..size).map(|index| (index % 251) as u8).collect();
            let (source, sealed, opened) = (dir.join("plain"), dir.join("sealed"), dir.join("opened"));
            std::fs::write(&source, &plain).unwrap();
            assert_eq!(cipher.seal_file(&source, &sealed).unwrap(), size as u64);
            assert_eq!(
                std::fs::metadata(&sealed).unwrap().len(),
                BackupCipher::sealed_len(size as u64),
                "size {size}"
            );
            cipher.open_file(&sealed, &opened).unwrap();
            assert_eq!(std::fs::read(&opened).unwrap(), plain, "size {size}");
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_sealed_file_shows_nothing_of_its_content() {
        let dir = folder("opaque");
        let cipher = cipher();
        let (source, sealed) = (dir.join("plain"), dir.join("sealed"));
        std::fs::write(&source, b"The refund policy is confidential and must not appear.").unwrap();
        cipher.seal_file(&source, &sealed).unwrap();
        let bytes = std::fs::read(&sealed).unwrap();
        assert!(!bytes.windows(6).any(|window| window == b"refund"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_changed_byte_a_cut_end_and_a_wrong_key_all_fail_and_leave_nothing() {
        let dir = folder("tamper");
        let cipher = cipher();
        let (source, sealed, opened) = (dir.join("plain"), dir.join("sealed"), dir.join("opened"));
        std::fs::write(&source, vec![7u8; CHUNK_BYTES * 2 + 5]).unwrap();
        cipher.seal_file(&source, &sealed).unwrap();
        let good = std::fs::read(&sealed).unwrap();

        let mut changed = good.clone();
        changed[HEADER_BYTES + 10] ^= 1;
        std::fs::write(&sealed, &changed).unwrap();
        assert!(cipher.open_file(&sealed, &opened).is_err());
        assert!(!opened.exists(), "a failed opening leaves no partial file");

        // The end dropped: the last whole chunk is no longer marked last.
        let cut = HEADER_BYTES + 2 * (CHUNK_BYTES + TAG_BYTES);
        std::fs::write(&sealed, &good[..cut]).unwrap();
        assert!(cipher.open_file(&sealed, &opened).is_err());
        std::fs::write(&sealed, &good[..good.len() - 3]).unwrap();
        assert!(cipher.open_file(&sealed, &opened).is_err());

        // Two chunks swapped.
        let mut swapped = good.clone();
        let (first, second) = (HEADER_BYTES, HEADER_BYTES + CHUNK_BYTES + TAG_BYTES);
        let chunk = CHUNK_BYTES + TAG_BYTES;
        let (a, b) = (
            good[first..first + chunk].to_vec(),
            good[second..second + chunk].to_vec(),
        );
        swapped[first..first + chunk].copy_from_slice(&b);
        swapped[second..second + chunk].copy_from_slice(&a);
        std::fs::write(&sealed, &swapped).unwrap();
        assert!(cipher.open_file(&sealed, &opened).is_err());

        // Another key, even with the right salt.
        std::fs::write(&sealed, &good).unwrap();
        let other = BackupCipher::derive("another passphrase, also long", cipher.salt, ITERATIONS).unwrap();
        assert!(other.open_file(&sealed, &opened).is_err());
        cipher.open_file(&sealed, &opened).unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_manifest_description_recognises_only_the_right_passphrase() {
        let cipher = cipher();
        let description = cipher.description();
        assert!(description.salt.len() == SALT_BYTES * 2 && description.key_check.len() == 64);
        assert!(!serde_json::to_string(&description).unwrap().contains(PASSPHRASE));
        assert!(BackupCipher::open(PASSPHRASE, &description).is_ok());
        assert!(matches!(
            BackupCipher::open("a different passphrase of length", &description),
            Err(KeyError::Mismatch)
        ));
        assert!(matches!(
            BackupCipher::open("short", &description),
            Err(KeyError::Unusable(_))
        ));
        let mut odd = description.clone();
        odd.cipher = "rot13".to_owned();
        assert!(matches!(
            BackupCipher::open(PASSPHRASE, &odd),
            Err(KeyError::Unusable(_))
        ));
        // The same salt and passphrase make the same key.
        let again = BackupCipher::open(PASSPHRASE, &description).unwrap();
        assert_eq!(again.description(), description);
    }

    #[test]
    fn passphrases_must_be_long_enough() {
        assert!(validate_passphrase("short").is_err());
        assert!(validate_passphrase(PASSPHRASE).is_ok());
    }

    #[test]
    fn a_file_sealed_again_opens_with_the_new_key_only_and_holds_the_same_bytes() {
        let dir = folder("reseal");
        let (old, new) = (
            cipher(),
            BackupCipher::create("a second long enough passphrase").unwrap(),
        );
        for size in [0usize, 1, CHUNK_BYTES, CHUNK_BYTES + 1, CHUNK_BYTES * 3 + 17] {
            let plain: Vec<u8> = (0..size).map(|index| (index % 241) as u8).collect();
            let (source, sealed, again, opened) = (
                dir.join("plain"),
                dir.join("sealed"),
                dir.join("again"),
                dir.join("opened"),
            );
            std::fs::write(&source, &plain).unwrap();
            old.seal_file(&source, &sealed).unwrap();
            old.reseal_file(&new, &sealed, &again).unwrap();
            assert_eq!(
                std::fs::metadata(&again).unwrap().len(),
                BackupCipher::sealed_len(size as u64),
                "size {size}"
            );
            assert_ne!(
                std::fs::read(&sealed).unwrap(),
                std::fs::read(&again).unwrap(),
                "size {size}"
            );
            assert!(
                old.open_file(&again, &opened).is_err(),
                "the old key no longer opens it"
            );
            assert!(!opened.exists());
            new.open_file(&again, &opened).unwrap();
            assert_eq!(std::fs::read(&opened).unwrap(), plain, "size {size}");
        }

        // Sealing again fails like opening: a wrong key, a changed byte, a cut end; and leaves nothing.
        let (source, sealed, again) = (dir.join("plain"), dir.join("sealed"), dir.join("again"));
        std::fs::write(&source, vec![9u8; CHUNK_BYTES * 2]).unwrap();
        old.seal_file(&source, &sealed).unwrap();
        assert!(new.reseal_file(&old, &sealed, &again).is_err(), "sealed by another key");
        assert!(!again.exists());
        let mut bytes = std::fs::read(&sealed).unwrap();
        let last = bytes.len() - 20;
        bytes[last] ^= 1;
        std::fs::write(&sealed, &bytes).unwrap();
        assert!(old.reseal_file(&new, &sealed, &again).is_err(), "a changed byte");
        assert!(!again.exists());
        bytes.truncate(HEADER_BYTES + CHUNK_BYTES + TAG_BYTES);
        std::fs::write(&sealed, &bytes).unwrap();
        assert!(old.reseal_file(&new, &sealed, &again).is_err(), "a cut end");
        assert!(!again.exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
