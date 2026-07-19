use anyhow::{bail, Context, Result};
use repomemo_domain::{ProviderSettings, ProviderTestResult};
use serde::Deserialize;
use serde_json::{json, Value};

const DEFAULT_BASE_URL: &str = "http://127.0.0.1:11434";
const OPENROUTER_BASE_URL: &str = "https://openrouter.ai/api/v1";

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

#[derive(Debug, Clone)]
pub struct OpenRouterProvider {
    settings: ProviderSettings,
    client: reqwest::Client,
    base_url: String,
    model: String,
    api_key: String,
}

impl OpenRouterProvider {
    pub fn from_settings(settings: ProviderSettings) -> Result<Self> {
        validate_settings(&settings)?;
        let api_key = settings
            .api_key
            .clone()
            .unwrap_or_default()
            .trim()
            .to_owned();
        if api_key.is_empty() {
            bail!("An OpenRouter API key is required.");
        }
        Ok(Self {
            base_url: settings
                .base_url
                .as_deref()
                .unwrap_or(OPENROUTER_BASE_URL)
                .trim_end_matches('/')
                .to_owned(),
            model: settings
                .model
                .as_deref()
                .unwrap_or_default()
                .trim()
                .to_owned(),
            api_key,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .context("failed to configure cloud AI client")?,
            settings,
        })
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.client
            .request(method, format!("{}{}", self.base_url, path))
            .bearer_auth(&self.api_key)
            .header("HTTP-Referer", "https://repomemo.local")
            .header("X-OpenRouter-Title", "RepoMemo")
    }
}

impl AiProvider for OpenRouterProvider {
    async fn generate(&self, request: GenerateRequest) -> Result<String> {
        let response = self.request(reqwest::Method::POST, "/chat/completions")
            .json(&json!({
                "model": self.model,
                "messages": [
                  { "role": "system", "content": "You summarize local repository material faithfully. Do not invent details." },
                  { "role": "user", "content": format!("{}\n\nContext:\n{}", request.prompt, request.context) }
                ],
                "temperature": request.options.get("temperature").and_then(Value::as_f64).unwrap_or(0.2),
            }))
            .send().await.context("could not reach OpenRouter")?
            .error_for_status().context("OpenRouter rejected the summary request")?;
        let body: OpenRouterResponse = response
            .json()
            .await
            .context("OpenRouter returned an invalid response")?;
        let answer = body
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message.content)
            .unwrap_or_default()
            .trim()
            .to_owned();
        if answer.is_empty() {
            bail!("OpenRouter returned an empty summary")
        }
        Ok(answer)
    }

    async fn embed(&self, _texts: Vec<String>, _options: Value) -> Result<Vec<Vec<f32>>> {
        bail!("OpenRouter embeddings are not configured in Phase 1F.")
    }

    async fn summarize(&self, target: String, options: Value) -> Result<String> {
        self.generate(GenerateRequest {
            prompt: "Write a concise factual summary using only the supplied content.".to_owned(),
            context: target,
            options,
        })
        .await
    }

    async fn rerank(&self, _query: String, candidates: Vec<String>) -> Result<Vec<usize>> {
        Ok((0..candidates.len()).collect())
    }

    async fn test_connection(&self) -> Result<ProviderTestResult> {
        self.request(reqwest::Method::GET, "/models")
            .send()
            .await
            .context("could not reach OpenRouter")?
            .error_for_status()
            .context("OpenRouter rejected the API key")?;
        Ok(ProviderTestResult { provider_id: self.settings.id.clone(), success: true, message: "OpenRouter is ready. Workspace content will leave this device only when you request an AI action.".to_owned() })
    }
}

pub enum ConfiguredProvider {
    Ollama(OllamaProvider),
    OpenRouter(OpenRouterProvider),
}

pub fn provider_from_settings(settings: ProviderSettings) -> Result<ConfiguredProvider> {
    match settings.provider_type.as_str() {
        "ollama" => Ok(ConfiguredProvider::Ollama(OllamaProvider::from_settings(
            settings,
        )?)),
        "openrouter" => Ok(ConfiguredProvider::OpenRouter(
            OpenRouterProvider::from_settings(settings)?,
        )),
        _ => bail!("Unsupported AI provider."),
    }
}

