//! A provider for any server that speaks the OpenAI API: OpenAI, Mistral, Groq, Together, vLLM,
//! LM Studio, the llama.cpp server and others. The base URL, an optional key and the model are
//! all it needs. Text goes through `/chat/completions` and vectors through `/embeddings`.

use std::time::Duration;

use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use repomemo_domain::{ProviderSettings, ProviderTestResult};
use serde_json::{json, Value};

use crate::{
    checked, ensure_embeddings, error_message, is_cloud_endpoint, llm_rerank, provider_client, read_json,
    unreachable_result, validate_settings, AiProvider, ChatRequest, ChatResponse, GenerateRequest,
    ImageAnalysisRequest,
};
use crate::{openrouter_chat_body, openrouter_chat_response};

/// The path OpenAI-compatible servers put their API under.
pub(crate) const OPENAI_COMPATIBLE_PATH: &str = "/v1";

#[derive(Debug, Clone)]
pub struct OpenAiCompatibleProvider {
    settings: ProviderSettings,
    client: reqwest::Client,
    base_url: String,
    model: String,
    api_key: Option<String>,
}

impl OpenAiCompatibleProvider {
    pub fn from_settings(settings: ProviderSettings) -> Result<Self> {
        validate_settings(&settings)?;
        let api_key = settings
            .api_key
            .as_deref()
            .map(str::trim)
            .filter(|key| !key.is_empty())
            .map(str::to_owned);
        Ok(Self {
            base_url: settings
                .base_url
                .as_deref()
                .unwrap_or_default()
                .trim()
                .trim_end_matches('/')
                .to_owned(),
            model: settings.model.as_deref().unwrap_or_default().trim().to_owned(),
            api_key,
            client: provider_client(Duration::from_secs(90)).context("failed to configure the AI client")?,
            settings,
        })
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let request = self.client.request(method, format!("{}{}", self.base_url, path));
        match &self.api_key {
            Some(key) => request.bearer_auth(key),
            None => request,
        }
    }

    /// What this endpoint is called to a person: it can be a machine of their own.
    fn label(&self) -> &'static str {
        "the OpenAI-compatible provider"
    }

    async fn completion(&self, body: Value, action: &str) -> Result<Value> {
        let response = self
            .request(reqwest::Method::POST, "/chat/completions")
            .json(&body)
            .send()
            .await
            .with_context(|| format!("could not reach {}", self.label()))?;
        let response = checked(response, action).await?;
        let body: Value = read_json(response, "the OpenAI-compatible provider returned an invalid response").await?;
        if let Some(message) = error_message(&body) {
            tracing::warn!(detail = %message, "the OpenAI-compatible provider could not complete the request");
            bail!("The OpenAI-compatible provider could not complete the request. The server log has the provider's message.")
        }
        Ok(body)
    }
}

/// One vector per input row, in the order of the rows' `index`.
pub(crate) fn parse_embeddings(body: &Value) -> Result<Vec<Vec<f32>>> {
    let mut rows = body["data"]
        .as_array()
        .context("the provider returned no embeddings")?
        .iter()
        .map(|row| {
            let index = row["index"].as_u64().unwrap_or(0) as usize;
            let vector = row["embedding"]
                .as_array()
                .map(|values| {
                    values
                        .iter()
                        .filter_map(Value::as_f64)
                        .map(|value| value as f32)
                        .collect()
                })
                .unwrap_or_default();
            (index, vector)
        })
        .collect::<Vec<(usize, Vec<f32>)>>();
    rows.sort_by_key(|(index, _)| *index);
    Ok(rows.into_iter().map(|(_, vector)| vector).collect())
}

/// The model's window when the server lists one: OpenAI-style lists rarely do, vLLM calls it
/// `max_model_len`, others `context_length` or `context_window`.
pub(crate) fn context_length(models: &Value, model: &str) -> Option<u32> {
    let entry = models["data"]
        .as_array()?
        .iter()
        .find(|entry| entry["id"].as_str() == Some(model))?;
    [
        "context_length",
        "max_model_len",
        "context_window",
        "max_context_length",
    ]
    .iter()
    .find_map(|key| entry[*key].as_u64())
    .map(|tokens| tokens.min(u64::from(u32::MAX)) as u32)
}

