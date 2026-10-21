//! Faithful previews of Office files: LibreOffice converts Word, Excel and
//! PowerPoint documents to PDF once, and the PDF is cached by content hash.
//!
//! LibreOffice is optional and off by default. Nothing is converted unless the
//! operator sets `REPOMEMO_OFFICE_PREVIEWS`, LibreOffice is installed, and the
//! release it reports is at or past the fixed ones (see [`release_problem`]);
//! otherwise the converter is disabled and the client keeps showing the
//! extracted-text previews. [`Converter::status`] says which of these it is.
//!
//! Hardening: the input is written under a fixed name (the upload's file name
//! never reaches the command line), only one conversion runs at a time, each
//! has a timeout after which the whole process tree is killed, and the output
//! size is capped. LibreOffice starts with a profile the server writes before
//! every conversion (see [`PROFILE_SETTINGS`]): macros disabled at the highest
//! security level, no active content, links never updated or followed from
//! untrusted documents, no Java and no update check, so a document cannot
//! change those settings for the next one.
//!
//! Sandboxing (review M10): LibreOffice parses files strangers uploaded, on
//! the server's own host. It runs without the server's secrets (every
//! `REPOMEMO_*` variable and anything named like a key, token or password is
//! removed from its environment), in a folder of its own, and, when the
//! operator sets `REPOMEMO_SOFFICE_WRAPPER`, inside that command (a container
//! or a namespace without network and without the server's data folder; see
//! the operations notes). The version it reports is shown in System ›
//! Overview.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use repomemo_domain::ArtifactSummary;
use repomemo_storage::StorageEngine;
use tokio::{process::Command, sync::Semaphore};

/// Under the cache folder, one folder per workspace for what that workspace has been told.
const WORKSPACES_DIR: &str = "workspaces";

/// What the server writes into LibreOffice's profile (`user/registrymodifications.xcu`) before every
/// conversion: (configuration path, property, value). The numbers follow LibreOffice's own schema:
/// macro security level 3 is "very high", and "update links" is 2 (never) in Writer but 1 (never)
/// in Calc, whose numbering differs.
const PROFILE_SETTINGS: &[(&str, &str, &str)] = &[
    (
        "/org.openoffice.Office.Common/Security/Scripting",
        "MacroSecurityLevel",
        "3",
    ),
    (
        "/org.openoffice.Office.Common/Security/Scripting",
        "DisableMacrosExecution",
        "true",
    ),
    (
        "/org.openoffice.Office.Common/Security/Scripting",
        "DisableActiveContent",
        "true",
    ),
    (
        "/org.openoffice.Office.Common/Security/Scripting",
        "BlockUntrustedRefererLinks",
        "true",
    ),
    (
        "/org.openoffice.Office.Common/Security/Scripting",
        "DisableOLEAutomation",
        "true",
    ),
    (
        "/org.openoffice.Office.Common/Security/Scripting",
        "DisablePythonRuntime",
        "true",
    ),
    ("/org.openoffice.Office.Writer/Content/Update", "Link", "2"),
    ("/org.openoffice.Office.Calc/Content/Update", "Link", "1"),
    (
        "/org.openoffice.Office.Jobs/Jobs/org.openoffice.Office.Jobs:Job['UpdateCheck']/Arguments",
        "AutoCheckEnabled",
        "false",
    ),
];

/// LibreOffice's Java settings file, read from the path given by `UNO_JAVA_JFW_USER_DATA`: Java is off.
const JAVA_SETTINGS_FILE: &str = "javasettings.xml";
const JAVA_DISABLED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<java xmlns="http://openoffice.org/2004/java/framework/1.0" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
<enabled xsi:nil="false">false</enabled>
<userClassPath xsi:nil="true"/>
<vmParameters xsi:nil="true"/>
<jreLocations xsi:nil="true"/>
<javaInfo xsi:nil="true"/>
</java>
"#;

