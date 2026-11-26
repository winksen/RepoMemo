//! Moves every workspace's AI providers into the model catalog (V0.9.78). Each distinct server (the same kind, address and key) becomes
//! one connection, each model a catalog model, and each provider is linked to its model with `cairn_model_id`, so which workspace
//! used which model, and in which CAIRN mode, stays on the provider. Keys are sealed again for their connection and never appear in
//! clear. CAIRN still resolves a workspace's model from its own provider in this release; the later rows read the catalog.
//!
//! It runs at startup and is safe to run again: a provider that is already linked is left alone, and one saved since is linked now.

use std::collections::BTreeMap;

use anyhow::Result;
use repomemo_domain::ProviderSettings;
use repomemo_storage::{CairnConnection, NewCairnConnection, NewCairnModel, StorageEngine};
use serde_json::Value;

/// What a pass moved.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct CatalogMigration {
    pub(crate) connections_made: usize,
    pub(crate) models_made: usize,
    pub(crate) providers_linked: usize,
}

/// A distinct server: the kind, the address as the provider sends it (no trailing slash, any case) and the key, if any.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Server {
    kind: String,
    address: String,
    key: Option<String>,
}

fn address_of(provider: &ProviderSettings) -> String {
    let given = provider
        .base_url
        .as_deref()
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .unwrap_or(match provider.provider_type.as_str() {
            "ollama" => "http://127.0.0.1:11434",
            "openrouter" => "https://openrouter.ai/api/v1",
            _ => "",
        });
    given.trim_end_matches('/').to_owned()
}

fn model_of(provider: &ProviderSettings) -> String {
    let name = if provider.purpose() == "embedding" {
        provider.embedding_model_name()
    } else {
        provider.model.as_deref().unwrap_or_default()
    };
    name.trim().to_owned()
}

fn label(kind: &str) -> &'static str {
    match kind {
        "ollama" => "Ollama",
        "openrouter" => "OpenRouter",
        _ => "OpenAI-compatible server",
    }
}

fn host_of(address: &str) -> String {
    address
        .split("://")
        .nth(1)
        .unwrap_or(address)
        .split('/')
        .next()
        .unwrap_or(address)
        .to_owned()
}

