//! Errors a person caused or can fix, told apart from failures of the server.
//!
//! The crates below the HTTP layer return `anyhow::Error`. Most of what goes
//! wrong is the server's problem (a database that is busy, a file that cannot
//! be read) and must not reach a client as anything but a generic failure. The
//! rest is about the request: a record that does not exist, input that is not
//! acceptable, a name that is taken. Those are created with [`not_found!`],
//! [`invalid!`] and [`conflict!`] (or the `bail_*` forms) and carry their kind,
//! so the HTTP layer answers from the kind instead of matching message text.

use std::fmt;

/// What kind of problem a [`DomainError`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// The record asked for does not exist (404).
    NotFound,
    /// The request cannot be accepted as it is (400).
    Invalid,
    /// The request clashes with what exists (409).
    Conflict,
}

/// A problem with a request, with a message safe to show to the person.
#[derive(Debug, Clone)]
pub struct DomainError {
    pub kind: ErrorKind,
    pub message: String,
}

impl DomainError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn into_anyhow(self) -> anyhow::Error {
        anyhow::Error::new(self)
    }
}

impl fmt::Display for DomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for DomainError {}

/// The kind and message of an error, when it is one a person can act on.
/// Looks through `context` layers.
pub fn classify(error: &anyhow::Error) -> Option<(ErrorKind, &str)> {
    error
        .chain()
        .find_map(|cause| cause.downcast_ref::<DomainError>())
        .map(|found| (found.kind, found.message.as_str()))
}

/// An error for a record that does not exist.
#[macro_export]
macro_rules! not_found {
    ($($argument:tt)*) => {
        $crate::DomainError::new($crate::ErrorKind::NotFound, format!($($argument)*)).into_anyhow()
    };
}

/// An error for a request that cannot be accepted as it is.
#[macro_export]
macro_rules! invalid {
    ($($argument:tt)*) => {
        $crate::DomainError::new($crate::ErrorKind::Invalid, format!($($argument)*)).into_anyhow()
    };
}

/// An error for a request that clashes with what exists.
#[macro_export]
macro_rules! conflict {
    ($($argument:tt)*) => {
        $crate::DomainError::new($crate::ErrorKind::Conflict, format!($($argument)*)).into_anyhow()
    };
}

/// `return Err(not_found!(…))`.
#[macro_export]
macro_rules! bail_not_found {
    ($($argument:tt)*) => { return Err($crate::not_found!($($argument)*)) };
}

/// `return Err(invalid!(…))`.
#[macro_export]
macro_rules! bail_invalid {
    ($($argument:tt)*) => { return Err($crate::invalid!($($argument)*)) };
}

/// `return Err(conflict!(…))`.
#[macro_export]
macro_rules! bail_conflict {
    ($($argument:tt)*) => { return Err($crate::conflict!($($argument)*)) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;

    #[test]
    fn kinds_survive_context_layers() {
        let error: anyhow::Error = not_found!("Workspace {} was not found.", "w1");
        assert_eq!(
            classify(&error),
            Some((ErrorKind::NotFound, "Workspace w1 was not found."))
        );
        let wrapped = Err::<(), _>(error).context("while loading the page").unwrap_err();
        assert_eq!(classify(&wrapped).map(|found| found.0), Some(ErrorKind::NotFound));
        assert!(classify(&anyhow::anyhow!("disk is full")).is_none());
    }

    #[test]
    fn the_bail_forms_return() {
        fn check(value: i32) -> anyhow::Result<i32> {
            if value < 0 {
                bail_invalid!("{value} is negative");
            }
            Ok(value)
        }
        assert_eq!(classify(&check(-1).unwrap_err()).unwrap().1, "-1 is negative");
        assert_eq!(check(2).unwrap(), 2);
    }
}
