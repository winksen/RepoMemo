use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
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

#[derive(Debug, Clone)]
pub struct ImageAnalysisRequest {
    pub prompt: String,
    pub image_bytes: Vec<u8>,
    pub mime_type: String,
}

#[allow(async_fn_in_trait)]
pub trait AiProvider {
    async fn generate(&self, request: GenerateRequest) -> Result<String>;
    async fn embed(&self, texts: Vec<String>, options: Value) -> Result<Vec<Vec<f32>>>;
    async fn summarize(&self, target: String, options: Value) -> Result<String>;
    async fn rerank(&self, query: String, candidates: Vec<String>) -> Result<Vec<usize>>;
    async fn analyze_image(&self, request: ImageAnalysisRequest) -> Result<String>;
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
            .context("could not reach the local Ollama provider")?;
        let response = checked(response, "The local Ollama provider rejected the request").await?;
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

    async fn analyze_image(&self, request: ImageAnalysisRequest) -> Result<String> {
        let response = self
            .client
            .post(self.endpoint("/api/generate"))
            .json(&json!({
                "model": self.model,
                "prompt": request.prompt,
                "images": [STANDARD.encode(request.image_bytes)],
                "stream": false,
            }))
            .send()
            .await
            .context("could not reach the local Ollama vision model")?;
        let response = checked(response, "The configured Ollama model could not analyze this image; choose a vision-capable model").await?;
        ollama_image_answer(response).await
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
            .send().await.context("could not reach OpenRouter")?;
        let response = checked(response, "OpenRouter rejected the request").await?;
        openrouter_answer(response, "OpenRouter returned an empty answer").await
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

    async fn analyze_image(&self, request: ImageAnalysisRequest) -> Result<String> {
        let image_url = format!(
            "data:{};base64,{}",
            request.mime_type,
            STANDARD.encode(request.image_bytes)
        );
        let response = self
            .request(reqwest::Method::POST, "/chat/completions")
            .json(&json!({
                "model": self.model,
                "messages": [
                  { "role": "system", "content": "Describe repository images faithfully. Do not invent details." },
                  { "role": "user", "content": [
                    { "type": "text", "text": request.prompt },
                    { "type": "image_url", "image_url": { "url": image_url } }
                  ] }
                ],
                "temperature": 0.1,
            }))
            .send()
            .await
            .context("could not reach OpenRouter for image analysis")?;
        let response = checked(response, "The configured OpenRouter model could not analyze this image; choose a vision-capable model").await?;
        openrouter_answer(response, "OpenRouter returned an empty image description").await
    }

    /// Checks the key, the model id and finally a tiny completion, because the
    /// public model list answers even when the key, credits or privacy settings
    /// would make every real request fail.
    async fn test_connection(&self) -> Result<ProviderTestResult> {
        let result = |success: bool, message: String| ProviderTestResult {
            provider_id: self.settings.id.clone(),
            success,
            message,
        };
        let key = self
            .request(reqwest::Method::GET, "/key")
            .send()
            .await
            .context("could not reach OpenRouter")?;
        if matches!(key.status().as_u16(), 401 | 403) {
            return Ok(result(false, "OpenRouter rejected the API key. Paste a valid key and save the provider again.".to_owned()));
        }

        let models = self
            .request(reqwest::Method::GET, "/models")
            .send()
            .await
            .context("could not reach OpenRouter")?;
        let models: Value = checked(models, "OpenRouter could not list its models")
            .await?
            .json()
            .await
            .context("OpenRouter returned an invalid model list")?;
        let ids = models["data"]
            .as_array()
            .map(|entries| entries.iter().filter_map(|entry| entry["id"].as_str()).collect::<Vec<_>>());
        if let Some(ids) = ids.filter(|ids| !ids.contains(&self.model.as_str())) {
            // A model that is only served for free is listed as `<id>:free`,
            // and the plain id then has no endpoints.
            let base = self.model.split(':').next().unwrap_or_default();
            let suggestion = ids
                .iter()
                .find(|id| id.split(':').next() == Some(base))
                .map(|id| format!(" Did you mean '{id}'?"))
                .unwrap_or_else(|| " Copy the exact id from openrouter.ai/models, for example 'openai/gpt-4o-mini'.".to_owned());
            return Ok(result(false, format!("OpenRouter does not serve a model with the id '{}'.{suggestion}", self.model)));
        }

        let probe = self
            .request(reqwest::Method::POST, "/chat/completions")
            .json(&json!({
                "model": self.model,
                "messages": [{ "role": "user", "content": "Reply with OK." }],
                "max_tokens": 16,
            }))
            .send()
            .await
            .context("could not reach OpenRouter")?;
        if let Err(error) = async { openrouter_answer(checked(probe, "OpenRouter rejected a test request").await?, "").await }.await {
            return Ok(result(false, error.to_string()));
        }
        Ok(result(true, format!("OpenRouter is ready with {}. Workspace content will leave this device only when you request an AI action.", self.model)))
    }
}

/// Passes a successful response through. Otherwise fails with the provider's
/// own explanation, which both Ollama and OpenRouter put in an `error` field,
/// plus a hint for the statuses with a known fix.
async fn checked(response: reqwest::Response, action: &str) -> Result<reqwest::Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let body = response.text().await.unwrap_or_default();
    let detail = serde_json::from_str::<Value>(&body)
        .ok()
        .and_then(|value| error_message(&value))
        .unwrap_or_else(|| body.trim().chars().take(300).collect());
    let hint = match status.as_u16() {
        401 | 403 => " Check the API key in the workspace AI settings.",
        402 => " The provider account has run out of credits.",
        404 => " Check the model id; on OpenRouter also check the privacy settings, which can rule out every provider for a model.",
        429 => " The provider is rate limiting requests; try again shortly.",
        _ => "",
    };
    if detail.is_empty() {
        bail!("{action} (HTTP {}).{hint}", status.as_u16())
    }
    bail!("{action} (HTTP {}): {detail}.{hint}", status.as_u16())
}