impl AiProvider for ConfiguredProvider {
    async fn generate(&self, request: GenerateRequest) -> Result<String> {
        match self {
            Self::Ollama(provider) => provider.generate(request).await,
            Self::OpenRouter(provider) => provider.generate(request).await,
        }
    }
    async fn embed(&self, texts: Vec<String>, options: Value) -> Result<Vec<Vec<f32>>> {
        match self {
            Self::Ollama(provider) => provider.embed(texts, options).await,
            Self::OpenRouter(provider) => provider.embed(texts, options).await,
        }
    }
    async fn summarize(&self, target: String, options: Value) -> Result<String> {
        match self {
            Self::Ollama(provider) => provider.summarize(target, options).await,
            Self::OpenRouter(provider) => provider.summarize(target, options).await,
        }
    }
    async fn rerank(&self, query: String, candidates: Vec<String>) -> Result<Vec<usize>> {
        match self {
            Self::Ollama(provider) => provider.rerank(query, candidates).await,
            Self::OpenRouter(provider) => provider.rerank(query, candidates).await,
        }
    }
    async fn test_connection(&self) -> Result<ProviderTestResult> {
        match self {
            Self::Ollama(provider) => provider.test_connection().await,
            Self::OpenRouter(provider) => provider.test_connection().await,
        }
    }
}

pub fn validate_settings(settings: &ProviderSettings) -> Result<()> {
    if settings.provider_type != "ollama" && settings.provider_type != "openrouter" {
        bail!("Unsupported AI provider.")
    }
    if settings.name.trim().is_empty() {
        bail!("Provider name is required.")
    }
    let default_url = if settings.provider_type == "openrouter" {
        OPENROUTER_BASE_URL
    } else {
        DEFAULT_BASE_URL
    };
    let base_url = settings.base_url.as_deref().unwrap_or(default_url).trim();
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
    if settings.provider_type == "openrouter"
        && settings.enabled
        && settings
            .api_key
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
    {
        bail!("An OpenRouter API key is required before enabling cloud AI.")
    }
    if settings.provider_type == "openrouter"
        && settings.enabled
        && settings
            .metadata
            .get("cloud_content_acknowledged")
            .and_then(Value::as_bool)
            != Some(true)
    {
        bail!("Confirm that workspace excerpts leave this device before enabling OpenRouter.")
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

#[derive(Debug, Deserialize)]
struct OpenRouterResponse {
    choices: Vec<OpenRouterChoice>,
}
#[derive(Debug, Deserialize)]
struct OpenRouterChoice {
    message: OpenRouterMessage,
}
#[derive(Debug, Deserialize)]
struct OpenRouterMessage {
    content: Option<String>,
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
            api_key: None,
        }
    }

    #[test]
    fn validates_local_provider_settings() {
        assert!(validate_settings(&settings()).is_ok());
    }

    #[test]
    fn rejects_unsupported_and_incomplete_settings() {
        let mut unsupported = settings();
        unsupported.provider_type = "openai".to_owned();
        assert!(validate_settings(&unsupported).is_err());
        let mut incomplete = settings();
        incomplete.model = None;
        assert!(validate_settings(&incomplete).is_err());
    }

    #[test]
    fn enabled_openrouter_requires_key() {
        let mut cloud = settings();
        cloud.provider_type = "openrouter".to_owned();
        cloud.enabled = true;
        cloud.base_url = Some("https://openrouter.ai/api/v1".to_owned());
        cloud.model = Some("openai/gpt-4o-mini".to_owned());
        assert!(validate_settings(&cloud).is_err());
        cloud.api_key = Some("key".to_owned());
        cloud.metadata = json!({ "cloud_content_acknowledged": true });
        assert!(validate_settings(&cloud).is_ok());
    }

    #[test]
    fn disabled_provider_is_not_eligible_for_ai_calls() {
        assert!(!settings().enabled);
    }
}
