//! Template files from a folder the operator chose (V0.2.189, `REPOMEMO_TEMPLATES_DIR`).
//!
//! The folder is read once, when the server starts. What is read is only ever text handed to
//! [`Catalogue::extended`](crate::Catalogue::extended), which applies the rules every template file is held to; this
//! module decides which files are worth reading at all, and says why it left the others out. Nothing here can stop the
//! server: an unreadable folder, a file that is too big and a link are problems to report, not errors.

use std::fs;
use std::io::Read;
use std::path::Path;

use crate::{limits, SourceProblem, TemplateIssue};

/// A folder gives at most this many files.
pub const FOLDER_FILES_MAX: usize = 64;

/// What was found in a folder: the files to load, as (file name, text), and the ones left out.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FolderRead {
    pub sources: Vec<(String, String)>,
    pub problems: Vec<SourceProblem>,
}

fn problem(source: &str, message: String) -> SourceProblem {
    SourceProblem {
        source: source.to_owned(),
        issues: vec![TemplateIssue::new("", message)],
    }
}

/// Reads the `*.json` files of `folder`, in file-name order. A file whose name starts with `_` is left alone, as in the
/// folder of built-in templates, and so is anything that is not a plain file of that folder: a link is never followed
/// and a folder is not entered. At most [`FOLDER_FILES_MAX`] files are read, each at most
/// [`limits::FILE_BYTES`] bytes.
pub fn read_folder(folder: &Path) -> FolderRead {
    let mut read = FolderRead::default();
    let entries = match fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(error) => {
            read.problems.push(problem(
                &folder.display().to_string(),
                format!("the folder cannot be read: {error}"),
            ));
            return read;
        }
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.ends_with(".json") && !name.starts_with('_'))
        .collect();
    names.sort();
    for (index, name) in names.iter().enumerate() {
        if index >= FOLDER_FILES_MAX {
            read.problems.push(problem(
                name,
                format!("the folder has more than {FOLDER_FILES_MAX} template files; only the first {FOLDER_FILES_MAX} are read"),
            ));
            continue;
        }
        let path = folder.join(name);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) => {
                read.problems
                    .push(problem(name, format!("the file cannot be read: {error}")));
                continue;
            }
        };
        if metadata.file_type().is_symlink() {
            read.problems
                .push(problem(name, "the file is a link; links are not followed".to_owned()));
            continue;
        }
        if !metadata.is_file() {
            continue;
        }
        if metadata.len() > limits::FILE_BYTES as u64 {
            read.problems.push(problem(
                name,
                format!(
                    "the file is {} bytes; a template is at most {}",
                    metadata.len(),
                    limits::FILE_BYTES
                ),
            ));
            continue;
        }
        // Reading stops one byte past the limit, so a file that grew since it was measured is still refused.
        let mut bytes = Vec::new();
        let result =
            fs::File::open(&path).and_then(|file| file.take(limits::FILE_BYTES as u64 + 1).read_to_end(&mut bytes));
        if let Err(error) = result {
            read.problems
                .push(problem(name, format!("the file cannot be read: {error}")));
        } else if bytes.len() > limits::FILE_BYTES {
            read.problems.push(problem(
                name,
                format!(
                    "the file is more than {} bytes; a template is at most that",
                    limits::FILE_BYTES
                ),
            ));
        } else {
            match String::from_utf8(bytes) {
                Ok(text) => read.sources.push((name.clone(), text)),
                Err(_) => read
                    .problems
                    .push(problem(name, "the file is not UTF-8 text".to_owned())),
            }
        }
    }
    read
}