fn error_message(value: &Value) -> Option<String> {
    let error = value.get("error")?;
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .or_else(|| error.as_str())?
        .trim()
        .trim_end_matches('.')
        .to_owned();
    // OpenRouter nests the upstream provider's reason under metadata.raw.
    let upstream = error
        .pointer("/metadata/raw")
        .and_then(Value::as_str)
        .map(|raw| raw.chars().take(200).collect::<String>());
    Some(match upstream {
        Some(raw) if !raw.is_empty() && !message.contains(&raw) => format!("{message} ({raw})"),
        _ => message,
    })
}

/// The first choice's text. OpenRouter can answer 200 with an `error` object
/// when the upstream model fails, so that is checked first.
async fn openrouter_answer(response: reqwest::Response, empty_message: &str) -> Result<String> {
    let body: Value = response
        .json()
        .await
        .context("OpenRouter returned an invalid response")?;
    if let Some(message) = error_message(&body) {
        bail!("OpenRouter could not complete the request: {message}")
    }
    let answer = body
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_owned();
    if answer.is_empty() && !empty_message.is_empty() {
        bail!("{empty_message}")
    }
    Ok(answer)
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
    async fn analyze_image(&self, request: ImageAnalysisRequest) -> Result<String> {
        match self {
            Self::Ollama(provider) => provider.analyze_image(request).await,
            Self::OpenRouter(provider) => provider.analyze_image(request).await,
        }
    }
    async fn test_connection(&self) -> Result<ProviderTestResult> {
        match self {
            Self::Ollama(provider) => provider.test_connection().await,
            Self::OpenRouter(provider) => provider.test_connection().await,
        }
    }
}

async fn ollama_image_answer(response: reqwest::Response) -> Result<String> {
    let body: OllamaGenerateResponse = response
        .json()
        .await
        .context("the vision provider returned an invalid response")?;
    let answer = body.response.trim().to_owned();
    if answer.is_empty() {
        bail!("the vision provider returned an empty image description")
    }
    Ok(answer)
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

#[cfg(test)]
mod tests {
    use super::{error_message, validate_settings};
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
    fn reads_provider_error_messages() {
        assert_eq!(
            error_message(&json!({"error": {"code": 402, "message": "Insufficient credits."}})).as_deref(),
            Some("Insufficient credits")
        );
        assert_eq!(
            error_message(&json!({"error": {"message": "Provider returned error", "metadata": {"raw": "model overloaded"}}})).as_deref(),
            Some("Provider returned error (model overloaded)")
        );
        assert_eq!(error_message(&json!({"error": "model 'llama9' not found"})).as_deref(), Some("model 'llama9' not found"));
        assert_eq!(error_message(&json!({"choices": []})), None);
    }

    #[test]
    fn disabled_provider_is_not_eligible_for_ai_calls() {
        assert!(!settings().enabled);
    }
}
