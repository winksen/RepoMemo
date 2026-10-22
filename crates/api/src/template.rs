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
use repomemo_domain::*;
use repomemo_retrieval::RankingProfile;
use repomemo_storage::StarterSetup;
use repomemo_templates::{Catalogue, Origin, Overrides, TemplateDefinition};

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
    /// True when the template in force came from the operator's folder.
    pub custom: bool,
}

impl EffectiveTemplate {
    /// The answer in the shape the API gives.
    pub fn view(&self) -> WorkspaceTemplateView {
        let (source, reason) = match &self.origin {
            Origin::Default => ("default", None),
            Origin::Workspace => ("workspace", None),
            Origin::Fallback { reason } => ("fallback", Some(reason.clone())),
        };
        let detail = detail_of(&self.template, self.custom);
        WorkspaceTemplateView {
            template: detail.template,
            source: source.to_owned(),
            reason,
            chosen_id: self.reference.as_ref().map(|reference| reference.id.clone()),
            overrides: self
                .reference
                .as_ref()
                .map(|reference| reference.overrides.clone())
                .unwrap_or_default(),
            sections: detail.sections,
            labels: detail.labels,
            overview: detail.overview,
            evidence: detail.evidence,
            assistant: detail.assistant,
            retrieval: detail.retrieval,
            ranking: detail.ranking,
            health: detail.health,
            review: detail.review,
            map: detail.map,
            repositories: detail.repositories,
        }
    }
}

/// A template as a person looks at it: everything it arranges and names, resolved (V0.2.177).
pub fn detail_of(template: &TemplateDefinition, custom: bool) -> TemplateDetail {
    let sections = template.sections();
    let labels = template.labels();
    let evidence = template.evidence();
    let assistant = template.assistant();
    TemplateDetail {
        template: summary_of(template, custom),
        sections: TemplateSections {
            primary: sections.primary,
            more: sections.more,
            landing: sections.landing,
        },
        labels: TemplateLabels {
            sections: labels.sections,
            artifact_types: labels.artifact_types,
        },
        overview: TemplateOverview {
            cards: template.overview_cards(),
            headline: template.overview_headline(),
        },
        evidence: TemplateEvidence {
            default_view: evidence.default_view,
            types_first: evidence.types_first,
        },
        assistant: TemplateAssistant {
            order: assistant.order,
            suggestions: assistant.suggestions,
            phrases: assistant
                .phrases
                .into_iter()
                .map(|rule| TemplatePhrase {
                    phrase: rule.phrase,
                    capability: rule.capability,
                })
                .collect(),
            focus: assistant.focus,
        },
        retrieval: TemplateRetrieval {
            type_chips: template.retrieval().type_chips,
        },
        map: {
            let map = template.map_emphasis();
            TemplateMap {
                coverage_first: map.coverage_first,
                relations_first: map.relations_first,
            }
        },
        review: TemplateReview {
            order: template.review_order(),
        },
        repositories: {
            let rules = template.repository_rules();
            TemplateRepositories {
                include: rules.include,
                exclude: rules.exclude,
            }
        },
        health: TemplateHealth {
            detectors: template.health_detectors(),
            stale_after_days: template.stale_after_days(),
        },
        ranking: {
            let ranking = template.ranking();
            TemplateRanking {
                verified: ranking.verified,
                boosts: ranking
                    .boosts
                    .into_iter()
                    .map(|boost| TemplateBoost {
                        artifact_type: boost.artifact_type,
                        document_kind: boost.document_kind,
                        path: boost.path,
                        factor: boost.factor,
                    })
                    .collect(),
                recency: ranking.recency.map(|recency| TemplateRecency {
                    half_life_days: recency.half_life_days,
                    max_factor: recency.max_factor,
                }),
            }
        },
    }
}

/// What identifies a template, for a list.
pub fn summary_of(template: &TemplateDefinition, custom: bool) -> TemplateSummary {
    TemplateSummary {
        id: template.meta.id.clone(),
        version: template.meta.version,
        name: template.meta.name.clone(),
        summary: template.meta.summary.clone(),
        icon: template.meta.icon.clone(),
        custom,
    }
}

