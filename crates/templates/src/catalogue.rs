use std::collections::HashSet;
use std::sync::OnceLock;

use crate::{general, TemplateDefinition, TemplateIssue, GENERAL_ID};

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
}

impl Catalogue {
    /// Only `general`.
    pub fn general_only() -> Self {
        Self {
            templates: vec![general()],
        }
    }

    /// Reads templates from `(file name, text)` pairs. A file that is not
    /// valid, is not named after its id, repeats an id or tries to replace
    /// `general` is left out and returned as a problem; the others are kept.
    /// The result does not depend on the order of the pairs.
    pub fn load<'a>(sources: impl IntoIterator<Item = (&'a str, &'a str)>) -> (Self, Vec<SourceProblem>) {
        let mut sources: Vec<(&str, &str)> = sources.into_iter().collect();
        sources.sort();
        let mut problems = Vec::new();
        let mut loaded: Vec<TemplateDefinition> = Vec::new();
        let mut taken: HashSet<String> = HashSet::from([GENERAL_ID.to_owned()]);
        for (name, text) in sources {
            let template = match TemplateDefinition::parse(text) {
                Ok(template) => template,
                Err(issues) => {
                    problems.push(SourceProblem {
                        source: name.to_owned(),
                        issues,
                    });
                    continue;
                }
            };
            let id = template.id().to_owned();
            let mismatch = name.strip_suffix(".json") != Some(id.as_str());
            let problem = if id == GENERAL_ID {
                Some("general is defined by the server and cannot be replaced".to_owned())
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
                    source: name.to_owned(),
                    issues: vec![TemplateIssue::new("meta.id", message)],
                }),
                None => {
                    taken.insert(id);
                    loaded.push(template);
                }
            }
        }
        loaded.sort_by_cached_key(|template| (template.meta.name.to_lowercase(), template.meta.id.clone()));
        let mut templates = vec![general()];
        templates.extend(loaded);
        (Self { templates }, problems)
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
