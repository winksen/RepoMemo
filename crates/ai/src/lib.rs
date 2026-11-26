use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use repomemo_domain::{ProviderSettings, ProviderTestResult};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::{json, Value};

mod catalog;
mod chat;
mod fallback;
mod openai;
mod secrets;

pub use catalog::{
    connection_base_url, is_cloud_connection, list_connection_models, probe_connection, test_catalog_model,
    ConnectionReport, ModelReport,
};
pub use chat::*;
use chat::{ollama_chat_body, ollama_chat_response, openrouter_chat_body, openrouter_chat_response};
pub use fallback::{is_fallback_error, FallbackProvider};
pub use openai::OpenAiCompatibleProvider;
pub use secrets::{mask_secrets, MARKER as SECRET_MARKER};

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

/// The hosts AI providers may be reached at when the operator lists none: this machine's Ollama
/// (its own port only, so no other service on this machine can be reached through a provider
/// address) and OpenRouter on the HTTPS port.
pub const DEFAULT_ALLOWED_HOSTS: &[&str] = &["127.0.0.1:11434", "localhost:11434", "[::1]:11434", "openrouter.ai:443"];

/// The one answer for an address nothing answers at: a refused connection, a reset, a timeout and a
/// name that resolves nowhere allowed look the same, so the connection test cannot be used to learn
/// which ports of this machine or its network are open.
const UNREACHABLE: &str = "The provider could not be reached at this address.";

/// Which addresses AI providers may be reached at. The server sets it once at
/// startup from its configuration; link-local and cloud-metadata addresses are
/// always refused, whatever it says.
#[derive(Debug, Clone, Default)]
pub struct EndpointPolicy {
    /// When set, a provider's host must equal one of these names, or end with
    /// one that starts with a dot (`.example.com`). An entry may name a port
    /// (`127.0.0.1:11434`, `[::1]:11434`, `.example.com:443`): then the address
    /// must use that port; an entry without one allows any port, as an operator
    /// who wrote it chose. When not set, every host is allowed (the operator
    /// asked for that explicitly).
    pub allowed_hosts: Option<Vec<String>>,
    /// Names other than `localhost` must resolve to publicly routable
    /// addresses, so a name on the list cannot be pointed at the server's own
    /// network (DNS rebinding). On for the built-in list; an operator's own
    /// list is trusted to name internal services.
    pub public_hostnames_only: bool,
    /// A provider address is a host and a port and nothing else: no query or fragment, and no
    /// path beyond the provider's own (`/api/v1` for OpenRouter, none for Ollama), so the
    /// server cannot be pointed at `/api/pull` and the like. On for the built-in list; an
    /// operator's own list may name gateways with paths of their own.
    pub provider_paths_only: bool,
}

