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
use repomemo_domain::WorkspaceTemplateRef;
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
