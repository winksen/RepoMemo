//! Limits that keep one person or workspace from filling the disk: storage
//! per workspace, uploads per hour, workspaces per person, and a floor of
//! free disk space (roadmap O4). All are run-time settings; 0 turns one off.

use std::path::Path;

use crate::*;

/// Free bytes on the disk that holds `path`, when the platform tells.
pub(crate) fn free_disk_bytes(path: &Path) -> Option<u64> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        extern "system" {
            fn GetDiskFreeSpaceExW(
                directory: *const u16,
                available_to_caller: *mut u64,
                total: *mut u64,
                total_free: *mut u64,
            ) -> i32;
        }
        let wide = path.as_os_str().encode_wide().chain(Some(0)).collect::<Vec<u16>>();
        let (mut available, mut total, mut free) = (0_u64, 0_u64, 0_u64);
        // SAFETY: `wide` is a NUL-terminated UTF-16 path that outlives the
        // call, and the three pointers are to live, writable u64 values.
        let succeeded = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut available, &mut total, &mut free) };
        (succeeded != 0).then_some(available)
    }
    #[cfg(unix)]
    {
        let output = std::process::Command::new("df").arg("-Pk").arg(path).output().ok()?;
        let text = String::from_utf8(output.stdout).ok()?;
        let kilobytes = text.lines().nth(1)?.split_whitespace().nth(3)?.parse::<u64>().ok()?;
        Some(kilobytes.saturating_mul(1024))
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = path;
        None
    }
}

const MIB: u64 = 1024 * 1024;

/// Refuses new content when the disk is nearly full, the workspace is over
/// its storage allowance, or (for an upload) the person has uploaded too much
/// this hour. `incoming` is the size of what is about to be stored.
pub(crate) async fn check_incoming(
    state: &AppState,
    subject: &AuthenticatedSubject,
    workspace_id: &str,
    incoming: u64,
    is_upload: bool,
) -> Result<(), ApiError> {
    let settings = state.settings();
    if settings.disk_floor_mb > 0 {
        let free = free_disk_bytes(Path::new(&state.instance.data_dir));
        if free.is_some_and(|free| free < settings.disk_floor_mb * MIB + incoming) {
            tracing::warn!(target: "audit", free_bytes = ?free, "An upload was refused: the disk is nearly full");
            return Err(ApiError::new(
                StatusCode::INSUFFICIENT_STORAGE,
                "disk_low",
                "The server is nearly out of disk space, so nothing can be stored right now. Tell an administrator.",
            ));
        }
    }
    if settings.workspace_storage_mb > 0 {
        let used = state
            .storage
            .workspace_stored_bytes(workspace_id)
            .await
            .map_err(map_storage_error)?
            .max(0) as u64;
        let allowance = settings.workspace_storage_mb * MIB;
        if used + incoming > allowance {
            return Err(ApiError::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "workspace_full",
                format!(
                    "This workspace holds {} of its {} allowance. Delete evidence or ask an administrator for more room.",
                    format_bytes(used),
                    format_bytes(allowance)
                ),
            ));
        }
    }
    if is_upload {
        state
            .guards
            .uploads
            .check(&subject.user_id, settings.upload_quota)
            .map_err(|wait| {
                too_many_requests(
                    format!(
                        "You have reached the limit of {} uploads per hour on this server. Try again in {}.",
                        settings.upload_quota.limit,
                        wait_text(wait)
                    ),
                    wait,
                )
            })?;
    }
    Ok(())
}

/// Refuses a new workspace when the person already owns the allowed number.
pub(crate) async fn check_new_workspace(state: &AppState, subject: &AuthenticatedSubject) -> Result<(), ApiError> {
    let limit = state.settings().workspaces_per_person;
    if limit == 0 {
        return Ok(());
    }
    let owned = state
        .storage
        .count_owned_workspaces(&subject.user_id)
        .await
        .map_err(map_storage_error)?;
    if owned >= i64::from(limit) {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "workspace_limit",
            format!("You already own {owned} workspaces, which is the limit on this server."),
        ));
    }
    Ok(())
}

/// "1.5 GiB", "300 MiB"
pub(crate) fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["bytes", "KiB", "MiB", "GiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} bytes")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_written_for_people() {
        assert_eq!(format_bytes(0), "0 bytes");
        assert_eq!(format_bytes(1_536), "1.5 KiB");
        assert_eq!(format_bytes(300 * MIB), "300.0 MiB");
        assert_eq!(format_bytes(5 * 1024 * MIB), "5.0 GiB");
    }

    #[test]
    fn the_data_disk_reports_free_space() {
        let free = free_disk_bytes(&std::env::temp_dir());
        assert!(free.is_some_and(|free| free > 0), "free space was {free:?}");
    }
}
