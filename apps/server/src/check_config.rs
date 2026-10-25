//! `repomemo-server --check-config`: reads the environment the way a start would, reports what it
//! would use (secrets hidden) and the state of the data folder, and exits without serving.

use crate::config::env_text;
use crate::*;
use std::path::Path;

/// Every secret the server reads, with the variable that holds it.
const SECRETS: [&str; 5] = [
    "REPOMEMO_JWT_SECRET",
    "REPOMEMO_SECRET_KEY",
    "REPOMEMO_BACKUP_KEY",
    "REPOMEMO_BACKUP_KEY_PREVIOUS",
    "REPOMEMO_SETUP_CODE",
];

/// How errors about the folders begin; they are exit code 2.
const FOLDERS: [&str; 4] = ["data folder", "database folder", "files folder", "backups folder"];

/// What the check found: lines to print, and what is wrong (errors) or doubtful (warnings).
#[derive(Debug, Default)]
pub struct CheckReport {
    pub lines: Vec<String>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

impl CheckReport {
    /// 0 when the configuration can start, 1 for an invalid configuration, 2 for a data folder
    /// the server could not use.
    pub fn exit_code(&self) -> i32 {
        match (
            self.errors.is_empty(),
            self.errors
                .iter()
                .any(|error| FOLDERS.iter().any(|folder| error.starts_with(folder))),
        ) {
            (true, _) => 0,
            (false, true) => 2,
            (false, false) => 1,
        }
    }

    pub fn render(&self) -> String {
        let mut text = format!("RepoMemo {} configuration check\n", env!("CARGO_PKG_VERSION"));
        for line in &self.lines {
            text.push_str(&format!("  {line}\n"));
        }
        for warning in &self.warnings {
            text.push_str(&format!("warning: {warning}\n"));
        }
        for error in &self.errors {
            text.push_str(&format!("error: {error}\n"));
        }
        text.push_str(if self.errors.is_empty() {
            "The configuration can start.\n"
        } else {
            "The configuration cannot start.\n"
        });
        text
    }
}

/// Reads the environment as a start would and checks the data folder; prints the report and
/// returns the exit code.
pub fn check_config() -> i32 {
    let report = check_config_report(ServerConfig::from_env());
    let text = report.render();
    if report.errors.is_empty() {
        print!("{text}");
    } else {
        eprint!("{text}");
    }
    report.exit_code()
}

pub(crate) fn check_config_report(config: Result<ServerConfig>) -> CheckReport {
    let mut report = CheckReport::default();
    let config = match config {
        Ok(config) => config,
        Err(error) => {
            report.errors.push(format!("{error:#}"));
            return report;
        }
    };
    report.lines.push(format!("listens on        {}", config.bind_address));
    report.lines.push(format!(
        "public address    {}",
        config.public_url.as_deref().unwrap_or("not set")
    ));
    report.lines.push(format!(
        "allowed origins   {}",
        config
            .allowed_origins
            .iter()
            .filter_map(|origin| origin.to_str().ok())
            .collect::<Vec<_>>()
            .join(", ")
    ));
    report.lines.push(format!(
        "proxy trusted     {}",
        if config.trust_proxy { "yes" } else { "no" }
    ));
    for name in SECRETS {
        report.lines.push(secret_line(name, &mut report.warnings));
    }
    folders(&config, &mut report);
    for (label, folder) in [
        ("backup copy      ", config.backup_copy_dir.as_deref()),
        ("templates folder ", config.templates_dir.as_deref()),
    ] {
        if let Some(folder) = folder {
            report.lines.push(format!("{label} {}", folder.display()));
            if !folder.is_dir() {
                report
                    .warnings
                    .push(format!("{} is not a folder that exists", folder.display()));
            }
        }
    }
    report.lines.push(format!(
        "backups           every {} hours, keeping {}{}",
        config.backup_interval_hours,
        config.backup_keep,
        if config.backup_key.is_some() { ", sealed" } else { "" }
    ));
    report.lines.push(format!(
        "sign-up           {}",
        if config.allow_registration {
            "open"
        } else {
            "by invitation"
        }
    ));
    report
}

/// One line about a secret: where it comes from and how long it is, never the secret itself.
fn secret_line(name: &str, warnings: &mut Vec<String>) -> String {
    let file_variable = format!("{name}_FILE");
    let label = format!(
        "{:<17}",
        name.trim_start_matches("REPOMEMO_")
            .to_ascii_lowercase()
            .replace('_', " ")
    );
    if let Some(path) = env_text(&file_variable) {
        if let Some(problem) = readable_by_others(Path::new(&path)) {
            warnings.push(format!("{file_variable} ({path}) {problem}"));
        }
        return format!("{label} from the file {path}");
    }
    match env_text(name) {
        Some(value) => format!("{label} set in the environment ({} characters)", value.chars().count()),
        None => format!("{label} not set"),
    }
}

/// On Unix, a secret file that other accounts can read.
#[cfg(unix)]
fn readable_by_others(path: &Path) -> Option<&'static str> {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(path).ok()?.permissions().mode();
    (mode & 0o077 != 0).then_some("can be read by other accounts; restrict it to the server's account (chmod 600)")
}

