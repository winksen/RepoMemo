//! Profile pictures: upload, removal and download.
//!
//! The server never trusts what a client says an upload is. It looks at the
//! bytes, accepts only PNG, JPEG and WebP, reads the pixel dimensions from the
//! file's own header, and enforces a size and shape limit. SVG (which can carry
//! script) and animated WebP are refused. The web client additionally redraws
//! the picture on a canvas before sending, which drops metadata and any bytes
//! hidden after the image data.

use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Path, State},
    http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, put},
    Router,
};

use crate::{
    map_storage_error,
    security::{too_many_requests, wait_text, Quota},
    ApiError, AppState, AuthenticatedSubject,
};

/// Largest accepted picture, in bytes.
pub const AVATAR_MAX_BYTES: usize = 512 * 1024;
/// Allowed side length, in pixels. Pictures must be square.
pub const AVATAR_MIN_SIDE: u32 = 64;
pub const AVATAR_MAX_SIDE: u32 = 1024;
/// Uploads and removals one person may make per hour.
const AVATAR_CHANGES_PER_HOUR: u32 = 20;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/profile/avatar",
            put(upload_avatar)
                .delete(remove_avatar)
                // The global limit is far larger; pictures are tiny.
                .layer(DefaultBodyLimit::max(AVATAR_MAX_BYTES + 1024)),
        )
        .route("/v1/users/{user_id}/avatar", get(download_avatar))
}

/// Checks the bytes are a PNG, JPEG or static WebP of an acceptable size and
/// shape. Returns the content type to store.
pub fn validate_avatar(data: &[u8]) -> Result<&'static str, ApiError> {
    if data.is_empty() {
        return Err(ApiError::bad_request("The picture is empty."));
    }
    if data.len() > AVATAR_MAX_BYTES {
        return Err(ApiError::bad_request(format!(
            "The picture must be at most {} KiB.",
            AVATAR_MAX_BYTES / 1024
        )));
    }
    let (content_type, dimensions) = if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        ("image/png", png_dimensions(data))
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        ("image/jpeg", jpeg_dimensions(data))
    } else if data.len() > 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        ("image/webp", webp_dimensions(data))
    } else {
        return Err(ApiError::bad_request(
            "The picture must be a PNG, JPEG or WebP image.",
        ));
    };
    let (width, height) = dimensions
        .ok_or_else(|| ApiError::bad_request("The picture is damaged or not a still image."))?;
    if width != height {
        return Err(ApiError::bad_request("The picture must be square."));
    }
    if !(AVATAR_MIN_SIDE..=AVATAR_MAX_SIDE).contains(&width) {
        return Err(ApiError::bad_request(format!(
            "The picture must be between {AVATAR_MIN_SIDE} and {AVATAR_MAX_SIDE} pixels wide."
        )));
    }
    Ok(content_type)
}

fn png_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    if data.len() < 24 || &data[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(data[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(data[20..24].try_into().ok()?);
    Some((width, height))
}

fn jpeg_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    let mut position = 2;
    while position + 4 <= data.len() {
        if data[position] != 0xFF {
            return None;
        }
        let marker = data[position + 1];
        match marker {
            0xFF => {
                position += 1;
                continue;
            }
            // Markers without a length.
            0x01 | 0xD0..=0xD8 => {
                position += 2;
                continue;
            }
            // End of image or start of scan before any frame header.
            0xD9 | 0xDA => return None,
            _ => {}
        }
        let length = u16::from_be_bytes([data[position + 2], data[position + 3]]) as usize;
        if length < 2 {
            return None;
        }
        let is_frame_header =
            matches!(marker, 0xC0..=0xCF) && !matches!(marker, 0xC4 | 0xC8 | 0xCC);
        if is_frame_header {
            let frame = data.get(position + 4..position + 9)?;
            let height = u16::from_be_bytes([frame[1], frame[2]]) as u32;
            let width = u16::from_be_bytes([frame[3], frame[4]]) as u32;
            return Some((width, height));
        }
        position += 2 + length;
    }
    None
}

fn webp_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    match data.get(12..16)? {
        b"VP8 " => {
            let frame = data.get(20..30)?;
            if frame[3..6] != [0x9D, 0x01, 0x2A] {
                return None;
            }
            let width = u16::from_le_bytes([frame[6], frame[7]]) as u32 & 0x3FFF;
            let height = u16::from_le_bytes([frame[8], frame[9]]) as u32 & 0x3FFF;
            Some((width, height))
        }
        b"VP8L" => {
            let header = data.get(20..25)?;
            if header[0] != 0x2F {
                return None;
            }
            let bits = u32::from_le_bytes([header[1], header[2], header[3], header[4]]);
            Some(((bits & 0x3FFF) + 1, ((bits >> 14) & 0x3FFF) + 1))
        }
        b"VP8X" => {
            let header = data.get(20..30)?;
            // Bit 1 of the flags marks an animation.
            if header[0] & 0x02 != 0 {
                return None;
            }
            let width = u32::from_le_bytes([header[4], header[5], header[6], 0]) + 1;
            let height = u32::from_le_bytes([header[7], header[8], header[9], 0]) + 1;
            Some((width, height))
        }
        _ => None,
    }
}