/// The result of choosing a template for a workspace.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateChange {
    /// False when the workspace was already on that template (nothing to record).
    pub changed: bool,
    /// True when the adjustments saved on the template are not what they were.
    pub adjusted: bool,
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
        // The adjustments saved on the workspace's own template are laid over it; those saved on a template this
        // server could not use are not applied.
        let mut template = resolution.template;
        if let (Origin::Workspace, Some(reference)) = (&resolution.origin, &reference) {
            let overrides = Overrides::from_stored(&reference.overrides);
            if !overrides.is_empty() {
                template = template.with_overrides(&overrides);
            }
        }
        let custom = matches!(resolution.origin, Origin::Workspace) && self.catalogue.is_custom(template.id());
        let answer = EffectiveTemplate {
            template,
            origin: resolution.origin,
            reference: reference.clone(),
            custom,
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

    /// The template a workspace uses, settled from the settings that were loaded with it: no read of the database, so a
    /// list of workspaces (the dashboard) is not a read for each (V0.2.187).
    pub fn effective_template_of(&self, workspace: &Workspace) -> EffectiveTemplate {
        let reference = workspace
            .settings
            .get("template")
            .and_then(WorkspaceTemplateRef::from_setting);
        self.templates.resolve(&workspace.id, reference)
    }

    /// The workspace's setup as a template file (V0.2.191): the template in force with the adjustments saved on it
    /// folded in, under an id and a name chosen for the new template, at version 1, as JSON that passes the checks
    /// every template file passes. It holds what a template holds (sections, names, cards, suggestions, weights) and
    /// nothing of the workspace: no evidence, no people, no settings, no rights. The id must not be one this server
    /// already has, since the file would be refused when it is loaded here.
    pub async fn export_workspace_template(&self, workspace_id: &str, id: &str, name: &str) -> Result<String> {
        let effective = self.effective_template(workspace_id).await?;
        let mut template = effective.template;
        template.meta.id = id.trim().to_owned();
        template.meta.name = name.trim().to_owned();
        template.meta.version = 1;
        if self.templates.catalogue.get(&template.meta.id).is_some() {
            repomemo_domain::bail_conflict!(
                "The id {} is already a template of this server. Choose another id for the new template.",
                template.meta.id
            );
        }
        let mut text = serde_json::to_string_pretty(&template)?;
        text.push('\n');
        if let Err(issues) = TemplateDefinition::parse(&text) {
            let message = issues.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ");
            repomemo_domain::bail_invalid!("{message}");
        }
        Ok(text)
    }

    /// The templates this server offers, `general` first.
    pub fn template_catalogue(&self) -> &Catalogue {
        &self.templates.catalogue
    }

    /// Replaces the templates this server offers (the operator's folder, tests). The answers already kept are dropped.
    pub fn with_template_catalogue(mut self, catalogue: Catalogue) -> Self {
        self.templates = TemplateResolver {
            catalogue: Arc::new(catalogue),
            kept: Arc::default(),
        };
        self
    }

    /// What a workspace made from the template starts with (the choice to save and the folders to make), or `None`
    /// when the catalogue has no such template. Nothing is written: the caller hands it to the storage with the
    /// workspace, so the workspace and its folders are made together or not at all.
    pub fn workspace_starter(&self, template_id: &str) -> Option<StarterSetup> {
        let template = self.templates.catalogue.get(template_id)?;
        Some(StarterSetup {
            reference: WorkspaceTemplateRef {
                id: template.meta.id.clone(),
                version: template.meta.version,
                overrides: serde_json::Value::Null,
            },
            template_name: template.meta.name.clone(),
            folders: template
                .starter_folders
                .iter()
                .map(|folder| (folder.name.clone(), folder.children.clone()))
                .collect(),
        })
    }

    /// The phrases the workspace's template adds to the assistant's router (V0.2.185). If the template cannot be read
    /// there are none: the router's own rules are all there is.
    pub(crate) async fn assistant_phrases(&self, workspace_id: &str) -> Vec<crate::agent::PhraseRule> {
        let Ok(effective) = self.effective_template(workspace_id).await else {
            return Vec::new();
        };
        effective
            .template
            .assistant()
            .phrases
            .into_iter()
            .filter_map(|rule| {
                Some(crate::agent::PhraseRule {
                    capability: serde_json::from_value(serde_json::json!(rule.capability)).ok()?,
                    phrase: rule.phrase,
                })
            })
            .collect()
    }

    /// How the workspace's search weighs what it finds: its template's ranking, or the usual one when the template
    /// cannot be read (a ranking must never be the reason a search fails).
    pub(crate) async fn ranking_profile(&self, workspace_id: &str) -> RankingProfile {
        match self.effective_template(workspace_id).await {
            Ok(effective) => {
                let ranking = effective.template.ranking();
                let mut profile = RankingProfile::with_verified(ranking.verified);
                for boost in &ranking.boosts {
                    // A selector this server cannot read is skipped; the template was validated, so it is only a
                    // file written for a newer server.
                    if let Some(kind) = &boost.artifact_type {
                        if let Ok(artifact_type) = serde_json::from_value(serde_json::json!(kind)) {
                            profile = profile.boost_type(artifact_type, boost.factor);
                        }
                    } else if let Some(kind) = &boost.document_kind {
                        profile = profile.boost_document_kind(kind, boost.factor);
                    } else if let Some(pattern) = &boost.path {
                        profile = profile.boost_path(pattern, boost.factor);
                    }
                }
                if let Some(recency) = &ranking.recency {
                    profile = profile.prefer_recent(f64::from(recency.half_life_days), recency.max_factor);
                }
                profile
            }
            Err(_) => RankingProfile::general(),
        }
    }

    /// A template of the catalogue as a person looks at it before choosing it, or `None` when there is no such one.
    pub fn template_detail(&self, template_id: &str) -> Option<TemplateDetail> {
        let catalogue = &self.templates.catalogue;
        catalogue
            .get(template_id)
            .map(|template| detail_of(template, catalogue.is_custom(template_id)))
    }

    /// The catalogue as a picker lists it.
    pub fn template_summaries(&self) -> Vec<TemplateSummary> {
        let catalogue = &self.templates.catalogue;
        catalogue
            .list()
            .iter()
            .map(|template| summary_of(template, catalogue.is_custom(template.id())))
            .collect()
    }

    /// Chooses the workspace's template, or returns it to the default with `None`. An id the catalogue lacks is
    /// "not found" and writes nothing. Choosing the template the workspace already has changes nothing, and the
    /// adjustments saved on it stay.
    pub async fn set_workspace_template(
        &self,
        workspace_id: &str,
        template_id: Option<&str>,
        overrides: Option<&serde_json::Value>,
    ) -> Result<TemplateChange> {
        let before = self.storage.workspace_template_ref(workspace_id).await?;
        let after = match template_id {
            None => None,
            Some(id) => {
                let Some(template) = self.templates.catalogue.get(id) else {
                    repomemo_domain::bail_not_found!("That template was not found.");
                };
                let overrides = match overrides {
                    // Left out: the adjustments stay when the same template is chosen again.
                    None => before
                        .as_ref()
                        .filter(|reference| reference.id == id)
                        .map(|reference| reference.overrides.clone())
                        .unwrap_or_default(),
                    // Nothing, or an empty object: no adjustments.
                    Some(value) if value.is_null() || value.as_object().is_some_and(|map| map.is_empty()) => {
                        serde_json::Value::Null
                    }
                    Some(value) => match Overrides::parse(value) {
                        Ok(parsed) => serde_json::to_value(parsed)?,
                        Err(issues) => {
                            let message = issues.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ");
                            repomemo_domain::bail_invalid!("{message}");
                        }
                    },
                };
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
        let overrides_of = |reference: &Option<WorkspaceTemplateRef>| {
            reference
                .as_ref()
                .map(|reference| reference.overrides.clone())
                .unwrap_or_default()
        };
        let adjusted = overrides_of(&before) != overrides_of(&after);
        Ok(TemplateChange {
            changed,
            adjusted,
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
            .set_workspace_template(&workspace.id, Some("no-such-template"), None)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("not found"), "{error}");
        assert_eq!(
            core.storage().workspace_template_ref(&workspace.id).await.unwrap(),
            None
        );
        assert!(core
            .set_workspace_template("no-such-workspace", Some(GENERAL_ID), None)
            .await
            .is_err());

        // Choosing a template changes the workspace once; choosing it again changes nothing.
        let first = core
            .set_workspace_template(&workspace.id, Some(GENERAL_ID), None)
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
            .set_workspace_template(&workspace.id, Some(GENERAL_ID), None)
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
        core.set_workspace_template(&workspace.id, Some(GENERAL_ID), None)
            .await
            .unwrap();
        let kept = core.storage().workspace_template_ref(&workspace.id).await.unwrap();
        assert_eq!(kept, Some(adjusted));

        // Back to the default.
        let reset = core.set_workspace_template(&workspace.id, None, None).await.unwrap();
        assert!(reset.changed);
        assert_eq!(reset.effective.view().source, "default");
        assert!(
            !core
                .set_workspace_template(&workspace.id, None, None)
                .await
                .unwrap()
                .changed
        );

        // The catalogue as a picker lists it: General first.
        assert_eq!(core.template_summaries()[0].id, GENERAL_ID);
        assert_eq!(core.template_summaries().len(), core.template_catalogue().list().len());

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn a_starter_comes_from_the_catalogue_and_carries_the_templates_folders() {
        let file = r#"{
            "format": 1,
            "meta": { "id": "operations", "version": 2, "name": "Operations and incidents",
                      "summary": "Runbooks and incidents.", "icon": "heartbeat" },
            "starter_folders": [ { "name": "Runbooks" }, { "name": "Incidents", "children": ["2026", "Postmortems"] } ]
        }"#;
        let (catalogue, problems) = Catalogue::load([("operations.json", file)]);
        assert!(problems.is_empty(), "{problems:?}");
        let (core, data_dir) = core("starter").await;
        let core = core.with_template_catalogue(catalogue);

        let starter = core.workspace_starter("operations").expect("the catalogue has it");
        assert_eq!(starter.reference.id, "operations");
        assert_eq!(starter.reference.version, 2);
        assert_eq!(starter.template_name, "Operations and incidents");
        assert_eq!(
            starter.folders,
            [
                ("Runbooks".to_owned(), vec![]),
                (
                    "Incidents".to_owned(),
                    vec!["2026".to_owned(), "Postmortems".to_owned()]
                )
            ]
        );
        // General makes none, and an id the catalogue lacks makes nothing.
        let general = core.workspace_starter(GENERAL_ID).unwrap();
        assert!(general.folders.is_empty());
        assert_eq!(general.reference.id, GENERAL_ID);
        assert!(core.workspace_starter("no-such-template").is_none());
        assert_eq!(
            core.template_summaries()
                .iter()
                .map(|template| template.id.as_str())
                .collect::<Vec<_>>(),
            [GENERAL_ID, "operations"]
        );

        let _ = std::fs::remove_dir_all(data_dir);
    }

    /// The kinds of link a template can put first are the knowledge map's edge kinds.
    #[test]
    fn the_links_a_template_can_put_first_are_the_maps_edge_kinds() {
        use repomemo_domain::KnowledgeEdgeKind;
        let id = |kind: &KnowledgeEdgeKind| match kind {
            KnowledgeEdgeKind::Similar => "similar",
            KnowledgeEdgeKind::Cites => "cites",
        };
        let every = [KnowledgeEdgeKind::Similar, KnowledgeEdgeKind::Cites];
        let named: Vec<&str> = every.iter().map(id).collect();
        assert_eq!(named, repomemo_templates::MAP_RELATIONS);
        for kind in &every {
            assert_eq!(serde_json::to_value(kind).unwrap(), json!(id(kind)));
        }
    }

    /// V0.2.186: the patterns a template offers the link form mean to the sync what they say. Each one is run through
    /// the matcher that decides which files a repository indexes.
    #[test]
    fn the_rules_a_template_offers_the_link_form_do_what_they_say_when_a_sync_applies_them() {
        use repomemo_ingestion::repo::{classify_repo_file, RepoFileRules};
        let catalogue = Catalogue::builtin();
        let kept = |id: &str, path: &str| {
            let rules = catalogue.get(id).unwrap().repository_rules();
            let rules = RepoFileRules {
                include: rules.include,
                exclude: rules.exclude,
            };
            classify_repo_file(path, 2_000, &rules).is_ok()
        };
        // Software engineering: snapshots and generated files are left out, the code stays.
        assert!(!kept("software-engineering", "src/__snapshots__/app.test.snap"));
        assert!(!kept("software-engineering", "ui/button.snap"));
        assert!(!kept("software-engineering", "api/types.generated.ts"));
        assert!(!kept("software-engineering", "pkg/testdata/case.json"));
        assert!(kept("software-engineering", "src/main.rs"));
        // Operations: state and logs are left out, runbooks and infrastructure code stay.
        assert!(!kept("operations", "infra/prod.tfstate"));
        assert!(!kept("operations", "infra/prod.tfstate.backup"));
        assert!(!kept("operations", "var/deploy.log"));
        assert!(kept("operations", "runbooks/restart-payments.md"));
        assert!(kept("operations", "infra/main.tf"));
        // The documents-first templates ask for documents only, in any folder and at the top.
        assert!(kept("product-decisions", "README.md"));
        assert!(kept("product-decisions", "specs/2026/billing.docx"));
        assert!(!kept("product-decisions", "src/main.rs"));
        assert!(kept("knowledge-base", "policies/security.pdf"));
        assert!(kept("knowledge-base", "finance.xlsx"));
        assert!(!kept("knowledge-base", "src/main.rs"));
    }

    /// The health checks a template can choose are exactly the API's detectors, in the order Health has always had.
    #[test]
    fn the_checks_a_template_can_choose_are_the_health_detectors() {
        use repomemo_domain::HealthDetector;
        let ids: Vec<&str> = HealthDetector::ALL.iter().map(|detector| detector.as_str()).collect();
        assert_eq!(ids, repomemo_templates::HEALTH_DETECTORS);
        let defaults: Vec<&str> = HealthDetector::DEFAULT
            .iter()
            .map(|detector| detector.as_str())
            .collect();
        assert_eq!(defaults, repomemo_templates::DEFAULT_DETECTORS);
        for detector in HealthDetector::ALL {
            assert_eq!(HealthDetector::from_id(detector.as_str()), Some(detector));
            assert_eq!(serde_json::to_value(detector).unwrap(), json!(detector.as_str()));
        }
        assert_eq!(HealthDetector::from_id("mind_reading"), None);
    }

    /// What the assistant can be told to do is exactly what the API's capabilities are (V0.2.176).
    #[test]
    fn the_jobs_a_template_can_order_are_the_assistants_capabilities() {
        use repomemo_domain::AgentCapability;
        let id = |job: &AgentCapability| match job {
            AgentCapability::FindFiles => "find_files",
            AgentCapability::FindSymbol => "find_symbol",
            AgentCapability::SearchContent => "search_content",
            AgentCapability::SummarizeFile => "summarize_file",
            AgentCapability::AskQuestion => "ask_question",
            AgentCapability::WorkspaceOverview => "workspace_overview",
        };
        let every = [
            AgentCapability::FindFiles,
            AgentCapability::FindSymbol,
            AgentCapability::SearchContent,
            AgentCapability::SummarizeFile,
            AgentCapability::AskQuestion,
            AgentCapability::WorkspaceOverview,
        ];
        let named: Vec<&str> = every.iter().map(id).collect();
        assert_eq!(named, repomemo_templates::CAPABILITIES);
        for job in &every {
            assert_eq!(serde_json::to_value(job).unwrap(), json!(id(job)));
        }
    }

    /// The kinds of evidence a template can rename are exactly the ones the API has: adding an `ArtifactType`
    /// stops this from compiling until the list in `repomemo-templates` has it too.
    #[test]
    fn the_kinds_of_evidence_a_template_can_name_are_the_artifact_types() {
        use repomemo_domain::ArtifactType;
        let id = |kind: &ArtifactType| match kind {
            ArtifactType::File => "file",
            ArtifactType::MarkdownDoc => "markdown_doc",
            ArtifactType::CodeFile => "code_file",
            ArtifactType::Image => "image",
            ArtifactType::Issue => "issue",
            ArtifactType::Pr => "pr",
            ArtifactType::Decision => "decision",
            ArtifactType::Incident => "incident",
            ArtifactType::Runbook => "runbook",
            ArtifactType::ApiSpec => "api_spec",
            ArtifactType::Note => "note",
            ArtifactType::Repository => "repository",
        };
        let every = [
            ArtifactType::File,
            ArtifactType::MarkdownDoc,
            ArtifactType::CodeFile,
            ArtifactType::Image,
            ArtifactType::Issue,
            ArtifactType::Pr,
            ArtifactType::Decision,
            ArtifactType::Incident,
            ArtifactType::Runbook,
            ArtifactType::ApiSpec,
            ArtifactType::Note,
            ArtifactType::Repository,
        ];
        let named: Vec<&str> = every.iter().map(id).collect();
        assert_eq!(named, repomemo_templates::ARTIFACT_TYPES);
        // And the id is what the API writes for it.
        for kind in &every {
            assert_eq!(serde_json::to_value(kind).unwrap(), json!(id(kind)));
        }
    }

    /// V0.2.191: a workspace's setup leaves as a template file that carries its adjustments and nothing of the workspace,
    /// and loading that file gives the same arrangement.
    #[tokio::test]
    async fn a_workspaces_setup_is_exported_as_a_template_that_loads_back_the_same() {
        let (core, data_dir) = core("export").await;
        let workspace = core.create_workspace("Runbooks".to_owned()).await.unwrap();
        core.set_workspace_template(&workspace.id, Some("operations"), None)
            .await
            .unwrap();
        let adjustments = json!({ "landing": "memory", "more": ["map", "health"], "suggestions": ["Who is on call?"] });
        core.set_workspace_template(&workspace.id, Some("operations"), Some(&adjustments))
            .await
            .unwrap();

        let text = core
            .export_workspace_template(&workspace.id, "night-shift", "Night shift")
            .await
            .unwrap();
        let exported =
            TemplateDefinition::parse(&text).expect("the exported file passes the checks of every template file");
        assert_eq!(
            (exported.id(), exported.meta.version, exported.meta.name.as_str()),
            ("night-shift", 1, "Night shift")
        );
        // Nothing but a template is in it: no workspace id, no name of the workspace, no setting but the blocks.
        assert!(!text.contains(&workspace.id) && !text.contains("Runbooks"), "{text}");
        let keys: Vec<String> = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&text)
            .unwrap()
            .keys()
            .cloned()
            .collect();
        assert!(
            keys.iter().all(|key| [
                "format",
                "meta",
                "starter_folders",
                "sections",
                "labels",
                "overview",
                "evidence",
                "assistant",
                "retrieval",
                "ranking",
                "health",
                "review",
                "map",
                "repositories"
            ]
            .contains(&key.as_str())),
            "{keys:?}"
        );

        // Loaded by a server that has it, the file gives the arrangement the workspace had, adjustments included.
        let (catalogue, problems) = Catalogue::builtin().extended([("night-shift.json", text.as_str())]);
        assert!(problems.is_empty(), "{problems:?}");
        let there = catalogue.get("night-shift").unwrap();
        let here = core.effective_template(&workspace.id).await.unwrap().template;
        assert_eq!(there.sections(), here.sections());
        assert_eq!(there.sections().landing, "memory");
        assert_eq!(there.sections().more, ["map", "health"]);
        assert_eq!(there.labels(), here.labels());
        assert_eq!(there.assistant().suggestions, ["Who is on call?"]);
        assert_eq!(there.ranking(), here.ranking());
        assert_eq!(there.health_detectors(), here.health_detectors());
        assert_eq!(there.overview_cards(), here.overview_cards());
        assert!(catalogue.is_custom("night-shift"));

        // An id this server has, a bad id and a bad name are refused with the reason, and nothing is exported.
        let taken = core
            .export_workspace_template(&workspace.id, "operations", "Operations")
            .await
            .unwrap_err();
        assert!(
            taken.to_string().contains("already a template of this server"),
            "{taken}"
        );
        let general = core
            .export_workspace_template(&workspace.id, "general", "General")
            .await
            .unwrap_err();
        assert!(general.to_string().contains("already a template"), "{general}");
        for (bad_id, bad_name) in [
            ("Night Shift", "Night shift"),
            ("x", "Night shift"),
            ("night-shift", ""),
            ("night-shift", &"n".repeat(41)),
        ] {
            let error = core
                .export_workspace_template(&workspace.id, bad_id, bad_name)
                .await
                .unwrap_err();
            assert!(error.to_string().contains("meta."), "{bad_id:?} {bad_name:?}: {error}");
        }
        // A workspace on General exports General under the new id, and the file still passes.
        let plain = core.create_workspace("Plain".to_owned()).await.unwrap();
        let general_copy = core
            .export_workspace_template(&plain.id, "my-general", "My general")
            .await
            .unwrap();
        assert!(TemplateDefinition::parse(&general_copy).is_ok());
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn adjustments_are_checked_saved_laid_over_the_template_kept_cleared_and_dropped_with_it() {
        let (core, data_dir) = core("adjust").await;
        let workspace = core.create_workspace("Runbooks".to_owned()).await.unwrap();
        let adjustments = json!({ "landing": "review", "more": ["map"], "suggestions": ["Who is on call?"] });

        // Invalid adjustments are refused and nothing is written.
        for bad in [
            json!({ "landing": "settings" }),
            json!({ "more": ["nowhere"] }),
            json!({ "colour": "red" }),
            json!("review"),
        ] {
            let error = core
                .set_workspace_template(&workspace.id, Some(GENERAL_ID), Some(&bad))
                .await
                .unwrap_err();
            assert!(
                error.to_string().contains("overrides") || error.to_string().contains("invalid"),
                "{error}"
            );
        }
        assert_eq!(
            core.storage().workspace_template_ref(&workspace.id).await.unwrap(),
            None
        );

        // Saved, they are laid over the template, and recorded as an adjustment (not a change of template).
        let saved = core
            .set_workspace_template(&workspace.id, Some(GENERAL_ID), Some(&adjustments))
            .await
            .unwrap();
        assert!(saved.changed && saved.adjusted);
        let view = saved.effective.view();
        assert_eq!(view.sections.landing, "review");
        assert_eq!(view.sections.more, ["map"]);
        assert_eq!(view.assistant.suggestions, ["Who is on call?"]);
        assert_eq!(view.overrides, adjustments);

        // Chosen again without adjustments, they stay; with an empty object they are cleared.
        let again = core
            .set_workspace_template(&workspace.id, Some(GENERAL_ID), None)
            .await
            .unwrap();
        assert!(!again.changed && !again.adjusted);
        assert_eq!(again.effective.view().sections.landing, "review");
        let changed_only = json!({ "landing": "memory" });
        let second = core
            .set_workspace_template(&workspace.id, Some(GENERAL_ID), Some(&changed_only))
            .await
            .unwrap();
        assert!(!second.changed && second.adjusted);
        let cleared = core
            .set_workspace_template(&workspace.id, Some(GENERAL_ID), Some(&json!({})))
            .await
            .unwrap();
        assert!(!cleared.changed && cleared.adjusted);
        assert_eq!(cleared.effective.view().sections.landing, "overview");
        assert!(cleared.effective.view().overrides.is_null());

        // Returning to the default drops them with the choice.
        core.set_workspace_template(&workspace.id, Some(GENERAL_ID), Some(&adjustments))
            .await
            .unwrap();
        let reset = core
            .set_workspace_template(&workspace.id, None, Some(&adjustments))
            .await
            .unwrap();
        assert_eq!(reset.effective.view().source, "default");
        assert_eq!(reset.effective.view().sections.landing, "overview");

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn adjustments_saved_on_a_template_this_server_cannot_use_are_not_applied() {
        let resolver = TemplateResolver::default();
        let ghost = WorkspaceTemplateRef {
            id: "ghost".to_owned(),
            version: 1,
            overrides: json!({ "landing": "review" }),
        };
        let view = resolver.resolve("w", Some(ghost)).view();
        assert_eq!(view.source, "fallback");
        assert_eq!(view.sections.landing, "overview");
        // A stale key is dropped and a good one applies.
        let stale = WorkspaceTemplateRef {
            id: GENERAL_ID.to_owned(),
            version: 1,
            overrides: json!({ "landing": "review", "more": ["nowhere"] }),
        };
        let view = resolver.resolve("w2", Some(stale)).view();
        assert_eq!(view.sections.landing, "review");
        assert!(view.sections.more.is_empty());
    }

    #[test]
    fn a_template_can_be_looked_at_before_it_is_chosen() {
        let resolver = TemplateResolver::default();
        let general = detail_of(&resolver.catalogue.get(GENERAL_ID).unwrap().clone(), false);
        assert_eq!(general.template.id, GENERAL_ID);
        assert_eq!(general.sections.primary.len(), 12);
        assert_eq!(general.overview.cards.len(), 9);
        for template in resolver.catalogue.list() {
            let detail = detail_of(template, false);
            assert_eq!(detail.template.id, template.id());
            assert_eq!(
                detail.sections.primary.len() + detail.sections.more.len(),
                12,
                "{}",
                template.id()
            );
        }
    }

    #[tokio::test]
    async fn a_workspace_ranks_by_its_templates_factor_and_by_the_usual_one_without() {
        let file = r#"{
            "format": 1,
            "meta": { "id": "operations", "version": 1, "name": "Operations", "summary": "Runbooks.", "icon": "heartbeat" },
            "ranking": { "verified": 1.2, "boosts": [
                { "artifact_type": "runbook", "factor": 1.25 },
                { "document_kind": "pdf", "factor": 1.1 },
                { "path": "docs/adr/**", "factor": 1.15 }
            ], "recency": { "half_life_days": 60, "max_factor": 1.1 } }
        }"#;
        let (catalogue, problems) = Catalogue::load([("operations.json", file)]);
        assert!(problems.is_empty(), "{problems:?}");
        let (core, data_dir) = core("ranking").await;
        let core = core.with_template_catalogue(catalogue);
        let workspace = core.create_workspace("Runbooks".to_owned()).await.unwrap();
        // No choice: the ranking every workspace had before templates.
        assert_eq!(core.ranking_profile(&workspace.id).await, RankingProfile::general());
        assert_eq!(core.ranking_profile(&workspace.id).await.verified_factor(), Some(1.1));
        // Operations: its factor.
        core.set_workspace_template(&workspace.id, Some("operations"), None)
            .await
            .unwrap();
        let profile = core.ranking_profile(&workspace.id).await;
        assert_eq!(profile.verified_factor(), Some(1.2));
        // Its boosts become rules of the profile, each by what it selects.
        use repomemo_domain::ArtifactType;
        use repomemo_retrieval::RankingRule;
        assert_eq!(profile.rules().len(), 5);
        assert!(profile.rules().contains(&RankingRule::Recency {
            half_life_days: 60.0,
            max_factor: 1.1
        }));
        assert!(profile.rules().contains(&RankingRule::ArtifactType {
            artifact_type: ArtifactType::Runbook,
            factor: 1.25
        }));
        assert!(profile.rules().contains(&RankingRule::DocumentKind {
            kind: "pdf".to_owned(),
            factor: 1.1
        }));
        assert!(profile.rules().contains(&RankingRule::Path {
            pattern: "docs/adr/**".to_owned(),
            factor: 1.15
        }));
        // General named on purpose, and a workspace that does not exist: the usual one.
        core.set_workspace_template(&workspace.id, Some(GENERAL_ID), None)
            .await
            .unwrap();
        assert_eq!(core.ranking_profile(&workspace.id).await, RankingProfile::general());
        assert_eq!(
            core.ranking_profile("no-such-workspace").await,
            RankingProfile::general()
        );
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