#[cfg(not(unix))]
fn readable_by_others(_path: &Path) -> Option<&'static str> {
    None
}

/// A volume with less free space than this cannot take a database: the start is refused (V0.4.5).
const FULL_BELOW_BYTES: u64 = 16 * 1024 * 1024;

/// The check at start (V0.4.5): the folders the server will write to, so a read-only, full or
/// networked volume is named in the first log line and not by a failure later on.
pub fn startup_report(config: &ServerConfig) -> CheckReport {
    let mut report = CheckReport::default();
    folders(config, &mut report);
    report
}

/// The data folder and the folders split from it.
fn folders(config: &ServerConfig, report: &mut CheckReport) {
    let Some(dir) = config.data_dir.as_deref() else {
        report
            .lines
            .push("data folder       not set: the server starts detached and an administrator picks one".to_owned());
        return;
    };
    let floor = config.disk_floor_mb.saturating_mul(1024 * 1024);
    data_folder(dir, "data folder      ", floor, report);
    for (label, folder) in [
        ("database folder  ", config.db_dir.as_deref()),
        ("files folder     ", config.blob_dir.as_deref()),
        ("backups folder   ", config.backup_dir.as_deref()),
    ] {
        if let Some(folder) = folder {
            data_folder(folder, label, floor, report);
        }
    }
}

/// A folder: present or creatable, writable (a small file is made and removed to be sure), with room
/// left, and not on a network file system, which SQLite's write-ahead log cannot work on.
fn data_folder(dir: &Path, label: &str, floor: u64, report: &mut CheckReport) {
    let name = label.trim();
    if dir.is_dir() {
        let probe = dir.join(format!(".repomemo-check-{}", std::process::id()));
        match std::fs::write(&probe, b"") {
            Ok(()) => {
                let _ = std::fs::remove_file(&probe);
                report
                    .lines
                    .push(format!("{label} {} (exists, writable)", dir.display()));
            }
            Err(error) => {
                report.errors.push(format!(
                    "{name} {} is not writable: {error}.{} Give the server's account write access to it, or mount the volume read-write.",
                    dir.display(),
                    owner_hint(dir)
                ));
                return;
            }
        }
        match crate::quotas::free_disk_bytes(dir) {
            Some(free) if free < FULL_BELOW_BYTES => report.errors.push(format!(
                "{name} {} is full: {} MiB free. Free some room or mount a larger volume.",
                dir.display(),
                free / (1024 * 1024)
            )),
            Some(free) if free < floor => report.warnings.push(format!(
                "{name} {} has {} MiB free, under the floor of {} MiB where uploads are refused (REPOMEMO_DISK_FLOOR_MB).",
                dir.display(),
                free / (1024 * 1024),
                floor / (1024 * 1024)
            )),
            _ => {}
        }
        if let Some(kind) = network_file_system(&mount_table(), dir) {
            report.warnings.push(format!(
                "{name} {} is on a network file system ({kind}). SQLite's write-ahead log needs shared memory and locks such file systems do not give, so a database kept there can be corrupted; use a local volume for the database.",
                dir.display()
            ));
        }
    } else if dir.exists() {
        report
            .errors
            .push(format!("{name} {} exists but is not a folder", dir.display()));
    } else {
        match dir.ancestors().skip(1).find(|parent| parent.is_dir()) {
            Some(parent) if parent.metadata().is_ok_and(|meta| !meta.permissions().readonly()) => {
                report.lines.push(format!(
                    "{label} {} (will be created inside {})",
                    dir.display(),
                    parent.display()
                ));
            }
            _ => report.errors.push(format!(
                "{name} {} does not exist and cannot be created there",
                dir.display()
            )),
        }
    }
}

/// On Unix, who owns the folder and who the server runs as, when they differ (the usual cause of a
/// volume that is not writable: it belongs to root and the server is not).
#[cfg(unix)]
fn owner_hint(dir: &Path) -> String {
    use std::os::unix::fs::MetadataExt;
    let owner = std::fs::metadata(dir).map(|meta| meta.uid());
    // The owner of /proc/self is the account the process runs as.
    let me = std::fs::metadata("/proc/self").map(|meta| meta.uid());
    match (owner, me) {
        (Ok(owner), Ok(me)) if owner != me => {
            format!(" The folder belongs to user {owner} and the server runs as user {me}.")
        }
        _ => String::new(),
    }
}

#[cfg(not(unix))]
fn owner_hint(_dir: &Path) -> String {
    String::new()
}

