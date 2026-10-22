//! Files received from the network, kept out of memory: written to a temporary
//! file in `<data>/tmp/` while their SHA-256 and size are worked out, then
//! moved into the blob store.

use super::*;
use tokio::io::AsyncWriteExt;

/// A file being received. Each chunk goes to disk and into the hash, so what is
/// held in memory is one chunk, whatever the file's size.
pub struct StagedBlobWriter {
    path: PathBuf,
    file: tokio::fs::File,
    hasher: Sha256,
    size: u64,
    /// Removes the file when the upload fails or is abandoned.
    keep: bool,
}

/// A received file, complete and hashed, waiting to join the blob store. A
/// staged file that is dropped without being stored is deleted.
#[derive(Debug)]
pub struct StagedBlob {
    path: Option<PathBuf>,
    pub content_hash: String,
    pub size_bytes: u64,
}

impl StagedBlobWriter {
    pub async fn write(&mut self, chunk: &[u8]) -> std::io::Result<()> {
        self.file.write_all(chunk).await?;
        self.hasher.update(chunk);
        self.size += chunk.len() as u64;
        Ok(())
    }

    /// Bytes written so far.
    pub fn size(&self) -> u64 {
        self.size
    }

    pub async fn finish(mut self) -> std::io::Result<StagedBlob> {
        self.file.flush().await?;
        self.keep = true;
        Ok(StagedBlob {
            path: Some(self.path.clone()),
            content_hash: hex::encode(self.hasher.clone().finalize()),
            size_bytes: self.size,
        })
    }
}

impl Drop for StagedBlobWriter {
    fn drop(&mut self) {
        if !self.keep {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

impl Drop for StagedBlob {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

impl StorageEngine {
    /// Starts receiving a file into `<data>/tmp/`.
    pub async fn begin_staged_blob(&self) -> Result<StagedBlobWriter> {
        let directory = self.data_dir.join("tmp");
        tokio::fs::create_dir_all(&directory).await?;
        let path = directory.join(format!("upload-{}.part", Uuid::new_v4().simple()));
        let file = tokio::fs::File::create(&path)
            .await
            .with_context(|| format!("failed to create {}", path.display()))?;
        Ok(StagedBlobWriter {
            path,
            file,
            hasher: Sha256::new(),
            size: 0,
            keep: false,
        })
    }

    /// Moves a received file into the blob store under its hash, with the same
    /// bookkeeping as [`StorageEngine::store_blob`]. A blob that is already
    /// stored keeps its file and the received copy is deleted.
    pub async fn store_staged_blob(&self, mut staged: StagedBlob, mime_type: Option<&str>) -> Result<String> {
        let content_hash = staged.content_hash.clone();
        if content_hash.len() < 4 || !content_hash.chars().all(|value| value.is_ascii_alphanumeric()) {
            bail!("content hash is invalid");
        }
        let blob_path = self.blob_path(&content_hash);
        if let Some(parent) = blob_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let storage_uri = self.blob_storage_uri(&content_hash);

        let _guard = self.blob_lock.lock().await;
        self.record_blob(&content_hash, &storage_uri, staged.size_bytes as i64, mime_type)
            .await?;
        let Some(source) = staged.path.take() else {
            bail!("the received file is gone");
        };
        if tokio::fs::metadata(&blob_path).await.is_err() {
            // The same volume, so this is a rename and never a second copy.
            if let Err(error) = tokio::fs::rename(&source, &blob_path).await {
                let _ = tokio::fs::remove_file(&source).await;
                return Err(error).with_context(|| format!("failed to move the upload to {}", blob_path.display()));
            }
        } else {
            let _ = tokio::fs::remove_file(&source).await;
        }
        Ok(storage_uri)
    }
}
