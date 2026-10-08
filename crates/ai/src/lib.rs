use std::net::IpAddr;
use std::sync::OnceLock;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use repomemo_domain::{ProviderSettings, ProviderTestResult};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::{json, Value};

const DEFAULT_BASE_URL: &str = "http://127.0.0.1:11434";
const OPENROUTER_BASE_URL: &str = "https://openrouter.ai/api/v1";

/// Context window requested from Ollama. Its default is small enough that
/// retrieved context is cut off silently, so ask for room explicitly.
const OLLAMA_NUM_CTX: u64 = 8_192;
/// Largest provider response read into memory. Model lists and embedding
/// batches are a few megabytes at most; anything bigger is a misconfigured or
/// hostile endpoint.
const MAX_RESPONSE_BYTES: usize = 32 * 1024 * 1024;
/// Error bodies are only read for their message.
const MAX_ERROR_BODY_BYTES: usize = 64 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Host names of cloud instance-metadata services. A provider URL pointing at
/// one could read the server's cloud credentials.
const METADATA_HOSTS: &[&str] = &[
    "metadata",
    "metadata.google.internal",
    "metadata.goog",
    "instance-data",
    "instance-data.ec2.internal",
    "metadata.azure.internal",
];

/// Which addresses AI providers may be reached at. The server sets it once at
/// startup from its configuration; without it every host is allowed except
/// link-local and cloud-metadata addresses, which are always refused.
#[derive(Debug, Clone, Default)]
pub struct EndpointPolicy {
    /// When set, a provider's host must equal one of these names, or end with
    /// one that starts with a dot (`.example.com`).
    pub allowed_hosts: Option<Vec<String>>,
}

static ENDPOINT_POLICY: OnceLock<EndpointPolicy> = OnceLock::new();

/// Installs the endpoint policy for this process. Returns false when one was
/// already installed (the first one stays).
pub fn set_endpoint_policy(policy: EndpointPolicy) -> bool {
    ENDPOINT_POLICY.set(policy).is_ok()
}

fn endpoint_policy() -> &'static EndpointPolicy {
    ENDPOINT_POLICY.get_or_init(EndpointPolicy::default)
}

/// Refuses provider URLs that would turn the server into a proxy towards
/// places it should never call: credentials embedded in the URL, link-local
/// and cloud-metadata addresses, and hosts outside the operator's allow-list.
pub fn check_provider_url(base_url: &str) -> Result<()> {
    check_provider_url_with(base_url, endpoint_policy())
}

fn check_provider_url_with(base_url: &str, policy: &EndpointPolicy) -> Result<()> {
    let url = reqwest::Url::parse(base_url.trim())
        .map_err(|_| anyhow::anyhow!("Provider base URL is not a valid URL."))?;
    if !matches!(url.scheme(), "http" | "https") {
        bail!("Provider base URL must start with http:// or https://.")
    }
    if !url.username().is_empty() || url.password().is_some() {
        bail!("Remove the user name and password from the provider base URL; use the API key field instead.")
    }
    let host = url
        .host_str()
        .map(|host| {
            host.trim_start_matches('[')
                .trim_end_matches(']')
                .trim_end_matches('.')
                .to_ascii_lowercase()
        })
        .filter(|host| !host.is_empty())
        .context("Provider base URL must name a host.")?;
    if METADATA_HOSTS.contains(&host.as_str()) {
        bail!("Provider base URL points at a cloud metadata service, which is not allowed.")
    }
    if let Ok(address) = host.parse::<IpAddr>() {
        if is_forbidden_address(&address) {
            bail!("Provider base URL points at a link-local or reserved address ({address}), which is not allowed.")
        }
    }
    if let Some(allowed) = &policy.allowed_hosts {
        let permitted = allowed.iter().any(|entry| {
            let entry = entry.trim().to_ascii_lowercase();
            match entry.strip_prefix('.') {
                Some(suffix) => host == suffix || host.ends_with(&format!(".{suffix}")),
                None => host == entry,
            }
        });
        if !permitted {
            bail!(
                "The server only allows AI providers at: {}. Ask the server operator to add {host}.",
                allowed.join(", ")
            )
        }
    }
    Ok(())
}