/// The mounted file systems, on Linux; empty elsewhere.
fn mount_table() -> String {
    std::fs::read_to_string("/proc/self/mountinfo").unwrap_or_default()
}

/// Names the network file system `dir` is on, from the text of `/proc/self/mountinfo` (the longest mount
/// point that holds it decides), or from the shape of a Windows path.
pub(crate) fn network_file_system(mountinfo: &str, dir: &Path) -> Option<String> {
    const NETWORK: [&str; 10] = [
        "nfs",
        "nfs4",
        "cifs",
        "smb3",
        "smbfs",
        "9p",
        "afs",
        "ncpfs",
        "fuse.sshfs",
        "glusterfs",
    ];
    let text = dir.to_string_lossy();
    if text.starts_with(r"\\?\UNC\") || (text.starts_with(r"\\") && !text.starts_with(r"\\?\")) {
        return Some("a network share".to_owned());
    }
    let resolved = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    mountinfo
        .lines()
        .filter_map(|line| {
            let (before, after) = line.split_once(" - ")?;
            let mount_point = before.split_whitespace().nth(4)?;
            let kind = after.split_whitespace().next()?;
            resolved
                .starts_with(mount_point)
                .then(|| (mount_point.len(), kind.to_owned()))
        })
        .max_by_key(|(length, _)| *length)
        .map(|(_, kind)| kind)
        .filter(|kind| NETWORK.contains(&kind.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("repomemo-check-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_valid_configuration_is_described_without_its_secrets() {
        let dir = temp_dir();
        let config = ServerConfig::for_test(dir.clone());
        let secret = config.jwt_secret.clone();
        let report = check_config_report(Ok(config));
        assert!(report.errors.is_empty());
        assert_eq!(report.exit_code(), 0);
        let text = report.render();
        assert!(text.contains("(exists, writable)"));
        assert!(text.contains("The configuration can start."));
        assert!(!text.contains(&secret));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_invalid_configuration_is_an_error_with_exit_code_one() {
        let report = check_config_report(Err(anyhow::anyhow!(
            "REPOMEMO_JWT_SECRET must contain at least 32 characters"
        )));
        assert_eq!(report.exit_code(), 1);
        assert!(report.render().contains("The configuration cannot start."));
    }

    #[test]
    fn a_data_folder_that_is_a_file_is_exit_code_two() {
        let dir = temp_dir();
        let file = dir.join("not-a-folder");
        std::fs::write(&file, b"x").unwrap();
        let report = check_config_report(Ok(ServerConfig::for_test(file)));
        assert_eq!(report.exit_code(), 2);
        assert!(report.errors[0].contains("is not a folder"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_missing_data_folder_inside_a_writable_one_will_be_created() {
        let dir = temp_dir();
        let report = check_config_report(Ok(ServerConfig::for_test(dir.join("later"))));
        assert!(report.errors.is_empty());
        assert!(report.render().contains("will be created inside"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[cfg(test)]
mod folder_tests {
    use super::*;

    const MOUNTS: &str = "\
24 1 8:1 / / rw,relatime shared:1 - ext4 /dev/sda1 rw
31 24 0:27 / /srv rw,relatime shared:2 - nfs4 server:/export rw,vers=4.2
32 31 0:28 / /srv/local rw,relatime shared:3 - ext4 /dev/sdb1 rw
40 24 0:30 / /mnt/share rw,relatime shared:4 - cifs //nas/share rw
";

    #[test]
    fn a_network_file_system_is_named_by_its_longest_mount_point() {
        assert_eq!(
            network_file_system(MOUNTS, Path::new("/srv/data")).as_deref(),
            Some("nfs4")
        );
        assert_eq!(
            network_file_system(MOUNTS, Path::new("/mnt/share/memo")).as_deref(),
            Some("cifs")
        );
        assert_eq!(network_file_system(MOUNTS, Path::new("/srv/local/data")), None);
        assert_eq!(network_file_system(MOUNTS, Path::new("/var/lib/repomemo")), None);
        assert_eq!(network_file_system("", Path::new("/anything")), None);
    }

    #[test]
    fn a_windows_network_path_is_a_share() {
        assert!(network_file_system("", Path::new(r"\nas\memo")).is_some());
        assert!(network_file_system("", Path::new(r"\?\C:\memo")).is_none());
    }

    #[test]
    fn a_folder_under_the_disk_floor_is_warned_about() {
        let dir = std::env::temp_dir().join(format!("repomemo-room-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        if crate::quotas::free_disk_bytes(&dir).is_some() {
            let mut warned = CheckReport::default();
            data_folder(&dir, "data folder      ", u64::MAX, &mut warned);
            assert!(warned.errors.is_empty());
            assert!(warned
                .warnings
                .iter()
                .any(|warning| warning.contains("under the floor")));
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}