/// The text of `registrymodifications.xcu` for [`PROFILE_SETTINGS`].
fn profile_registry() -> String {
    let mut text = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<oor:items xmlns:oor=\"http://openoffice.org/2001/registry\" \
         xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\n",
    );
    for (path, property, value) in PROFILE_SETTINGS {
        text.push_str(&format!(
            "<item oor:path=\"{path}\"><prop oor:name=\"{property}\" oor:op=\"fuse\"><value>{value}</value></prop></item>\n"
        ));
    }
    text.push_str("</oor:items>\n");
    text
}
const CONVERSION_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_PDF_BYTES: u64 = 100 * 1024 * 1024;
const SUPPORTED_EXTENSIONS: &[&str] = &["doc", "docx", "xls", "xlsx", "xlsm", "ppt", "pptx"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderState {
    /// Office previews are not available on this server: they are off,
    /// LibreOffice is missing or its release is refused.
    Disabled,
    /// This file type is not converted.
    Unsupported,
    Converting,
    Ready,
    Failed(String),
}

/// Why the converter does or does not convert, for System › Overview and the
/// server check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConverterStatus {
    /// LibreOffice was not found.
    NotInstalled,
    /// LibreOffice is installed and `REPOMEMO_OFFICE_PREVIEWS` is not on.
    Off,
    /// Previews are on and the release LibreOffice reports is not known yet.
    Checking,
    /// Previews are on, and this release is refused (the text says why).
    Refused(String),
    /// Previews are on and the release is accepted.
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum VersionCheck {
    Pending,
    Accepted,
    Refused(String),
}

struct Inner {
    /// `REPOMEMO_OFFICE_PREVIEWS`: nothing is converted while it is off.
    previews: bool,
    soffice: Option<PathBuf>,
    /// A command that LibreOffice is started through (`REPOMEMO_SOFFICE_WRAPPER`),
    /// with `{work}` and `{profile}` replaced by the folders it needs.
    wrapper: Vec<String>,
    version: Mutex<Option<String>>,
    check: Mutex<VersionCheck>,
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
    /// prepares the cache under the server data directory. LibreOffice is
    /// found even when `previews` is off, so System › Overview can say it is
    /// installed; it is not started, and nothing is converted, until it is on.
    pub fn detect(data_dir: &Path, wrapper: Vec<String>, previews: bool) -> Self {
        let explicit = std::env::var_os("REPOMEMO_SOFFICE").map(PathBuf::from);
        Self::with_options(data_dir, explicit.or_else(find_soffice), wrapper, previews)
    }

    pub fn with_options(data_dir: &Path, soffice: Option<PathBuf>, wrapper: Vec<String>, previews: bool) -> Self {
        let cache_dir = data_dir.join("previews");
        let _ = std::fs::create_dir_all(&cache_dir);
        // A restart is a fresh chance for files that failed before: the remembered failures of every
        // workspace go (and the ones an earlier release kept next to the PDFs).
        remove_failures(&cache_dir);
        if let Ok(workspaces) = std::fs::read_dir(cache_dir.join(WORKSPACES_DIR)) {
            for workspace in workspaces.flatten() {
                remove_failures(&workspace.path());
            }
        }
        let _ = std::fs::remove_dir_all(cache_dir.join("tmp"));
        Self {
            inner: Arc::new(Inner {
                previews,
                soffice: soffice.filter(|path| path.exists() || path.components().count() == 1),
                wrapper,
                version: Mutex::new(None),
                check: Mutex::new(VersionCheck::Pending),
                profile_dir: cache_dir.join("profile"),
                cache_dir,
                gate: Semaphore::new(1),
                in_flight: Mutex::new(HashSet::new()),
            }),
        }
    }

    /// Writes the settings LibreOffice is started with, over whatever the last document left in
    /// the profile: the registry of [`PROFILE_SETTINGS`] and the file that turns Java off.
    async fn write_profile(&self) -> std::io::Result<()> {
        let user = self.inner.profile_dir.join("user");
        tokio::fs::create_dir_all(&user).await?;
        tokio::fs::write(user.join("registrymodifications.xcu"), profile_registry()).await?;
        tokio::fs::write(self.inner.profile_dir.join(JAVA_SETTINGS_FILE), JAVA_DISABLED).await
    }

    /// Whether LibreOffice is started inside the operator's sandbox command.
    pub fn sandboxed(&self) -> bool {
        !self.inner.wrapper.is_empty()
    }

    /// The version LibreOffice reported, once [`Converter::probe_version`] has run.
    pub fn version(&self) -> Option<String> {
        self.inner.version.lock().ok().and_then(|version| version.clone())
    }

    /// Asks LibreOffice for its version and decides whether it may convert:
    /// the release must be at or past the fixed ones. Does nothing unless
    /// previews are on and LibreOffice is installed (an off converter never
    /// starts it). Call it once at start, before the server takes requests;
    /// until it has answered nothing is converted. A version that cannot be
    /// read is refused too, because it cannot be compared.
    pub async fn probe_version(&self) {
        let Some(soffice) = self.inner.soffice.clone() else {
            return;
        };
        if !self.inner.previews {
            return;
        }
        let mut command = self.command(&soffice, &self.inner.cache_dir.join("tmp"));
        command
            .arg("--version")
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        let reported = match tokio::time::timeout(Duration::from_secs(30), command.output()).await {
            Ok(Ok(output)) => String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .map(|line| line.chars().take(120).collect::<String>()),
            _ => None,
        };
        let check = match reported.as_deref().map(release_problem) {
            Some(None) => VersionCheck::Accepted,
            Some(Some(problem)) => VersionCheck::Refused(problem),
            None => VersionCheck::Refused(
                "its version could not be read, so it cannot be compared with the fixed releases".to_owned(),
            ),
        };
        if let Ok(mut version) = self.inner.version.lock() {
            *version = reported;
        }
        if let Ok(mut state) = self.inner.check.lock() {
            *state = check;
        }
    }

    /// Whether the operator turned previews on (`REPOMEMO_OFFICE_PREVIEWS`).
    pub fn requested(&self) -> bool {
        self.inner.previews
    }

    /// Whether LibreOffice was found on this machine.
    pub fn installed(&self) -> bool {
        self.inner.soffice.is_some()
    }

    /// Why the converter does or does not convert.
    pub fn status(&self) -> ConverterStatus {
        if self.inner.soffice.is_none() {
            return ConverterStatus::NotInstalled;
        }
        if !self.inner.previews {
            return ConverterStatus::Off;
        }
        match self.inner.check.lock().map(|check| check.clone()) {
            Ok(VersionCheck::Accepted) => ConverterStatus::Ready,
            Ok(VersionCheck::Refused(problem)) => ConverterStatus::Refused(problem),
            _ => ConverterStatus::Checking,
        }
    }

    /// The command that starts LibreOffice: the wrapper (when set), then the
    /// binary, with no secrets of the server's in its environment.
    fn command(&self, soffice: &Path, work: &Path) -> Command {
        let replace = |argument: &str| {
            argument
                .replace("{work}", &work.to_string_lossy())
                .replace("{profile}", &self.inner.profile_dir.to_string_lossy())
        };
        let mut command = match self.inner.wrapper.split_first() {
            Some((program, arguments)) => {
                let mut command = Command::new(replace(program));
                command
                    .args(arguments.iter().map(|argument| replace(argument)))
                    .arg(soffice);
                command
            }
            None => Command::new(soffice),
        };
        for (name, _) in std::env::vars() {
            if is_secret_variable(&name) {
                command.env_remove(name);
            }
        }
        #[cfg(windows)]
        {
            // CREATE_NO_WINDOW: LibreOffice's launcher must never open a console.
            command.creation_flags(0x0800_0000);
        }
        command
    }

    /// Conversions started and not finished yet.
    pub fn running_count(&self) -> usize {
        self.inner.in_flight.lock().map(|running| running.len()).unwrap_or(0)
    }

    /// Whether files are converted: previews are on, LibreOffice is installed
    /// and its release is accepted.
    pub fn enabled(&self) -> bool {
        self.status() == ConverterStatus::Ready
    }

    pub fn supports(path: &str) -> bool {
        path.rsplit('.')
            .next()
            .map(str::to_ascii_lowercase)
            .is_some_and(|extension| SUPPORTED_EXTENSIONS.contains(&extension.as_str()))
    }

    /// The converted PDF, shared by every workspace that holds the same bytes.
    fn pdf_path(&self, content_hash: &str) -> PathBuf {
        self.inner.cache_dir.join(format!("{content_hash}.pdf"))
    }

    /// What a workspace has been told about a file: `<hash>.ready` once its own request found the PDF
    /// done, `<hash>.failed` (with the reason) when it failed. Kept per workspace, so a workspace
    /// cannot tell from a preview being "ready" at once that another one holds the same file (E19).
    fn marker_path(&self, workspace_id: &str, content_hash: &str, kind: &str) -> PathBuf {
        self.inner
            .cache_dir
            .join(WORKSPACES_DIR)
            .join(workspace_id)
            .join(format!("{content_hash}.{kind}"))
    }

    fn ready_path(&self, workspace_id: &str, content_hash: &str) -> PathBuf {
        self.marker_path(workspace_id, content_hash, "ready")
    }

    fn failure_path(&self, workspace_id: &str, content_hash: &str) -> PathBuf {
        self.marker_path(workspace_id, content_hash, "failed")
    }

    /// Deletes the cached preview (and any remembered state) of a content
    /// hash whose blob is gone. Returns true when a cached PDF was removed.
    pub fn forget(&self, content_hash: &str) -> bool {
        if !safe_hash(content_hash) {
            return false;
        }
        if let Ok(workspaces) = std::fs::read_dir(self.inner.cache_dir.join(WORKSPACES_DIR)) {
            for workspace in workspaces.flatten() {
                let _ = std::fs::remove_file(workspace.path().join(format!("{content_hash}.ready")));
                let _ = std::fs::remove_file(workspace.path().join(format!("{content_hash}.failed")));
            }
        }
        let _ = std::fs::remove_file(self.inner.cache_dir.join(format!("{content_hash}.failed")));
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
            let suffix = format!("/{hash}");
            let in_flight = self
                .inner
                .in_flight
                .lock()
                .map(|running| running.iter().any(|key| key.ends_with(&suffix)))
                .unwrap_or(true);
            if !in_flight && !storage.blob_exists(&hash).await? && tokio::fs::remove_file(&path).await.is_ok() {
                removed += 1;
            }
        }
        Ok(removed)
    }

    /// The cached PDF for a file in a workspace, once that workspace's own conversion request has
    /// finished (a PDF another workspace made is not served until this one has asked for it).
    pub fn cached_pdf(&self, workspace_id: &str, content_hash: &str) -> Option<PathBuf> {
        (safe_hash(content_hash) && safe_id(workspace_id) && self.ready_path(workspace_id, content_hash).exists())
            .then(|| self.pdf_path(content_hash))
            .filter(|path| path.exists())
    }

    /// The current state, starting a conversion when this workspace has not asked for one yet.
    pub fn state(&self, storage: &StorageEngine, summary: &ArtifactSummary) -> RenderState {
        if !Self::supports(&summary.path) {
            return RenderState::Unsupported;
        }
        if !self.enabled() || !safe_hash(&summary.content_hash) || !safe_id(&summary.workspace_id) {
            return RenderState::Disabled;
        }
        if self.cached_pdf(&summary.workspace_id, &summary.content_hash).is_some() {
            return RenderState::Ready;
        }
        if let Ok(message) = std::fs::read_to_string(self.failure_path(&summary.workspace_id, &summary.content_hash)) {
            return RenderState::Failed(message);
        }
        self.ensure(storage.clone(), summary.clone());
        RenderState::Converting
    }

    /// Starts a background conversion for a workspace unless it is ready, failed or running there.
    /// When another workspace already made the PDF, the conversion finds it and only records that
    /// this workspace has asked, so the answer goes through the same states in every workspace.
    pub fn ensure(&self, storage: StorageEngine, summary: ArtifactSummary) {
        if !self.enabled()
            || !Self::supports(&summary.path)
            || !safe_hash(&summary.content_hash)
            || !safe_id(&summary.workspace_id)
            || self.cached_pdf(&summary.workspace_id, &summary.content_hash).is_some()
            || self.failure_path(&summary.workspace_id, &summary.content_hash).exists()
        {
            return;
        }
        let key = format!("{}/{}", summary.workspace_id, summary.content_hash);
        let newly_started = self
            .inner
            .in_flight
            .lock()
            .map(|mut running| running.insert(key.clone()))
            .unwrap_or(false);
        if !newly_started {
            return;
        }
        let converter = self.clone();
        tokio::spawn(async move {
            let outcome = converter.convert(&storage, &summary).await;
            let markers = converter.marker_path(&summary.workspace_id, &summary.content_hash, "ready");
            match outcome {
                Ok(()) => {
                    if let Some(folder) = markers.parent() {
                        let _ = std::fs::create_dir_all(folder);
                    }
                    let _ = std::fs::write(&markers, b"");
                }
                Err(message) => {
                    tracing::warn!(artifact_id = %summary.id, error = %message, "Office preview conversion failed");
                    if let Some(folder) = markers.parent() {
                        let _ = std::fs::create_dir_all(folder);
                    }
                    let _ = std::fs::write(
                        converter.failure_path(&summary.workspace_id, &summary.content_hash),
                        message,
                    );
                }
            }
            if let Ok(mut running) = converter.inner.in_flight.lock() {
                running.remove(&key);
            }
        });
    }

    async fn convert(&self, storage: &StorageEngine, summary: &ArtifactSummary) -> Result<(), String> {
        if !self.enabled() {
            return Err("Office previews are off, or this LibreOffice release is refused.".to_owned());
        }
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

        let work = self.inner.cache_dir.join("tmp").join(uuid_like(&summary.content_hash));
        tokio::fs::create_dir_all(&work)
            .await
            .map_err(|error| error.to_string())?;
        self.write_profile()
            .await
            .map_err(|error| format!("could not prepare LibreOffice's profile: {error}"))?;
        let input = work.join(format!("input.{extension}"));
        tokio::fs::write(&input, &bytes)
            .await
            .map_err(|error| error.to_string())?;

        let result = self.run(&soffice, &input, &work).await;
        let output = work.join("input.pdf");
        let finished = match result {
            Ok(()) => match tokio::fs::metadata(&output).await {
                Ok(meta) if meta.len() == 0 => Err("The converter produced an empty PDF.".to_owned()),
                Ok(meta) if meta.len() > MAX_PDF_BYTES => Err("The converted PDF is too large to preview.".to_owned()),
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
        let mut command = self.command(soffice, outdir);
        command
            .arg("--headless")
            .arg("--norestore")
            .arg("--nolockcheck")
            .arg("--nodefault")
            .arg("--nologo")
            .arg(format!("-env:UserInstallation={}", file_uri(&self.inner.profile_dir)))
            .arg(format!(
                "-env:UNO_JAVA_JFW_USER_DATA={}",
                file_uri(&self.inner.profile_dir.join(JAVA_SETTINGS_FILE))
            ))
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

/// The year, month and micro numbers of a `soffice --version` line such as
/// `LibreOffice 26.2.5.2 abc(Build:2)` (an older `7.6.7.2` reads as 7, 6, 7).
fn parse_release(line: &str) -> Option<(u32, u32, u32)> {
    let release = line
        .split_whitespace()
        .find(|word| word.chars().next().is_some_and(|first| first.is_ascii_digit()))?;
    let mut numbers = release.split('.').map(|part| part.parse::<u32>().ok());
    Some((numbers.next()??, numbers.next()??, numbers.next()??))
}

/// What is wrong with the release a `soffice --version` line reports, or
/// `None` when it is at or past the releases that fixed the advisories of
/// 2026-10-05: 26.2.5 on the 26.2 line, and 26.8.0 and every later line.
/// Anything older, and anything that cannot be read, is refused.
fn release_problem(line: &str) -> Option<String> {
    let patched = match parse_release(line) {
        Some((year, month, micro)) => year > 26 || (year == 26 && (month > 2 || (month == 2 && micro >= 5))),
        None => false,
    };
    (!patched).then(|| {
        format!(
            "{line} is older than the fixed releases (26.2.5 on the 26.2 line, 26.8.0 and later); update LibreOffice"
        )
    })
}

/// Variables LibreOffice has no use for and a hostile document must not reach:
/// the server's own settings and anything that looks like a credential.
fn is_secret_variable(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    upper.starts_with("REPOMEMO_")
        || upper == "RUST_LOG"
        || ["KEY", "SECRET", "TOKEN", "PASSWORD", "PASSWD", "CREDENTIAL"]
            .iter()
            .any(|word| upper.contains(word))
}

fn safe_hash(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|character| character.is_ascii_alphanumeric())
}

/// A workspace id is used as a folder name: letters, digits and hyphens only.
fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
}

/// Removes the remembered failures (`*.failed`) in a folder: a restart is a fresh chance.
fn remove_failures(folder: &Path) {
    if let Ok(entries) = std::fs::read_dir(folder) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|extension| extension == "failed") {
                let _ = std::fs::remove_file(path);
            }
        }
    }
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
        &[
            "/usr/bin/soffice",
            "/usr/lib/libreoffice/program/soffice",
            "/opt/libreoffice/program/soffice",
        ]
    };
    known.iter().map(PathBuf::from).find(|candidate| candidate.is_file())
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
    fn secrets_of_the_server_stay_out_of_libreoffice() {
        for secret in [
            "REPOMEMO_SECRET_KEY",
            "repomemo_setup_code",
            "AWS_SECRET_ACCESS_KEY",
            "GITHUB_TOKEN",
            "DB_PASSWORD",
            "RUST_LOG",
        ] {
            assert!(is_secret_variable(secret), "{secret}");
        }
        for harmless in ["PATH", "SystemRoot", "TEMP", "HOME", "LANG"] {
            assert!(!is_secret_variable(harmless), "{harmless}");
        }
    }

    #[test]
    fn the_wrapper_comes_before_the_binary() {
        let converter = Converter::with_options(
            &std::env::temp_dir().join("repomemo-wrapper-test"),
            None,
            vec![
                "sandbox".to_owned(),
                "--bind={work}".to_owned(),
                "--net=none".to_owned(),
            ],
            true,
        );
        assert!(converter.sandboxed());
        let command = converter.command(Path::new("/usr/bin/soffice"), Path::new("/work/dir"));
        let std_command = command.as_std();
        assert_eq!(std_command.get_program().to_string_lossy(), "sandbox");
        let arguments = std_command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(arguments, vec!["--bind=/work/dir", "--net=none", "/usr/bin/soffice"]);
    }

    #[test]
    fn only_the_fixed_libreoffice_releases_are_accepted() {
        for fixed in [
            "LibreOffice 26.2.5.2 abc(Build:2)",
            "LibreOffice 26.2.6.1",
            "LibreOffice 26.8.0.3 def(Build:3)",
            "LibreOffice 26.8.2.1",
            "LibreOffice 27.2.0.1",
        ] {
            assert_eq!(release_problem(fixed), None, "{fixed}");
        }
        for old in [
            "LibreOffice 26.2.4.2 abc(Build:2)",
            "LibreOffice 26.2.0.1",
            "LibreOffice 25.8.4.2",
            "LibreOffice 25.2.7.2",
            "LibreOffice 24.8.7.2 480(Build:2)",
            "LibreOffice 7.6.7.2 60(Build:2)",
            "LibreOffice",
            "",
        ] {
            let problem = release_problem(old).unwrap_or_else(|| panic!("{old} was accepted"));
            assert!(problem.contains("26.2.5") && problem.contains("26.8.0"), "{problem}");
            assert!(problem.starts_with(old), "the release is named: {problem}");
        }
    }

    #[test]
    fn nothing_is_converted_while_previews_are_off() {
        let directory = std::env::temp_dir().join("repomemo-previews-off-test");
        let itself = std::env::current_exe().unwrap();
        let off = Converter::with_options(&directory, Some(itself.clone()), Vec::new(), false);
        assert!(off.installed() && !off.requested());
        assert_eq!(off.status(), ConverterStatus::Off);
        assert!(!off.enabled());
        // Turned on, a converter waits for its version check before it converts.
        let on = Converter::with_options(&directory, Some(itself), Vec::new(), true);
        assert_eq!(on.status(), ConverterStatus::Checking);
        assert!(!on.enabled());
        let missing = Converter::with_options(&directory, None, Vec::new(), true);
        assert_eq!(missing.status(), ConverterStatus::NotInstalled);
        assert!(!missing.enabled());
    }

    #[tokio::test]
    async fn the_profile_is_written_before_each_conversion_and_locks_libreoffice_down() {
        let data = std::env::temp_dir().join(format!("repomemo-profile-test-{}", std::process::id()));
        let converter = Converter::with_options(&data, None, Vec::new(), true);
        // A document (or an earlier run) left the macro level low and Java on.
        let user = data.join("previews").join("profile").join("user");
        std::fs::create_dir_all(&user).unwrap();
        std::fs::write(
            user.join("registrymodifications.xcu"),
            "<item>MacroSecurityLevel 0</item>",
        )
        .unwrap();
        converter.write_profile().await.unwrap();

        let registry = std::fs::read_to_string(user.join("registrymodifications.xcu")).unwrap();
        assert!(!registry.contains("MacroSecurityLevel 0"), "the old file is replaced");
        for (path, property, value) in PROFILE_SETTINGS {
            let item = format!("<item oor:path=\"{path}\"><prop oor:name=\"{property}\" oor:op=\"fuse\"><value>{value}</value></prop></item>");
            assert!(registry.contains(&item), "{property} is written: {item}");
        }
        // The headline items: macros off at the highest level, no active content, links never updated.
        for needle in [
            "oor:name=\"MacroSecurityLevel\" oor:op=\"fuse\"><value>3</value>",
            "oor:name=\"DisableMacrosExecution\" oor:op=\"fuse\"><value>true</value>",
            "oor:name=\"DisableActiveContent\" oor:op=\"fuse\"><value>true</value>",
            "oor:path=\"/org.openoffice.Office.Writer/Content/Update\"><prop oor:name=\"Link\" oor:op=\"fuse\"><value>2</value>",
            "oor:path=\"/org.openoffice.Office.Calc/Content/Update\"><prop oor:name=\"Link\" oor:op=\"fuse\"><value>1</value>",
            "AutoCheckEnabled\" oor:op=\"fuse\"><value>false</value>",
        ] {
            assert!(registry.contains(needle), "{needle}");
        }
        let java = std::fs::read_to_string(data.join("previews").join("profile").join(JAVA_SETTINGS_FILE)).unwrap();
        assert!(java.contains("<enabled xsi:nil=\"false\">false</enabled>"), "{java}");
        let _ = std::fs::remove_dir_all(&data);
    }

    #[test]
    fn a_preview_another_workspace_made_is_not_ready_here_until_this_workspace_asked() {
        let data = std::env::temp_dir().join(format!("repomemo-scope-test-{}", std::process::id()));
        let converter = Converter::with_options(&data, None, Vec::new(), true);
        let hash = "abc123def456";
        // Workspace A converted the file: the PDF is on disk and A has been told so.
        std::fs::write(converter.pdf_path(hash), b"%PDF-1.4").unwrap();
        std::fs::create_dir_all(converter.ready_path("workspace-a", hash).parent().unwrap()).unwrap();
        std::fs::write(converter.ready_path("workspace-a", hash), b"").unwrap();
        assert!(converter.cached_pdf("workspace-a", hash).is_some());
        // Workspace B holds the same bytes but has not asked: nothing is ready, so nothing is served.
        assert!(
            converter.cached_pdf("workspace-b", hash).is_none(),
            "B must not learn that A has the file"
        );
        // Once B's own request has run, it is ready there too.
        std::fs::create_dir_all(converter.ready_path("workspace-b", hash).parent().unwrap()).unwrap();
        std::fs::write(converter.ready_path("workspace-b", hash), b"").unwrap();
        assert!(converter.cached_pdf("workspace-b", hash).is_some());
        // An id that is not a plain folder name is never looked up.
        assert!(converter.cached_pdf("../workspace-a", hash).is_none());
        // Forgetting a file whose blob is gone clears it for every workspace.
        assert!(converter.forget(hash));
        assert!(converter.cached_pdf("workspace-a", hash).is_none());
        assert!(!converter.ready_path("workspace-b", hash).exists());
        let _ = std::fs::remove_dir_all(&data);
    }

    #[test]
    fn workspace_ids_are_safe_folder_names() {
        assert!(safe_id("3f2a9c1e-77aa-4b0d-9d1e-0c2b4a6f8e10"));
        assert!(!safe_id(""));
        assert!(!safe_id("../other"));
        assert!(!safe_id("a/b"));
    }

    #[test]
    fn hashes_must_be_plain_alphanumerics() {
        assert!(safe_hash("abc123"));
        assert!(!safe_hash("../etc"));
        assert!(!safe_hash(""));
    }
}