/// Links every provider that is not yet linked to a catalog model, making the connections and models it needs.
pub(crate) async fn run(storage: &StorageEngine) -> Result<CatalogMigration> {
    let mut report = CatalogMigration::default();
    let pending = storage
        .list_all_provider_settings()
        .await?
        .into_iter()
        .filter(|provider| {
            provider
                .metadata
                .get("cairn_model_id")
                .and_then(Value::as_str)
                .is_none()
        })
        .filter(|provider| {
            matches!(
                provider.provider_type.as_str(),
                "ollama" | "openrouter" | "openai_compatible"
            )
        })
        .filter(|provider| !address_of(provider).is_empty())
        .collect::<Vec<_>>();
    if pending.is_empty() {
        return Ok(report);
    }

    // The connections that exist, with their keys, so a server already in the catalog is reused.
    let mut known: Vec<(Server, CairnConnection)> = Vec::new();
    for connection in storage.list_cairn_connections().await? {
        let key = storage.cairn_connection_key(&connection.id).await?;
        known.push((
            Server {
                kind: connection.kind.clone(),
                address: connection.base_url.trim_end_matches('/').to_ascii_lowercase(),
                key,
            },
            connection,
        ));
    }

    // Group the providers by server: a connection is on when any of its providers was, and acknowledged when any was.
    let mut groups: BTreeMap<Server, Vec<ProviderSettings>> = BTreeMap::new();
    for provider in pending {
        let server = Server {
            kind: provider.provider_type.clone(),
            address: address_of(&provider).to_ascii_lowercase(),
            key: provider
                .api_key
                .clone()
                .map(|key| key.trim().to_owned())
                .filter(|key| !key.is_empty()),
        };
        groups.entry(server).or_default().push(provider);
    }

    for (server, providers) in groups {
        let connection = match known.iter().find(|(existing, _)| *existing == server) {
            Some((_, connection)) => connection.clone(),
            None => {
                let address = address_of(&providers[0]);
                let cloud = repomemo_ai::is_cloud_connection(&server.kind, Some(&address));
                let acknowledged = providers.iter().any(|provider| {
                    provider
                        .metadata
                        .get("cloud_content_acknowledged")
                        .and_then(Value::as_bool)
                        == Some(true)
                });
                let made = storage
                    .import_cairn_connection(
                        NewCairnConnection {
                            kind: server.kind.clone(),
                            name: format!("{} · {}", label(&server.kind), host_of(&address)),
                            base_url: address,
                            api_key: server.key.clone(),
                            locality: if cloud { "cloud" } else { "local" }.to_owned(),
                            enabled: providers.iter().any(|provider| provider.enabled),
                        },
                        cloud && acknowledged,
                    )
                    .await?;
                report.connections_made += 1;
                known.push((
                    Server {
                        kind: server.kind.clone(),
                        address: server.address.clone(),
                        key: server.key.clone(),
                    },
                    made.clone(),
                ));
                made
            }
        };
        for provider in providers {
            let model_name = model_of(&provider);
            if model_name.is_empty() {
                continue;
            }
            let purpose = provider.purpose().to_owned();
            let existing = storage
                .list_cairn_models(Some(&connection.id))
                .await?
                .into_iter()
                .find(|model| model.model_id == model_name && model.purpose == purpose);
            let model = match existing {
                Some(model) => model,
                None => {
                    let traits = repomemo_ai::ModelTraits::from_metadata(&provider.metadata);
                    let made = storage
                        .import_cairn_model(NewCairnModel {
                            connection_id: connection.id.clone(),
                            model_id: model_name.clone(),
                            display_name: model_name.chars().take(repomemo_storage::CATALOG_NAME_CHARS).collect(),
                            vision: purpose == "vision",
                            purpose,
                            tool_mode: if provider.purpose() == "text" {
                                traits.tool_mode.as_str().to_owned()
                            } else {
                                "none".to_owned()
                            },
                            context_tokens: if provider.purpose() == "text" {
                                i64::from(traits.context_tokens)
                            } else {
                                0
                            },
                            evaluation: None,
                            fallback_model_id: None,
                            roles: None,
                            enabled: true,
                        })
                        .await?;
                    report.models_made += 1;
                    made
                }
            };
            let mut linked = provider;
            if let Some(object) = linked.metadata.as_object_mut() {
                object.insert("cairn_model_id".to_owned(), Value::String(model.id));
            }
            storage.save_provider_settings(linked).await?;
            report.providers_linked += 1;
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    async fn storage() -> (StorageEngine, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("repomemo-catalog-migration-{}", uuid::Uuid::new_v4()));
        let storage = StorageEngine::open(repomemo_storage::StorageConfig::new(dir.clone()))
            .await
            .unwrap();
        (storage, dir)
    }

    fn provider(
        workspace_id: &str,
        kind: &str,
        url: Option<&str>,
        key: Option<&str>,
        model: &str,
        extra: Value,
    ) -> ProviderSettings {
        let mut metadata = json!({ "cloud_content_acknowledged": kind == "openrouter" });
        if let (Some(target), Some(extra)) = (metadata.as_object_mut(), extra.as_object()) {
            target.extend(extra.clone());
        }
        ProviderSettings {
            id: String::new(),
            workspace_id: Some(workspace_id.to_owned()),
            provider_type: kind.to_owned(),
            name: format!("{kind} for {workspace_id}"),
            base_url: url.map(str::to_owned),
            model: Some(model.to_owned()),
            embedding_model: None,
            enabled: true,
            metadata,
            api_key: key.map(str::to_owned),
        }
    }

    #[tokio::test]
    async fn providers_become_one_connection_per_server_and_key_and_one_model_each() {
        let (storage, dir) = storage().await;
        let mut workspaces = Vec::new();
        for name in ["One", "Two", "Three", "Four"] {
            workspaces.push(storage.create_workspace(name).await.unwrap().id);
        }
        let secret = "sk-or-moved-secret";
        for settings in [
            // Two workspaces share a server, a key and a model.
            provider(
                &workspaces[0],
                "openrouter",
                None,
                Some(secret),
                "openai/gpt-4o-mini",
                json!({"cairn_mode": "agent", "tool_mode": "native", "context_tokens": 128000}),
            ),
            provider(
                &workspaces[1],
                "openrouter",
                Some("https://openrouter.ai/api/v1/"),
                Some(secret),
                "openai/gpt-4o-mini",
                json!({}),
            ),
            // The same server with another key is another connection; a local server needs none.
            provider(
                &workspaces[2],
                "openrouter",
                None,
                Some("sk-or-another-key"),
                "openai/gpt-4o-mini",
                json!({}),
            ),
            provider(
                &workspaces[3],
                "ollama",
                None,
                None,
                "llama3.1:8b",
                json!({"cairn_mode": "jobs"}),
            ),
        ] {
            storage.save_provider_settings(settings).await.unwrap();
        }
        // A workspace's embedding provider names its model apart.
        let mut embedding = provider(
            &workspaces[3],
            "ollama",
            None,
            None,
            "llama3.1:8b",
            json!({"purpose": "embedding"}),
        );
        embedding.embedding_model = Some("nomic-embed-text".to_owned());
        storage.save_provider_settings(embedding).await.unwrap();

        let report = run(&storage).await.unwrap();
        assert_eq!(
            report,
            CatalogMigration {
                connections_made: 3,
                models_made: 4,
                providers_linked: 5
            }
        );

        // One connection per distinct server and key; the cloud one that was acknowledged stays so.
        let connections = storage.list_cairn_connections().await.unwrap();
        assert_eq!(connections.len(), 3);
        let cloud = connections
            .iter()
            .filter(|connection| connection.locality == "cloud")
            .collect::<Vec<_>>();
        assert_eq!(cloud.len(), 2);
        assert!(cloud
            .iter()
            .all(|connection| connection.has_key && connection.cloud_acknowledged_at.is_some()));
        assert!(connections
            .iter()
            .any(|connection| connection.kind == "ollama" && connection.locality == "local" && !connection.has_key));

        // Every provider names its model, and the mode it had is kept on it.
        let providers = storage.list_all_provider_settings().await.unwrap();
        assert!(providers
            .iter()
            .all(|provider| provider.metadata.get("cairn_model_id").is_some()));
        let first = providers
            .iter()
            .find(|provider| provider.workspace_id.as_deref() == Some(workspaces[0].as_str()))
            .unwrap();
        assert_eq!(first.metadata["cairn_mode"], "agent");
        assert_eq!(
            first.api_key.as_deref(),
            Some(secret),
            "the workspace's own provider still works"
        );
        let model = storage
            .get_cairn_model(first.metadata["cairn_model_id"].as_str().unwrap())
            .await
            .unwrap();
        assert_eq!(
            (model.model_id.as_str(), model.tool_mode.as_str(), model.context_tokens),
            ("openai/gpt-4o-mini", "native", 128_000)
        );
        let shared = providers
            .iter()
            .filter(|provider| provider.metadata["cairn_model_id"] == first.metadata["cairn_model_id"])
            .count();
        assert_eq!(shared, 2, "two workspaces use one catalog model");
        assert!(storage.cairn_model_uses(&model.id).await.unwrap() >= 2);

        // No key is in clear in the catalog.
        let stored = sqlx_dump(&storage).await;
        assert!(
            !stored.contains(secret) && !stored.contains("sk-or-another-key"),
            "{stored}"
        );

        // Running again moves nothing.
        assert_eq!(run(&storage).await.unwrap(), CatalogMigration::default());
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Everything the catalog stores, as text.
    async fn sqlx_dump(storage: &StorageEngine) -> String {
        let mut text = serde_json::to_string(&storage.list_cairn_connections().await.unwrap()).unwrap();
        text.push_str(&serde_json::to_string(&storage.list_cairn_models(None).await.unwrap()).unwrap());
        text
    }

    #[tokio::test]
    async fn a_server_already_in_the_catalog_is_reused() {
        let (storage, dir) = storage().await;
        let workspace = storage.create_workspace("One").await.unwrap().id;
        storage
            .import_cairn_connection(
                NewCairnConnection {
                    kind: "ollama".to_owned(),
                    name: "Made by hand".to_owned(),
                    base_url: "http://127.0.0.1:11434".to_owned(),
                    api_key: None,
                    locality: "local".to_owned(),
                    enabled: true,
                },
                false,
            )
            .await
            .unwrap();
        storage
            .save_provider_settings(provider(&workspace, "ollama", None, None, "llama3.1:8b", json!({})))
            .await
            .unwrap();
        let report = run(&storage).await.unwrap();
        assert_eq!(
            report,
            CatalogMigration {
                connections_made: 0,
                models_made: 1,
                providers_linked: 1
            }
        );
        assert_eq!(storage.list_cairn_connections().await.unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }
}
