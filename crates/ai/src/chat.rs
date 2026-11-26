//! Conversations with a model: messages with roles, tool calls, what a model can do (its traits)
//! and the JSON protocol that gives a model without native tool calling the same loop.

use std::collections::VecDeque;
use std::fmt;
use std::sync::Mutex;

use anyhow::{bail, Result};
use repomemo_domain::ProviderTestResult;
use serde_json::{json, Value};

use crate::{AiProvider, GenerateRequest, ImageAnalysisRequest, OLLAMA_NUM_CTX};

/// Context window assumed when neither the provider nor an administrator says.
pub const DEFAULT_CONTEXT_TOKENS: u32 = 8_192;
const MIN_CONTEXT_TOKENS: u32 = 512;
const MAX_CONTEXT_TOKENS: u32 = 4_000_000;

/// Who said a message of a conversation with the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
    System,
    User,
    Assistant,
    Tool,
}

impl ChatRole {
    fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }
}

/// A tool the model may call: its name, what it does and the JSON Schema of its arguments.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
    /// Calls an assistant message made (to be answered by tool messages).
    pub tool_calls: Vec<ToolCall>,
    /// The call a tool message answers.
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    pub fn new(role: ChatRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    pub fn assistant_calls(content: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            tool_calls,
            ..Self::new(ChatRole::Assistant, content)
        }
    }

    pub fn tool_result(call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            tool_call_id: Some(call_id.into()),
            ..Self::new(ChatRole::Tool, content)
        }
    }
}

/// A conversation with the model: messages with roles, the tools it may call, and provider options.
#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolSpec>,
    pub options: Value,
}

/// Tokens a provider reports for one call, when it reports them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

impl Usage {
    fn add(self, other: Usage) -> Usage {
        Usage {
            input_tokens: self.input_tokens.saturating_add(other.input_tokens),
            output_tokens: self.output_tokens.saturating_add(other.output_tokens),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChatResponse {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Option<Usage>,
}

impl ChatResponse {
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            tool_calls: Vec::new(),
            usage: None,
        }
    }

    pub fn call(name: &str, arguments: Value) -> Self {
        Self {
            content: String::new(),
            tool_calls: vec![ToolCall {
                id: "call_0".to_owned(),
                name: name.to_owned(),
                arguments,
            }],
            usage: None,
        }
    }
}

impl GenerateRequest {
    /// The conversation `generate` has: the system prompt, then the prompt with its context.
    pub fn to_chat(&self) -> ChatRequest {
        ChatRequest {
            messages: vec![
                ChatMessage::new(ChatRole::System, self.system_prompt()),
                ChatMessage::new(ChatRole::User, format!("{}\n\nContext:\n{}", self.prompt, self.context)),
            ],
            tools: Vec::new(),
            options: self.options.clone(),
        }
    }
}

fn token_count(value: Option<u64>) -> Option<u32> {
    value.map(|count| count.min(u64::from(u32::MAX)) as u32)
}

fn usage_from(input: Option<u32>, output: Option<u32>) -> Option<Usage> {
    (input.is_some() || output.is_some()).then(|| Usage {
        input_tokens: input.unwrap_or_default(),
        output_tokens: output.unwrap_or_default(),
    })
}

fn tools_json(tools: &[ToolSpec]) -> Vec<Value> {
    tools
        .iter()
        .map(|tool| {
            json!({
                "type": "function",
                "function": { "name": tool.name, "description": tool.description, "parameters": tool.parameters },
            })
        })
        .collect()
}

/// A message in the OpenAI shape. Plain messages are exactly `{role, content}`.
fn openai_message(message: &ChatMessage) -> Value {
    let mut value = json!({ "role": message.role.as_str(), "content": message.content });
    if let Some(id) = &message.tool_call_id {
        value["tool_call_id"] = json!(id);
    }
    if !message.tool_calls.is_empty() {
        value["tool_calls"] = Value::Array(
            message
                .tool_calls
                .iter()
                .map(|call| {
                    json!({
                        "id": call.id,
                        "type": "function",
                        "function": { "name": call.name, "arguments": call.arguments.to_string() },
                    })
                })
                .collect(),
        );
    }
    value
}

/// A message in Ollama's shape, whose tool arguments are objects, not strings.
fn ollama_message(message: &ChatMessage) -> Value {
    let mut value = json!({ "role": message.role.as_str(), "content": message.content });
    if !message.tool_calls.is_empty() {
        value["tool_calls"] = Value::Array(
            message
                .tool_calls
                .iter()
                .map(|call| json!({ "function": { "name": call.name, "arguments": call.arguments } }))
                .collect(),
        );
    }
    value
}

/// A tool call's arguments arrive as an object (Ollama) or as a JSON string (OpenAI style).
fn parse_arguments(value: Option<&Value>) -> Value {
    match value {
        Some(Value::String(text)) if text.trim().is_empty() => json!({}),
        Some(Value::String(text)) => serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.clone())),
        Some(Value::Null) | None => json!({}),
        Some(other) => other.clone(),
    }
}