impl EndpointPolicy {
    /// The policy used when the operator lists no hosts.
    pub fn built_in() -> Self {
        Self {
            allowed_hosts: Some(DEFAULT_ALLOWED_HOSTS.iter().map(|host| (*host).to_owned()).collect()),
            public_hostnames_only: true,
            provider_paths_only: true,
        }
    }
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
///
/// `provider_path` is the path the provider's own API lives at (`/api/v1` for OpenRouter, empty
/// for Ollama); under a policy that limits paths, the address may carry no other.
pub fn check_provider_url(base_url: &str, provider_path: &str) -> Result<()> {
    check_provider_url_for(base_url, provider_path, endpoint_policy())
}

/// An entry of the allowed hosts: a name or address, whether it also covers the names below it
/// (`.example.com`), and the port it is limited to, if any.
#[derive(Debug, PartialEq, Eq)]
struct AllowedHost {
    host: String,
    suffix: bool,
    port: Option<u16>,
}

/// Reads `host`, `host:port`, `[::1]`, `[::1]:port`, a bare IPv6 address, and each with a leading dot.
fn parse_allowed_host(entry: &str) -> Option<AllowedHost> {
    let entry = entry.trim().to_ascii_lowercase();
    let (rest, suffix) = match entry.strip_prefix('.') {
        Some(rest) => (rest, true),
        None => (entry.as_str(), false),
    };
    let (host, port) = if let Some(inner) = rest.strip_prefix('[') {
        let (host, after) = inner.split_once(']')?;
        let port = match after {
            "" => None,
            after => Some(after.strip_prefix(':')?.parse::<u16>().ok()?),
        };
        (host, port)
    } else if rest.matches(':').count() == 1 {
        let (host, port) = rest.split_once(':')?;
        (host, Some(port.parse::<u16>().ok()?))
    } else {
        // No colon, or several: a bare IPv6 address has no port.
        (rest, None)
    };
    (!host.is_empty()).then(|| AllowedHost {
        host: host.to_owned(),
        suffix,
        port,
    })
}

/// Whether an entry of `REPOMEMO_AI_ALLOWED_HOSTS` can be read, so a typo is found at start.
pub fn validate_allowed_host(entry: &str) -> Result<()> {
    parse_allowed_host(entry)
        .map(|_| ())
        .with_context(|| format!("\"{entry}\" is not a host name, address or .suffix, optionally followed by :port"))
}

fn check_provider_url_for(base_url: &str, provider_path: &str, policy: &EndpointPolicy) -> Result<()> {
    let url =
        reqwest::Url::parse(base_url.trim()).map_err(|_| anyhow::anyhow!("Provider base URL is not a valid URL."))?;
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
    let port = url.port_or_known_default();
    if let Some(allowed) = &policy.allowed_hosts {
        let permitted = allowed
            .iter()
            .filter_map(|entry| parse_allowed_host(entry))
            .any(|entry| {
                let host_matches = if entry.suffix {
                    host == entry.host || host.ends_with(&format!(".{}", entry.host))
                } else {
                    host == entry.host
                };
                host_matches && entry.port.is_none_or(|wanted| Some(wanted) == port)
            });
        if !permitted {
            let at = port.map_or(host.clone(), |port| format!("{host}:{port}"));
            bail!(
                "The server only allows AI providers at: {}. Ask the server operator to add {at}.",
                allowed.join(", ")
            )
        }
    }
    if policy.provider_paths_only {
        // The built-in list contains both Ollama and OpenRouter. A permitted
        // host alone does not make it the host of the requested provider.
        if policy.public_hostnames_only
            && ((provider_path == "/api/v1" && host != "openrouter.ai")
                || (provider_path.is_empty() && host == "openrouter.ai"))
        {
            bail!("The provider base URL is not the built-in host for this provider.")
        }
        if url.query().is_some() || url.fragment().is_some() {
            bail!("The provider base URL must not carry a query or a fragment.")
        }
        let path = url.path().trim_end_matches('/');
        if !path.is_empty() && path != provider_path {
            bail!(
                "The server only reaches a provider at its own address{}; remove the path {path} from the base URL.",
                if provider_path.is_empty() {
                    String::new()
                } else {
                    format!(" (the path is empty or {provider_path})")
                }
            )
        }
    }
    Ok(())
}

/// Addresses nothing of this server may call, whatever an operator allows:
/// link-local, unspecified, broadcast, multicast and cloud-metadata addresses.
pub fn is_forbidden_address(address: &IpAddr) -> bool {
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

/// Addresses that are not on the public internet: this machine, private and
/// shared ranges, link-local and documentation addresses.
pub fn is_non_public_address(address: &IpAddr) -> bool {
    is_non_public(address)
}

/// A host name of a cloud instance-metadata service.
pub fn is_metadata_host(host: &str) -> bool {
    METADATA_HOSTS.contains(&host)
}

fn is_non_public(address: &IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => {
            let octets = v4.octets();
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_multicast()
                || octets[0] == 0
                // 100.64.0.0/10, carrier-grade NAT.
                || (octets[0] == 100 && (octets[1] & 0xc0) == 64)
        }
        IpAddr::V6(v6) => {
            if let Some(mapped) = v6.to_ipv4_mapped() {
                return is_non_public(&IpAddr::V4(mapped));
            }
            let first = v6.segments()[0];
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                // fc00::/7 unique local, fe80::/10 link-local.
                || (first & 0xfe00) == 0xfc00
                || (first & 0xffc0) == 0xfe80
        }
    }
}

/// Which of `addresses` a provider connection may use (see [`EndpointPolicy`]).
fn permitted_addresses(host: &str, addresses: Vec<SocketAddr>, public_only: bool) -> Vec<SocketAddr> {
    let is_localhost = host.eq_ignore_ascii_case("localhost");
    addresses
        .into_iter()
        .filter(|address| !is_forbidden_address(&address.ip()))
        .filter(|address| is_localhost || !public_only || !is_non_public(&address.ip()))
        .collect()
}

/// Looks provider names up itself, so the address that is checked is the
/// address that is connected to: a name that resolves somewhere forbidden,
/// now or after the URL was saved, is not reachable.
struct CheckedResolver {
    public_only: bool,
}

impl reqwest::dns::Resolve for CheckedResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let public_only = self.public_only;
        Box::pin(async move {
            let host = name.as_str().to_owned();
            let found: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), 0)).await?.collect();
            let allowed = permitted_addresses(&host, found, public_only);
            if allowed.is_empty() {
                let refusal: Box<dyn std::error::Error + Send + Sync> =
                    "the provider host resolves only to addresses that are not allowed".into();
                return Err(refusal);
            }
            let addresses: reqwest::dns::Addrs = Box::new(allowed.into_iter());
            Ok(addresses)
        })
    }
}

/// An HTTP client for provider calls. Redirects are not followed: a provider
/// answering with one could otherwise bounce requests (and API keys) to an
/// address the URL checks never saw. Names are resolved through
/// [`CheckedResolver`].
fn provider_client(timeout: Duration) -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(timeout)
        .connect_timeout(CONNECT_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .dns_resolver(Arc::new(CheckedResolver {
            public_only: endpoint_policy().public_hostnames_only,
        }))
        .build()
}

/// The result of a connection test that got no answer (see [`UNREACHABLE`]); the cause stays in the log.
fn unreachable_result(provider_id: &str, error: &reqwest::Error) -> ProviderTestResult {
    tracing::warn!(error = %error, "AI provider connection test got no answer");
    ProviderTestResult {
        provider_id: provider_id.to_owned(),
        success: false,
        message: UNREACHABLE.to_owned(),
    }
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
    /// One call with message roles: the answer and the tokens the provider reports.
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse>;
    /// A system prompt and one user message, answered as text: `chat` with those two messages.
    async fn generate(&self, request: GenerateRequest) -> Result<String>;
    /// The context window of the configured model in tokens, when the provider says.
    async fn context_tokens(&self) -> Result<Option<u32>>;
    async fn embed(&self, texts: Vec<String>, options: Value) -> Result<Vec<Vec<f32>>>;
    async fn summarize(&self, target: String, options: Value) -> Result<String>;
    async fn rerank(&self, query: String, candidates: Vec<String>) -> Result<Vec<usize>>;
    async fn analyze_image(&self, request: ImageAnalysisRequest) -> Result<String>;
    async fn test_connection(&self) -> Result<ProviderTestResult>;
    /// How many secrets were replaced by a marker before text went to a model outside the network (V0.9.94).
    fn secrets_masked(&self) -> usize {
        0
    }
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
        let model = settings.model.as_deref().unwrap_or_default().trim().to_owned();
        Ok(Self {
            settings,
            client: provider_client(Duration::from_secs(45)).context("failed to configure local AI client")?,
            base_url,
            model,
        })
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }
}

