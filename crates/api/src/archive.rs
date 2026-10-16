//! The container of a workspace archive: a gzip-compressed tar.
//!
//! Plain tar (ustar, regular files only, names under 100 bytes) keeps an
//! archive readable with any tool (`tar -tzf`), and lets the writer produce it
//! in pieces: a file's bytes are compressed as they are handed over, so a
//! large workspace is never held in memory twice. What goes in it, and what
//! it means, is described in `repomemo_domain::archive`.

use std::io::{Read, Write};

use anyhow::{bail, Context, Result};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};

const BLOCK: usize = 512;
const NAME_BYTES: usize = 100;
/// Entry sizes are written as eleven octal digits.
const MAX_ENTRY_BYTES: u64 = 0o77_777_777_777;

fn octal(value: u64, field: &mut [u8]) {
    // Digits, then a NUL: the field is one byte longer than its digits.
    let digits = field.len() - 1;
    let text = format!("{value:0digits$o}");
    field[..digits].copy_from_slice(&text.as_bytes()[text.len() - digits..]);
    field[digits] = 0;
}

fn header(name: &str, size: u64, modified: i64) -> Result<[u8; BLOCK]> {
    if name.is_empty() || name.len() > NAME_BYTES || !name.is_ascii() {
        bail!("archive entry name {name:?} cannot be written");
    }
    if size > MAX_ENTRY_BYTES {
        bail!("{name} is too large for an archive entry");
    }
    let mut block = [0u8; BLOCK];
    block[..name.len()].copy_from_slice(name.as_bytes());
    octal(0o644, &mut block[100..108]);
    octal(0, &mut block[108..116]);
    octal(0, &mut block[116..124]);
    octal(size, &mut block[124..136]);
    octal(modified.max(0) as u64, &mut block[136..148]);
    block[148..156].fill(b' ');
    block[156] = b'0';
    block[257..263].copy_from_slice(b"ustar\0");
    block[263..265].copy_from_slice(b"00");
    let checksum: u32 = block.iter().map(|byte| u32::from(*byte)).sum();
    let text = format!("{checksum:06o}");
    block[148..154].copy_from_slice(text.as_bytes());
    block[154] = 0;
    block[155] = b' ';
    Ok(block)
}

/// Writes entries one after another and hands back the compressed bytes as
/// they are produced.
pub struct ArchiveEncoder {
    gzip: GzEncoder<Vec<u8>>,
    open: Option<OpenEntry>,
}

struct OpenEntry {
    name: String,
    size: u64,
    written: u64,
}

impl Default for ArchiveEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl ArchiveEncoder {
    pub fn new() -> Self {
        Self {
            gzip: GzEncoder::new(Vec::new(), Compression::new(6)),
            open: None,
        }
    }

    /// Starts an entry of exactly `size` bytes, to be filled by [`Self::write`].
    pub fn start(&mut self, name: &str, size: u64, modified: i64) -> Result<()> {
        if self.open.is_some() {
            bail!("an archive entry is still open");
        }
        let block = header(name, size, modified)?;
        self.gzip.write_all(&block)?;
        self.open = Some(OpenEntry {
            name: name.to_owned(),
            size,
            written: 0,
        });
        Ok(())
    }

    pub fn write(&mut self, bytes: &[u8]) -> Result<()> {
        let open = self.open.as_mut().context("no archive entry is open")?;
        open.written += bytes.len() as u64;
        if open.written > open.size {
            bail!("{} is longer than the size it was started with", open.name);
        }
        self.gzip.write_all(bytes)?;
        Ok(())
    }

    /// Closes the entry; its bytes must add up to the size it was started with.
    pub fn end(&mut self) -> Result<()> {
        let open = self.open.take().context("no archive entry is open")?;
        if open.written != open.size {
            bail!(
                "{} was started with {} bytes and given {}",
                open.name,
                open.size,
                open.written
            );
        }
        let padding = (BLOCK - (open.size as usize % BLOCK)) % BLOCK;
        self.gzip.write_all(&[0u8; BLOCK][..padding])?;
        Ok(())
    }

    /// A whole entry at once.
    pub fn add(&mut self, name: &str, bytes: &[u8], modified: i64) -> Result<()> {
        self.start(name, bytes.len() as u64, modified)?;
        self.write(bytes)?;
        self.end()
    }

    /// The compressed bytes produced so far, leaving the encoder empty.
    pub fn drain(&mut self) -> Vec<u8> {
        std::mem::take(self.gzip.get_mut())
    }

    /// Ends the archive and returns what has not been drained yet.
    pub fn finish(mut self) -> Result<Vec<u8>> {
        if let Some(open) = &self.open {
            bail!("{} was not finished", open.name);
        }
        self.gzip.write_all(&[0u8; BLOCK * 2])?;
        Ok(self.gzip.finish()?)
    }
}

