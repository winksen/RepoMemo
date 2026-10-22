//! Writes the list of built-in templates: every `definitions/*.json` file whose
//! name does not start with `_`, in file-name order, each as an `include_str!`.
//! A new file is registered by being put in the folder; no Rust is edited.

use std::path::PathBuf;
use std::{env, fs};

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let folder = manifest.join("definitions");
    println!("cargo:rerun-if-changed={}", folder.display());

    let mut files: Vec<PathBuf> = fs::read_dir(&folder)
        .map(|entries| entries.filter_map(Result::ok).map(|entry| entry.path()).collect())
        .unwrap_or_default();
    files.retain(|path| {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        path.is_file() && name.ends_with(".json") && !name.starts_with('_')
    });
    files.sort();

    let mut source = String::from("/// The file name and the text of each built-in template, in file-name order.\n");
    source.push_str("pub(crate) const BUILTIN_SOURCES: &[(&str, &str)] = &[\n");
    for path in &files {
        println!("cargo:rerun-if-changed={}", path.display());
        let name = path
            .file_name()
            .expect("a file has a name")
            .to_string_lossy()
            .into_owned();
        let location = path.to_string_lossy().replace('\\', "/");
        source.push_str(&format!("    ({name:?}, include_str!({location:?})),\n"));
    }
    source.push_str("];\n");

    let out = PathBuf::from(env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join("builtin_templates.rs");
    fs::write(out, source).expect("the list of built-in templates is written");
}
