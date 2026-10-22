//! The bounds of the format. Each is checked in `validate` and has a test; the
//! checker of V0.2.188 and the operator folder of V0.2.189 use the same values.

/// A template file is at most this many bytes.
pub const FILE_BYTES: usize = 64 * 1024;
/// `{` and `[` nest at most this deep.
pub const NESTING: usize = 8;

/// A template id is 2 to 40 characters of `a-z`, `0-9` and `-`.
pub const ID_MIN: usize = 2;
pub const ID_MAX: usize = 40;
/// The version of a template, from 1.
pub const VERSION_MAX: u32 = 1_000_000;
/// Characters, not bytes.
pub const NAME_MAX: usize = 40;
pub const SUMMARY_MAX: usize = 160;

/// A template creates at most this many folders in all, two levels deep.
pub const STARTER_FOLDERS_MAX: usize = 12;
/// The name of a starter folder, in characters.
pub const FOLDER_NAME_MAX: usize = 40;