/// Reads every entry of an archive, refusing one larger than `max_entry_bytes`.
/// Directories and other non-file entries are skipped.
pub fn read_archive_entries<R: Read>(reader: R, max_entry_bytes: u64) -> Result<Vec<(String, Vec<u8>)>> {
    let mut decoder = GzDecoder::new(reader);
    let mut entries = Vec::new();
    loop {
        let mut block = [0u8; BLOCK];
        decoder
            .read_exact(&mut block)
            .context("the archive ends before its end marker")?;
        if block.iter().all(|byte| *byte == 0) {
            break;
        }
        let name_end = block[..NAME_BYTES].iter().position(|byte| *byte == 0).unwrap_or(NAME_BYTES);
        let name = String::from_utf8_lossy(&block[..name_end]).into_owned();
        let size_text = String::from_utf8_lossy(&block[124..135]).trim().to_owned();
        let size = u64::from_str_radix(size_text.trim_matches('\0').trim(), 8)
            .with_context(|| format!("{name} has an unreadable size"))?;
        if size > max_entry_bytes {
            bail!("{name} is larger than the {max_entry_bytes} bytes an entry may have");
        }
        let mut bytes = vec![0u8; size as usize];
        decoder
            .read_exact(&mut bytes)
            .with_context(|| format!("the archive ends inside {name}"))?;
        let padding = (BLOCK - (size as usize % BLOCK)) % BLOCK;
        let mut skipped = [0u8; BLOCK];
        decoder.read_exact(&mut skipped[..padding])?;
        if matches!(block[156], b'0' | 0) {
            entries.push((name, bytes));
        }
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checksum_of(block: &[u8; BLOCK]) -> u32 {
        block
            .iter()
            .enumerate()
            .map(|(index, byte)| if (148..156).contains(&index) { 32 } else { u32::from(*byte) })
            .sum()
    }

    #[test]
    fn a_header_is_a_valid_ustar_block() {
        let block = header("blobs/abc", 1234, 1_700_000_000).unwrap();
        assert_eq!(&block[257..262], b"ustar");
        let stored = u32::from_str_radix(String::from_utf8_lossy(&block[148..154]).as_ref(), 8).unwrap();
        assert_eq!(stored, checksum_of(&block));
        assert_eq!(&block[124..135], b"00000002322");
    }

    #[test]
    fn names_that_cannot_be_written_are_refused() {
        assert!(header("", 0, 0).is_err());
        assert!(header(&"a".repeat(101), 0, 0).is_err());
        assert!(header("tëst", 0, 0).is_err());
        assert!(header("big", MAX_ENTRY_BYTES + 1, 0).is_err());
    }

    #[test]
    fn entries_round_trip_in_pieces_and_whole() {
        let mut encoder = ArchiveEncoder::new();
        encoder.add("manifest.json", b"{\"a\":1}", 0).unwrap();
        encoder.start("blobs/one", 1000, 0).unwrap();
        let mut compressed = Vec::new();
        for chunk in [vec![7u8; 400], vec![9u8; 600]] {
            encoder.write(&chunk).unwrap();
            compressed.extend(encoder.drain());
        }
        encoder.end().unwrap();
        encoder.add("empty", b"", 0).unwrap();
        compressed.extend(encoder.finish().unwrap());

        let entries = read_archive_entries(compressed.as_slice(), 10_000).unwrap();
        let names = entries.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>();
        assert_eq!(names, ["manifest.json", "blobs/one", "empty"]);
        assert_eq!(entries[0].1, b"{\"a\":1}");
        assert_eq!(entries[1].1.len(), 1000);
        assert_eq!(entries[1].1[399], 7);
        assert_eq!(entries[1].1[400], 9);
        assert!(entries[2].1.is_empty());
    }

    #[test]
    fn an_entry_must_have_the_size_it_started_with() {
        let mut encoder = ArchiveEncoder::new();
        encoder.start("short", 10, 0).unwrap();
        encoder.write(b"12345").unwrap();
        assert!(encoder.end().is_err());

        let mut encoder = ArchiveEncoder::new();
        encoder.start("long", 2, 0).unwrap();
        assert!(encoder.write(b"123").is_err());
    }

    #[test]
    fn a_truncated_archive_is_refused_and_so_is_an_oversized_entry() {
        let mut encoder = ArchiveEncoder::new();
        encoder.add("data", &vec![1u8; 5000], 0).unwrap();
        let bytes = encoder.finish().unwrap();
        assert!(read_archive_entries(&bytes[..bytes.len() / 2], 10_000).is_err());
        assert!(read_archive_entries(bytes.as_slice(), 100).is_err());
        assert!(read_archive_entries(bytes.as_slice(), 10_000).is_ok());
    }
}
