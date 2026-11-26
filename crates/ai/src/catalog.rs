//! Connections of the model catalog (V0.9.76): reaching a model server without naming a model. The same endpoint policy as a
//! workspace's provider applies (allowed hosts, no redirects, no metadata services), and the key is only ever sent to the address
//! it was entered for. A report says whether the server answered and whether it took the key; it never carries the key or the
//! server's own words.

use anyhow::{bail, Result};
use serde::Serialize;
use serde_json::Value;
use std::time::Duration;

use crate::{
    check_provider_url, detect_model_traits, is_cloud_endpoint, provider_client, provider_from_settings, read_json,
    AiProvider, DEFAULT_BASE_URL, OPENROUTER_BASE_URL, UNREACHABLE,
};

/// What a connection test found.
#[derive(Debug, Clone, Serialize)]
pub struct ConnectionReport {
    /// The server answered.
    pub reachable: bool,
    /// Whether the server took the key; `None` when there is no key to try or the server was not reached.
    pub key_accepted: Option<bool>,
    /// How many models the server lists, when it lists them.
    pub model_count: Option<usize>,
    pub message: String,
}

/// The address requests go to, with the default of the kind filled in and the endpoint policy checked. Fails with a message that
/// says what is wrong with the kind or the address.
pub fn connection_base_url(kind: &str, base_url: Option<&str>) -> Result<String> {
    let (default_url, provider_path) = match kind {
        "ollama" => (DEFAULT_BASE_URL, ""),
        "openrouter" => (OPENROUTER_BASE_URL, "/api/v1"),
        "openai_compatible" => ("", crate::openai::OPENAI_COMPATIBLE_PATH),
        _ => bail!("A connection is an Ollama, OpenRouter or OpenAI-compatible server."),
    };
    let base_url = base_url
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .unwrap_or(default_url)
        .trim_end_matches('/')
        .to_owned();
    if base_url.is_empty() {
        bail!("A base URL is required, for example https://api.example.com/v1.")
    }
    if !(base_url.starts_with("http://") || base_url.starts_with("https://")) {
        bail!("The base URL must start with http:// or https://.")
    }
    check_provider_url(&base_url, provider_path)?;
    Ok(base_url)
}

/// Whether what CAIRN sends to this connection leaves the device and the private network: always for OpenRouter, and for an
/// OpenAI-compatible server by its host. An Ollama server counts as local, as the endpoint policy keeps it so.
pub fn is_cloud_connection(kind: &str, base_url: Option<&str>) -> bool {
    is_cloud_endpoint(kind, base_url)
}

/// The identifiers of the models a server lists, or `None` for one that does not list them.
async fn listed_models(
    client: &reqwest::Client,
    kind: &str,
    base_url: &str,
    key: Option<&str>,
) -> Result<(u16, Option<Vec<String>>)> {
    let path = if kind == "ollama" { "/api/tags" } else { "/models" };
    let mut request = client.get(format!("{base_url}{path}"));
    if let Some(key) = key {
        request = request.bearer_auth(key);
    }
    let response = request.send().await?;
    let status = response.status().as_u16();
    if !response.status().is_success() {
        return Ok((status, None));
    }
    let body: Value = read_json(response, "invalid model list").await?;
    let ids = if kind == "ollama" {
        body["models"].as_array().map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry["name"].as_str().map(str::to_owned))
                .collect::<Vec<_>>()
        })
    } else {
        body["data"].as_array().map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry["id"].as_str().map(str::to_owned))
                .collect::<Vec<_>>()
        })
    };
    Ok((status, ids))
}

