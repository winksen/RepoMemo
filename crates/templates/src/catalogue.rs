use std::collections::{BTreeSet, HashSet};
use std::sync::OnceLock;

use crate::{clean_text, general, TemplateDefinition, TemplateIssue, GENERAL_ID};

include!(concat!(env!("OUT_DIR"), "/builtin_templates.rs"));

/// A file that was left out of a catalogue, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceProblem {
    /// The file name.
    pub source: String,
    pub issues: Vec<TemplateIssue>,
}

/// The templates a server knows. `general` is always there and always first;
/// the others follow by name.
#[derive(Debug, Clone, PartialEq)]
pub struct Catalogue {
    templates: Vec<TemplateDefinition>,
    /// The ids that came from the operator's folder rather than from this server's own files (V0.2.189).
    custom: BTreeSet<String>,
}

/// Reads the files in order and keeps the valid ones. `reserved` holds the ids that are already taken (the built-in
/// templates, when files are added to them); the returned templates are in file-name order.
fn admit(sources: Vec<(&str, &str)>, reserved: &HashSet<String>) -> (Vec<TemplateDefinition>, Vec<SourceProblem>) {
    let mut problems = Vec::new();
    let mut loaded: Vec<TemplateDefinition> = Vec::new();
    let mut taken: HashSet<String> = HashSet::new();
    for (name, text) in sources {
        let template = match TemplateDefinition::parse(text) {
            Ok(template) => template,
            Err(issues) => {
                problems.push(SourceProblem {
                    source: clean_text(name),
                    issues,
                });
                continue;
            }
        };
        let id = template.id().to_owned();
        let mismatch = name.strip_suffix(".json") != Some(id.as_str());
        let problem = if id == GENERAL_ID {
            Some("general is defined by the server and cannot be replaced".to_owned())
        } else if reserved.contains(&id) {
            Some(format!(
                "{id} is a template built into this server; a file cannot replace it"
            ))
        } else if mismatch {
            Some(format!(
                "the file is named {name} and the id is {id}; a file is named after its id"
            ))
        } else if taken.contains(&id) {
            Some(format!("another file already has the id {id}"))
        } else {
            None
        };
        match problem {
            Some(message) => problems.push(SourceProblem {
                source: clean_text(name),
                issues: vec![TemplateIssue::new("meta.id", message)],
            }),
            None => {
                taken.insert(id);
                loaded.push(template);
            }
        }
    }
    (loaded, problems)
}

impl Catalogue {
    /// Only `general`.
    pub fn general_only() -> Self {
        Self {
            templates: vec![general()],
            custom: BTreeSet::new(),
        }
    }

    /// Reads templates from `(file name, text)` pairs. A file that is not
    /// valid, is not named after its id, repeats an id or tries to replace
    /// `general` is left out and returned as a problem; the others are kept.
    /// The result does not depend on the order of the pairs.
    pub fn load<'a>(sources: impl IntoIterator<Item = (&'a str, &'a str)>) -> (Self, Vec<SourceProblem>) {
        let mut sources: Vec<(&str, &str)> = sources.into_iter().collect();
        sources.sort();
        let (mut loaded, problems) = admit(sources, &HashSet::new());
        loaded.sort_by_cached_key(|template| (template.meta.name.to_lowercase(), template.meta.id.clone()));
        let mut templates = vec![general()];
        templates.extend(loaded);
        (
            Self {
                templates,
                custom: BTreeSet::new(),
            },
            problems,
        )
    }

    /// This catalogue and the templates of more files (the operator's folder, V0.2.189), which are marked custom. The
    /// files are held to the rules of every template file, and an id of this catalogue (`general` and the built-in
    /// templates included) cannot be taken by a file: it is left out and returned as a problem, the others are kept.
    /// The result does not depend on the order of the pairs.
    pub fn extended<'a>(&self, sources: impl IntoIterator<Item = (&'a str, &'a str)>) -> (Self, Vec<SourceProblem>) {
        let mut sources: Vec<(&str, &str)> = sources.into_iter().collect();
        sources.sort();
        let reserved: HashSet<String> = self.templates.iter().map(|template| template.id().to_owned()).collect();
        let (added, problems) = admit(sources, &reserved);
        let mut custom = self.custom.clone();
        custom.extend(added.iter().map(|template| template.id().to_owned()));
        let mut rest: Vec<TemplateDefinition> = self.templates.iter().skip(1).cloned().chain(added).collect();
        rest.sort_by_cached_key(|template| (template.meta.name.to_lowercase(), template.meta.id.clone()));
        let mut templates = vec![general()];
        templates.extend(rest);
        (Self { templates, custom }, problems)
    }

    /// Whether the template came from the operator's folder rather than from this server's own files.
    pub fn is_custom(&self, id: &str) -> bool {
        self.custom.contains(id)
    }

    /// The templates built into this server.
    pub fn builtin() -> &'static Catalogue {
        &builtin().0
    }

    /// Why a built-in file was left out. Empty while the tests pass.
    pub fn builtin_problems() -> &'static [SourceProblem] {
        &builtin().1
    }

    pub fn get(&self, id: &str) -> Option<&TemplateDefinition> {
        self.templates.iter().find(|template| template.id() == id)
    }

    /// Every template, `general` first.
    pub fn list(&self) -> &[TemplateDefinition] {
        &self.templates
    }
}

fn builtin() -> &'static (Catalogue, Vec<SourceProblem>) {
    static BUILTIN: OnceLock<(Catalogue, Vec<SourceProblem>)> = OnceLock::new();
    BUILTIN.get_or_init(|| Catalogue::load(BUILTIN_SOURCES.iter().copied()))
}
