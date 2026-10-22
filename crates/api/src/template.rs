//! The template a workspace uses (roadmap F26a, V0.2.166).
//!
//! One resolver answers for every part of the product: it reads the workspace's
//! saved choice, settles it against the catalogue, and keeps the answer until
//! the choice changes. It never fails for want of a template: no choice, a
//! template this server lacks or one saved by a newer server all give
//! `general`, and the answer says why.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use repomemo_domain::{TemplateSummary, WorkspaceTemplateRef, WorkspaceTemplateView};
use repomemo_templates::{Catalogue, Origin, TemplateDefinition};

use crate::RepoMemoCore;

/// Workspaces whose answer is kept at once.
const MAX_CACHED: usize = 256;

/// What a workspace actually uses.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectiveTemplate {
    /// The template in force: the workspace's choice, or `general`.
    pub template: TemplateDefinition,
    /// Where it came from, and why when it is not the workspace's own choice.
    pub origin: Origin,
    /// The choice as saved, whether or not it could be used.
    pub reference: Option<WorkspaceTemplateRef>,
}

impl EffectiveTemplate {
    /// The answer in the shape the API gives.
    pub fn view(&self) -> WorkspaceTemplateView {
        let (source, reason) = match &self.origin {
            Origin::Default => ("default", None),
            Origin::Workspace => ("workspace", None),
            Origin::Fallback { reason } => ("fallback", Some(reason.clone())),
        };
        WorkspaceTemplateView {
            template: summary_of(&self.template),
            source: source.to_owned(),
            reason,
            chosen_id: self.reference.as_ref().map(|reference| reference.id.clone()),
            overrides: self
                .reference
                .as_ref()
                .map(|reference| reference.overrides.clone())
                .unwrap_or_default(),
        }
    }
}

/// What identifies a template, for a list.
pub fn summary_of(template: &TemplateDefinition) -> TemplateSummary {
    TemplateSummary {
        id: template.meta.id.clone(),
        version: template.meta.version,
        name: template.meta.name.clone(),
        summary: template.meta.summary.clone(),
        icon: template.meta.icon.clone(),
    }
}

/// The result of choosing a template for a workspace.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateChange {
    /// False when the workspace was already on that template (nothing to record).
    pub changed: bool,
    pub effective: EffectiveTemplate,
}

#[derive(Debug, Clone)]
pub(crate) struct TemplateResolver {
    catalogue: Arc<Catalogue>,
    /// Per workspace: the saved choice the answer was made for, and the answer.
    kept: Arc<Mutex<HashMap<String, (Option<WorkspaceTemplateRef>, EffectiveTemplate)>>>,
}

impl Default for TemplateResolver {
    fn default() -> Self {
        Self {
            catalogue: Arc::new(Catalogue::builtin().clone()),
            kept: Arc::default(),
        }
    }
}

impl TemplateResolver {
    fn resolve(&self, workspace_id: &str, reference: Option<WorkspaceTemplateRef>) -> EffectiveTemplate {
        if let Ok(kept) = self.kept.lock() {
            if let Some((made_for, answer)) = kept.get(workspace_id) {
                if *made_for == reference {
                    return answer.clone();
                }
            }
        }
        let chosen = reference
            .as_ref()
            .map(|reference| (reference.id.as_str(), reference.version));
        let resolution = self.catalogue.resolve(chosen);
        let answer = EffectiveTemplate {
            template: resolution.template,
            origin: resolution.origin,
            reference: reference.clone(),
        };
        if let Ok(mut kept) = self.kept.lock() {
            if kept.len() >= MAX_CACHED && !kept.contains_key(workspace_id) {
                kept.clear();
            }
            kept.insert(workspace_id.to_owned(), (reference, answer.clone()));
        }
        answer
    }

    /// Forgets a workspace's answer (it is deleted).
    pub(crate) fn forget(&self, workspace_id: &str) {
        if let Ok(mut kept) = self.kept.lock() {
            kept.remove(workspace_id);
        }
    }
}