fn is_forbidden_address(address: &IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => {
            v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_multicast()
                // Alibaba Cloud's metadata service.
                || v4.octets() == [100, 100, 100, 200]
        }
        IpAddr::V6(v6) => {
            if let Some(mapped) = v6.to_ipv4_mapped() {
                return is_forbidden_address(&IpAddr::V4(mapped));
            }
            let first = v6.segments()[0];
            v6.is_unspecified()
                || v6.is_multicast()
                // fe80::/10 link-local.
                || (first & 0xffc0) == 0xfe80
                // AWS's IPv6 metadata endpoint fd00:ec2::254.
                || v6.segments() == [0xfd00, 0x0ec2, 0, 0, 0, 0, 0, 0x0254]
        }
    }
}

/// An HTTP client for provider calls. Redirects are not followed: a provider
/// answering with one could otherwise bounce requests (and API keys) to an
/// address the URL checks never saw.
fn provider_client(timeout: Duration) -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(timeout)
        .connect_timeout(CONNECT_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
}

/// Reads at most `limit` bytes of a response body.
async fn read_body_limited(mut response: reqwest::Response, limit: usize) -> Result<Vec<u8>> {
    let too_large = || anyhow::anyhow!("the provider response is larger than {} KiB", limit / 1024);
    if response.content_length().is_some_and(|length| length > limit as u64) {
        return Err(too_large());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if body.len() + chunk.len() > limit {
            return Err(too_large());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// Decodes a JSON response body, refusing oversized ones.
async fn read_json<T: DeserializeOwned>(response: reqwest::Response, invalid: &str) -> Result<T> {
    let body = read_body_limited(response, MAX_RESPONSE_BYTES)
        .await
        .with_context(|| invalid.to_owned())?;
    serde_json::from_slice(&body).with_context(|| invalid.to_owned())
}

const DEFAULT_SYSTEM_PROMPT: &str = "You are RepoMemo's assistant for a workspace of technical material. Use only the supplied context, never invent details, and say so when the context is not enough.";

#[derive(Debug, Clone)]
pub struct GenerateRequest {
    /// Instructions for the role the model plays in this task; a generic
    /// grounded-assistant prompt is used when omitted.
    pub system: Option<String>,
    pub prompt: String,
    pub context: String,
    pub options: Value,
}

impl GenerateRequest {
    fn system_prompt(&self) -> &str {
        self.system.as_deref().unwrap_or(DEFAULT_SYSTEM_PROMPT)
    }
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
            client: provider_client(Duration::from_secs(45))
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
        let mut options = request.options.clone();
        if let Some(options) = options.as_object_mut() {
            options.entry("num_ctx").or_insert(json!(OLLAMA_NUM_CTX));
        }
        let response = self
            .client
            .post(self.endpoint("/api/generate"))
            .json(&json!({
                "model": self.model,
                "system": request.system_prompt(),
                "prompt": format!("{}\n\nContext:\n{}", request.prompt, request.context),
                "stream": false,
                "options": options,
            }))
            .send()
            .await
            .context("could not reach the local Ollama provider")?;
        let response = checked(response, "The local Ollama provider rejected the request").await?;
        let body: OllamaGenerateResponse =
            read_json(response, "local Ollama provider returned an invalid response").await?;
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
        let count = texts.len();
        let response = self
            .client
            .post(self.endpoint("/api/embed"))
            .json(&json!({ "model": model, "input": texts }))
            .send()
            .await
            .context("could not reach the local Ollama provider")?;
        let response = checked(response, "The local Ollama provider could not build embeddings").await?;
        let body: OllamaEmbedResponse =
            read_json(response, "local Ollama provider returned invalid embeddings").await?;
        ensure_embeddings(body.embeddings, count)
    }

    async fn summarize(&self, target: String, options: Value) -> Result<String> {
        self.generate(GenerateRequest {
            system: None,
            prompt: "Write a concise factual summary. Use only the supplied content and do not invent details.".to_owned(),
            context: target,
            options,
        })
        .await
    }

    async fn rerank(&self, query: String, candidates: Vec<String>) -> Result<Vec<usize>> {
        llm_rerank(self, &query, &candidates).await
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
            .context("could not reach the local Ollama provider")?;
        let response = checked(response, "local Ollama provider rejected the connection test").await?;
        let body: OllamaTagsResponse =
            read_json(response, "local Ollama provider returned an invalid response").await?;
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
            client: provider_client(Duration::from_secs(60))
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
                  { "role": "system", "content": request.system_prompt() },
                  { "role": "user", "content": format!("{}\n\nContext:\n{}", request.prompt, request.context) }
                ],
                "temperature": request.options.get("temperature").and_then(Value::as_f64).unwrap_or(0.2),
            }))
            .send().await.context("could not reach OpenRouter")?;
        let response = checked(response, "OpenRouter rejected the request").await?;
        openrouter_answer(response, "OpenRouter returned an empty answer").await
    }

    /// OpenRouter's OpenAI-compatible embeddings endpoint.
    async fn embed(&self, texts: Vec<String>, _options: Value) -> Result<Vec<Vec<f32>>> {
        let model = self
            .settings
            .embedding_model
            .as_deref()
            .unwrap_or(&self.model);
        let count = texts.len();
        let response = self
            .request(reqwest::Method::POST, "/embeddings")
            .json(&json!({ "model": model, "input": texts }))
            .send()
            .await
            .context("could not reach OpenRouter")?;
        let body: Value = read_json(
            checked(response, "OpenRouter could not build embeddings").await?,
            "OpenRouter returned invalid embeddings",
        )
        .await?;
        if let Some(message) = error_message(&body) {
            bail!("OpenRouter could not build embeddings: {message}")
        }
        let mut rows = body["data"]
            .as_array()
            .context("OpenRouter returned no embeddings")?
            .iter()
            .map(|row| {
                let index = row["index"].as_u64().unwrap_or(0) as usize;
                let vector = row["embedding"]
                    .as_array()
                    .map(|values| values.iter().filter_map(Value::as_f64).map(|value| value as f32).collect())
                    .unwrap_or_default();
                (index, vector)
            })
            .collect::<Vec<(usize, Vec<f32>)>>();
        rows.sort_by_key(|(index, _)| *index);
        ensure_embeddings(rows.into_iter().map(|(_, vector)| vector).collect(), count)
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
        let models: Value = read_json(
            checked(models, "OpenRouter could not list its models").await?,
            "OpenRouter returned an invalid model list",
        )
        .await?;
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

/// One vector per input, none empty, so a caller can never store a partial
/// batch or loop on chunks that silently got no embedding.
fn ensure_embeddings(vectors: Vec<Vec<f32>>, expected: usize) -> Result<Vec<Vec<f32>>> {
    if vectors.len() != expected {
        bail!("The provider returned {} embeddings for {expected} inputs.", vectors.len())
    }
    if vectors.iter().any(Vec::is_empty) {
        bail!("The provider returned an empty embedding; check that the model is an embedding model.")
    }
    Ok(vectors)
}

/// Longest excerpt of each candidate shown to the model when reranking.
const RERANK_EXCERPT_CHARS: usize = 700;

/// Orders `candidates` by relevance to `query` by asking the chat model,
/// which judges relevance far better than keyword or vector scores alone.
/// Returns every index exactly once: indices the model leaves out keep their
/// original order after the ones it ranked.
pub async fn llm_rerank<P: AiProvider + ?Sized>(
    provider: &P,
    query: &str,
    candidates: &[String],
) -> Result<Vec<usize>> {
    if candidates.len() < 2 {
        return Ok((0..candidates.len()).collect());
    }
    let listing = candidates
        .iter()
        .enumerate()
        .map(|(index, text)| {
            let excerpt = text.chars().take(RERANK_EXCERPT_CHARS).collect::<String>();
            format!("[{index}] {}", excerpt.replace('\n', " "))
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let raw = provider
        .generate(GenerateRequest {
            system: Some("You rank search results for relevance. You reply with JSON only.".to_owned()),
            prompt: format!(
                "Query: {query}\n\nRank the passages in the context by how well they help answer the query, most useful first. Leave out passages that are irrelevant. Reply with only a JSON array of passage numbers, for example [3, 0, 5]."
            ),
            context: listing,
            options: json!({ "temperature": 0.0 }),
        })
        .await?;
    Ok(parse_ranking(&raw, candidates.len()))
}

fn parse_ranking(raw: &str, count: usize) -> Vec<usize> {
    let ranked = raw
        .find('[')
        .zip(raw.rfind(']'))
        .and_then(|(start, end)| (start < end).then(|| &raw[start..=end]))
        .and_then(|array| serde_json::from_str::<Vec<Value>>(array).ok())
        .unwrap_or_default();
    let mut order = Vec::with_capacity(count);
    for value in ranked {
        let index = value
            .as_u64()
            .or_else(|| value.as_str().and_then(|text| text.trim().parse().ok()));
        if let Some(index) = index.map(|index| index as usize) {
            if index < count && !order.contains(&index) {
                order.push(index);
            }
        }
    }
    for index in 0..count {
        if !order.contains(&index) {
            order.push(index);
        }
    }
    order
}

/// Passes a successful response through. Otherwise fails with the provider's
/// own explanation, which both Ollama and OpenRouter put in an `error` field,
/// plus a hint for the statuses with a known fix.
async fn checked(response: reqwest::Response, action: &str) -> Result<reqwest::Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let body = read_body_limited(response, MAX_ERROR_BODY_BYTES)
        .await
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default();
    let detail = serde_json::from_str::<Value>(&body)
        .ok()
        .and_then(|value| error_message(&value))
        .unwrap_or_else(|| body.trim().chars().take(300).collect());
    let hint = match status.as_u16() {
        401 | 403 => " Check the API key in the workspace AI settings.",
        402 => " The provider account has run out of credits.",
        404 => " Check the model id; on OpenRouter also check the privacy settings, which can rule out every provider for a model.",
        429 => " The provider is rate limiting requests; try again shortly.",
        300..=399 => " The provider answered with a redirect, which is not followed; enter the final address as the base URL.",
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
    let body: Value = read_json(response, "OpenRouter returned an invalid response").await?;
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
    let body: OllamaGenerateResponse =
        read_json(response, "the vision provider returned an invalid response").await?;
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
    check_provider_url(base_url)?;
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
    use super::{ensure_embeddings, error_message, parse_ranking, validate_settings};

    #[test]
    fn rankings_cover_every_candidate_once() {
        assert_eq!(parse_ranking("Here you go: [2, 0, 2, 9, \"1\"]", 4), vec![2, 0, 1, 3]);
        assert_eq!(parse_ranking("no idea", 3), vec![0, 1, 2]);
    }

    #[test]
    fn embeddings_must_match_inputs() {
        assert!(ensure_embeddings(vec![vec![0.1], vec![0.2]], 2).is_ok());
        assert!(ensure_embeddings(vec![vec![0.1]], 2).is_err());
        assert!(ensure_embeddings(vec![vec![0.1], Vec::new()], 2).is_err());
    }
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
    fn provider_urls_cannot_reach_metadata_or_embed_credentials() {
        use super::{check_provider_url_with, EndpointPolicy};
        let open = EndpointPolicy::default();
        for allowed in [
            "http://127.0.0.1:11434",
            "http://localhost:11434",
            "https://openrouter.ai/api/v1",
            "http://192.168.1.20:11434",
            "http://[::1]:11434",
        ] {
            assert!(check_provider_url_with(allowed, &open).is_ok(), "{allowed} should be allowed");
        }
        for refused in [
            "http://169.254.169.254/latest/meta-data",
            "http://metadata.google.internal/computeMetadata/v1",
            "http://[fe80::1]:11434",
            "http://[fd00:ec2::254]/",
            "http://[::ffff:169.254.169.254]/",
            "http://0.0.0.0:11434",
            "http://100.100.100.200/",
            "http://user:secret@example.com/",
            "ftp://example.com/",
            "not a url",
        ] {
            assert!(check_provider_url_with(refused, &open).is_err(), "{refused} should be refused");
        }

        let restricted = EndpointPolicy {
            allowed_hosts: Some(vec!["127.0.0.1".to_owned(), ".openrouter.ai".to_owned()]),
        };
        assert!(check_provider_url_with("http://127.0.0.1:11434", &restricted).is_ok());
        assert!(check_provider_url_with("https://openrouter.ai/api/v1", &restricted).is_ok());
        assert!(check_provider_url_with("https://eu.openrouter.ai/api/v1", &restricted).is_ok());
        assert!(check_provider_url_with("https://evil-openrouter.ai/api/v1", &restricted).is_err());
        assert!(check_provider_url_with("http://10.0.0.5:11434", &restricted).is_err());
    }

    #[test]
    fn disabled_provider_is_not_eligible_for_ai_calls() {
        assert!(!settings().enabled);
    }
}
