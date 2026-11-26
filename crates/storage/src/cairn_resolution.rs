//! Which text model serves a workspace (V0.9.84). CAIRN, Ask and the summaries all use the one this resolves, so a workspace has one text
//! configuration. Until a workspace chooses its own default (V0.9.85), a person chooses a model per chat (V0.9.86) or the person's
//! preferences name one (V0.9.114), the order is:
//!
//! 1. the workspace's own default, when it saved settings (V0.9.85);
//! 2. the model the workspace used before (its own provider, linked to the catalog by V0.9.78) while it is offered;
//! 3. the organization's default model, while the workspace offers it;
//! 4. none: CAIRN answers by its rules, as with no provider today.
//!
//! A model is offered when the organization enables it (or never saved settings) and the workspace offers it (or never saved
//! settings). When neither saved any, nothing changes for anyone: each workspace keeps the model it had. A model that is turned off
//! is never replaced silently by another: the answer says which one answered.

use repomemo_domain::ProviderSettings;
use serde_json::{json, Value};

use super::*;
use crate::cairn_catalog::{CairnConnection, CairnModel};

/// The prefix of the id of a provider that is made from the catalog rather than stored: `catalog:<workspace id>:<model id>`.
pub const CATALOG_PROVIDER_PREFIX: &str = "catalog:";

/// The text model that serves a workspace, and what to tell the person when it is not the one the workspace had.
#[derive(Debug, Clone)]
pub struct ResolvedTextProvider {
    pub provider: ProviderSettings,
    /// Said in the answer: the workspace's model is not offered by its organization, so another answered.
    pub notice: Option<String>,
}

/// What one confirmation of CAIRN's proposals may change in a workspace (V0.9.92).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeLimits {
    /// The most changes in one confirmation.
    pub max_changes: i64,
    /// The number of changes above which the person types the number to confirm, besides the rule for content from outside.
    pub type_to_confirm_from: Option<i64>,
    /// Who set the cap: `organization`, `workspace` or `default`.
    pub set_by: &'static str,
}

/// What a resolution found: a model, or why there is none to say.
#[derive(Debug, Clone, Default)]
pub struct TextResolution {
    pub resolved: Option<ResolvedTextProvider>,
    /// Said when there is no model because the organization does not offer the workspace's own.
    pub notice: Option<String>,
}

/// A provider as the organization's Agent-mode default sets it: a workspace that chose its own mode keeps it.
fn with_agent_mode(mut provider: ProviderSettings, mode: &str) -> ProviderSettings {
    if mode != "auto" && provider.metadata.get("cairn_mode").and_then(Value::as_str).is_none() {
        if let Some(object) = provider.metadata.as_object_mut() {
            object.insert("cairn_mode".to_owned(), json!(mode));
        }
    }
    provider
}

fn catalog_provider_id(workspace_id: &str, model_id: &str) -> String {
    format!("{CATALOG_PROVIDER_PREFIX}{workspace_id}:{model_id}")
}

impl StorageEngine {
    /// A provider made from a catalog model for a workspace, with the connection's key and the model's traits.
    async fn catalog_provider(
        &self,
        workspace_id: &str,
        model: &CairnModel,
        connection: &CairnConnection,
    ) -> Result<ProviderSettings> {
        Ok(ProviderSettings {
            id: catalog_provider_id(workspace_id, &model.id),
            workspace_id: Some(workspace_id.to_owned()),
            provider_type: connection.kind.clone(),
            name: model.display_name.clone(),
            base_url: Some(connection.base_url.clone()),
            model: Some(model.model_id.clone()),
            embedding_model: None,
            enabled: true,
            metadata: json!({
                "purpose": "text",
                "cloud_content_acknowledged": connection.cloud_acknowledged_at.is_some(),
                "tool_mode": model.tool_mode,
                "context_tokens": model.context_tokens.max(0),
                "traits_manual": true,
                "cairn_model_id": model.id,
            }),
            api_key: self.cairn_connection_key(&connection.id).await?,
        })
    }

    /// The provider behind an id made by [`CATALOG_PROVIDER_PREFIX`], so every path that reads a provider by id reads the model the
    /// workspace's organization chose.
    pub(crate) async fn catalog_provider_by_id(&self, id: &str) -> Result<ProviderSettings> {
        let rest = id
            .strip_prefix(CATALOG_PROVIDER_PREFIX)
            .ok_or_else(|| repomemo_domain::not_found!("provider settings were not found"))?;
        let (workspace_id, model_id) = rest
            .split_once(':')
            .ok_or_else(|| repomemo_domain::not_found!("provider settings were not found"))?;
        let model = self.get_cairn_model(model_id).await?;
        let connection = self.get_cairn_connection(&model.connection_id).await?;
        let mode = match self.workspace_organization(workspace_id).await? {
            Some(organization_id) => {
                self.get_organization_cairn_settings(&organization_id)
                    .await?
                    .policy
                    .agent_mode
            }
            None => "auto".to_owned(),
        };
        Ok(with_agent_mode(
            self.catalog_provider(workspace_id, &model, &connection).await?,
            &mode,
        ))
    }