fn parse_tool_calls(calls: Option<&Value>) -> Vec<ToolCall> {
    calls
        .and_then(Value::as_array)
        .map(|calls| {
            calls
                .iter()
                .enumerate()
                .filter_map(|(index, call)| {
                    let name = call.pointer("/function/name").and_then(Value::as_str)?.trim();
                    (!name.is_empty()).then(|| ToolCall {
                        id: call
                            .get("id")
                            .and_then(Value::as_str)
                            .filter(|id| !id.is_empty())
                            .map_or_else(|| format!("call_{index}"), str::to_owned),
                        name: name.to_owned(),
                        arguments: parse_arguments(call.pointer("/function/arguments")),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The body Ollama's `/api/chat` receives. The context window is asked for explicitly because
/// Ollama's default cuts retrieved context off silently.
pub(crate) fn ollama_chat_body(model: &str, request: &ChatRequest) -> Value {
    let mut options = request.options.clone();
    if let Some(options) = options.as_object_mut() {
        options.entry("num_ctx").or_insert(json!(OLLAMA_NUM_CTX));
    }
    let mut body = json!({
        "model": model,
        "messages": request.messages.iter().map(ollama_message).collect::<Vec<_>>(),
        "stream": false,
        "options": options,
    });
    if !request.tools.is_empty() {
        body["tools"] = Value::Array(tools_json(&request.tools));
    }
    body
}

pub(crate) fn ollama_chat_response(body: &Value) -> ChatResponse {
    let count = |key: &str| token_count(body.get(key).and_then(Value::as_u64));
    ChatResponse {
        content: body
            .pointer("/message/content")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_owned(),
        tool_calls: parse_tool_calls(body.pointer("/message/tool_calls")),
        usage: usage_from(count("prompt_eval_count"), count("eval_count")),
    }
}

/// The body an OpenAI-style `/chat/completions` receives.
pub(crate) fn openrouter_chat_body(model: &str, request: &ChatRequest) -> Value {
    let mut body = json!({
        "model": model,
        "messages": request.messages.iter().map(openai_message).collect::<Vec<_>>(),
        "temperature": request.options.get("temperature").and_then(Value::as_f64).unwrap_or(0.2),
    });
    if !request.tools.is_empty() {
        body["tools"] = Value::Array(tools_json(&request.tools));
    }
    body
}

pub(crate) fn openrouter_chat_response(body: &Value) -> ChatResponse {
    let count = |key: &str| token_count(body.pointer(&format!("/usage/{key}")).and_then(Value::as_u64));
    ChatResponse {
        content: body
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_owned(),
        tool_calls: parse_tool_calls(body.pointer("/choices/0/message/tool_calls")),
        usage: usage_from(count("prompt_tokens"), count("completion_tokens")),
    }
}

/// How a model takes tools: the provider's own tool calling, the JSON protocol below, or not at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolMode {
    Native,
    Json,
    None,
}

impl ToolMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Json => "json",
            Self::None => "none",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "native" => Some(Self::Native),
            "json" => Some(Self::Json),
            "none" => Some(Self::None),
            _ => Option::None,
        }
    }
}

/// What a provider's model can do, kept in the provider's metadata (`tool_mode`, `context_tokens`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelTraits {
    pub tool_mode: ToolMode,
    pub context_tokens: u32,
}

impl Default for ModelTraits {
    fn default() -> Self {
        Self {
            tool_mode: ToolMode::None,
            context_tokens: DEFAULT_CONTEXT_TOKENS,
        }
    }
}

impl ModelTraits {
    /// The traits stored in a provider's metadata; a model nobody tested takes no tools.
    pub fn from_metadata(metadata: &Value) -> Self {
        Self {
            tool_mode: metadata
                .get("tool_mode")
                .and_then(Value::as_str)
                .and_then(ToolMode::parse)
                .unwrap_or(ToolMode::None),
            context_tokens: metadata
                .get("context_tokens")
                .and_then(Value::as_u64)
                .map_or(DEFAULT_CONTEXT_TOKENS, |tokens| {
                    tokens.clamp(u64::from(MIN_CONTEXT_TOKENS), u64::from(MAX_CONTEXT_TOKENS)) as u32
                }),
        }
    }

    /// Whether an administrator or a test has set the traits at all.
    pub fn is_known(metadata: &Value) -> bool {
        metadata.get("tool_mode").is_some()
    }

    /// Writes the traits into a provider's metadata object.
    pub fn write_to(&self, metadata: &mut Value) {
        if !metadata.is_object() {
            *metadata = json!({});
        }
        metadata["tool_mode"] = json!(self.tool_mode.as_str());
        metadata["context_tokens"] = json!(self.context_tokens);
    }
}

static AGENT_DEFAULT_MODELS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();

/// Installs the models the maintainer has recorded a passing live evaluation report for (V0.9.47). A workspace whose text provider
/// has native tool calling and one of these models starts in Agent mode unless an administrator chose a mode. Set once at
/// startup; returns false when a list was already set.
pub fn set_agent_default_models(models: Vec<String>) -> bool {
    let models = models
        .into_iter()
        .map(|model| model.trim().to_ascii_lowercase())
        .filter(|model| !model.is_empty())
        .collect();
    AGENT_DEFAULT_MODELS.set(models).is_ok()
}

/// The catalog models (V0.9.79) that take tools natively and have a passing live evaluation report attached. A workspace whose
/// provider is linked to one of them starts in Agent mode, as one whose model the environment lists does. Replaced whenever a report
/// is attached or a model changes.
static CATALOG_PASSED_MODELS: std::sync::RwLock<Vec<String>> = std::sync::RwLock::new(Vec::new());

pub fn set_catalog_passed_models(ids: Vec<String>) {
    *CATALOG_PASSED_MODELS
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = ids;
}

/// Whether a catalog model has a passing report on record.
pub fn catalog_model_passed(id: &str) -> bool {
    CATALOG_PASSED_MODELS
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
        .any(|passed| passed == id)
}

/// The models installed by [`set_agent_default_models`], for the administration page; empty until one was set.
pub fn agent_default_models() -> Vec<String> {
    AGENT_DEFAULT_MODELS.get().cloned().unwrap_or_default()
}

/// Whether a model has a passing live evaluation report on record, so its workspaces default to Agent mode. False for every model
/// until the maintainer lists one.
pub fn agent_default_for(model: Option<&str>) -> bool {
    AGENT_DEFAULT_MODELS
        .get()
        .is_some_and(|models| model_is_listed(models, model))
}

fn model_is_listed(models: &[String], model: Option<&str>) -> bool {
    model
        .map(|model| model.trim().to_ascii_lowercase())
        .filter(|model| !model.is_empty())
        .is_some_and(|model| models.contains(&model))
}

/// The reply of a model on the JSON protocol could not be read, even after being asked again.
#[derive(Debug)]
pub struct InvalidToolReply {
    pub detail: String,
}

impl fmt::Display for InvalidToolReply {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "The model did not answer in the tool format: {}",
            self.detail
        )
    }
}

