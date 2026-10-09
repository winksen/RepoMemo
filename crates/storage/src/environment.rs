//! Verification of an environment folder before a server attaches to it.
//!
//! An environment is a folder holding one RepoMemo database (`repomemo.sqlite`),
//! its blobs and logs. Attaching the server to some other folder, or to a
//! damaged or newer database, must fail with a clear reason instead of
//! creating a second database next to someone's files or migrating it blindly.

use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

use super::{DATABASE_FILE, MIGRATOR};

/// Names tolerated next to nothing else in a folder that still counts as empty.
const IGNORED_WHEN_EMPTY: &[&str] = &[".gitkeep", ".gitignore", ".DS_Store", "Thumbs.db"];
/// Tables every RepoMemo database has after its migrations.
const REQUIRED_TABLES: &[&str] = &["_sqlx_migrations", "users", "workspaces", "artifacts", "blobs"];
const SQLITE_HEADER: &[u8; 16] = b"SQLite format 3\0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvironmentState {
    /// The folder is missing or has nothing in it: a new environment can be
    /// started there.
    Empty,
    /// A RepoMemo environment this version can open.
    Valid,
    /// Something else, damaged, or from a newer version; the reason is shown
    /// to the administrator.
    Invalid(String),
}

/// Looks into `dir` without changing anything.
pub async fn inspect_environment(dir: &Path) -> EnvironmentState {
    match inspect(dir).await {
        Ok(state) => state,
        Err(reason) => EnvironmentState::Invalid(reason),
    }
}

async fn inspect(dir: &Path) -> Result<EnvironmentState, String> {
    let metadata = match tokio::fs::metadata(dir).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(EnvironmentState::Empty),
        Err(error) => return Err(format!("The folder cannot be read: {error}.")),
    };
    if !metadata.is_dir() {
        return Err("This is a file, not a folder.".to_owned());
    }

    let mut entries = tokio::fs::read_dir(dir)
        .await
        .map_err(|error| format!("The folder cannot be read: {error}."))?;
    let mut empty = true;
    while let Some(entry) = entries.next_entry().await.map_err(|error| error.to_string())? {
        if !IGNORED_WHEN_EMPTY.iter().any(|ignored| entry.file_name() == *ignored) {
            empty = false;
            break;
        }
    }
    if empty {
        return Ok(EnvironmentState::Empty);
    }

    let database = dir.join(DATABASE_FILE);
    let header = match tokio::fs::read(&database).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return Err(format!(
                "{DATABASE_FILE} is missing: this folder has other content and is not a RepoMemo environment."
            ))
        }
    };
    if header.len() < SQLITE_HEADER.len() || &header[..SQLITE_HEADER.len()] != SQLITE_HEADER {
        return Err(format!("{DATABASE_FILE} is not a SQLite database."));
    }
    if let Ok(blobs) = tokio::fs::metadata(dir.join("blobs")).await {
        if !blobs.is_dir() {
            return Err("blobs exists but is not a folder.".to_owned());
        }
    }

    let options = SqliteConnectOptions::new().filename(&database).read_only(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(|error| format!("{DATABASE_FILE} cannot be opened: {error}."))?;
    let verdict = check_schema(&pool).await;
    pool.close().await;
    verdict.map(|()| EnvironmentState::Valid)
}

async fn check_schema(pool: &sqlx::SqlitePool) -> Result<(), String> {
    let tables: Vec<String> = sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type = 'table'")
        .fetch_all(pool)
        .await
        .map_err(|error| format!("{DATABASE_FILE} is not readable: {error}."))?;
    if let Some(missing) = REQUIRED_TABLES.iter().find(|table| !tables.iter().any(|name| name == *table)) {
        return Err(format!(
            "{DATABASE_FILE} does not have the RepoMemo structure (table `{missing}` is missing)."
        ));
    }
    let applied: Vec<(i64, bool)> = sqlx::query_as("SELECT version, success FROM _sqlx_migrations")
        .fetch_all(pool)
        .await
        .map_err(|error| format!("The migration history cannot be read: {error}."))?;
    if applied.iter().any(|(_, success)| !success) {
        return Err("A database migration failed here earlier; the database needs repair.".to_owned());
    }
    let newest_known = MIGRATOR.iter().map(|migration| migration.version).max().unwrap_or(0);
    if applied.iter().any(|(version, _)| *version > newest_known) {
        return Err("This environment was written by a newer version of RepoMemo.".to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("repomemo-env-{label}-{}", uuid::Uuid::new_v4()))
    }

    #[tokio::test]
    async fn missing_and_empty_folders_are_new_environments() {
        let dir = temp("empty");
        assert_eq!(inspect_environment(&dir).await, EnvironmentState::Empty);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(".gitkeep"), b"").unwrap();
        assert_eq!(inspect_environment(&dir).await, EnvironmentState::Empty);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn foreign_content_is_refused_and_an_opened_environment_is_valid() {
        let dir = temp("foreign");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("notes.txt"), b"hello").unwrap();
        assert!(matches!(inspect_environment(&dir).await, EnvironmentState::Invalid(_)));
        std::fs::write(dir.join(DATABASE_FILE), b"not a database at all").unwrap();
        assert!(matches!(inspect_environment(&dir).await, EnvironmentState::Invalid(_)));
        let _ = std::fs::remove_dir_all(&dir);

        let real = temp("real");
        let engine = super::super::StorageEngine::open(super::super::StorageConfig { data_dir: real.clone(), master_key: None })
            .await
            .unwrap();
        engine.close().await;
        assert_eq!(inspect_environment(&real).await, EnvironmentState::Valid);
        let _ = std::fs::remove_dir_all(real);
    }
}
