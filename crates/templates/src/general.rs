use crate::{TemplateDefinition, TemplateMeta, FORMAT};

/// The id of the template every workspace has until an administrator chooses another.
pub const GENERAL_ID: &str = "general";

/// The template that reproduces RepoMemo as it was before templates: every
/// block is left out, and a left-out block means "as today". It is defined in
/// code, not in a file, so no catalogue, folder or setting can lose it.
pub fn general() -> TemplateDefinition {
    TemplateDefinition {
        format: FORMAT,
        meta: TemplateMeta {
            id: GENERAL_ID.to_owned(),
            version: 1,
            name: "General".to_owned(),
            summary: "Every section in its usual order, with the usual ranking, checks and suggestions.".to_owned(),
            icon: "stack".to_owned(),
        },
    }
}
