//! Faithful previews of Office files: LibreOffice converts Word, Excel and
//! PowerPoint documents to PDF once, and the PDF is cached by content hash.
//!
//! LibreOffice is optional. When it is not installed the converter is
//! disabled and the client keeps showing the extracted-text previews.
//!
//! Hardening: the input is written under a fixed name (the upload's file name
//! never reaches the command line), only one conversion runs at a time, each
//! has a timeout after which the whole process tree is killed, and the output
//! size is capped. Conversion never executes macros: LibreOffice's default
//! macro security blocks them in headless mode.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use repomemo_domain::ArtifactSummary;
use repomemo_storage::StorageEngine;
use tokio::{process::Command, sync::Semaphore};

const CONVERSION_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_PDF_BYTES: u64 = 100 * 1024 * 1024;
const SUPPORTED_EXTENSIONS: &[&str] = &["doc", "docx", "xls", "xlsx", "xlsm", "ppt", "pptx"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderState {
    /// LibreOffice is not available on this server.
    Disabled,
    /// This file type is not converted.
    Unsupported,
    Converting,
    Ready,
    Failed(String),
}

struct Inner {
    soffice: Option<PathBuf>,
    cache_dir: PathBuf,
    profile_dir: PathBuf,
    gate: Semaphore,
    in_flight: Mutex<HashSet<String>>,
}

#[derive(Clone)]
pub struct Converter {
    inner: Arc<Inner>,
}

impl Converter {
    /// Looks for LibreOffice (`REPOMEMO_SOFFICE`, then the usual locations) and
    /// prepares the cache under the server data directory.
    pub fn detect(data_dir: &Path) -> Self {
        let explicit = std::env::var_os("REPOMEMO_SOFFICE").map(PathBuf::from);
        Self::with_binary(data_dir, explicit.or_else(find_soffice))
    }

    pub fn with_binary(data_dir: &Path, soffice: Option<PathBuf>) -> Self {
        let cache_dir = data_dir.join("previews");
        let _ = std::fs::create_dir_all(&cache_dir);
        // A restart is a fresh chance for files that failed before.
        if let Ok(entries) = std::fs::read_dir(&cache_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|extension| extension == "failed") {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
        let _ = std::fs::remove_dir_all(cache_dir.join("tmp"));
        Self {
            inner: Arc::new(Inner {
                soffice: soffice.filter(|path| path.exists() || path.components().count() == 1),
                profile_dir: cache_dir.join("profile"),
                cache_dir,
                gate: Semaphore::new(1),
                in_flight: Mutex::new(HashSet::new()),
            }),
        }
    }

    pub fn enabled(&self) -> bool {
        self.inner.soffice.is_some()
    }

    pub fn supports(path: &str) -> bool {
        path.rsplit('.')
            .next()
            .map(str::to_ascii_lowercase)
            .is_some_and(|extension| SUPPORTED_EXTENSIONS.contains(&extension.as_str()))
    }

    fn pdf_path(&self, content_hash: &str) -> PathBuf {
        self.inner.cache_dir.join(format!("{content_hash}.pdf"))
    }

    fn failure_path(&self, content_hash: &str) -> PathBuf {
        self.inner.cache_dir.join(format!("{content_hash}.failed"))
    }

    /// Deletes the cached preview (and any remembered failure) of a content
    /// hash whose blob is gone. Returns true when a cached PDF was removed.
    pub fn forget(&self, content_hash: &str) -> bool {
        if !safe_hash(content_hash) {
            return false;
        }
        let _ = std::fs::remove_file(self.failure_path(content_hash));
        std::fs::remove_file(self.pdf_path(content_hash)).is_ok()
    }

    /// Deletes cached previews whose blob no longer exists, for example ones
    /// left from before blob garbage collection. Returns how many were removed.
    pub async fn prune_orphans(&self, storage: &StorageEngine) -> anyhow::Result<usize> {
        let mut entries = match tokio::fs::read_dir(&self.inner.cache_dir).await {
            Ok(entries) => entries,
            Err(_) => return Ok(0),
        };
        let mut removed = 0;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("pdf") {
                continue;
            }
            let Some(hash) = path.file_stem().and_then(|stem| stem.to_str()).map(str::to_owned) else {
                continue;
            };
            if !safe_hash(&hash) {
                continue;
            }
            let in_flight = self
                .inner
                .in_flight
                .lock()
                .map(|running| running.contains(&hash))
                .unwrap_or(true);
            if !in_flight && !storage.blob_exists(&hash).await? && tokio::fs::remove_file(&path).await.is_ok() {
                removed += 1;
            }
        }
        Ok(removed)
    }

    /// The cached PDF for a file, when its conversion has finished.
    pub fn cached_pdf(&self, content_hash: &str) -> Option<PathBuf> {
        safe_hash(content_hash)
            .then(|| self.pdf_path(content_hash))
            .filter(|path| path.exists())
    }

    /// The current state, starting a conversion when none has run yet.
    pub fn state(&self, storage: &StorageEngine, summary: &ArtifactSummary) -> RenderState {
        if !Self::supports(&summary.path) {
            return RenderState::Unsupported;
        }
        if !self.enabled() || !safe_hash(&summary.content_hash) {
            return RenderState::Disabled;
        }
        if self.pdf_path(&summary.content_hash).exists() {
            return RenderState::Ready;
        }
        if let Ok(message) = std::fs::read_to_string(self.failure_path(&summary.content_hash)) {
            return RenderState::Failed(message);
        }
        self.ensure(storage.clone(), summary.clone());
        RenderState::Converting
    }

    /// Starts a background conversion unless one is cached, failed or running.
    pub fn ensure(&self, storage: StorageEngine, summary: ArtifactSummary) {
        if !self.enabled()
            || !Self::supports(&summary.path)
            || !safe_hash(&summary.content_hash)
            || self.pdf_path(&summary.content_hash).exists()
            || self.failure_path(&summary.content_hash).exists()
        {
            return;
        }
        let newly_started = self
            .inner
            .in_flight
            .lock()
            .map(|mut running| running.insert(summary.content_hash.clone()))
            .unwrap_or(false);
        if !newly_started {
            return;
        }
        let converter = self.clone();
        tokio::spawn(async move {
            let outcome = converter.convert(&storage, &summary).await;
            if let Err(message) = outcome {
                tracing::warn!(artifact_id = %summary.id, error = %message, "Office preview conversion failed");
                let _ = std::fs::write(converter.failure_path(&summary.content_hash), message);
            }
            if let Ok(mut running) = converter.inner.in_flight.lock() {
                running.remove(&summary.content_hash);
            }
        });
    }

    async fn convert(&self, storage: &StorageEngine, summary: &ArtifactSummary) -> Result<(), String> {
        let soffice = self.inner.soffice.clone().ok_or("LibreOffice is not available.")?;
        let _turn = self
            .inner
            .gate
            .acquire()
            .await
            .map_err(|_| "The converter is shutting down.".to_owned())?;
        if self.pdf_path(&summary.content_hash).exists() {
            return Ok(());
        }

        let bytes = storage
            .read_artifact_blob(&summary.id)
            .await
            .map_err(|error| format!("could not read the file: {error}"))?;
        let extension = summary
            .path
            .rsplit('.')
            .next()
            .map(str::to_ascii_lowercase)
            .filter(|extension| SUPPORTED_EXTENSIONS.contains(&extension.as_str()))
            .ok_or("This file type is not converted.")?;

        let work = self
            .inner
            .cache_dir
            .join("tmp")
            .join(uuid_like(&summary.content_hash));
        tokio::fs::create_dir_all(&work)
            .await
            .map_err(|error| error.to_string())?;
        tokio::fs::create_dir_all(&self.inner.profile_dir)
            .await
            .map_err(|error| error.to_string())?;
        let input = work.join(format!("input.{extension}"));
        tokio::fs::write(&input, &bytes)
            .await
            .map_err(|error| error.to_string())?;

        let result = self.run(&soffice, &input, &work).await;
        let output = work.join("input.pdf");
        let finished = match result {
            Ok(()) => match tokio::fs::metadata(&output).await {
                Ok(meta) if meta.len() == 0 => Err("The converter produced an empty PDF.".to_owned()),
                Ok(meta) if meta.len() > MAX_PDF_BYTES => {
                    Err("The converted PDF is too large to preview.".to_owned())
                }
                Ok(_) => {
                    let target = self.pdf_path(&summary.content_hash);
                    let staging = target.with_extension("pdf.part");
                    tokio::fs::copy(&output, &staging)
                        .await
                        .and_then(|_| std::fs::rename(&staging, &target))
                        .map_err(|error| error.to_string())
                }
                Err(_) => Err("LibreOffice could not convert this file.".to_owned()),
            },
            Err(message) => Err(message),
        };
        let _ = tokio::fs::remove_dir_all(&work).await;
        finished
    }

    async fn run(&self, soffice: &Path, input: &Path, outdir: &Path) -> Result<(), String> {
        let mut command = Command::new(soffice);
        command
            .arg("--headless")
            .arg("--norestore")
            .arg("--nolockcheck")
            .arg("--nodefault")
            .arg("--nologo")
            .arg(format!("-env:UserInstallation={}", file_uri(&self.inner.profile_dir)))
            .arg("--convert-to")
            .arg("pdf")
            .arg("--outdir")
            .arg(outdir)
            .arg(input)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        let mut child = command
            .spawn()
            .map_err(|error| format!("could not start LibreOffice: {error}"))?;
        let pid = child.id();
        match tokio::time::timeout(CONVERSION_TIMEOUT, child.wait()).await {
            Ok(Ok(status)) if status.success() => Ok(()),
            Ok(Ok(status)) => Err(format!("LibreOffice exited with {status}.")),
            Ok(Err(error)) => Err(format!("LibreOffice failed: {error}")),
            Err(_) => {
                kill_tree(pid).await;
                let _ = child.kill().await;
                Err("The conversion took too long and was stopped.".to_owned())
            }
        }
    }
}

fn safe_hash(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|character| character.is_ascii_alphanumeric())
}