impl std::error::Error for InvalidToolReply {}

/// The system text that teaches the JSON protocol: one object per reply.
pub fn json_tool_instructions(tools: &[ToolSpec]) -> String {
    let listing = tools
        .iter()
        .map(|tool| {
            format!(
                "- {}: {} Arguments (JSON Schema): {}",
                tool.name, tool.description, tool.parameters
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "You can call tools. Reply with exactly one JSON object and nothing else.\nTo call a tool: {{\"tool\": \"<name>\", \"arguments\": {{...}}}}\nTo answer: {{\"answer\": \"<markdown>\"}}\nTools:\n{listing}"
    )
}

/// One reply on the JSON protocol: a tool call or the answer. The error says what was wrong, so
/// it can be sent back to the model.
pub fn parse_json_turn(raw: &str, tools: &[ToolSpec]) -> std::result::Result<(Option<ToolCall>, String), String> {
    let object = raw
        .find('{')
        .zip(raw.rfind('}'))
        .and_then(|(start, end)| (start < end).then(|| &raw[start..=end]))
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
        .filter(Value::is_object)
        .ok_or_else(|| "the reply was not a JSON object".to_owned())?;
    if let Some(answer) = object.get("answer").and_then(Value::as_str) {
        return Ok((None, answer.trim().to_owned()));
    }
    let Some(name) = object.get("tool").and_then(Value::as_str) else {
        return Err("the object has neither \"tool\" nor \"answer\"".to_owned());
    };
    if !tools.iter().any(|tool| tool.name == name) {
        let known = tools
            .iter()
            .map(|tool| tool.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!("there is no tool named \"{name}\"; the tools are: {known}"));
    }
    Ok((
        Some(ToolCall {
            id: "call_0".to_owned(),
            name: name.to_owned(),
            arguments: parse_arguments(object.get("arguments")),
        }),
        String::new(),
    ))
}

/// What a model did in one turn of a tool loop: said something, called tools, or both.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolTurn {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Option<Usage>,
}

/// Messages rewritten for the JSON protocol, which has no tool role: a call becomes the assistant's
/// own JSON object and its result a user message.
fn flatten_for_json(messages: &[ChatMessage]) -> Vec<ChatMessage> {
    let mut names = Vec::new();
    messages
        .iter()
        .map(|message| match message.role {
            ChatRole::Assistant if !message.tool_calls.is_empty() => {
                let call = &message.tool_calls[0];
                names.push((call.id.clone(), call.name.clone()));
                ChatMessage::new(
                    ChatRole::Assistant,
                    json!({ "tool": call.name, "arguments": call.arguments }).to_string(),
                )
            }
            ChatRole::Tool => {
                let name = message
                    .tool_call_id
                    .as_ref()
                    .and_then(|id| names.iter().find(|(known, _)| known == id))
                    .map_or("tool", |(_, name)| name.as_str());
                ChatMessage::new(ChatRole::User, format!("Result of {name}:\n{}", message.content))
            }
            _ => ChatMessage::new(message.role, message.content.clone()),
        })
        .collect()
}

/// One turn on the JSON protocol. A reply that cannot be read is sent back once with what was
/// wrong; a second unreadable reply is an [`InvalidToolReply`].
pub async fn json_tool_turn<P: AiProvider + ?Sized>(provider: &P, request: ChatRequest) -> Result<ToolTurn> {
    let mut messages = flatten_for_json(&request.messages);
    let instructions = json_tool_instructions(&request.tools);
    match messages.first_mut().filter(|message| message.role == ChatRole::System) {
        Some(system) => system.content = format!("{}\n\n{instructions}", system.content),
        None => messages.insert(0, ChatMessage::new(ChatRole::System, instructions)),
    }
    let mut usage: Option<Usage> = None;
    let mut detail = String::new();
    for attempt in 0..2 {
        let response = provider
            .chat(ChatRequest {
                messages: messages.clone(),
                tools: Vec::new(),
                options: request.options.clone(),
            })
            .await?;
        usage = match (usage, response.usage) {
            (Some(total), Some(more)) => Some(total.add(more)),
            (total, more) => total.or(more),
        };
        match parse_json_turn(&response.content, &request.tools) {
            Ok((call, answer)) => {
                return Ok(ToolTurn {
                    content: answer,
                    tool_calls: call.into_iter().collect(),
                    usage,
                })
            }
            Err(error) => {
                detail = error;
                if attempt == 0 {
                    messages.push(ChatMessage::new(ChatRole::Assistant, response.content));
                    messages.push(ChatMessage::new(
                        ChatRole::User,
                        format!("That was not valid: {detail}. Reply again with exactly one JSON object."),
                    ));
                }
            }
        }
    }
    Err(anyhow::Error::new(InvalidToolReply { detail }))
}

/// One turn of a tool loop, in the way the model takes tools.
pub async fn tool_turn<P: AiProvider + ?Sized>(provider: &P, mode: ToolMode, request: ChatRequest) -> Result<ToolTurn> {
    match mode {
        ToolMode::Json if !request.tools.is_empty() => json_tool_turn(provider, request).await,
        ToolMode::Native => {
            let response = provider.chat(request).await?;
            Ok(ToolTurn {
                content: response.content,
                tool_calls: response.tool_calls,
                usage: response.usage,
            })
        }
        ToolMode::Json | ToolMode::None => {
            let response = provider
                .chat(ChatRequest {
                    tools: Vec::new(),
                    ..request
                })
                .await?;
            Ok(ToolTurn {
                content: response.content,
                tool_calls: Vec::new(),
                usage: response.usage,
            })
        }
    }
}

fn probe_tools() -> Vec<ToolSpec> {
    vec![ToolSpec {
        name: "echo".to_owned(),
        description: "Repeats the text it is given.".to_owned(),
        parameters: json!({
            "type": "object",
            "properties": { "text": { "type": "string" } },
            "required": ["text"],
        }),
    }]
}

fn probe_request() -> ChatRequest {
    ChatRequest {
        messages: vec![ChatMessage::new(
            ChatRole::User,
            "Use the echo tool with the text \"ok\". Do not answer in words.",
        )],
        tools: probe_tools(),
        options: json!({ "temperature": 0.0 }),
    }
}

/// Finds how a model takes tools with a tiny call: answered with a tool call it is `native`; a
/// valid reply on the JSON protocol makes it `json`; otherwise `none`.
pub async fn detect_tool_mode<P: AiProvider + ?Sized>(provider: &P) -> ToolMode {
    if let Ok(response) = provider.chat(probe_request()).await {
        if response.tool_calls.iter().any(|call| call.name == "echo") {
            return ToolMode::Native;
        }
    }
    match json_tool_turn(provider, probe_request()).await {
        Ok(_) => ToolMode::Json,
        Err(_) => ToolMode::None,
    }
}

/// The traits a test of the connection records: the tool mode and, when the provider says, the
/// context window.
pub async fn detect_model_traits<P: AiProvider + ?Sized>(provider: &P) -> ModelTraits {
    ModelTraits {
        tool_mode: detect_tool_mode(provider).await,
        context_tokens: provider
            .context_tokens()
            .await
            .ok()
            .flatten()
            .map_or(DEFAULT_CONTEXT_TOKENS, |tokens| {
                tokens.clamp(MIN_CONTEXT_TOKENS, MAX_CONTEXT_TOKENS)
            }),
    }
}

/// A provider that answers from a script, for tests of anything that talks to a model. It keeps
/// every request it received.
pub struct ScriptedProvider {
    replies: Mutex<VecDeque<Result<ChatResponse>>>,
    seen: Mutex<Vec<ChatRequest>>,
    context_tokens: Option<u32>,
}

impl ScriptedProvider {
    pub fn new(replies: Vec<ChatResponse>) -> Self {
        Self {
            replies: Mutex::new(replies.into_iter().map(Ok).collect()),
            seen: Mutex::new(Vec::new()),
            context_tokens: None,
        }
    }

    /// A script where a call can fail, as a model that rejects tools does.
    pub fn with_results(replies: Vec<Result<ChatResponse>>) -> Self {
        Self {
            replies: Mutex::new(replies.into()),
            seen: Mutex::new(Vec::new()),
            context_tokens: None,
        }
    }

    pub fn with_context_tokens(mut self, tokens: u32) -> Self {
        self.context_tokens = Some(tokens);
        self
    }

    pub fn requests(&self) -> Vec<ChatRequest> {
        self.seen.lock().map(|seen| seen.clone()).unwrap_or_default()
    }
}

impl AiProvider for ScriptedProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        if let Ok(mut seen) = self.seen.lock() {
            seen.push(request);
        }
        match self.replies.lock().ok().and_then(|mut replies| replies.pop_front()) {
            Some(reply) => reply,
            None => bail!("the script has no more replies"),
        }
    }

    async fn generate(&self, request: GenerateRequest) -> Result<String> {
        Ok(self.chat(request.to_chat()).await?.content)
    }

    async fn context_tokens(&self) -> Result<Option<u32>> {
        Ok(self.context_tokens)
    }

    async fn embed(&self, _texts: Vec<String>, _options: Value) -> Result<Vec<Vec<f32>>> {
        bail!("a scripted provider has no embeddings")
    }

    async fn summarize(&self, target: String, _options: Value) -> Result<String> {
        Ok(target)
    }

    async fn rerank(&self, _query: String, candidates: Vec<String>) -> Result<Vec<usize>> {
        Ok((0..candidates.len()).collect())
    }

    async fn analyze_image(&self, _request: ImageAnalysisRequest) -> Result<String> {
        bail!("a scripted provider has no vision")
    }

    async fn test_connection(&self) -> Result<ProviderTestResult> {
        Ok(ProviderTestResult {
            provider_id: "scripted".to_owned(),
            success: true,
            message: "Scripted.".to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn echo_only() -> Vec<ToolSpec> {
        probe_tools()
    }

    fn request_with_tools() -> ChatRequest {
        ChatRequest {
            messages: vec![
                ChatMessage::new(ChatRole::System, "Be brief."),
                ChatMessage::new(ChatRole::User, "Echo ok."),
            ],
            tools: echo_only(),
            options: json!({}),
        }
    }

    #[test]
    fn a_model_defaults_to_agent_mode_only_when_it_is_listed() {
        let listed = vec!["llama3.1:8b".to_owned(), "openai/gpt-4o-mini".to_owned()];
        assert!(model_is_listed(&listed, Some(" OpenAI/GPT-4o-mini ")));
        assert!(!model_is_listed(&listed, Some("llama3.2")));
        assert!(!model_is_listed(&listed, Some("")));
        assert!(!model_is_listed(&listed, None));
        assert!(
            !model_is_listed(&[], Some("llama3.1:8b")),
            "no report on record, no default"
        );
    }

    #[test]
    fn tools_are_sent_in_each_providers_shape() {
        let request = request_with_tools();
        let open = openrouter_chat_body("m", &request);
        assert_eq!(open["tools"][0]["type"], "function");
        assert_eq!(open["tools"][0]["function"]["name"], "echo");
        let ollama = ollama_chat_body("m", &request);
        assert_eq!(ollama["tools"][0]["function"]["parameters"]["required"][0], "text");
        let plain = ChatRequest {
            tools: Vec::new(),
            ..request
        };
        assert!(openrouter_chat_body("m", &plain).get("tools").is_none());
        assert!(ollama_chat_body("m", &plain).get("tools").is_none());
    }

    #[test]
    fn a_call_and_its_result_go_back_to_the_model() {
        let call = ToolCall {
            id: "call_7".to_owned(),
            name: "echo".to_owned(),
            arguments: json!({ "text": "ok" }),
        };
        let messages = vec![
            ChatMessage::assistant_calls("", vec![call]),
            ChatMessage::tool_result("call_7", "ok"),
        ];
        let request = ChatRequest {
            messages,
            tools: Vec::new(),
            options: json!({}),
        };
        let open = openrouter_chat_body("m", &request);
        assert_eq!(open["messages"][0]["tool_calls"][0]["id"], "call_7");
        assert_eq!(
            open["messages"][0]["tool_calls"][0]["function"]["arguments"],
            "{\"text\":\"ok\"}"
        );
        assert_eq!(
            open["messages"][1],
            json!({ "role": "tool", "tool_call_id": "call_7", "content": "ok" })
        );
        let ollama = ollama_chat_body("m", &request);
        assert_eq!(
            ollama["messages"][0]["tool_calls"][0]["function"]["arguments"]["text"],
            "ok"
        );
        assert_eq!(ollama["messages"][1], json!({ "role": "tool", "content": "ok" }));
    }

    #[test]
    fn tool_calls_are_read_with_string_or_object_arguments() {
        let open = openrouter_chat_response(&json!({
            "choices": [{ "message": { "content": null, "tool_calls": [
                { "id": "abc", "type": "function", "function": { "name": "echo", "arguments": "{\"text\":\"hi\"}" } }
            ] } }]
        }));
        assert_eq!(open.tool_calls.len(), 1);
        assert_eq!(open.tool_calls[0].id, "abc");
        assert_eq!(open.tool_calls[0].arguments, json!({ "text": "hi" }));
        let ollama = ollama_chat_response(&json!({
            "message": { "content": "", "tool_calls": [
                { "function": { "name": "echo", "arguments": { "text": "yo" } } },
                { "function": { "name": "" , "arguments": {} } }
            ] }
        }));
        assert_eq!(ollama.tool_calls.len(), 1);
        assert_eq!(ollama.tool_calls[0].id, "call_0");
        assert_eq!(ollama.tool_calls[0].arguments, json!({ "text": "yo" }));
        let broken = openrouter_chat_response(&json!({
            "choices": [{ "message": { "tool_calls": [
                { "id": "x", "function": { "name": "echo", "arguments": "not json" } }
            ] } }]
        }));
        assert_eq!(broken.tool_calls[0].arguments, json!("not json"));
    }

    #[test]
    fn traits_are_read_from_metadata_and_written_back() {
        let metadata = json!({ "tool_mode": "native", "context_tokens": 128000 });
        let traits = ModelTraits::from_metadata(&metadata);
        assert_eq!(traits.tool_mode, ToolMode::Native);
        assert_eq!(traits.context_tokens, 128_000);
        assert!(ModelTraits::is_known(&metadata));
        let unknown = ModelTraits::from_metadata(&json!({}));
        assert_eq!(unknown, ModelTraits::default());
        assert!(!ModelTraits::is_known(&json!({})));
        assert_eq!(
            ModelTraits::from_metadata(&json!({ "context_tokens": 3 })).context_tokens,
            512
        );
        let mut target = json!({ "purpose": "text" });
        traits.write_to(&mut target);
        assert_eq!(
            target,
            json!({ "purpose": "text", "tool_mode": "native", "context_tokens": 128000 })
        );
    }

    #[tokio::test]
    async fn native_tool_calls_are_returned() {
        let provider = ScriptedProvider::new(vec![ChatResponse::call("echo", json!({ "text": "ok" }))]);
        let turn = tool_turn(&provider, ToolMode::Native, request_with_tools())
            .await
            .unwrap();
        assert_eq!(turn.tool_calls[0].name, "echo");
        assert_eq!(provider.requests()[0].tools.len(), 1);
    }

    #[tokio::test]
    async fn a_json_protocol_call_is_read() {
        let provider = ScriptedProvider::new(vec![ChatResponse::text(
            "```json\n{\"tool\": \"echo\", \"arguments\": {\"text\": \"ok\"}}\n```",
        )]);
        let turn = tool_turn(&provider, ToolMode::Json, request_with_tools())
            .await
            .unwrap();
        assert_eq!(turn.tool_calls[0].name, "echo");
        assert_eq!(turn.tool_calls[0].arguments, json!({ "text": "ok" }));
        let sent = &provider.requests()[0];
        assert!(sent.tools.is_empty());
        assert!(sent.messages[0].content.contains("exactly one JSON object"));
        assert!(sent.messages[0].content.contains("echo"));
    }

    #[tokio::test]
    async fn an_invalid_json_reply_is_repaired_once() {
        let provider = ScriptedProvider::new(vec![
            ChatResponse::text("Sure, I will echo it."),
            ChatResponse::text("{\"answer\": \"ok\"}"),
        ]);
        let turn = tool_turn(&provider, ToolMode::Json, request_with_tools())
            .await
            .unwrap();
        assert_eq!(turn.content, "ok");
        assert!(turn.tool_calls.is_empty());
        let second = &provider.requests()[1];
        assert_eq!(second.messages.last().unwrap().role, ChatRole::User);
        assert!(second.messages.last().unwrap().content.contains("not a JSON object"));
    }

    #[tokio::test]
    async fn two_invalid_json_replies_fail_in_a_way_the_loop_can_recognise() {
        let provider = ScriptedProvider::new(vec![
            ChatResponse::text("no"),
            ChatResponse::text("{\"tool\": \"missing\"}"),
        ]);
        let error = tool_turn(&provider, ToolMode::Json, request_with_tools())
            .await
            .unwrap_err();
        let invalid = error.downcast_ref::<InvalidToolReply>().expect("an InvalidToolReply");
        assert!(invalid.detail.contains("no tool named \"missing\""));
    }

    #[tokio::test]
    async fn a_model_without_tools_is_asked_without_them() {
        let provider = ScriptedProvider::new(vec![ChatResponse::text("Hello")]);
        let turn = tool_turn(&provider, ToolMode::None, request_with_tools())
            .await
            .unwrap();
        assert_eq!(turn.content, "Hello");
        assert!(provider.requests()[0].tools.is_empty());
    }

    #[tokio::test]
    async fn tool_results_become_user_messages_on_the_json_protocol() {
        let mut request = request_with_tools();
        request.messages.push(ChatMessage::assistant_calls(
            "",
            vec![ToolCall {
                id: "c1".to_owned(),
                name: "echo".to_owned(),
                arguments: json!({ "text": "ok" }),
            }],
        ));
        request.messages.push(ChatMessage::tool_result("c1", "ok"));
        let provider = ScriptedProvider::new(vec![ChatResponse::text("{\"answer\": \"done\"}")]);
        tool_turn(&provider, ToolMode::Json, request).await.unwrap();
        let sent = &provider.requests()[0].messages;
        assert_eq!(sent[3].role, ChatRole::User);
        assert_eq!(sent[3].content, "Result of echo:\nok");
        assert_eq!(sent[2].role, ChatRole::Assistant);
    }

    #[tokio::test]
    async fn detection_tells_native_json_and_none_apart() {
        let native = ScriptedProvider::new(vec![ChatResponse::call("echo", json!({ "text": "ok" }))]);
        assert_eq!(detect_tool_mode(&native).await, ToolMode::Native);

        let json_only = ScriptedProvider::with_results(vec![
            Err(anyhow::anyhow!("this model does not support tools")),
            Ok(ChatResponse::text(
                "{\"tool\": \"echo\", \"arguments\": {\"text\": \"ok\"}}",
            )),
        ]);
        assert_eq!(detect_tool_mode(&json_only).await, ToolMode::Json);

        let neither = ScriptedProvider::new(vec![
            ChatResponse::text("ok"),
            ChatResponse::text("ok"),
            ChatResponse::text("ok"),
        ]);
        assert_eq!(detect_tool_mode(&neither).await, ToolMode::None);
    }

    #[tokio::test]
    async fn detected_traits_carry_the_context_size() {
        let provider = ScriptedProvider::new(vec![ChatResponse::call("echo", json!({}))]).with_context_tokens(32_000);
        let traits = detect_model_traits(&provider).await;
        assert_eq!(traits.tool_mode, ToolMode::Native);
        assert_eq!(traits.context_tokens, 32_000);
        let unknown = ScriptedProvider::new(vec![ChatResponse::call("echo", json!({}))]);
        assert_eq!(
            detect_model_traits(&unknown).await.context_tokens,
            DEFAULT_CONTEXT_TOKENS
        );
    }
}