impl AiProvider for OpenAiCompatibleProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        let body = self
            .completion(
                openrouter_chat_body(&self.model, &request),
                "The OpenAI-compatible provider rejected the request",
            )
            .await?;
        Ok(openrouter_chat_response(&body))
    }

    async fn generate(&self, request: GenerateRequest) -> Result<String> {
        let answer = self.chat(request.to_chat()).await?.content;
        if answer.is_empty() {
            bail!("The OpenAI-compatible provider returned an empty answer")
        }
        Ok(answer)
    }

    async fn context_tokens(&self) -> Result<Option<u32>> {
        let response = self
            .request(reqwest::Method::GET, "/models")
            .send()
            .await
            .context("could not reach the OpenAI-compatible provider")?;
        let models: Value = read_json(
            checked(response, "The OpenAI-compatible provider could not list its models").await?,
            "The OpenAI-compatible provider returned an invalid model list",
        )
        .await?;
        Ok(context_length(&models, &self.model))
    }

    async fn embed(&self, texts: Vec<String>, _options: Value) -> Result<Vec<Vec<f32>>> {
        let model = self.settings.embedding_model.as_deref().unwrap_or(&self.model);
        let count = texts.len();
        let response = self
            .request(reqwest::Method::POST, "/embeddings")
            .json(&json!({ "model": model, "input": texts }))
            .send()
            .await
            .context("could not reach the OpenAI-compatible provider")?;
        let body: Value = read_json(
            checked(response, "The OpenAI-compatible provider could not build embeddings").await?,
            "The OpenAI-compatible provider returned invalid embeddings",
        )
        .await?;
        if let Some(message) = error_message(&body) {
            tracing::warn!(detail = %message, "the OpenAI-compatible provider could not build embeddings");
            bail!(
                "The OpenAI-compatible provider could not build embeddings. The server log has the provider's message."
            )
        }
        ensure_embeddings(parse_embeddings(&body)?, count)
    }

    async fn summarize(&self, target: String, options: Value) -> Result<String> {
        self.generate(GenerateRequest {
            system: None,
            prompt: "Write a concise factual summary using only the supplied content.".to_owned(),
            context: target,
            options,
        })
        .await
    }

    async fn rerank(&self, query: String, candidates: Vec<String>) -> Result<Vec<usize>> {
        llm_rerank(self, &query, &candidates).await
    }

    async fn analyze_image(&self, request: ImageAnalysisRequest) -> Result<String> {
        let image_url = format!(
            "data:{};base64,{}",
            request.mime_type,
            STANDARD.encode(request.image_bytes)
        );
        let body = self
            .completion(
                json!({
                    "model": self.model,
                    "messages": [
                      { "role": "system", "content": "Describe repository images faithfully. Do not invent details." },
                      { "role": "user", "content": [
                        { "type": "text", "text": request.prompt },
                        { "type": "image_url", "image_url": { "url": image_url } }
                      ] }
                    ],
                    "temperature": 0.1,
                }),
                "The configured model could not analyze this image; choose a vision-capable model",
            )
            .await?;
        let answer = openrouter_chat_response(&body).content;
        if answer.is_empty() {
            bail!("The OpenAI-compatible provider returned an empty image description")
        }
        Ok(answer)
    }

    /// Checks the key and the model id where the server lists models, then a tiny completion,
    /// because a model list can answer when every real request would fail.
    async fn test_connection(&self) -> Result<ProviderTestResult> {
        let result = |success: bool, message: String| ProviderTestResult {
            provider_id: self.settings.id.clone(),
            success,
            message,
        };
        let listed = match self.request(reqwest::Method::GET, "/models").send().await {
            Ok(response) => response,
            Err(error) => return Ok(unreachable_result(&self.settings.id, &error)),
        };
        if matches!(listed.status().as_u16(), 401 | 403) {
            return Ok(result(
                false,
                "The provider rejected the API key. Enter a valid key and save the provider again.".to_owned(),
            ));
        }
        // Not every server lists its models; one that does must list this one.
        if listed.status().is_success() {
            if let Ok(models) = read_json::<Value>(listed, "invalid model list").await {
                let ids = models["data"].as_array().map(|entries| {
                    entries
                        .iter()
                        .filter_map(|entry| entry["id"].as_str())
                        .collect::<Vec<_>>()
                });
                if let Some(ids) = ids.filter(|ids| !ids.is_empty() && !ids.contains(&self.model.as_str())) {
                    return Ok(result(
                        false,
                        format!(
                            "The provider does not serve a model with the id '{}'. It lists {} model(s), for example '{}'.",
                            self.model,
                            ids.len(),
                            ids[0]
                        ),
                    ));
                }
            }
        }
        let probe = self
            .completion(
                json!({
                    "model": self.model,
                    "messages": [{ "role": "user", "content": "Reply with OK." }],
                    "max_tokens": 16,
                }),
                "The provider rejected a test request",
            )
            .await;
        if let Err(error) = probe {
            return Ok(result(false, error.to_string()));
        }
        let leaves = if is_cloud_endpoint(&self.settings.provider_type, self.settings.base_url.as_deref()) {
            " Workspace content will leave this device only when you request an AI action."
        } else {
            ""
        };
        Ok(result(
            true,
            format!("The provider is ready with {}.{leaves}", self.model),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{context_length, parse_embeddings};
    use serde_json::json;

    #[test]
    fn embeddings_come_back_in_input_order() {
        let body = json!({ "data": [
            { "index": 1, "embedding": [0.3, 0.4] },
            { "index": 0, "embedding": [0.1, 0.2] }
        ] });
        let vectors = parse_embeddings(&body).unwrap();
        assert_eq!(vectors, vec![vec![0.1, 0.2], vec![0.3, 0.4]]);
        assert!(parse_embeddings(&json!({})).is_err());
    }

    #[test]
    fn the_context_window_is_read_under_the_names_servers_use() {
        let models = json!({ "data": [
            { "id": "vllm-model", "max_model_len": 32768 },
            { "id": "other", "context_length": 16000 },
            { "id": "plain" }
        ] });
        assert_eq!(context_length(&models, "vllm-model"), Some(32_768));
        assert_eq!(context_length(&models, "other"), Some(16_000));
        assert_eq!(context_length(&models, "plain"), None);
        assert_eq!(context_length(&models, "missing"), None);
    }
}