impl RepoMemoCore {
    /// The template the workspace uses and where it came from. The saved choice is read on each call (one small
    /// read), so a change made by any handle of the storage is seen at once; the work of settling it is kept.
    /// Fails only when the workspace does not exist.
    pub async fn effective_template(&self, workspace_id: &str) -> Result<EffectiveTemplate> {
        let reference = self.storage.workspace_template_ref(workspace_id).await?;
        Ok(self.templates.resolve(workspace_id, reference))
    }

    /// The templates this server offers, `general` first.
    pub fn template_catalogue(&self) -> &Catalogue {
        &self.templates.catalogue
    }

    /// The catalogue as a picker lists it.
    pub fn template_summaries(&self) -> Vec<TemplateSummary> {
        self.templates.catalogue.list().iter().map(summary_of).collect()
    }

    /// Chooses the workspace's template, or returns it to the default with `None`. An id the catalogue lacks is
    /// "not found" and writes nothing. Choosing the template the workspace already has changes nothing, and the
    /// adjustments saved on it stay.
    pub async fn set_workspace_template(
        &self,
        workspace_id: &str,
        template_id: Option<&str>,
    ) -> Result<TemplateChange> {
        let before = self.storage.workspace_template_ref(workspace_id).await?;
        let after = match template_id {
            None => None,
            Some(id) => {
                let Some(template) = self.templates.catalogue.get(id) else {
                    repomemo_domain::bail_not_found!("That template was not found.");
                };
                let overrides = before
                    .as_ref()
                    .filter(|reference| reference.id == id)
                    .map(|reference| reference.overrides.clone())
                    .unwrap_or_default();
                Some(WorkspaceTemplateRef {
                    id: id.to_owned(),
                    version: template.meta.version,
                    overrides,
                })
            }
        };
        if before != after {
            self.storage
                .set_workspace_template_ref(workspace_id, after.as_ref())
                .await?;
        }
        let changed = before.as_ref().map(|reference| &reference.id) != after.as_ref().map(|reference| &reference.id);
        Ok(TemplateChange {
            changed,
            effective: self.effective_template(workspace_id).await?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use repomemo_templates::GENERAL_ID;
    use serde_json::json;

    async fn core(label: &str) -> (RepoMemoCore, std::path::PathBuf) {
        let data_dir = std::env::temp_dir().join(format!("repomemo-template-{label}-{}", uuid_like()));
        (RepoMemoCore::boot(data_dir.clone()).await.unwrap(), data_dir)
    }

    fn uuid_like() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }

    #[tokio::test]
    async fn a_workspace_with_no_choice_resolves_to_general_and_a_missing_one_with_a_reason() {
        let (core, data_dir) = core("resolve").await;
        let workspace = core.create_workspace("Runbooks".to_owned()).await.unwrap();

        let effective = core.effective_template(&workspace.id).await.unwrap();
        assert_eq!(effective.template.id(), GENERAL_ID);
        assert_eq!(effective.origin, Origin::Default);
        assert_eq!(effective.reference, None);

        // A choice naming a template this server lacks.
        let missing = WorkspaceTemplateRef {
            id: "no-such-template".to_owned(),
            version: 1,
            overrides: json!(null),
        };
        core.storage()
            .set_workspace_template_ref(&workspace.id, Some(&missing))
            .await
            .unwrap();
        let effective = core.effective_template(&workspace.id).await.unwrap();
        assert_eq!(effective.template.id(), GENERAL_ID);
        let Origin::Fallback { reason } = &effective.origin else {
            panic!("a fallback was expected")
        };
        assert!(reason.contains("no-such-template"), "{reason}");
        assert_eq!(effective.reference, Some(missing));

        // The choice is chosen again: general named on purpose is the workspace's own choice.
        let general = WorkspaceTemplateRef {
            id: GENERAL_ID.to_owned(),
            version: 1,
            overrides: json!(null),
        };
        core.storage()
            .set_workspace_template_ref(&workspace.id, Some(&general))
            .await
            .unwrap();
        let effective = core.effective_template(&workspace.id).await.unwrap();
        assert_eq!(effective.origin, Origin::Workspace);

        // Cleared, it is the default again; and a workspace that does not exist is an error, not general.
        core.storage()
            .set_workspace_template_ref(&workspace.id, None)
            .await
            .unwrap();
        assert_eq!(
            core.effective_template(&workspace.id).await.unwrap().origin,
            Origin::Default
        );
        assert!(core.effective_template("no-such-workspace").await.is_err());

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn choosing_a_template_checks_the_catalogue_writes_once_and_keeps_the_adjustments() {
        let (core, data_dir) = core("choose").await;
        let workspace = core.create_workspace("Runbooks".to_owned()).await.unwrap();

        // An id the catalogue lacks is not found, and nothing is written.
        let error = core
            .set_workspace_template(&workspace.id, Some("no-such-template"))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("not found"), "{error}");
        assert_eq!(
            core.storage().workspace_template_ref(&workspace.id).await.unwrap(),
            None
        );
        assert!(core
            .set_workspace_template("no-such-workspace", Some(GENERAL_ID))
            .await
            .is_err());

        // Choosing a template changes the workspace once; choosing it again changes nothing.
        let first = core
            .set_workspace_template(&workspace.id, Some(GENERAL_ID))
            .await
            .unwrap();
        assert!(first.changed);
        assert_eq!(first.effective.origin, Origin::Workspace);
        let view = first.effective.view();
        assert_eq!(
            (view.source.as_str(), view.chosen_id.as_deref()),
            ("workspace", Some(GENERAL_ID))
        );
        assert_eq!(view.template.id, GENERAL_ID);
        assert!(view.reason.is_none());
        let again = core
            .set_workspace_template(&workspace.id, Some(GENERAL_ID))
            .await
            .unwrap();
        assert!(!again.changed);

        // The adjustments saved on the template stay when it is chosen again, and go with a different choice.
        let adjusted = WorkspaceTemplateRef {
            id: GENERAL_ID.to_owned(),
            version: 1,
            overrides: json!({ "landing": "review" }),
        };
        core.storage()
            .set_workspace_template_ref(&workspace.id, Some(&adjusted))
            .await
            .unwrap();
        core.set_workspace_template(&workspace.id, Some(GENERAL_ID))
            .await
            .unwrap();
        let kept = core.storage().workspace_template_ref(&workspace.id).await.unwrap();
        assert_eq!(kept, Some(adjusted));

        // Back to the default.
        let reset = core.set_workspace_template(&workspace.id, None).await.unwrap();
        assert!(reset.changed);
        assert_eq!(reset.effective.view().source, "default");
        assert!(!core.set_workspace_template(&workspace.id, None).await.unwrap().changed);

        // The catalogue as a picker lists it: General first.
        assert_eq!(core.template_summaries()[0].id, GENERAL_ID);
        assert_eq!(core.template_summaries().len(), core.template_catalogue().list().len());

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn a_fallback_is_reported_with_the_choice_that_could_not_be_used() {
        let resolver = TemplateResolver::default();
        let missing = WorkspaceTemplateRef {
            id: "ghost".to_owned(),
            version: 1,
            overrides: json!(null),
        };
        let view = resolver.resolve("w", Some(missing)).view();
        assert_eq!(view.source, "fallback");
        assert_eq!(view.template.id, GENERAL_ID);
        assert_eq!(view.chosen_id.as_deref(), Some("ghost"));
        assert!(view.reason.unwrap().contains("ghost"));
    }

    #[test]
    fn the_answer_is_kept_until_the_choice_changes() {
        let resolver = TemplateResolver::default();
        let none = resolver.resolve("w", None);
        assert_eq!(none.origin, Origin::Default);
        // The same choice is answered from memory, a different one is settled again.
        let named = WorkspaceTemplateRef {
            id: GENERAL_ID.to_owned(),
            version: 1,
            overrides: json!(null),
        };
        assert_eq!(resolver.resolve("w", Some(named.clone())).origin, Origin::Workspace);
        assert_eq!(resolver.resolve("w", Some(named)).origin, Origin::Workspace);
        assert_eq!(resolver.resolve("w", None).origin, Origin::Default);
        assert_eq!(resolver.kept.lock().unwrap().len(), 1);
        resolver.forget("w");
        assert!(resolver.kept.lock().unwrap().is_empty());
    }
}