    /// The CAIRN tools and jobs turned off in a workspace, by name, with who turned each off (V0.9.91): the organization, whose
    /// switches a workspace cannot undo, or the workspace itself.
    pub async fn tools_off_for_workspace(&self, workspace_id: &str) -> Result<Vec<(String, &'static str)>> {
        let mut off: Vec<(String, &'static str)> = Vec::new();
        if let Some(organization_id) = self.workspace_organization(workspace_id).await? {
            for name in self
                .get_organization_cairn_settings(&organization_id)
                .await?
                .policy
                .guardrails
                .tools_off
            {
                off.push((name, "organization"));
            }
        }
        for name in self
            .get_workspace_cairn_settings(workspace_id)
            .await?
            .policy
            .guardrails
            .tools_off
        {
            if !off.iter().any(|(existing, _)| *existing == name) {
                off.push((name, "workspace"));
            }
        }
        Ok(off)
    }

    /// The folders closed to CAIRN in a workspace and every folder inside them (V0.9.93).
    pub async fn closed_folder_ids(&self, workspace_id: &str) -> Result<std::collections::HashSet<String>> {
        let settings = self.get_workspace_cairn_settings(workspace_id).await?;
        let closed = settings.policy.guardrails.closed_folder_ids;
        if closed.is_empty() {
            return Ok(Default::default());
        }
        let folders = self.list_folders(workspace_id).await?;
        let mut all: std::collections::HashSet<String> = closed.into_iter().collect();
        loop {
            let before = all.len();
            for folder in &folders {
                if folder.parent_id.as_ref().is_some_and(|parent| all.contains(parent)) {
                    all.insert(folder.id.clone());
                }
            }
            if all.len() == before {
                break;
            }
        }
        Ok(all)
    }

    /// The files in the folders closed to CAIRN (V0.9.93): CAIRN's tools, its chats across workspaces and the MCP endpoint never return
    /// them. Ask outside CAIRN is unchanged.
    pub async fn closed_artifact_ids(&self, workspace_id: &str) -> Result<std::collections::HashSet<String>> {
        let folders = self.closed_folder_ids(workspace_id).await?;
        if folders.is_empty() {
            return Ok(Default::default());
        }
        let folders = serde_json::to_string(&folders.into_iter().collect::<Vec<_>>())?;
        Ok(sqlx::query_scalar::<_, String>(
            "SELECT id FROM artifacts WHERE workspace_id = ?1 AND folder_id IN (SELECT value FROM json_each(?2))",
        )
        .bind(workspace_id)
        .bind(folders)
        .fetch_all(self.reader())
        .await?
        .into_iter()
        .collect())
    }

    /// What happens to a secret on its way to a model outside the network in a workspace: `refuse` when the organization or the
    /// workspace says so, else `mask` (V0.9.94).
    pub async fn secrets_mode_for_workspace(&self, workspace_id: &str) -> Result<&'static str> {
        let refuses = |guardrails: &crate::organization_cairn::Guardrails| {
            guardrails.secrets_for_cloud.as_deref() == Some("refuse")
        };
        if let Some(organization_id) = self.workspace_organization(workspace_id).await? {
            if refuses(
                &self
                    .get_organization_cairn_settings(&organization_id)
                    .await?
                    .policy
                    .guardrails,
            ) {
                return Ok("refuse");
            }
        }
        Ok(
            if refuses(&self.get_workspace_cairn_settings(workspace_id).await?.policy.guardrails) {
                "refuse"
            } else {
                "mask"
            },
        )
    }

    /// A resolution whose provider carries the organization's choice about secrets (V0.9.94).
    async fn guard_secrets(&self, workspace_id: &str, mut resolution: TextResolution) -> Result<TextResolution> {
        let mut personal = self
            .get_workspace_cairn_settings(workspace_id)
            .await?
            .policy
            .guardrails
            .mask_personal_data;
        if let Some(organization_id) = self.workspace_organization(workspace_id).await? {
            personal |= self
                .get_organization_cairn_settings(&organization_id)
                .await?
                .policy
                .guardrails
                .mask_personal_data;
        }
        if let (true, Some(resolved)) = (personal, resolution.resolved.as_mut()) {
            if let Some(object) = resolved.provider.metadata.as_object_mut() {
                object.insert("personal_mode".to_owned(), json!("mask"));
            }
        }
        if let Some(resolved) = resolution.resolved.as_mut() {
            if self.secrets_mode_for_workspace(workspace_id).await? == "refuse" {
                if let Some(object) = resolved.provider.metadata.as_object_mut() {
                    object.insert("secrets_mode".to_owned(), json!("refuse"));
                }
            }
        }
        Ok(resolution)
    }

