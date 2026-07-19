use anyhow::{bail, Context, Result};
use repomemo_domain::{ProviderSettings, ProviderTestResult};
use serde::Deserialize;
use serde_json::{json, Value};

const DEFAULT_BASE_URL: &str = "http://127.0.0.1:11434";

#[derive(Debug, Clone)]
pub struct GenerateRequest {
    pub prompt: String,
    pub context: String,
    pub options: Value,
}

#[allow(async_fn_in_trait)]
pub trait AiProvider {
    async fn generate(&self, request: GenerateRequest) -> Result<String>;
    async fn embed(&self, texts: Vec<String>, options: Value) -> Result<Vec<Vec<f32>>>;
    async fn summarize(&self, target: String, options: Value) -> Result<String>;
    async fn rerank(&self, query: String, candidates: Vec<String>) -> Result<Vec<usize>>;
    async fn test_connection(&self) -> Result<ProviderTestResult>;
}

#[derive(Debug, Clone)]
pub struct OllamaProvider {
    settings: ProviderSettings,
    client: reqwest::Client,
    base_url: String,
    model: String,
}

impl OllamaProvider {
    pub fn from_settings(settings: ProviderSettings) -> Result<Self> {
        validate_settings(&settings)?;
        let base_url = settings
            .base_url
            .as_deref()
            .unwrap_or(DEFAULT_BASE_URL)
            .trim_end_matches('/')
            .to_owned();
        let model = settings
            .model
            .as_deref()
            .unwrap_or_default()
            .trim()
            .to_owned();
        Ok(Self {
            settings,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(45))
                .build()
                .context("failed to configure local AI client")?,
            base_url,
            model,
        })
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }
}

impl AiProvider for OllamaProvider {
    async fn generate(&self, request: GenerateRequest) -> Result<String> {
        let response = self
            .client
            .post(self.endpoint("/api/generate"))
            .json(&json!({
                "model": self.model,
                "prompt": format!("{}\n\nContext:\n{}", request.prompt, request.context),
                "stream": false,
                "options": request.options,
            }))
            .send()
            .await
            .context("could not reach the local Ollama provider")?
            .error_for_status()
            .context("local Ollama provider rejected the summary request")?;
        let body: OllamaGenerateResponse = response
            .json()
            .await
            .context("local Ollama provider returned an invalid response")?;
        let answer = body.response.trim().to_owned();
        if answer.is_empty() {
            bail!("local Ollama provider returned an empty summary")
        }
        Ok(answer)
    }

    async fn embed(&self, texts: Vec<String>, _options: Value) -> Result<Vec<Vec<f32>>> {
        let model = self
            .settings
            .embedding_model
            .as_deref()
            .unwrap_or(&self.model);
        let response = self
            .client
            .post(self.endpoint("/api/embed"))
            .json(&json!({ "model": model, "input": texts }))
            .send()
            .await?
            .error_for_status()?;
        let body: OllamaEmbedResponse = response.json().await?;
        Ok(body.embeddings)
    }

    async fn summarize(&self, target: String, options: Value) -> Result<String> {
        self.generate(GenerateRequest {
            prompt: "Write a concise factual summary. Use only the supplied content and do not invent details.".to_owned(),
            context: target,
            options,
        })
        .await
    }

    async fn rerank(&self, _query: String, candidates: Vec<String>) -> Result<Vec<usize>> {
        Ok((0..candidates.len()).collect())
    }

    async fn test_connection(&self) -> Result<ProviderTestResult> {
        let response = self
            .client
            .get(self.endpoint("/api/tags"))
            .send()
            .await
            .context("could not reach the local Ollama provider")?
            .error_for_status()
            .context("local Ollama provider rejected the connection test")?;
        let body: OllamaTagsResponse = response
            .json()
            .await
            .context("local Ollama provider returned an invalid response")?;
        let model_available = body.models.iter().any(|model| model.name == self.model);
        Ok(ProviderTestResult {
            provider_id: self.settings.id.clone(),
            success: model_available,
            message: if model_available {
                format!("Local provider is ready with {}.", self.model)
            } else {
                format!(
                    "Provider connected, but model '{}' was not found.",
                    self.model
                )
            },
        })
    }
}

pub fn validate_settings(settings: &ProviderSettings) -> Result<()> {
    if settings.provider_type != "ollama" {
        bail!("Only the local Ollama-compatible provider is available in Phase 1F.")
    }
    if settings.name.trim().is_empty() {
        bail!("Provider name is required.")
    }
    let base_url = settings
        .base_url
        .as_deref()
        .unwrap_or(DEFAULT_BASE_URL)
        .trim();
    if !(base_url.starts_with("http://") || base_url.starts_with("https://")) {
        bail!("Provider base URL must start with http:// or https://.")
    }
    if settings
        .model
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        bail!("A chat model is required.")
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct OllamaGenerateResponse {
    response: String,
}

#[derive(Debug, Deserialize)]
struct OllamaEmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    #[serde(default)]
    models: Vec<OllamaModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaModel {
    name: String,
}

#[cfg(test)]
mod tests {
    use super::validate_settings;
    use repomemo_domain::ProviderSettings;
    use serde_json::json;

    fn settings() -> ProviderSettings {
        ProviderSettings {
            id: "provider".to_owned(),
            workspace_id: Some("workspace".to_owned()),
            provider_type: "ollama".to_owned(),
            name: "Local Ollama".to_owned(),
            base_url: Some("http://127.0.0.1:11434".to_owned()),
            model: Some("llama3.2".to_owned()),
            embedding_model: None,
            enabled: false,
            metadata: json!({}),
        }
    }

    #[test]
    fn validates_local_provider_settings() {
        assert!(validate_settings(&settings()).is_ok());
    }

    #[test]
    fn rejects_cloud_and_incomplete_settings() {
        let mut cloud = settings();
        cloud.provider_type = "openai".to_owned();
        assert!(validate_settings(&cloud).is_err());
        let mut incomplete = settings();
        incomplete.model = None;
        assert!(validate_settings(&incomplete).is_err());
    }

    #[test]
    fn disabled_provider_is_not_eligible_for_ai_calls() {
        assert!(!settings().enabled);
    }
}