fn uuid_like(seed: &str) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("{}-{nanos}", seed.chars().take(12).collect::<String>())
}

/// `file:///` URI for LibreOffice's `-env:UserInstallation`.
fn file_uri(path: &Path) -> String {
    let absolute = std::fs::canonicalize(path)
        .or_else(|_| std::env::current_dir().map(|current| current.join(path)))
        .unwrap_or_else(|_| path.to_path_buf());
    let mut text = absolute.to_string_lossy().replace('\\', "/");
    if let Some(stripped) = text.strip_prefix("//?/") {
        text = stripped.to_owned();
    }
    let encoded = text.replace('%', "%25").replace(' ', "%20");
    if encoded.starts_with('/') {
        format!("file://{encoded}")
    } else {
        format!("file:///{encoded}")
    }
}

async fn kill_tree(pid: Option<u32>) {
    let Some(pid) = pid else { return };
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .output()
            .await;
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("pkill")
            .args(["-KILL", "-P", &pid.to_string()])
            .output()
            .await;
    }
}

fn find_soffice() -> Option<PathBuf> {
    let names: &[&str] = if cfg!(windows) {
        &["soffice.exe", "soffice.com"]
    } else {
        &["soffice", "libreoffice"]
    };
    if let Some(paths) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&paths) {
            for name in names {
                let candidate = directory.join(name);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    let known: &[&str] = if cfg!(windows) {
        &[
            "C:\\Program Files\\LibreOffice\\program\\soffice.exe",
            "C:\\Program Files (x86)\\LibreOffice\\program\\soffice.exe",
        ]
    } else if cfg!(target_os = "macos") {
        &["/Applications/LibreOffice.app/Contents/MacOS/soffice"]
    } else {
        &["/usr/bin/soffice", "/usr/lib/libreoffice/program/soffice", "/opt/libreoffice/program/soffice"]
    };
    known
        .iter()
        .map(PathBuf::from)
        .find(|candidate| candidate.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_office_files_are_converted() {
        assert!(Converter::supports("Budget.XLSX"));
        assert!(Converter::supports("deck.pptx"));
        assert!(!Converter::supports("notes.pdf"));
        assert!(!Converter::supports("mail.msg"));
    }

    #[test]
    fn file_uris_are_percent_encoded() {
        let uri = file_uri(Path::new("C:/Users/A B/profile"));
        assert!(uri.starts_with("file://"));
        assert!(uri.contains("A%20B"));
    }

    #[test]
    fn hashes_must_be_plain_alphanumerics() {
        assert!(safe_hash("abc123"));
        assert!(!safe_hash("../etc"));
        assert!(!safe_hash(""));
    }
}