    /// What one confirmation may change in a workspace, and from how many changes the person types the number (V0.9.92): the stricter
    /// of the organization's and the workspace's, and never more than the default cap.
    pub async fn change_limits_for_workspace(&self, workspace_id: &str) -> Result<ChangeLimits> {
        let mut limits = ChangeLimits {
            max_changes: crate::organization_cairn::DEFAULT_MAX_CHANGES,
            type_to_confirm_from: None,
            set_by: "default",
        };
        let mut apply = |guardrails: &crate::organization_cairn::Guardrails, by: &'static str| {
            if let Some(max) = guardrails.max_changes.filter(|max| *max < limits.max_changes) {
                limits.max_changes = max;
                limits.set_by = by;
            }
            if let Some(from) = guardrails.type_to_confirm_from {
                if limits.type_to_confirm_from.is_none_or(|current| from < current) {
                    limits.type_to_confirm_from = Some(from);
                }
            }
        };
        if let Some(organization_id) = self.workspace_organization(workspace_id).await? {
            apply(
                &self
                    .get_organization_cairn_settings(&organization_id)
                    .await?
                    .policy
                    .guardrails,
                "organization",
            );
        }
        apply(
            &self.get_workspace_cairn_settings(workspace_id).await?.policy.guardrails,
            "workspace",
        );
        Ok(limits)
    }

    /// A catalog model as a provider for a workspace, when it can be used now: on, a text model, on a connection that is usable.
    async fn usable_catalog_provider(
        &self,
        workspace_id: &str,
        model_id: &str,
        mode: &str,
    ) -> Option<ProviderSettings> {
        let model = self.get_cairn_model(model_id).await.ok()?;
        let connection = self.get_cairn_connection(&model.connection_id).await.ok()?;
        if !(model.enabled && model.purpose == "text" && connection.is_usable()) {
            return None;
        }
        let provider = self.catalog_provider(workspace_id, &model, &connection).await.ok()?;
        Some(with_agent_mode(provider, mode))
    }

    /// The models a role may not use in a workspace (V0.9.87): those the organization, the workspace or the catalog limit to other
    /// roles. Owners and admins may use every model; no role at all (a background task) is not limited.
    async fn denied_for_role(
        &self,
        organization: &Option<crate::organization_cairn::OrganizationCairnSettings>,
        workspace: &crate::workspace_cairn::WorkspaceCairnSettings,
        role: Option<&str>,
    ) -> Result<Vec<String>> {
        let Some(role) = role.filter(|role| !matches!(*role, "owner" | "admin")) else {
            return Ok(Vec::new());
        };
        let limits =
            |roles: Option<&Vec<String>>| roles.is_some_and(|roles| !roles.iter().any(|allowed| allowed == role));
        let mut denied = Vec::new();
        for model in self.list_cairn_models(None).await? {
            let by_organization = organization
                .as_ref()
                .is_some_and(|settings| limits(settings.policy.model_roles.get(&model.id)));
            if by_organization || limits(workspace.policy.model_roles.get(&model.id)) || limits(model.roles.as_ref()) {
                denied.push(model.id);
            }
        }
        Ok(denied)
    }

    /// Resolves the text model of a workspace (see the module's description) for no one in particular.
    pub async fn resolve_text_provider(&self, workspace_id: &str) -> Result<TextResolution> {
        self.resolve_text_provider_as(workspace_id, None).await
    }

    /// Resolves the text model of a workspace for a person with a role: a model limited to other roles is not used (V0.9.87).
    pub async fn resolve_text_provider_as(&self, workspace_id: &str, role: Option<&str>) -> Result<TextResolution> {
        let resolution = self.resolve_inner(workspace_id, role).await?;
        self.guard_secrets(workspace_id, resolution).await
    }

    async fn resolve_inner(&self, workspace_id: &str, role: Option<&str>) -> Result<TextResolution> {
        let former = self
            .list_provider_settings(workspace_id)
            .await?
            .into_iter()
            .find(|setting| setting.enabled && setting.purpose() == "text");
        let organization = match self.workspace_organization(workspace_id).await? {
            Some(organization_id) => Some(self.get_organization_cairn_settings(&organization_id).await?),
            None => None,
        };
        let workspace = self.get_workspace_cairn_settings(workspace_id).await?;
        let organization_decided = organization.as_ref().is_some_and(|settings| settings.version > 0);
        // Neither the organization nor the workspace saved settings: every workspace keeps the model it had.
        if !organization_decided && workspace.version == 0 {
            return Ok(TextResolution {
                resolved: former.map(|provider| ResolvedTextProvider { provider, notice: None }),
                notice: None,
            });
        }
        let (enabled_by_organization, mode, default_model) =
            match organization.as_ref().filter(|settings| settings.version > 0) {
                Some(settings) => (
                    Some(&settings.policy.enabled_model_ids),
                    settings.policy.agent_mode.as_str(),
                    settings.policy.default_model_id.as_deref(),
                ),
                None => (None, "auto", None),
            };
        let offered_by_organization =
            |id: &str| enabled_by_organization.is_none_or(|ids| ids.iter().any(|enabled| enabled == id));
        let offered_by_workspace = |id: &str| {
            workspace.version == 0
                || workspace.policy.offered_model_ids.is_empty()
                || workspace.policy.offered_model_ids.iter().any(|offered| offered == id)
        };
        let denied = self.denied_for_role(&organization, &workspace, role).await?;
        let offered = |id: &str| {
            offered_by_organization(id) && offered_by_workspace(id) && !denied.iter().any(|denied| denied == id)
        };

        // 1. The workspace's own default, which is its own provider's model when it is the same.
        let linked = former
            .as_ref()
            .and_then(|provider| provider.metadata.get("cairn_model_id"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        if workspace.version > 0 {
            if let Some(default_id) = workspace.policy.default_model_id.as_deref().filter(|id| offered(id)) {
                if linked.as_deref() == Some(default_id) {
                    if let Some(former) = former.clone() {
                        return Ok(TextResolution {
                            resolved: Some(ResolvedTextProvider {
                                provider: with_agent_mode(former, mode),
                                notice: None,
                            }),
                            notice: None,
                        });
                    }
                }
                if let Some(provider) = self.usable_catalog_provider(workspace_id, default_id, mode).await {
                    return Ok(TextResolution {
                        resolved: Some(ResolvedTextProvider {
                            notice: self.lost_notice(&former, &provider, linked.as_deref(), &offered_by_organization),
                            provider,
                        }),
                        notice: None,
                    });
                }
            }
        }
        // 2. The model the workspace had, while it is offered.
        if let (Some(former), Some(linked)) = (former.as_ref(), linked.as_deref()) {
            if offered(linked) {
                return Ok(TextResolution {
                    resolved: Some(ResolvedTextProvider {
                        provider: with_agent_mode(former.clone(), mode),
                        notice: None,
                    }),
                    notice: None,
                });
            }
        }
        // 3. The organization's default, while the workspace offers it.
        if let Some(default_id) = default_model.filter(|id| offered(id)) {
            if let Some(provider) = self.usable_catalog_provider(workspace_id, default_id, mode).await {
                return Ok(TextResolution {
                    resolved: Some(ResolvedTextProvider {
                        notice: self.lost_notice(&former, &provider, linked.as_deref(), &offered_by_organization),
                        provider,
                    }),
                    notice: None,
                });
            }
        }
        Ok(TextResolution {
            resolved: None,
            notice: former.as_ref().map(|provider| {
                let name = provider.model.clone().unwrap_or_else(|| provider.name.clone());
                let who = if linked.as_deref().is_some_and(|id| !offered_by_organization(id)) {
                    "Your organization does not"
                } else {
                    "This workspace does not"
                };
                format!("{who} offer {name} and no other model is available, so CAIRN answers by its rules.")
            }),
        })
    }

    /// The text models offered in a workspace and usable now, by id (V0.9.86). Where neither the organization nor the workspace saved
    /// settings, only the model the workspace had is offered, so nothing changes for anyone until an administrator changes it.
    pub async fn offered_text_model_ids(&self, workspace_id: &str) -> Result<Vec<String>> {
        self.offered_text_model_ids_for(workspace_id, None).await
    }

    /// The same for a person with a role: a model limited to other roles is not offered (V0.9.87).
    pub async fn offered_text_model_ids_for(&self, workspace_id: &str, role: Option<&str>) -> Result<Vec<String>> {
        let organization = match self.workspace_organization(workspace_id).await? {
            Some(id) => Some(self.get_organization_cairn_settings(&id).await?),
            None => None,
        };
        let workspace = self.get_workspace_cairn_settings(workspace_id).await?;
        let decided = organization.as_ref().is_some_and(|settings| settings.version > 0);
        let mut ids = Vec::new();
        if !decided && workspace.version == 0 {
            let linked = self
                .list_provider_settings(workspace_id)
                .await?
                .into_iter()
                .find(|setting| setting.enabled && setting.purpose() == "text")
                .and_then(|provider| {
                    provider
                        .metadata
                        .get("cairn_model_id")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                });
            ids.extend(linked);
        } else {
            for model in self.list_cairn_models(None).await? {
                let by_organization = organization
                    .as_ref()
                    .filter(|settings| settings.version > 0)
                    .is_none_or(|settings| settings.policy.enabled_model_ids.contains(&model.id));
                let by_workspace = workspace.version == 0
                    || workspace.policy.offered_model_ids.is_empty()
                    || workspace.policy.offered_model_ids.contains(&model.id);
                if by_organization && by_workspace {
                    ids.push(model.id);
                }
            }
        }
        let denied = self.denied_for_role(&organization, &workspace, role).await?;
        ids.retain(|id| !denied.contains(id));
        let mut usable = Vec::new();
        for id in ids {
            if let Ok(model) = self.get_cairn_model(&id).await {
                if let Ok(connection) = self.get_cairn_connection(&model.connection_id).await {
                    if model.enabled && model.purpose == "text" && connection.is_usable() {
                        usable.push(id);
                    }
                }
            }
        }
        Ok(usable)
    }

    /// The text model for a message in a chat whose person chose `chosen` (V0.9.86): that model while it is offered, else what the
    /// workspace resolves, with a note that the chosen one is no longer offered.
    pub async fn resolve_text_provider_for(
        &self,
        workspace_id: &str,
        chosen: Option<&str>,
        role: Option<&str>,
    ) -> Result<TextResolution> {
        let resolution = self.resolve_for_inner(workspace_id, chosen, role).await?;
        self.guard_secrets(workspace_id, resolution).await
    }

    async fn resolve_for_inner(
        &self,
        workspace_id: &str,
        chosen: Option<&str>,
        role: Option<&str>,
    ) -> Result<TextResolution> {
        let Some(chosen) = chosen else {
            return self.resolve_text_provider_as(workspace_id, role).await;
        };
        if self
            .offered_text_model_ids_for(workspace_id, role)
            .await?
            .iter()
            .any(|id| id == chosen)
        {
            let former = self
                .list_provider_settings(workspace_id)
                .await?
                .into_iter()
                .find(|setting| setting.enabled && setting.purpose() == "text");
            let mode = match self.workspace_organization(workspace_id).await? {
                Some(id) => self.get_organization_cairn_settings(&id).await?.policy.agent_mode,
                None => "auto".to_owned(),
            };
            let provider = match former {
                Some(former) if former.metadata.get("cairn_model_id").and_then(Value::as_str) == Some(chosen) => {
                    Some(with_agent_mode(former, &mode))
                }
                _ => self.usable_catalog_provider(workspace_id, chosen, &mode).await,
            };
            if let Some(provider) = provider {
                return Ok(TextResolution {
                    resolved: Some(ResolvedTextProvider { provider, notice: None }),
                    notice: None,
                });
            }
        }
        let name = match self.get_cairn_model(chosen).await {
            Ok(model) => model.display_name,
            Err(_) => "the model".to_owned(),
        };
        let mut resolution = self.resolve_text_provider_as(workspace_id, role).await?;
        let said = match resolution.resolved.as_ref() {
            Some(resolved) => format!(
                "{name}, which you chose for this chat, is no longer offered, so {} answered.",
                resolved
                    .provider
                    .model
                    .clone()
                    .unwrap_or_else(|| resolved.provider.name.clone())
            ),
            None => {
                format!("{name}, which you chose for this chat, is no longer offered, and no other model is available.")
            }
        };
        match resolution.resolved.as_mut() {
            Some(resolved) => resolved.notice = Some(said),
            None => resolution.notice = Some(said),
        }
        Ok(resolution)
    }

    /// What to say when another model answers instead of the one the workspace had; nothing when it had none.
    fn lost_notice(
        &self,
        former: &Option<ProviderSettings>,
        answering: &ProviderSettings,
        linked: Option<&str>,
        offered_by_organization: &dyn Fn(&str) -> bool,
    ) -> Option<String> {
        let former = former.as_ref()?;
        let name = former.model.clone().unwrap_or_else(|| former.name.clone());
        let answered = answering.model.clone().unwrap_or_else(|| answering.name.clone());
        let who = if linked.is_some_and(|id| !offered_by_organization(id)) {
            "Your organization does not"
        } else {
            "This workspace does not"
        };
        Some(format!("{who} offer {name}, so {answered} answered."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cairn_catalog::{NewCairnConnection, NewCairnModel};
    use crate::organization_cairn::OrganizationCairnPolicy;

    struct Setup {
        storage: StorageEngine,
        dir: std::path::PathBuf,
        actor: String,
        organization_id: String,
        workspace_id: String,
        former: CairnModel,
        other: CairnModel,
    }

    async fn setup(mode: Option<&str>) -> Setup {
        let dir = std::env::temp_dir().join(format!("repomemo-resolution-{}", Uuid::new_v4()));
        let storage = StorageEngine::open(crate::StorageConfig::new(dir.clone()))
            .await
            .unwrap();
        let actor = storage
            .create_user("owner@example.com", "Owner", "hash")
            .await
            .unwrap()
            .id;
        let organization = storage
            .create_organization_with(&actor, "Org", None, None)
            .await
            .unwrap();
        let workspace = storage
            .create_shared_workspace_with_policy(&actor, &organization.id, "Space", None, false)
            .await
            .unwrap();
        let workspace_id = workspace.workspace.id;
        let connection = storage
            .create_cairn_connection(
                NewCairnConnection {
                    kind: "ollama".to_owned(),
                    name: "Ollama".to_owned(),
                    base_url: "http://127.0.0.1:11434".to_owned(),
                    api_key: None,
                    locality: "local".to_owned(),
                    enabled: true,
                },
                &actor,
            )
            .await
            .unwrap();
        let model = |name: &str| NewCairnModel {
            connection_id: connection.id.clone(),
            model_id: name.to_owned(),
            display_name: name.to_owned(),
            purpose: "text".to_owned(),
            tool_mode: "native".to_owned(),
            context_tokens: 32_000,
            vision: false,
            evaluation: None,
            fallback_model_id: None,
            roles: None,
            enabled: true,
        };
        let former = storage.create_cairn_model(model("llama3.1:8b"), &actor).await.unwrap();
        let other = storage.create_cairn_model(model("qwen2.5:7b"), &actor).await.unwrap();
        let mut metadata =
            json!({"purpose": "text", "tool_mode": "native", "context_tokens": 32000, "cairn_model_id": former.id});
        if let Some(mode) = mode {
            metadata["cairn_mode"] = json!(mode);
        }
        storage
            .save_provider_settings(ProviderSettings {
                id: String::new(),
                workspace_id: Some(workspace_id.clone()),
                provider_type: "ollama".to_owned(),
                name: "Workspace Ollama".to_owned(),
                base_url: Some("http://127.0.0.1:11434".to_owned()),
                model: Some("llama3.1:8b".to_owned()),
                embedding_model: None,
                enabled: true,
                metadata,
                api_key: None,
            })
            .await
            .unwrap();
        Setup {
            storage,
            dir,
            actor,
            organization_id: organization.id,
            workspace_id,
            former,
            other,
        }
    }

    async fn save(setup: &Setup, enabled: &[&CairnModel], default: Option<&CairnModel>, mode: &str) {
        let current = setup
            .storage
            .get_organization_cairn_settings(&setup.organization_id)
            .await
            .unwrap();
        setup
            .storage
            .save_organization_cairn_settings(
                &setup.organization_id,
                current.version,
                &OrganizationCairnPolicy {
                    enabled_model_ids: enabled.iter().map(|model| model.id.clone()).collect(),
                    default_model_id: default.map(|model| model.id.clone()),
                    agent_mode: mode.to_owned(),
                    model_roles: Default::default(),
                    guardrails: Default::default(),
                },
                &setup.actor,
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn an_organization_that_saved_nothing_changes_nothing_and_one_that_offers_the_model_keeps_it() {
        let setup = setup(None).await;
        let kept = setup.storage.resolve_text_provider(&setup.workspace_id).await.unwrap();
        let provider = kept.resolved.unwrap().provider;
        assert!(
            !provider.id.starts_with(CATALOG_PROVIDER_PREFIX),
            "the workspace's own provider, as before"
        );
        assert_eq!(provider.name, "Workspace Ollama");

        save(&setup, &[&setup.former, &setup.other], Some(&setup.other), "auto").await;
        let resolved = setup
            .storage
            .resolve_text_provider(&setup.workspace_id)
            .await
            .unwrap()
            .resolved
            .unwrap();
        assert_eq!(
            resolved.provider.name, "Workspace Ollama",
            "a model that is offered stays, though it is not the default"
        );
        assert!(resolved.notice.is_none());
        let _ = std::fs::remove_dir_all(setup.dir);
    }

    #[tokio::test]
    async fn a_model_the_organization_turned_off_is_replaced_by_its_default_and_the_answer_says_so() {
        let setup = setup(None).await;
        save(&setup, &[&setup.other], Some(&setup.other), "auto").await;
        let resolved = setup
            .storage
            .resolve_text_provider(&setup.workspace_id)
            .await
            .unwrap()
            .resolved
            .unwrap();
        assert_eq!(resolved.provider.model.as_deref(), Some("qwen2.5:7b"));
        assert_eq!(
            resolved.notice.as_deref(),
            Some("Your organization does not offer llama3.1:8b, so qwen2.5:7b answered.")
        );
        // The same model is read by its id on every path that reads a provider by id.
        assert!(resolved.provider.id.starts_with(CATALOG_PROVIDER_PREFIX));
        let by_id = setup
            .storage
            .get_provider_settings(&resolved.provider.id)
            .await
            .unwrap();
        assert_eq!(
            (by_id.model, by_id.workspace_id),
            (Some("qwen2.5:7b".to_owned()), Some(setup.workspace_id.clone()))
        );

        // With no default there is no model, and the answer says why.
        save(&setup, &[&setup.other], None, "auto").await;
        let none = setup.storage.resolve_text_provider(&setup.workspace_id).await.unwrap();
        assert!(none.resolved.is_none());
        assert!(none.notice.unwrap().contains("has no default model"));
        let _ = std::fs::remove_dir_all(setup.dir);
    }

    #[tokio::test]
    async fn a_workspace_with_no_provider_uses_the_organization_default_and_the_agent_mode_default_yields_to_its_own() {
        let setup = setup(Some("jobs")).await;
        // The organization's Agent default applies to a workspace that chose no mode, and not to one that chose.
        save(&setup, &[&setup.former, &setup.other], Some(&setup.other), "agent").await;
        let own = setup
            .storage
            .resolve_text_provider(&setup.workspace_id)
            .await
            .unwrap()
            .resolved
            .unwrap()
            .provider;
        assert_eq!(own.metadata["cairn_mode"], "jobs", "the workspace chose Jobs");

        let bare = setup
            .storage
            .create_shared_workspace_with_policy(&setup.actor, &setup.organization_id, "Bare", None, false)
            .await
            .unwrap();
        let served = setup
            .storage
            .resolve_text_provider(&bare.workspace.id)
            .await
            .unwrap()
            .resolved
            .unwrap();
        assert_eq!(served.provider.model.as_deref(), Some("qwen2.5:7b"));
        assert_eq!(served.provider.metadata["cairn_mode"], "agent");
        assert!(served.notice.is_none(), "it had no model to lose");
        let _ = std::fs::remove_dir_all(setup.dir);
    }

    #[tokio::test]
    async fn a_workspace_offers_a_subset_and_its_own_default_comes_first() {
        use crate::workspace_cairn::WorkspaceCairnPolicy;
        let setup = setup(None).await;
        let storage = &setup.storage;
        let save_workspace = |offered: Vec<String>, default: Option<String>, version: i64| {
            let (storage, id) = (setup.storage.clone(), setup.workspace_id.clone());
            async move {
                storage
                    .save_workspace_cairn_settings(
                        &id,
                        version,
                        &WorkspaceCairnPolicy {
                            offered_model_ids: offered,
                            default_model_id: default,
                            model_roles: Default::default(),
                            guardrails: Default::default(),
                        },
                        None,
                    )
                    .await
                    .unwrap()
            }
        };
        // The organization has not decided, and the workspace offers only the other model and makes it its default: it answers.
        save_workspace(vec![setup.other.id.clone()], Some(setup.other.id.clone()), 0).await;
        let chosen = storage
            .resolve_text_provider(&setup.workspace_id)
            .await
            .unwrap()
            .resolved
            .unwrap();
        assert_eq!(chosen.provider.model.as_deref(), Some("qwen2.5:7b"));
        assert_eq!(
            chosen.notice.as_deref(),
            Some("This workspace does not offer llama3.1:8b, so qwen2.5:7b answered.")
        );

        // Offering both with its former model as the default keeps the workspace's own provider, with its notice gone.
        save_workspace(
            vec![setup.former.id.clone(), setup.other.id.clone()],
            Some(setup.former.id.clone()),
            1,
        )
        .await;
        let kept = storage
            .resolve_text_provider(&setup.workspace_id)
            .await
            .unwrap()
            .resolved
            .unwrap();
        assert_eq!((kept.provider.name.as_str(), kept.notice), ("Workspace Ollama", None));

        // A workspace never offers what its organization does not: the organization enables only one model.
        save(&setup, &[&setup.other], Some(&setup.other), "auto").await;
        let narrowed = storage
            .resolve_text_provider(&setup.workspace_id)
            .await
            .unwrap()
            .resolved
            .unwrap();
        assert_eq!(narrowed.provider.model.as_deref(), Some("qwen2.5:7b"));
        assert!(narrowed
            .notice
            .unwrap()
            .starts_with("Your organization does not offer llama3.1:8b"));
        let _ = std::fs::remove_dir_all(&setup.dir);
    }

    #[tokio::test]
    async fn a_workspaces_model_settings_are_exported_by_name_and_only_when_it_saved_some() {
        use crate::workspace_cairn::WorkspaceCairnPolicy;
        let setup = setup(None).await;
        assert!(setup
            .storage
            .export_workspace_data(&setup.workspace_id)
            .await
            .unwrap()
            .cairn
            .is_none());
        setup
            .storage
            .save_workspace_cairn_settings(
                &setup.workspace_id,
                0,
                &WorkspaceCairnPolicy {
                    offered_model_ids: vec![setup.former.id.clone(), setup.other.id.clone()],
                    default_model_id: Some(setup.other.id.clone()),
                    model_roles: Default::default(),
                    guardrails: Default::default(),
                },
                Some(&setup.actor),
            )
            .await
            .unwrap();
        let cairn = setup
            .storage
            .export_workspace_data(&setup.workspace_id)
            .await
            .unwrap()
            .cairn
            .unwrap();
        let names = cairn
            .offered_models
            .iter()
            .map(|model| model.model.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec!["llama3.1:8b", "qwen2.5:7b"],
            "by the name the server knows each by, not by an id of this server"
        );
        assert_eq!(cairn.default_model.unwrap().model, "qwen2.5:7b");
        let written = serde_json::to_string(
            &setup
                .storage
                .export_workspace_data(&setup.workspace_id)
                .await
                .unwrap()
                .cairn,
        )
        .unwrap();
        assert!(
            !written.contains(&setup.former.id) && !written.contains(&setup.other.id),
            "{written}"
        );
        let _ = std::fs::remove_dir_all(&setup.dir);
    }

    #[tokio::test]
    async fn a_model_is_offered_in_a_workspace_only_when_the_organization_the_workspace_and_the_role_allow_it() {
        use crate::workspace_cairn::WorkspaceCairnPolicy;
        let setup = setup(None).await;
        let storage = &setup.storage;
        save(&setup, &[&setup.former, &setup.other], Some(&setup.former), "auto").await;
        let other_workspace = storage
            .create_shared_workspace_with_policy(&setup.actor, &setup.organization_id, "Other", None, false)
            .await
            .unwrap()
            .workspace
            .id;
        // Both workspaces offer both models to everyone, then the other workspace stops offering the first model.
        let offered = |workspace: &str, role: Option<&str>| {
            let (storage, workspace, role) = (storage.clone(), workspace.to_owned(), role.map(str::to_owned));
            async move {
                storage
                    .offered_text_model_ids_for(&workspace, role.as_deref())
                    .await
                    .unwrap()
            }
        };
        assert!(offered(&setup.workspace_id, Some("member"))
            .await
            .contains(&setup.former.id));
        assert!(offered(&other_workspace, Some("member"))
            .await
            .contains(&setup.former.id));
        storage
            .save_workspace_cairn_settings(
                &other_workspace,
                0,
                &WorkspaceCairnPolicy {
                    offered_model_ids: vec![setup.other.id.clone()],
                    default_model_id: None,
                    model_roles: Default::default(),
                    guardrails: Default::default(),
                },
                None,
            )
            .await
            .unwrap();
        assert!(
            !offered(&other_workspace, Some("member"))
                .await
                .contains(&setup.former.id),
            "a passage there never reaches it"
        );
        assert!(offered(&setup.workspace_id, Some("member"))
            .await
            .contains(&setup.former.id));

        // A limit by role in the home workspace's organization: a viewer is not offered the model anywhere.
        let current = storage
            .get_organization_cairn_settings(&setup.organization_id)
            .await
            .unwrap();
        let mut policy = current.policy.clone();
        policy.model_roles.insert(
            setup.former.id.clone(),
            vec!["owner".to_owned(), "admin".to_owned(), "member".to_owned()],
        );
        storage
            .save_organization_cairn_settings(&setup.organization_id, current.version, &policy, &setup.actor)
            .await
            .unwrap();
        assert!(!offered(&setup.workspace_id, Some("viewer"))
            .await
            .contains(&setup.former.id));
        assert!(offered(&setup.workspace_id, Some("member"))
            .await
            .contains(&setup.former.id));
        let _ = std::fs::remove_dir_all(&setup.dir);
    }

    #[tokio::test]
    async fn the_change_limits_are_the_stricter_of_the_organization_and_the_workspace() {
        use crate::organization_cairn::Guardrails;
        use crate::workspace_cairn::WorkspaceCairnPolicy;
        let setup = setup(None).await;
        let storage = &setup.storage;
        let defaults = storage.change_limits_for_workspace(&setup.workspace_id).await.unwrap();
        assert_eq!(
            (defaults.max_changes, defaults.type_to_confirm_from, defaults.set_by),
            (200, None, "default")
        );

        let current = storage
            .get_organization_cairn_settings(&setup.organization_id)
            .await
            .unwrap();
        let mut policy = current.policy.clone();
        policy.guardrails = Guardrails {
            max_changes: Some(50),
            type_to_confirm_from: Some(20),
            ..Default::default()
        };
        storage
            .save_organization_cairn_settings(&setup.organization_id, current.version, &policy, &setup.actor)
            .await
            .unwrap();
        let by_organization = storage.change_limits_for_workspace(&setup.workspace_id).await.unwrap();
        assert_eq!(
            (
                by_organization.max_changes,
                by_organization.type_to_confirm_from,
                by_organization.set_by
            ),
            (50, Some(20), "organization")
        );

        storage
            .save_workspace_cairn_settings(
                &setup.workspace_id,
                0,
                &WorkspaceCairnPolicy {
                    guardrails: Guardrails {
                        max_changes: Some(30),
                        type_to_confirm_from: Some(5),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                None,
            )
            .await
            .unwrap();
        let tightened = storage.change_limits_for_workspace(&setup.workspace_id).await.unwrap();
        assert_eq!(
            (tightened.max_changes, tightened.type_to_confirm_from, tightened.set_by),
            (30, Some(5), "workspace")
        );
        let _ = std::fs::remove_dir_all(&setup.dir);
    }
}
