//! A database written by a newer RepoMemo is refused, not opened (V0.4.36). An older program that opened it would run against
//! tables it does not know, and could damage them. Two signs are read before anything is changed: a marker file beside the
//! database that every start leaves (the release that last wrote it), and the migrations the database has applied.

use std::path::Path;

use anyhow::{bail, Result};
use sqlx::SqlitePool;

use super::MIGRATOR;

/// The file beside the database that names the release that last opened it.
pub(crate) const MARKER_FILE: &str = "repomemo.version";

/// This program's release.
pub(crate) const THIS_VERSION: &str = env!("CARGO_PKG_VERSION");

fn parse(version: &str) -> Option<(u64, u64, u64)> {
    let mut parts = version.trim().split(['.', '-', '+']);
    Some((
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    ))
}

/// Refuses when the marker names a release newer than `current`, or the database has a migration beyond `known`.
/// Changes nothing.
pub(crate) fn check(marker: Option<&str>, applied: Option<i64>, known: Option<i64>, current: &str) -> Result<()> {
    if let (Some(written), Some(now)) = (
        marker.and_then(|text| parse(text).map(|parsed| (text.trim(), parsed))),
        parse(current),
    ) {
        if written.1 > now {
            bail!(
                "this database was written by RepoMemo {}, which is newer than this program ({current}). Nothing was changed. Start {} or later, or restore a backup made before the upgrade (the upgrade guide, docs/DIRECT_UPGRADE.md).",
                written.0,
                written.0
            );
        }
    }
    if let (Some(applied), Some(known)) = (applied, known) {
        if applied > known {
            bail!(
                "this database has migration {applied}, which this program ({current}, which knows up to {known}) does not; it was written by a newer RepoMemo. Nothing was changed. Start the newer release, or restore a backup made before the upgrade."
            );
        }
    }
    Ok(())
}

/// Reads the two signs and refuses if they say the database is from the future.
pub(crate) async fn refuse_newer_database(pool: &SqlitePool, db_dir: &Path) -> Result<()> {
    let marker = std::fs::read_to_string(db_dir.join(MARKER_FILE)).ok();
    let has_table: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'")
            .fetch_optional(pool)
            .await?;
    let applied: Option<i64> = if has_table.is_some() {
        sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations")
            .fetch_one(pool)
            .await?
    } else {
        None
    };
    check(
        marker.as_deref(),
        applied,
        MIGRATOR.iter().map(|migration| migration.version).max(),
        THIS_VERSION,
    )
}

/// Leaves the marker: this release has opened the database. It never goes backwards.
pub(crate) fn leave_marker(db_dir: &Path) {
    let current = parse(THIS_VERSION);
    let written = std::fs::read_to_string(db_dir.join(MARKER_FILE))
        .ok()
        .and_then(|text| parse(&text));
    if written.is_none_or(|written| Some(written) < current) {
        let _ = std::fs::write(db_dir.join(MARKER_FILE), format!("{THIS_VERSION}\n"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_newer_marker_is_refused_naming_both_versions() {
        let error = check(Some("0.5.1\n"), None, None, "0.4.36").unwrap_err().to_string();
        assert!(
            error.contains("RepoMemo 0.5.1") && error.contains("(0.4.36)") && error.contains("Nothing was changed"),
            "{error}"
        );
        assert!(
            check(Some("0.4.37"), None, None, "0.4.36").is_err(),
            "a newer patch counts"
        );
    }

    #[test]
    fn the_same_or_an_older_marker_and_no_marker_are_fine() {
        for marker in [
            Some("0.4.36"),
            Some("0.4.35"),
            Some("0.3.27\n"),
            Some("not a version"),
            None,
        ] {
            assert!(check(marker, None, None, "0.4.36").is_ok(), "{marker:?}");
        }
    }

    #[test]
    fn a_migration_this_program_does_not_know_is_refused() {
        let error = check(None, Some(45), Some(43), "0.4.36").unwrap_err().to_string();
        assert!(
            error.contains("migration 45") && error.contains("knows up to 43") && error.contains("0.4.36"),
            "{error}"
        );
        assert!(check(None, Some(43), Some(43), "0.4.36").is_ok());
        assert!(check(None, None, Some(43), "0.4.36").is_ok(), "a new database has none");
    }

    #[test]
    fn the_marker_never_goes_backwards() {
        let dir = std::env::temp_dir().join(format!("repomemo-marker-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        leave_marker(&dir);
        assert_eq!(
            std::fs::read_to_string(dir.join(MARKER_FILE)).unwrap().trim(),
            THIS_VERSION
        );
        std::fs::write(dir.join(MARKER_FILE), "99.0.0\n").unwrap();
        leave_marker(&dir);
        assert_eq!(std::fs::read_to_string(dir.join(MARKER_FILE)).unwrap().trim(), "99.0.0");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
