use crate::*;

#[derive(Debug, Serialize)]
pub(crate) struct ErrorBody {
    pub(crate) error: ErrorDetail,
}

#[derive(Debug, Serialize)]
pub(crate) struct ErrorDetail {
    pub(crate) code: &'static str,
    pub(crate) message: String,
}

#[derive(Debug)]
pub(crate) struct ApiError {
    pub(crate) status: StatusCode,
    pub(crate) code: &'static str,
    pub(crate) message: String,
    /// Seconds to wait before retrying, sent as `Retry-After`.
    pub(crate) retry_after: Option<u64>,
}

impl ApiError {
    pub(crate) fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            retry_after: None,
        }
    }

    pub(crate) fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "bad_request", message)
    }

    pub(crate) fn unauthorized() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "A valid bearer token is required.",
        )
    }

    pub(crate) fn conflict(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, "conflict", message)
    }

    pub(crate) fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", message)
    }

    pub(crate) fn forbidden() -> Self {
        Self::forbidden_because("Your membership does not allow access to this workspace.")
    }

    pub(crate) fn forbidden_because(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, "forbidden", message)
    }

    pub(crate) fn internal(error: impl std::fmt::Display) -> Self {
        tracing::error!(error = %error, "Unhandled API error");
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "The server could not complete this request.",
        )
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = (
            self.status,
            Json(ErrorBody {
                error: ErrorDetail {
                    code: self.code,
                    message: self.message,
                },
            }),
        )
            .into_response();
        if let Some(seconds) = self.retry_after {
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from(seconds));
        }
        response
    }
}

/// A panic inside a handler becomes the usual JSON error instead of a dropped
/// connection, and is logged.
pub(crate) fn panic_response(panic: Box<dyn std::any::Any + Send + 'static>) -> Response {
    let detail = panic
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| panic.downcast_ref::<&str>().copied())
        .unwrap_or("unknown panic");
    ApiError::internal(format!("handler panicked: {detail}")).into_response()
}

pub(crate) fn invalid_credentials() -> ApiError {
    ApiError::new(
        StatusCode::UNAUTHORIZED,
        "invalid_credentials",
        "Email or password is incorrect.",
    )
}

/// Answers for an error from the storage or core layer: what a person caused
/// carries its kind (see `repomemo_domain::DomainError`) and its message is
/// shown; everything else is the server's problem and reaches the client as a
/// generic failure, with the detail in the log (review L6).
pub(crate) fn map_storage_error(error: anyhow::Error) -> ApiError {
    match repomemo_domain::classify(&error) {
        Some((ErrorKind::NotFound, message)) => ApiError::not_found(message),
        Some((ErrorKind::Invalid, message)) => ApiError::bad_request(message),
        Some((ErrorKind::Conflict, message)) => ApiError::conflict(message),
        None if repomemo_storage::is_unique_violation(&error) => ApiError::conflict("This record already exists."),
        None => ApiError::internal(error),
    }
}

pub(crate) fn map_core_error(error: anyhow::Error) -> ApiError {
    map_storage_error(error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use repomemo_domain::{conflict, invalid, not_found};

    #[test]
    fn what_a_person_caused_keeps_its_kind_and_server_failures_stay_generic() {
        let missing = map_storage_error(not_found!("Folder was not found."));
        assert_eq!(
            (missing.status, missing.message.as_str()),
            (StatusCode::NOT_FOUND, "Folder was not found.")
        );
        assert_eq!(
            map_storage_error(invalid!("Name is required.")).status,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(map_storage_error(conflict!("Taken.")).status, StatusCode::CONFLICT);

        // Even a message that says "was not found" is the server's problem when nobody typed it.
        let internal = map_storage_error(anyhow::anyhow!("disk error: C:/secret/place was not found"));
        assert_eq!(internal.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(!internal.message.contains("secret"));
        // …and a typed error survives the context layers storage code adds.
        let wrapped = Err::<(), _>(not_found!("Note was not found."))
            .map_err(|error| error.context("while opening the page"))
            .unwrap_err();
        assert_eq!(map_storage_error(wrapped).status, StatusCode::NOT_FOUND);
    }
}