impl AiProvider for OllamaProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        let response = self
            .client
            .post(self.endpoint("/api/chat"))
            .json(&ollama_chat_body(&self.model, &request))
            .send()
            .await
            .context("could not reach the local Ollama provider")?;
        let response = checked(response, "The local Ollama provider rejected the request").await?;
        let body: Value = read_json(response, "local Ollama provider returned an invalid response").await?;
        Ok(ollama_chat_response(&body))
    }

    async fn generate(&self, request: GenerateRequest) -> Result<String> {
        let answer = self.chat(request.to_chat()).await?.content;
        if answer.is_empty() {
            bail!("local Ollama provider returned an empty summary")
        }
        Ok(answer)
    }

    /// Ollama's `/api/show` lists the model's context length under a key named for its family.
    async fn context_tokens(&self) -> Result<Option<u32>> {
        let response = self
            .client
            .post(self.endpoint("/api/show"))
            .json(&json!({ "model": self.model }))
            .send()
            .await
            .context("could not reach the local Ollama provider")?;
        let response = checked(response, "The local Ollama provider could not describe the model").await?;
        let body: Value = read_json(response, "local Ollama provider returned an invalid response").await?;
        Ok(ollama_context_length(&body))
    }

    async fn embed(&self, texts: Vec<String>, _options: Value) -> Result<Vec<Vec<f32>>> {
        let model = self.settings.embedding_model.as_deref().unwrap_or(&self.model);
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
            prompt: "Write a concise factual summary. Use only the supplied content and do not invent details."
                .to_owned(),
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
        let response = checked(
            response,
            "The configured Ollama model could not analyze this image; choose a vision-capable model",
        )
        .await?;
        ollama_image_answer(response).await
    }

    async fn test_connection(&self) -> Result<ProviderTestResult> {
        let response = match self.client.get(self.endpoint("/api/tags")).send().await {
            Ok(response) => response,
            Err(error) => return Ok(unreachable_result(&self.settings.id, &error)),
        };
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
                format!("Provider connected, but model '{}' was not found.", self.model)
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
        let api_key = settings.api_key.clone().unwrap_or_default().trim().to_owned();
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
            model: settings.model.as_deref().unwrap_or_default().trim().to_owned(),
            api_key,
            client: provider_client(Duration::from_secs(60)).context("failed to configure cloud AI client")?,
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
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        let response = self
            .request(reqwest::Method::POST, "/chat/completions")
            .json(&openrouter_chat_body(&self.model, &request))
            .send()
            .await
            .context("could not reach OpenRouter")?;
        let response = checked(response, "OpenRouter rejected the request").await?;
        let body: Value = read_json(response, "OpenRouter returned an invalid response").await?;
        if let Some(message) = error_message(&body) {
            tracing::warn!(detail = %message, "OpenRouter could not complete the request");
            bail!("OpenRouter could not complete the request. The server log has the provider's message.")
        }
        Ok(openrouter_chat_response(&body))
    }

    async fn generate(&self, request: GenerateRequest) -> Result<String> {
        let answer = self.chat(request.to_chat()).await?.content;
        if answer.is_empty() {
            bail!("OpenRouter returned an empty answer")
        }
        Ok(answer)
    }

    /// The model's `context_length` in OpenRouter's model list.
    async fn context_tokens(&self) -> Result<Option<u32>> {
        let response = self
            .request(reqwest::Method::GET, "/models")
            .send()
            .await
            .context("could not reach OpenRouter")?;
        let models: Value = read_json(
            checked(response, "OpenRouter could not list its models").await?,
            "OpenRouter returned an invalid model list",
        )
        .await?;
        Ok(openrouter_context_length(&models, &self.model))
    }

    /// OpenRouter's OpenAI-compatible embeddings endpoint.
    async fn embed(&self, texts: Vec<String>, _options: Value) -> Result<Vec<Vec<f32>>> {
        let model = self.settings.embedding_model.as_deref().unwrap_or(&self.model);
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
            tracing::warn!(detail = %message, "OpenRouter could not build embeddings");
            bail!("OpenRouter could not build embeddings. The server log has the provider's message.")
        }
        let mut rows = body["data"]
            .as_array()
            .context("OpenRouter returned no embeddings")?
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
        let response = checked(
            response,
            "The configured OpenRouter model could not analyze this image; choose a vision-capable model",
        )
        .await?;
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
        let key = match self.request(reqwest::Method::GET, "/key").send().await {
            Ok(response) => response,
            Err(error) => return Ok(unreachable_result(&self.settings.id, &error)),
        };
        if matches!(key.status().as_u16(), 401 | 403) {
            return Ok(result(
                false,
                "OpenRouter rejected the API key. Paste a valid key and save the provider again.".to_owned(),
            ));
        }

        let models = match self.request(reqwest::Method::GET, "/models").send().await {
            Ok(response) => response,
            Err(error) => return Ok(unreachable_result(&self.settings.id, &error)),
        };
        let models: Value = read_json(
            checked(models, "OpenRouter could not list its models").await?,
            "OpenRouter returned an invalid model list",
        )
        .await?;
        let ids = models["data"].as_array().map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry["id"].as_str())
                .collect::<Vec<_>>()
        });
        if let Some(ids) = ids.filter(|ids| !ids.contains(&self.model.as_str())) {
            // A model that is only served for free is listed as `<id>:free`,
            // and the plain id then has no endpoints.
            let base = self.model.split(':').next().unwrap_or_default();
            let suggestion = ids
                .iter()
                .find(|id| id.split(':').next() == Some(base))
                .map(|id| format!(" Did you mean '{id}'?"))
                .unwrap_or_else(|| {
                    " Copy the exact id from openrouter.ai/models, for example 'openai/gpt-4o-mini'.".to_owned()
                });
            return Ok(result(
                false,
                format!(
                    "OpenRouter does not serve a model with the id '{}'.{suggestion}",
                    self.model
                ),
            ));
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
        if let Err(error) =
            async { openrouter_answer(checked(probe, "OpenRouter rejected a test request").await?, "").await }.await
        {
            return Ok(result(false, error.to_string()));
        }
        Ok(result(true, format!("OpenRouter is ready with {}. Workspace content will leave this device only when you request an AI action.", self.model)))
    }
}