/// Reaches a server and, with a key, tries it. A server that cannot be reached is a report, not an error.
pub async fn probe_connection(kind: &str, base_url: Option<&str>, api_key: Option<&str>) -> Result<ConnectionReport> {
    let base_url = connection_base_url(kind, base_url)?;
    let key = api_key.map(str::trim).filter(|key| !key.is_empty());
    let client = provider_client(Duration::from_secs(20))?;
    let unreachable = |error: &anyhow::Error| {
        tracing::warn!(error = %error, "A catalog connection test got no answer");
        ConnectionReport {
            reachable: false,
            key_accepted: None,
            model_count: None,
            message: UNREACHABLE.to_owned(),
        }
    };
    let (status, ids) = match listed_models(&client, kind, &base_url, key).await {
        Ok(listed) => listed,
        Err(error) => return Ok(unreachable(&error)),
    };
    if matches!(status, 401 | 403) {
        return Ok(ConnectionReport {
            reachable: true,
            key_accepted: Some(false),
            model_count: None,
            message: "The server rejected the key. Enter a valid key and save the connection again.".to_owned(),
        });
    }
    if !(200..300).contains(&status) {
        return Ok(ConnectionReport {
            reachable: true,
            key_accepted: None,
            model_count: None,
            message: format!(
                "The server answered with status {status}. Check the address; a redirect is not followed, so enter the final address."
            ),
        });
    }
    let model_count = ids.as_ref().map(Vec::len);
    let listed = match model_count {
        Some(count) => format!(" It lists {count} model{}.", if count == 1 { "" } else { "s" }),
        None => " It does not list its models.".to_owned(),
    };
    Ok(ConnectionReport {
        reachable: true,
        key_accepted: key.map(|_| true),
        model_count,
        message: format!(
            "The server answered{}.{listed}",
            if key.is_some() { " and took the key" } else { "" }
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_connection_takes_the_default_address_of_its_kind_and_refuses_what_a_provider_refuses() {
        assert_eq!(connection_base_url("ollama", None).unwrap(), "http://127.0.0.1:11434");
        assert_eq!(
            connection_base_url("openrouter", Some("")).unwrap(),
            "https://openrouter.ai/api/v1"
        );
        assert!(
            connection_base_url("openai_compatible", None).is_err(),
            "no default address"
        );
        assert!(
            connection_base_url("anthropic", None).is_err(),
            "not a kind of connection"
        );
        assert!(connection_base_url("ollama", Some("ftp://127.0.0.1:11434")).is_err());
        assert!(connection_base_url("ollama", Some("http://user:pass@127.0.0.1:11434")).is_err());
        assert!(connection_base_url("openai_compatible", Some("http://169.254.169.254/v1")).is_err());
        assert!(
            connection_base_url("openai_compatible", Some("https://elsewhere.example/v1")).is_err(),
            "outside the allowed hosts"
        );
    }

    #[test]
    fn locality_follows_the_kind_and_the_host() {
        assert!(is_cloud_connection("openrouter", None));
        assert!(!is_cloud_connection("ollama", Some("http://127.0.0.1:11434")));
        assert!(!is_cloud_connection(
            "openai_compatible",
            Some("http://192.168.1.4:8000/v1")
        ));
        assert!(is_cloud_connection(
            "openai_compatible",
            Some("https://api.example.com/v1")
        ));
    }
}

/// The models a server offers, by the name it knows them by (V0.9.77). A server that does not list its models, or cannot be reached,
/// is an error with a message that says so.
pub async fn list_connection_models(kind: &str, base_url: Option<&str>, api_key: Option<&str>) -> Result<Vec<String>> {
    let base_url = connection_base_url(kind, base_url)?;
    let key = api_key.map(str::trim).filter(|key| !key.is_empty());
    let client = provider_client(Duration::from_secs(20))?;
    let (status, ids) = match listed_models(&client, kind, &base_url, key).await {
        Ok(listed) => listed,
        Err(error) => {
            tracing::warn!(error = %error, "A catalog connection could not list its models");
            bail!("{UNREACHABLE}")
        }
    };
    if matches!(status, 401 | 403) {
        bail!("The server rejected the key.")
    }
    let mut ids =
        ids.ok_or_else(|| anyhow::anyhow!("The server does not list its models; add one by the name it knows it by."))?;
    ids.sort();
    ids.dedup();
    Ok(ids)
}

/// What testing a model found (V0.9.77).
#[derive(Debug, Clone, Serialize)]
pub struct ModelReport {
    pub success: bool,
    pub message: String,
    /// `none`, `json` or `native`, for a text model that answered.
    pub tool_mode: Option<String>,
    pub context_tokens: Option<u32>,
}

/// Tries one model of a connection as a workspace's test of a provider does: a small request, and for a text model the tool mode and
/// context size it takes, which fill the catalog entry and which an administrator may correct.
pub async fn test_catalog_model(
    kind: &str,
    base_url: Option<&str>,
    api_key: Option<&str>,
    model: &str,
    purpose: &str,
) -> Result<ModelReport> {
    let base_url = connection_base_url(kind, base_url)?;
    let settings = repomemo_domain::ProviderSettings {
        id: String::new(),
        workspace_id: None,
        provider_type: kind.to_owned(),
        name: "Catalog test".to_owned(),
        base_url: Some(base_url),
        model: Some(model.trim().to_owned()),
        embedding_model: (purpose == "embedding").then(|| model.trim().to_owned()),
        // Off, so the checks that only matter to a provider a workspace turns on (the acknowledgement) do not apply to a test.
        enabled: false,
        metadata: serde_json::json!({ "purpose": purpose }),
        api_key: api_key.map(str::trim).filter(|key| !key.is_empty()).map(str::to_owned),
    };
    let provider = provider_from_settings(settings)?;
    if purpose == "embedding" {
        return Ok(
            match provider
                .embed(vec!["RepoMemo model test".to_owned()], serde_json::json!({}))
                .await
            {
                Ok(vectors) => ModelReport {
                    success: true,
                    message: format!(
                        "Embeddings work with {model} ({} dimensions).",
                        vectors.first().map_or(0, Vec::len)
                    ),
                    tool_mode: None,
                    context_tokens: None,
                },
                Err(error) => {
                    tracing::warn!(error = %error, "A catalog embedding model test failed");
                    ModelReport {
                        success: false,
                        message:
                            "The model could not build embeddings. Check its name and that it is an embedding model."
                                .to_owned(),
                        tool_mode: None,
                        context_tokens: None,
                    }
                }
            },
        );
    }
    let result = provider.test_connection().await?;
    if !result.success || purpose != "text" {
        return Ok(ModelReport {
            success: result.success,
            message: result.message,
            tool_mode: None,
            context_tokens: None,
        });
    }
    let traits = detect_model_traits(&provider).await;
    Ok(ModelReport {
        success: true,
        message: format!(
            "{} Tools: {} and a context of {} tokens.",
            result.message,
            traits.tool_mode.as_str(),
            traits.context_tokens
        ),
        tool_mode: Some(traits.tool_mode.as_str().to_owned()),
        context_tokens: Some(traits.context_tokens),
    })
}