fn check_change_quota(state: &AppState, user_id: &str) -> Result<(), ApiError> {
    state
        .guards
        .auth
        .check(
            &format!("avatar:{user_id}"),
            Quota::per_hour(AVATAR_CHANGES_PER_HOUR),
        )
        .map_err(|wait| {
            too_many_requests(
                format!("Too many picture changes. Try again in {}.", wait_text(wait)),
                wait,
            )
        })
}

async fn upload_avatar(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, ApiError> {
    check_change_quota(&state, &subject.user_id)?;
    let content_type = validate_avatar(&body)?;
    // The declared type must agree with the bytes, so a mislabelled upload is
    // caught instead of silently relabelled.
    let declared = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.split(';').next().unwrap_or("").trim().to_ascii_lowercase());
    if declared.as_deref() != Some(content_type) {
        return Err(ApiError::bad_request(
            "The Content-Type does not match the picture.",
        ));
    }
    state
        .storage
        .set_user_avatar(&subject.user_id, content_type, &body)
        .await
        .map_err(map_storage_error)?;
    tracing::info!(target: "audit", user_id = %subject.user_id, bytes = body.len(), "Profile picture updated");
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_avatar(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<StatusCode, ApiError> {
    check_change_quota(&state, &subject.user_id)?;
    state
        .storage
        .delete_user_avatar(&subject.user_id)
        .await
        .map_err(map_storage_error)?;
    Ok(StatusCode::NO_CONTENT)
}

/// The picture, or 204 when there is none or the caller may not see it (the
/// two look the same, so the endpoint cannot be used to probe for accounts).
async fn download_avatar(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> Result<Response, ApiError> {
    let allowed = subject.is_system_admin
        || subject.user_id == user_id
        || state
            .storage
            .users_share_a_group(&subject.user_id, &user_id)
            .await
            .map_err(map_storage_error)?;
    if !allowed {
        return Ok(StatusCode::NO_CONTENT.into_response());
    }
    let Some((content_type, data)) = state
        .storage
        .get_user_avatar(&user_id)
        .await
        .map_err(map_storage_error)?
    else {
        return Ok(StatusCode::NO_CONTENT.into_response());
    };
    let mut response = data.into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&content_type).map_err(ApiError::internal)?,
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'none'; sandbox"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=60"),
    );
    headers.insert(
        HeaderName::from_static("cross-origin-resource-policy"),
        HeaderValue::from_static("cross-origin"),
    );
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut data = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        data.extend(width.to_be_bytes());
        data.extend(height.to_be_bytes());
        data.extend([8, 6, 0, 0, 0]);
        data
    }

    fn jpeg(width: u16, height: u16) -> Vec<u8> {
        let mut data = vec![0xFF, 0xD8, 0xFF, 0xE0, 0, 4, 0, 0, 0xFF, 0xC0, 0, 11, 8];
        data.extend(height.to_be_bytes());
        data.extend(width.to_be_bytes());
        data.extend([1, 1, 0x11, 0]);
        data
    }

    fn webp_lossless(width: u32, height: u32) -> Vec<u8> {
        let mut data = b"RIFF\0\0\0\0WEBPVP8L\0\0\0\0\x2f".to_vec();
        data.extend(((width - 1) | ((height - 1) << 14)).to_le_bytes());
        data
    }

    #[test]
    fn accepts_square_pictures_in_range() {
        assert_eq!(validate_avatar(&png(256, 256)).unwrap(), "image/png");
        assert_eq!(validate_avatar(&jpeg(128, 128)).unwrap(), "image/jpeg");
        assert_eq!(validate_avatar(&webp_lossless(256, 256)).unwrap(), "image/webp");
    }

    #[test]
    fn rejects_wrong_shape_size_and_type() {
        assert!(validate_avatar(&png(256, 128)).is_err(), "not square");
        assert!(validate_avatar(&png(32, 32)).is_err(), "too small");
        assert!(validate_avatar(&png(2048, 2048)).is_err(), "too large");
        assert!(validate_avatar(&[]).is_err());
        assert!(validate_avatar(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>").is_err());
        assert!(validate_avatar(b"GIF89a....").is_err());
        let mut oversized = png(256, 256);
        oversized.resize(AVATAR_MAX_BYTES + 1, 0);
        assert!(validate_avatar(&oversized).is_err());
        assert!(validate_avatar(&png(256, 256)[..20]).is_err(), "truncated header");
    }
}