fn ollama_context_length(body: &Value) -> Option<u32> {
    body.get("model_info")?
        .as_object()?
        .iter()
        .find(|(key, _)| key.ends_with(".context_length"))
        .and_then(|(_, value)| value.as_u64())
        .map(|tokens| tokens.min(u64::from(u32::MAX)) as u32)
}

fn openrouter_context_length(models: &Value, model: &str) -> Option<u32> {
    models["data"]
        .as_array()?
        .iter()
        .find(|entry| entry["id"].as_str() == Some(model))
        .and_then(|entry| entry["context_length"].as_u64())
        .map(|tokens| tokens.min(u64::from(u32::MAX)) as u32)
}

/// One vector per input, none empty, so a caller can never store a partial
/// batch or loop on chunks that silently got no embedding.
fn ensure_embeddings(vectors: Vec<Vec<f32>>, expected: usize) -> Result<Vec<Vec<f32>>> {
    if vectors.len() != expected {
        bail!(
            "The provider returned {} embeddings for {expected} inputs.",
            vectors.len()
        )
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
    // The provider's own words stay in the server log: they come from
    // whatever the configured address answers with, and nobody asked for
    // them to be shown to whoever triggered the call (review H2).
    let body = read_body_limited(response, MAX_ERROR_BODY_BYTES)
        .await
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default();
    let detail = serde_json::from_str::<Value>(&body)
        .ok()
        .and_then(|value| error_message(&value))
        .unwrap_or_else(|| body.trim().chars().take(300).collect::<String>());
    tracing::warn!(status = status.as_u16(), detail = %detail, "{action}");
    let hint = match status.as_u16() {
        401 | 403 => " Check the API key in the workspace AI settings.",
        402 => " The provider account has run out of credits.",
        404 => " Check the model id; on OpenRouter also check the privacy settings, which can rule out every provider for a model.",
        429 => " The provider is rate limiting requests; try again shortly.",
        300..=399 => " The provider answered with a redirect, which is not followed; enter the final address as the base URL.",
        _ => "",
    };
    Err(anyhow::Error::new(ProviderHttpError::new(
        status.as_u16(),
        format!("{action} (HTTP {}).{hint}", status.as_u16()),
    )))
}

/// A provider answered with an error status. It reads as before, and carries the status so a caller can tell a refusal that would
/// repeat from one a fallback model can answer (V0.9.80).
#[derive(Debug)]
pub struct ProviderHttpError {
    pub status: u16,
    message: String,
}

impl ProviderHttpError {
    pub fn new(status: u16, message: String) -> Self {
        Self { status, message }
    }
}

impl std::fmt::Display for ProviderHttpError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ProviderHttpError {}

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
        tracing::warn!(detail = %message, "OpenRouter could not complete the request");
        bail!("OpenRouter could not complete the request. The server log has the provider's message.")
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

enum Backend {
    Ollama(OllamaProvider),
    OpenRouter(OpenRouterProvider),
    OpenAiCompatible(OpenAiCompatibleProvider),
}

pub fn provider_from_settings(settings: ProviderSettings) -> Result<ConfiguredProvider> {
    let guard = SecretGuard::for_settings(&settings);
    let backend = match settings.provider_type.as_str() {
        "ollama" => Backend::Ollama(OllamaProvider::from_settings(settings)?),
        "openrouter" => Backend::OpenRouter(OpenRouterProvider::from_settings(settings)?),
        "openai_compatible" => Backend::OpenAiCompatible(OpenAiCompatibleProvider::from_settings(settings)?),
        _ => bail!("Unsupported AI provider."),
    };
    Ok(ConfiguredProvider { backend, guard })
}

/// What a provider keeps from a model that is outside the network (V0.9.94): what looks like a secret is replaced by a marker, or the
/// request is refused when the organization chose that. A model on the network is not guarded, and nothing is kept but how many.
struct SecretGuard {
    refuse: bool,
    /// Also replace email addresses and phone numbers (V0.9.95).
    personal: bool,
    masked: std::sync::atomic::AtomicUsize,
}

impl SecretGuard {
    fn for_settings(settings: &ProviderSettings) -> Option<Self> {
        if !is_cloud_endpoint(&settings.provider_type, settings.base_url.as_deref()) {
            return None;
        }
        Some(Self {
            refuse: settings.metadata.get("secrets_mode").and_then(Value::as_str) == Some("refuse"),
            personal: settings.metadata.get("personal_mode").and_then(Value::as_str) == Some("mask"),
            masked: std::sync::atomic::AtomicUsize::new(0),
        })
    }

    /// The text as it may be sent: with its secrets replaced, or an error when the organization does not send them.
    fn text(&self, text: &str) -> Result<String> {
        let (mut clean, mut found) = secrets::mask_secrets(text);
        if self.personal {
            let (personal, count) = secrets::mask_personal_data(&clean);
            clean = personal;
            found += count;
        }
        if found == 0 {
            return Ok(text.to_owned());
        }
        if self.refuse {
            bail!("{SECRETS_REFUSED}")
        }
        self.masked.fetch_add(found, std::sync::atomic::Ordering::SeqCst);
        Ok(clean)
    }
}

/// What a person is told when a secret was found and the organization does not send them to a cloud model.
pub const SECRETS_REFUSED: &str = "What CAIRN was about to send to the model holds a secret (a key, a password or a token), and your organization does not send those to a model outside its network. Nothing was sent.";

/// A provider as the workspace's settings describe it, guarded when its model is outside the network.
pub struct ConfiguredProvider {
    backend: Backend,
    guard: Option<SecretGuard>,
}

impl ConfiguredProvider {
    /// What was replaced by a marker so far.
    pub fn masked(&self) -> usize {
        self.guard
            .as_ref()
            .map_or(0, |guard| guard.masked.load(std::sync::atomic::Ordering::SeqCst))
    }

    fn clean(&self, text: &str) -> Result<String> {
        match &self.guard {
            Some(guard) => guard.text(text),
            None => Ok(text.to_owned()),
        }
    }

    fn clean_chat(&self, mut request: ChatRequest) -> Result<ChatRequest> {
        if self.guard.is_some() {
            for message in &mut request.messages {
                message.content = self.clean(&message.content)?;
            }
        }
        Ok(request)
    }
}

impl AiProvider for ConfiguredProvider {
    fn secrets_masked(&self) -> usize {
        self.masked()
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        let request = self.clean_chat(request)?;
        match &self.backend {
            Backend::Ollama(provider) => provider.chat(request).await,
            Backend::OpenRouter(provider) => provider.chat(request).await,
            Backend::OpenAiCompatible(provider) => provider.chat(request).await,
        }
    }
    async fn generate(&self, mut request: GenerateRequest) -> Result<String> {
        if self.guard.is_some() {
            request.prompt = self.clean(&request.prompt)?;
            request.context = self.clean(&request.context)?;
            if let Some(system) = request.system.take() {
                request.system = Some(self.clean(&system)?);
            }
        }
        match &self.backend {
            Backend::Ollama(provider) => provider.generate(request).await,
            Backend::OpenRouter(provider) => provider.generate(request).await,
            Backend::OpenAiCompatible(provider) => provider.generate(request).await,
        }
    }
    async fn context_tokens(&self) -> Result<Option<u32>> {
        match &self.backend {
            Backend::Ollama(provider) => provider.context_tokens().await,
            Backend::OpenRouter(provider) => provider.context_tokens().await,
            Backend::OpenAiCompatible(provider) => provider.context_tokens().await,
        }
    }
    async fn embed(&self, texts: Vec<String>, options: Value) -> Result<Vec<Vec<f32>>> {
        let texts = if self.guard.is_some() {
            texts.iter().map(|text| self.clean(text)).collect::<Result<Vec<_>>>()?
        } else {
            texts
        };
        match &self.backend {
            Backend::Ollama(provider) => provider.embed(texts, options).await,
            Backend::OpenRouter(provider) => provider.embed(texts, options).await,
            Backend::OpenAiCompatible(provider) => provider.embed(texts, options).await,
        }
    }
    async fn summarize(&self, target: String, options: Value) -> Result<String> {
        let target = self.clean(&target)?;
        match &self.backend {
            Backend::Ollama(provider) => provider.summarize(target, options).await,
            Backend::OpenRouter(provider) => provider.summarize(target, options).await,
            Backend::OpenAiCompatible(provider) => provider.summarize(target, options).await,
        }
    }
    async fn rerank(&self, query: String, candidates: Vec<String>) -> Result<Vec<usize>> {
        let query = self.clean(&query)?;
        let candidates = candidates
            .iter()
            .map(|text| self.clean(text))
            .collect::<Result<Vec<_>>>()?;
        match &self.backend {
            Backend::Ollama(provider) => provider.rerank(query, candidates).await,
            Backend::OpenRouter(provider) => provider.rerank(query, candidates).await,
            Backend::OpenAiCompatible(provider) => provider.rerank(query, candidates).await,
        }
    }
    async fn analyze_image(&self, request: ImageAnalysisRequest) -> Result<String> {
        match &self.backend {
            Backend::Ollama(provider) => provider.analyze_image(request).await,
            Backend::OpenRouter(provider) => provider.analyze_image(request).await,
            Backend::OpenAiCompatible(provider) => provider.analyze_image(request).await,
        }
    }
    async fn test_connection(&self) -> Result<ProviderTestResult> {
        match &self.backend {
            Backend::Ollama(provider) => provider.test_connection().await,
            Backend::OpenRouter(provider) => provider.test_connection().await,
            Backend::OpenAiCompatible(provider) => provider.test_connection().await,
        }
    }
}

async fn ollama_image_answer(response: reqwest::Response) -> Result<String> {
    let body: OllamaGenerateResponse = read_json(response, "the vision provider returned an invalid response").await?;
    let answer = body.response.trim().to_owned();
    if answer.is_empty() {
        bail!("the vision provider returned an empty image description")
    }
    Ok(answer)
}

pub fn validate_settings(settings: &ProviderSettings) -> Result<()> {
    if !matches!(
        settings.provider_type.as_str(),
        "ollama" | "openrouter" | "openai_compatible"
    ) {
        bail!("Unsupported AI provider.")
    }
    if settings.name.trim().is_empty() {
        bail!("Provider name is required.")
    }
    let (default_url, provider_path) = match settings.provider_type.as_str() {
        "openrouter" => (OPENROUTER_BASE_URL, "/api/v1"),
        "openai_compatible" => ("", openai::OPENAI_COMPATIBLE_PATH),
        _ => (DEFAULT_BASE_URL, ""),
    };
    let base_url = settings.base_url.as_deref().unwrap_or(default_url).trim();
    if base_url.is_empty() {
        bail!("A base URL is required, for example https://api.example.com/v1.")
    }
    if !(base_url.starts_with("http://") || base_url.starts_with("https://")) {
        bail!("Provider base URL must start with http:// or https://.")
    }
    check_provider_url(base_url, provider_path)?;
    if settings.model.as_deref().unwrap_or_default().trim().is_empty() {
        bail!("A chat model is required.")
    }
    if settings.provider_type == "openrouter"
        && settings.enabled
        && settings.api_key.as_deref().unwrap_or_default().trim().is_empty()
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
    if settings.provider_type == "openai_compatible"
        && settings.enabled
        && is_cloud_endpoint(&settings.provider_type, Some(base_url))
        && settings
            .metadata
            .get("cloud_content_acknowledged")
            .and_then(Value::as_bool)
            != Some(true)
    {
        bail!("Confirm that workspace excerpts leave your network before enabling a provider at this address.")
    }
    Ok(())
}

/// Whether workspace content sent to this provider leaves the device and the private network: always
/// for OpenRouter, never for Ollama (which the operator's allowed hosts already limit), and for an
/// OpenAI-compatible endpoint when its host is neither `localhost` nor a loopback, private or
/// link-local address. A name that is not `localhost` counts as cloud, because it can point
/// anywhere.
pub fn is_cloud_endpoint(provider_type: &str, base_url: Option<&str>) -> bool {
    match provider_type {
        "openrouter" => true,
        "openai_compatible" => {
            let host = base_url
                .and_then(|url| reqwest::Url::parse(url.trim()).ok())
                .and_then(|url| url.host_str().map(str::to_owned))
                .map(|host| {
                    host.trim_start_matches('[')
                        .trim_end_matches(']')
                        .trim_end_matches('.')
                        .to_ascii_lowercase()
                })
                .unwrap_or_default();
            if host == "localhost" || host.ends_with(".localhost") {
                return false;
            }
            match host.parse::<IpAddr>() {
                Ok(address) => !is_non_public(&address),
                Err(_) => true,
            }
        }
        _ => false,
    }
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
    use super::{
        ensure_embeddings, error_message, ollama_chat_body, ollama_chat_response, openrouter_chat_body,
        openrouter_chat_response, parse_ranking, validate_settings, ChatMessage, ChatRole, GenerateRequest, Usage,
    };

    fn generate_request() -> GenerateRequest {
        GenerateRequest {
            system: Some("Be brief.".to_owned()),
            prompt: "Question?".to_owned(),
            context: "Some context".to_owned(),
            options: json!({ "temperature": 0.5 }),
        }
    }

    #[test]
    fn generate_is_a_system_and_a_user_message() {
        let chat = generate_request().to_chat();
        assert_eq!(
            chat.messages,
            vec![
                ChatMessage::new(ChatRole::System, "Be brief."),
                ChatMessage::new(ChatRole::User, "Question?\n\nContext:\nSome context"),
            ]
        );
        let default = GenerateRequest {
            system: None,
            ..generate_request()
        };
        assert!(default.to_chat().messages[0]
            .content
            .starts_with("You are RepoMemo's assistant"));
    }

    #[test]
    fn ollama_chat_body_keeps_the_prompt_and_asks_for_room() {
        let body = ollama_chat_body("llama3.2", &generate_request().to_chat());
        assert_eq!(body["model"], "llama3.2");
        assert_eq!(body["stream"], false);
        assert_eq!(body["messages"][0], json!({ "role": "system", "content": "Be brief." }));
        assert_eq!(body["messages"][1]["role"], "user");
        assert_eq!(body["options"]["num_ctx"], 8192);
        assert_eq!(body["options"]["temperature"], 0.5);
        let mut request = generate_request().to_chat();
        request.options = json!({ "num_ctx": 2048 });
        assert_eq!(ollama_chat_body("m", &request)["options"]["num_ctx"], 2048);
    }

    #[test]
    fn openrouter_chat_body_is_the_request_generate_always_sent() {
        let body = openrouter_chat_body("openai/gpt-4o-mini", &generate_request().to_chat());
        assert_eq!(
            body,
            json!({
                "model": "openai/gpt-4o-mini",
                "messages": [
                    { "role": "system", "content": "Be brief." },
                    { "role": "user", "content": "Question?\n\nContext:\nSome context" }
                ],
                "temperature": 0.5,
            })
        );
        let mut request = generate_request().to_chat();
        request.options = json!({});
        assert_eq!(openrouter_chat_body("m", &request)["temperature"], 0.2);
    }

    #[test]
    fn usage_is_read_when_the_provider_returns_it() {
        let ollama = ollama_chat_response(&json!({
            "message": { "role": "assistant", "content": " Hi " },
            "prompt_eval_count": 12,
            "eval_count": 3
        }));
        assert_eq!(ollama.content, "Hi");
        assert_eq!(
            ollama.usage,
            Some(Usage {
                input_tokens: 12,
                output_tokens: 3
            })
        );
        let open = openrouter_chat_response(&json!({
            "choices": [{ "message": { "content": "Hello" } }],
            "usage": { "prompt_tokens": 20, "completion_tokens": 5, "total_tokens": 25 }
        }));
        assert_eq!(open.content, "Hello");
        assert_eq!(
            open.usage,
            Some(Usage {
                input_tokens: 20,
                output_tokens: 5
            })
        );
    }

    #[test]
    fn context_lengths_are_found_per_provider() {
        let ollama = json!({ "model_info": { "general.architecture": "llama", "llama.context_length": 131072 } });
        assert_eq!(super::ollama_context_length(&ollama), Some(131_072));
        assert_eq!(super::ollama_context_length(&json!({ "model_info": {} })), None);
        let models = json!({ "data": [
            { "id": "a/b", "context_length": 8000 },
            { "id": "openai/gpt-4o-mini", "context_length": 128000 }
        ] });
        assert_eq!(
            super::openrouter_context_length(&models, "openai/gpt-4o-mini"),
            Some(128_000)
        );
        assert_eq!(super::openrouter_context_length(&models, "missing"), None);
    }
    #[test]
    fn usage_is_none_when_the_provider_omits_it() {
        assert_eq!(
            ollama_chat_response(&json!({ "message": { "content": "x" } })).usage,
            None
        );
        let empty = openrouter_chat_response(&json!({ "choices": [] }));
        assert_eq!(empty.usage, None);
        assert_eq!(empty.content, "");
    }

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

    fn compatible(base_url: &str) -> ProviderSettings {
        ProviderSettings {
            provider_type: "openai_compatible".to_owned(),
            name: "Endpoint".to_owned(),
            base_url: Some(base_url.to_owned()),
            model: Some("some-model".to_owned()),
            enabled: true,
            metadata: json!({}),
            ..settings()
        }
    }

    #[test]
    fn loopback_and_private_hosts_are_local_and_others_are_cloud() {
        use super::is_cloud_endpoint;
        for local in [
            "http://localhost:1234/v1",
            "http://127.0.0.1:8000/v1",
            "http://192.168.1.20:8080/v1",
            "http://10.0.0.5/v1",
            "http://[::1]:8080/v1",
            "http://llm.localhost/v1",
        ] {
            assert!(!is_cloud_endpoint("openai_compatible", Some(local)), "{local}");
        }
        for cloud in [
            "https://api.openai.com/v1",
            "https://api.groq.com/openai/v1",
            "http://8.8.8.8/v1",
            "https://llm.internal.example/v1",
        ] {
            assert!(is_cloud_endpoint("openai_compatible", Some(cloud)), "{cloud}");
        }
        assert!(is_cloud_endpoint("openrouter", None));
        assert!(!is_cloud_endpoint("ollama", Some("http://example.com")));
    }

    #[test]
    fn a_compatible_endpoint_needs_an_address_a_model_and_for_cloud_the_acknowledgement() {
        let mut local = compatible("http://localhost:1234/v1");
        assert!(validate_settings(&local).is_ok());
        local.base_url = None;
        assert!(validate_settings(&local).is_err());
        let mut cloud = compatible("https://api.example.com/v1");
        assert!(validate_settings(&cloud).is_err());
        cloud.metadata = json!({ "cloud_content_acknowledged": true });
        assert!(validate_settings(&cloud).is_ok());
        cloud.api_key = None;
        assert!(validate_settings(&cloud).is_ok(), "the key is optional");
        cloud.model = None;
        assert!(validate_settings(&cloud).is_err());
        cloud.model = Some("m".to_owned());
        cloud.base_url = Some("ftp://api.example.com/v1".to_owned());
        assert!(validate_settings(&cloud).is_err());
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
            error_message(
                &json!({"error": {"message": "Provider returned error", "metadata": {"raw": "model overloaded"}}})
            )
            .as_deref(),
            Some("Provider returned error (model overloaded)")
        );
        assert_eq!(
            error_message(&json!({"error": "model 'llama9' not found"})).as_deref(),
            Some("model 'llama9' not found")
        );
        assert_eq!(error_message(&json!({"choices": []})), None);
    }

    #[test]
    fn provider_urls_cannot_reach_metadata_or_embed_credentials() {
        use super::{check_provider_url_for, EndpointPolicy};
        let check_provider_url_with = |url: &str, policy: &EndpointPolicy| check_provider_url_for(url, "", policy);
        let open = EndpointPolicy::default();
        for allowed in [
            "http://127.0.0.1:11434",
            "http://localhost:11434",
            "https://openrouter.ai/api/v1",
            "http://192.168.1.20:11434",
            "http://[::1]:11434",
        ] {
            assert!(
                check_provider_url_with(allowed, &open).is_ok(),
                "{allowed} should be allowed"
            );
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
            assert!(
                check_provider_url_with(refused, &open).is_err(),
                "{refused} should be refused"
            );
        }

        let restricted = EndpointPolicy {
            allowed_hosts: Some(vec!["127.0.0.1".to_owned(), ".openrouter.ai".to_owned()]),
            ..EndpointPolicy::default()
        };
        assert!(check_provider_url_with("http://127.0.0.1:11434", &restricted).is_ok());
        assert!(check_provider_url_with("https://openrouter.ai/api/v1", &restricted).is_ok());
        assert!(check_provider_url_with("https://eu.openrouter.ai/api/v1", &restricted).is_ok());
        assert!(check_provider_url_with("https://evil-openrouter.ai/api/v1", &restricted).is_err());
        assert!(check_provider_url_with("http://10.0.0.5:11434", &restricted).is_err());
    }

    #[test]
    fn the_built_in_policy_allows_this_machine_and_openrouter_only() {
        use super::{check_provider_url_for, EndpointPolicy};
        let built_in = EndpointPolicy::built_in();
        let ollama = |url: &str| check_provider_url_for(url, "", &built_in);
        let openrouter = |url: &str| check_provider_url_for(url, "/api/v1", &built_in);
        for allowed in [
            "http://127.0.0.1:11434",
            "http://localhost:11434",
            "http://localhost:11434/",
            "http://[::1]:11434",
        ] {
            assert!(ollama(allowed).is_ok(), "{allowed} should be allowed");
        }
        for allowed in [
            "https://openrouter.ai/api/v1",
            "https://openrouter.ai/api/v1/",
            "https://openrouter.ai:443/api/v1",
            "https://openrouter.ai",
        ] {
            assert!(openrouter(allowed).is_ok(), "{allowed} should be allowed");
        }
        for refused in [
            "http://192.168.1.20:11434",
            "http://10.0.0.5:11434",
            "https://api.openai.com/v1",
            "https://eu.openrouter.ai/api/v1",
        ] {
            assert!(ollama(refused).is_err(), "{refused} should be refused");
            assert!(openrouter(refused).is_err(), "{refused} should be refused");
        }
    }

    /// E2: a provider address is a host and a port, with no other service of this machine and no
    /// path beyond the provider's own.
    #[test]
    fn the_built_in_policy_reaches_neither_other_ports_nor_other_paths() {
        use super::{check_provider_url_for, EndpointPolicy};
        let built_in = EndpointPolicy::built_in();
        let ollama = |url: &str| check_provider_url_for(url, "", &built_in);
        let openrouter = |url: &str| check_provider_url_for(url, "/api/v1", &built_in);
        for refused in [
            // The server's own port, SSH, and the default port of http.
            "http://127.0.0.1:3020",
            "http://127.0.0.1:22",
            "http://127.0.0.1",
            "http://localhost:3021",
            "http://[::1]:8080",
            // Paths and queries of Ollama's other endpoints.
            "http://localhost:11434/api/pull",
            "http://127.0.0.1:11434/api/tags",
            "http://127.0.0.1:11434/?model=x",
            "http://127.0.0.1:11434/#frag",
        ] {
            let error = ollama(refused).expect_err(refused).to_string();
            assert!(!error.is_empty(), "{refused}");
        }
        for refused in [
            "https://openrouter.ai:8443/api/v1",
            "http://openrouter.ai/api/v1",
            "https://openrouter.ai/api/v2",
            "https://openrouter.ai/other",
            "https://openrouter.ai/api/v1?x=1",
        ] {
            assert!(openrouter(refused).is_err(), "{refused} should be refused");
        }
        // Another provider's path is not this provider's.
        assert!(check_provider_url_for("https://openrouter.ai/api/v1", "", &built_in).is_err());
        assert!(check_provider_url_for("http://127.0.0.1:11434/api/v1", "/api/v1", &built_in).is_err());
    }

    #[test]
    fn an_operators_list_may_name_ports_suffixes_and_gateways() {
        use super::{check_provider_url_for, parse_allowed_host, validate_allowed_host, AllowedHost, EndpointPolicy};
        assert_eq!(
            parse_allowed_host("[::1]:11434"),
            Some(AllowedHost {
                host: "::1".to_owned(),
                suffix: false,
                port: Some(11434)
            })
        );
        assert_eq!(
            parse_allowed_host(".Example.com:8443"),
            Some(AllowedHost {
                host: "example.com".to_owned(),
                suffix: true,
                port: Some(8443)
            })
        );
        assert_eq!(
            parse_allowed_host("::1"),
            Some(AllowedHost {
                host: "::1".to_owned(),
                suffix: false,
                port: None
            })
        );
        assert_eq!(parse_allowed_host("llm.internal").unwrap().port, None);
        for unreadable in ["", "host:", "host:99999", "host:abc", "[::1", "[::1]x", ":8080"] {
            assert!(parse_allowed_host(unreadable).is_none(), "{unreadable}");
            assert!(validate_allowed_host(unreadable).is_err(), "{unreadable}");
        }
        assert!(validate_allowed_host("llm.internal:8080").is_ok());

        // An entry with a port limits the port; one without allows any, and paths are the operator's.
        let operator = EndpointPolicy {
            allowed_hosts: Some(vec![
                "llm.internal:8080".to_owned(),
                "127.0.0.1".to_owned(),
                ".example.com:443".to_owned(),
            ]),
            ..EndpointPolicy::default()
        };
        let check = |url: &str| check_provider_url_for(url, "", &operator);
        assert!(check("http://llm.internal:8080/v1").is_ok());
        assert!(check("http://llm.internal:9090/v1").is_err());
        assert!(
            check("http://127.0.0.1:3020").is_ok(),
            "no port in the entry: any port, as chosen"
        );
        assert!(
            check("http://127.0.0.1:11434/api/pull").is_ok(),
            "paths are not limited outside the built-in list"
        );
        assert!(check("https://eu.example.com/v1").is_ok());
        assert!(check("https://eu.example.com:8443/v1").is_err());
    }

    #[test]
    fn resolved_addresses_are_filtered_before_connecting() {
        use super::permitted_addresses;
        use std::net::SocketAddr;
        let at = |text: &str| text.parse::<SocketAddr>().unwrap();
        let mixed = vec![
            at("10.0.0.5:0"),
            at("93.184.216.34:0"),
            at("169.254.169.254:0"),
            at("[fd00::1]:0"),
        ];
        assert_eq!(
            permitted_addresses("openrouter.ai", mixed.clone(), true),
            vec![at("93.184.216.34:0")],
            "public addresses only"
        );
        assert_eq!(
            permitted_addresses("llm.internal", mixed, false),
            vec![at("10.0.0.5:0"), at("93.184.216.34:0"), at("[fd00::1]:0")],
            "an operator's own list may name internal hosts, but never metadata addresses"
        );
        assert_eq!(
            permitted_addresses("localhost", vec![at("127.0.0.1:0")], true),
            vec![at("127.0.0.1:0")]
        );
        assert!(
            permitted_addresses("evil.example", vec![at("127.0.0.1:0")], true).is_empty(),
            "a name pointed at loopback is refused"
        );
    }

    #[test]
    fn disabled_provider_is_not_eligible_for_ai_calls() {
        assert!(!settings().enabled);
    }
}
