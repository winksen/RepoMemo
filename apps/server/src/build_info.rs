//! What was built: the version, and the commit, date and image tag a build was given
//! (build arguments of the Dockerfile, read at compile time). `/health` and `--version` print it,
//! so the exact image that answers can be named.

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct BuildInfo {
    pub version: &'static str,
    pub commit: &'static str,
    pub built_at: &'static str,
    pub image: &'static str,
}

pub const BUILD: BuildInfo = BuildInfo {
    version: env!("CARGO_PKG_VERSION"),
    commit: unknown_if_unset(option_env!("REPOMEMO_BUILD_COMMIT")),
    built_at: unknown_if_unset(option_env!("REPOMEMO_BUILD_DATE")),
    image: unknown_if_unset(option_env!("REPOMEMO_IMAGE_TAG")),
};

const fn unknown_if_unset(value: Option<&'static str>) -> &'static str {
    match value {
        Some(value) if !value.is_empty() => value,
        _ => "unknown",
    }
}

impl BuildInfo {
    /// The line `--version` prints.
    pub fn line(&self) -> String {
        format!(
            "repomemo-server {} (commit {}, built {}, image {})",
            self.version, self.commit, self.built_at, self.image
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unset_or_empty_build_argument_reads_unknown() {
        assert_eq!(unknown_if_unset(None), "unknown");
        assert_eq!(unknown_if_unset(Some("")), "unknown");
        assert_eq!(unknown_if_unset(Some("abc1234")), "abc1234");
    }

    #[test]
    fn the_version_line_names_the_package_version() {
        let line = BUILD.line();
        assert!(line.starts_with(&format!("repomemo-server {}", env!("CARGO_PKG_VERSION"))));
        assert!(line.contains("commit") && line.contains("image"));
    }
}
