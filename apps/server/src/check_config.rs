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
    match config.data_dir.as_deref() {
        Some(dir) => {
            data_folder(dir, "data folder      ", &mut report);
            for (label, folder) in [
                ("database folder  ", config.db_dir.as_deref()),
                ("files folder     ", config.blob_dir.as_deref()),
                ("backups folder   ", config.backup_dir.as_deref()),
            ] {
                if let Some(folder) = folder {
                    data_folder(folder, label, &mut report);
                }
            }
        }
        None => report
            .lines
            .push("data folder       not set: the server starts detached and an administrator picks one".to_owned()),
    }
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

/// The data folder: present or creatable, and writable. A small file is made and removed to be sure.
fn data_folder(dir: &Path, label: &str, report: &mut CheckReport) {
    if dir.is_dir() {
        let probe = dir.join(format!(".repomemo-check-{}", std::process::id()));
        match std::fs::write(&probe, b"") {
            Ok(()) => {
                let _ = std::fs::remove_file(&probe);
                report.lines.push(format!("{label} {} (exists, writable)", dir.display()));
            }
            Err(error) => report.errors.push(format!(
                "{} {} is not writable: {error}. Give the server's account write access to it, or mount the volume read-write.",
                label.trim(),
                dir.display()
            )),
        }
    } else if dir.exists() {
        report
            .errors
            .push(format!("{} {} exists but is not a folder", label.trim(), dir.display()));
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
                "{} {} does not exist and cannot be created there",
                label.trim(),
                dir.display()
            )),
        }
    }
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
