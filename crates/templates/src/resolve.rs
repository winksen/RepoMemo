use crate::{Catalogue, TemplateDefinition, GENERAL_ID};

/// Where the template a workspace uses came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// No choice was saved: `general`, as every workspace was before templates.
    Default,
    /// The template the workspace chose.
    Workspace,
    /// The workspace's choice cannot be used on this server: `general` instead, and why.
    Fallback { reason: String },
}

/// The template to use for one workspace and where it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolution {
    pub template: TemplateDefinition,
    pub origin: Origin,
}

impl Catalogue {
    /// Settles which template a workspace uses from the choice saved for it, as
    /// the template's id and the version it was chosen at. It never fails:
    ///
    /// - no choice: `general`;
    /// - an id the catalogue lacks: `general`, with the reason;
    /// - a version higher than the catalogue's, which means the choice was saved
    ///   by a newer server: `general`, with the reason;
    /// - otherwise the catalogue's template. A lower saved version is fine: the
    ///   file has improved since, and templates are data.
    pub fn resolve(&self, chosen: Option<(&str, u32)>) -> Resolution {
        let general = || self.get(GENERAL_ID).cloned().unwrap_or_else(crate::general);
        let Some((id, version)) = chosen else {
            return Resolution {
                template: general(),
                origin: Origin::Default,
            };
        };
        match self.get(id) {
            None => Resolution {
                template: general(),
                origin: Origin::Fallback {
                    reason: format!("The template {id} is not available on this server."),
                },
            },
            Some(template) if template.meta.version < version => Resolution {
                template: general(),
                origin: Origin::Fallback {
                    reason: format!("The template {id} was chosen on a newer server."),
                },
            },
            Some(template) => Resolution {
                template: template.clone(),
                origin: Origin::Workspace,
            },
        }
    }
}
