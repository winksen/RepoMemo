//! The bounded tool loop (V0.9.40). In *Agent* mode CAIRN is given the read tools and the proposing jobs as tools of the person's
//! role and calls them in turns until it answers, stops at a proposal, or reaches a budget: a number of model calls, a time, a
//! share of the model's window and the person's AI allowance, which each model call uses. Reading never changes anything and a
//! proposal waits for the person to confirm it, so the loop adds no way to change the workspace.
//!
//! Chatter, jobs picked from the + menu and workspaces without a tool-capable model keep the router.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use repomemo_ai::{
    provider_from_settings, tool_turn, AiProvider, ChatMessage, ChatRequest, ChatRole, InvalidToolReply, ToolCall,
    ToolMode, ToolSpec, Usage,
};
use repomemo_domain::{
    AgentCapability, AgentHistoryTurn, AgentReply, AgentRequest, AgentRouting, AgentStep, AgentStepEvidence,
    ArtifactSummary, MemoryCardSummary,
};
use serde_json::{json, Value};

use super::tools::atoms::{
    EvidenceLedger, ToolOutput, EVIDENCE_SEARCH_MAX, LIST_FILES_MAX, READ_FILE_CHUNKS_MAX, TOOL_OUTPUT_CHARS,
};
use super::tools::{tool_for, CairnTool};
use super::trace::RunTrace;
use super::{
    cairn_persona, fit_history, shorten, ASK_SOURCE_RULE, FILE_RESULT_LIMIT, MAX_MESSAGE_CHARS, MEMORY_CARDS_SHOWN,
};
use crate::RepoMemoCore;

/// What the server gives a run: how the model takes tools, the budgets, what the person may do and how to pay for a model call.
#[derive(Clone)]
pub struct AgentLoopSetup {
    pub tool_mode: ToolMode,
    pub context_tokens: u32,
    /// The most model calls one run makes.
    pub max_steps: usize,
    pub max_seconds: u64,
    /// Whether the person may change the workspace: a viewer's tool list holds no proposing tool.
    pub can_write: bool,
    /// Called before each model call. An error (the person's AI allowance is used up) ends the run as partial with that message.
    pub consume_quota: Arc<dyn Fn() -> std::result::Result<(), String> + Send + Sync>,
}

/// A message answered: the reply, and for a run of the loop its own trace (the router's runs are traced from the reply).
pub struct AgentOutcome {
    pub reply: AgentReply,
    pub trace: Option<RunTrace>,
}

/// Where a tool of the loop comes from.
enum Source {
    Atom,
    Job(&'static CairnTool),
}

struct LoopTool {
    name: &'static str,
    label: &'static str,
    description: String,
    parameters: Value,
    proposes: bool,
    source: Source,
}

impl LoopTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: self.name.to_owned(),
            description: self.description.clone(),
            parameters: self.parameters.clone(),
        }
    }
}

fn job(capability: AgentCapability) -> LoopTool {
    let tool = tool_for(capability);
    LoopTool {
        name: tool.name,
        label: tool.label,
        description: if tool.writes() {
            format!(
                "{} It only proposes: nothing changes until the person confirms. Give the request in plain words as the person would say it.",
                tool.description
            )
        } else {
            tool.description.to_owned()
        },
        parameters: (tool.parameters)(),
        proposes: tool.writes(),
        source: Source::Job(tool),
    }
}

fn atom(name: &'static str, label: &'static str, description: &str, parameters: Value) -> LoopTool {
    LoopTool {
        name,
        label,
        description: description.to_owned(),
        parameters,
        proposes: false,
        source: Source::Atom,
    }
}

/// The tools a run of this person's role may call. A person who cannot change the workspace is offered no proposing tool.
fn loop_tools(can_write: bool) -> Vec<LoopTool> {
    let mut tools = vec![
        atom(
            "search_evidence",
            "Search evidence",
            "Search the workspace's indexed files for passages that match some words or a question. Each passage comes with a handle such as E3 that you cite.",
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Words or a short question" },
                    "kinds": { "type": "array", "items": { "type": "string" }, "description": "File kinds such as pdf, word, markdown" },
                    "folder": { "type": "string", "description": "A folder name or path to look in" },
                    "limit": { "type": "integer", "description": format!("At most {EVIDENCE_SEARCH_MAX}") }
                },
                "required": ["query"]
            }),
        ),
        atom(
            "read_file",
            "Read file",
            "Read one file's text a few chunks at a time. Name it by title, path or a handle you were given.",
            json!({
                "type": "object",
                "properties": {
                    "file": { "type": "string" },
                    "from_chunk": { "type": "integer", "description": "Where to start, from 0" },
                    "chunks": { "type": "integer", "description": format!("At most {READ_FILE_CHUNKS_MAX}") }
                },
                "required": ["file"]
            }),
        ),
        atom(
            "list_files",
            "List files",
            "List the files a description means, such as \"the pdfs in Finance\" or \"markdown files named runbook\".",
            json!({
                "type": "object",
                "properties": {
                    "filter": { "type": "string" },
                    "limit": { "type": "integer", "description": format!("At most {LIST_FILES_MAX}") }
                },
                "required": ["filter"]
            }),
        ),
        atom(
            "file_facts",
            "File facts",
            "What is known about one file besides its text: its review state, owner, reviewer, versions, source and what names it.",
            json!({ "type": "object", "properties": { "file": { "type": "string" } }, "required": ["file"] }),
        ),
        atom(
            "workspace_brief",
            "Workspace brief",
            "What the workspace holds: its size, review states and main folders.",
            json!({ "type": "object", "properties": {} }),
        ),
        job(AgentCapability::FindSymbol),
        job(AgentCapability::RecallMemory),
    ];
    if can_write {
        tools.extend([
            job(AgentCapability::SaveMemory),
            job(AgentCapability::ManageMemory),
            job(AgentCapability::ManageFiles),
            job(AgentCapability::CreateNote),
        ]);
    }
    tools
}

/// The loop's rules, after CAIRN's persona and before the conversation.
fn loop_system(tools: &[LoopTool]) -> String {
    format!(
        "{} You work with tools: {}. Use them to look before you answer anything about the workspace, and keep calling them until you can answer; do not guess. Cite the evidence you rely on with its handle in square brackets, such as [E3], and only handles that a tool gave you. Say so when you could not find something. To change anything, call a proposing tool (its name starts with propose_) and say what you proposed; you cannot change anything yourself, and the person confirms. {ASK_SOURCE_RULE} Answer in the language the person writes in.",
        cairn_persona(),
        tools
            .iter()
            .map(|tool| tool.name)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// The earlier turns as the model's own conversation.
fn history_messages(history: &[AgentHistoryTurn]) -> Vec<ChatMessage> {
    history
        .iter()
        .flat_map(|turn| {
            [
                ChatMessage::new(ChatRole::User, turn.question.clone()),
                ChatMessage::new(ChatRole::Assistant, turn.answer.clone()),
            ]
        })
        .collect()
}

fn estimate_tokens(messages: &[ChatMessage]) -> usize {
    messages
        .iter()
        .map(|message| {
            message.content.chars().count()
                + message
                    .tool_calls
                    .iter()
                    .map(|call| call.arguments.to_string().chars().count() + call.name.len())
                    .sum::<usize>()
        })
        .sum::<usize>()
        / 4
}

const OMITTED: &str = "[an earlier result was left out to keep within the model's window]";

/// Keeps the conversation within half of the model's window: the oldest tool results are replaced by a note until it fits. The
/// system message, the person's messages and the newest results stay.
fn fit_to_budget(messages: &mut [ChatMessage], context_tokens: u32) {
    let budget = (context_tokens as usize / 2).max(256);
    for index in 0..messages.len() {
        if estimate_tokens(messages) <= budget {
            return;
        }
        // Never the newest tool message: the model is about to read it.
        let newest_tool = messages.iter().rposition(|message| message.role == ChatRole::Tool);
        if messages[index].role == ChatRole::Tool && Some(index) != newest_tool && messages[index].content != OMITTED {
            messages[index].content = OMITTED.to_owned();
        }
    }
}

fn arg_text(arguments: &Value, key: &str) -> String {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .to_owned()
}

fn arg_number(arguments: &Value, key: &str, default: usize) -> usize {
    arguments
        .get(key)
        .and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_str().and_then(|text| text.trim().parse().ok()))
        })
        .map_or(default, |value| value as usize)
}

fn arg_list(arguments: &Value, key: &str) -> Vec<String> {
    match arguments.get(key) {
        Some(Value::Array(items)) => items.iter().filter_map(Value::as_str).map(str::to_owned).collect(),
        Some(Value::String(text)) if !text.trim().is_empty() => vec![text.trim().to_owned()],
        _ => Vec::new(),
    }
}

/// What a run gathers besides the model's words.
#[derive(Default)]
struct Gathered {
    files: Vec<ArtifactSummary>,
    memory_cards: Vec<MemoryCardSummary>,
    proposals: Vec<repomemo_domain::AgentProposal>,
    warnings: Vec<String>,
    steps: Vec<AgentStep>,
    usage: Usage,
}

impl Gathered {
    fn step(
        &mut self,
        kind: &str,
        tool: Option<&str>,
        summary: String,
        evidence: Vec<AgentStepEvidence>,
        duration: Duration,
        error: Option<String>,
    ) {
        self.steps.push(AgentStep {
            position: self.steps.len() as i64 + 1,
            kind: kind.to_owned(),
            tool: tool.map(str::to_owned),
            summary: shorten(&summary, 160),
            evidence,
            duration_ms: duration.as_millis() as i64,
            error,
        });
    }
}

fn evidence_of(ledger: &EvidenceLedger, handles: &[String]) -> Vec<AgentStepEvidence> {
    let mut seen = Vec::<String>::new();
    let mut out = Vec::new();
    for handle in handles {
        if let Some(item) = ledger.get(handle) {
            if !seen.contains(&item.artifact_id) && out.len() < 12 {
                seen.push(item.artifact_id.clone());
                out.push(AgentStepEvidence {
                    artifact_id: item.artifact_id.clone(),
                    title: item.title.clone(),
                });
            }
        }
    }
    out
}

fn finish(
    request: &AgentRequest,
    markdown: String,
    gathered: Gathered,
    state: &str,
    history_turns: usize,
) -> AgentOutcome {
    let mut files = gathered.files;
    files.truncate(FILE_RESULT_LIMIT);
    let reply = AgentReply {
        capability: None,
        routing: AgentRouting::Agent,
        subject: shorten(request.message.trim(), 120),
        reply_markdown: markdown,
        generated: true,
        files,
        matches: Vec::new(),
        citations: Vec::new(),
        warnings: gathered.warnings,
        context_turns: history_turns as i64,
        proposals: gathered.proposals,
        file_set: None,
        memory_cards: gathered.memory_cards,
    };
    AgentOutcome {
        reply,
        trace: Some(RunTrace {
            state: state.to_owned(),
            steps: gathered.steps,
            input_tokens: i64::from(gathered.usage.input_tokens),
            output_tokens: i64::from(gathered.usage.output_tokens),
        }),
    }
}

/// What CAIRN says when a budget ends a run: what it did, and what is missing.
fn partial_markdown(why: &str, ran: &[String], draft: &str) -> String {
    let did = if ran.is_empty() {
        "I did not get to run a tool.".to_owned()
    } else {
        format!("I ran: {}.", ran.join(", "))
    };
    let draft = draft.trim();
    let found = if draft.is_empty() {
        String::new()
    } else {
        format!("\n\nWhat I have so far:\n\n{draft}")
    };
    format!(
        "I stopped before I finished, because {why}. {did} What is missing is a complete answer to your question from what I read; ask again, more narrowly, or tell me to continue.{found}"
    )
}

impl RepoMemoCore {
    /// Runs one tool of the loop and says what the model should read. A tool that does not exist, that a viewer may not use or
    /// that fails is answered to the model as an error it can recover from.
    async fn call_loop_tool(
        &self,
        tools: &[LoopTool],
        call: &ToolCall,
        request: &AgentRequest,
        history: &[AgentHistoryTurn],
        ledger: &mut EvidenceLedger,
        gathered: &mut Gathered,
    ) -> (String, Vec<AgentStepEvidence>, Option<String>, String) {
        let known = tools.iter().map(|tool| tool.name).collect::<Vec<_>>().join(", ");
        let Some(tool) = tools.iter().find(|tool| tool.name == call.name) else {
            let problem = format!("There is no tool named \"{}\". The tools are: {known}.", call.name);
            return (
                problem.clone(),
                Vec::new(),
                Some(problem),
                format!("Asked for an unknown tool \"{}\"", shorten(&call.name, 40)),
            );
        };
        let arguments = &call.arguments;
        let subject = arg_text(arguments, "subject");
        let described = match tool.name {
            "search_evidence" => arg_text(arguments, "query"),
            "read_file" | "file_facts" => arg_text(arguments, "file"),
            "list_files" => arg_text(arguments, "filter"),
            "workspace_brief" => String::new(),
            _ => subject.clone(),
        };
        let summary = if described.is_empty() {
            tool.label.to_owned()
        } else {
            format!("{}: {described}", tool.label)
        };
        let outcome: Result<ToolOutput> = match tool.name {
            "search_evidence" => {
                self.tool_search_evidence(
                    ledger,
                    &arg_text(arguments, "query"),
                    &arg_list(arguments, "kinds"),
                    Some(arg_text(arguments, "folder")).as_deref(),
                    arg_number(arguments, "limit", EVIDENCE_SEARCH_MAX),
                )
                .await
            }
            "read_file" => {
                self.tool_read_file(
                    ledger,
                    &arg_text(arguments, "file"),
                    arg_number(arguments, "from_chunk", 0),
                    arg_number(arguments, "chunks", 3),
                )
                .await
            }
            "list_files" => {
                self.tool_list_files(
                    ledger,
                    &arg_text(arguments, "filter"),
                    arg_number(arguments, "limit", 20),
                )
                .await
            }
            "file_facts" => self.tool_file_facts(ledger, &arg_text(arguments, "file")).await,
            "workspace_brief" => self.tool_workspace_brief(ledger).await,
            _ => {
                let Source::Job(job) = &tool.source else {
                    return ("This tool is not available.".to_owned(), Vec::new(), None, summary);
                };
                match self.run_tool(job.capability, request, &subject, history).await {
                    Ok(reply) => {
                        for file in reply.files {
                            if !gathered.files.iter().any(|known| known.id == file.id) {
                                gathered.files.push(file);
                            }
                        }
                        for card in reply.memory_cards {
                            if gathered.memory_cards.len() < MEMORY_CARDS_SHOWN
                                && !gathered.memory_cards.iter().any(|known| known.id == card.id)
                            {
                                gathered.memory_cards.push(card);
                            }
                        }
                        let proposed = !reply.proposals.is_empty();
                        gathered.proposals.extend(reply.proposals);
                        let mut text = reply.reply_markdown;
                        if proposed {
                            text.push_str("\n\n(This is a proposal. Nothing changes until the person confirms it.)");
                        }
                        Ok(ToolOutput {
                            for_model: shorten(&text, TOOL_OUTPUT_CHARS),
                            handles: Vec::new(),
                        })
                    }
                    Err(error) => Err(error),
                }
            }
        };
        match outcome {
            Ok(output) => {
                let evidence = evidence_of(ledger, &output.handles);
                (output.for_model, evidence, None, summary)
            }
            Err(error) => {
                let problem = format!("The tool failed: {error:#}");
                (problem.clone(), Vec::new(), Some(shorten(&problem, 200)), summary)
            }
        }
    }

    /// Answers a message with the loop. See the module's description.
    pub(in crate::agent) async fn run_loop(
        &self,
        request: AgentRequest,
        setup: AgentLoopSetup,
    ) -> Result<AgentOutcome> {
        let message = request.message.trim().to_owned();
        if message.is_empty() {
            repomemo_domain::bail_invalid!("A message is required.");
        }
        if message.chars().count() > MAX_MESSAGE_CHARS {
            repomemo_domain::bail_invalid!("Messages must be between 1 and {MAX_MESSAGE_CHARS} characters.");
        }
        let provider_id = request.provider_id.clone().unwrap_or_default();
        let settings = self.storage.get_provider_settings(&provider_id).await?;
        if !settings.enabled || settings.workspace_id.as_deref() != Some(request.workspace_id.as_str()) {
            repomemo_domain::bail_invalid!("The selected provider is not enabled for this workspace.");
        }
        let provider = provider_from_settings(settings)?;
        self.loop_with(&provider, request, setup).await
    }

    /// The loop over any provider, so that it can be run against a scripted one.
    pub(in crate::agent) async fn loop_with<P: AiProvider + ?Sized>(
        &self,
        provider: &P,
        request: AgentRequest,
        setup: AgentLoopSetup,
    ) -> Result<AgentOutcome> {
        let message = request.message.trim().to_owned();
        let history = fit_history(&request.history);
        let tools = loop_tools(setup.can_write);
        let specs = tools.iter().map(LoopTool::spec).collect::<Vec<_>>();
        let mut messages = vec![ChatMessage::new(ChatRole::System, loop_system(&tools))];
        messages.extend(history_messages(&history));
        messages.push(ChatMessage::new(ChatRole::User, message.clone()));

        let started = Instant::now();
        let limit = Duration::from_secs(setup.max_seconds.max(1));
        let mut ledger = EvidenceLedger::new(&request.workspace_id);
        let mut gathered = Gathered::default();
        let mut ran: Vec<String> = Vec::new();
        let mut draft = String::new();

        for step in 1..=setup.max_steps.max(1) {
            if started.elapsed() >= limit {
                let why = format!("I used the {} seconds I am given for one request", setup.max_seconds);
                let text = partial_markdown(&why, &ran, &draft);
                gathered.step(
                    "answer",
                    None,
                    "Stopped at the time limit".to_owned(),
                    Vec::new(),
                    Duration::ZERO,
                    None,
                );
                return Ok(finish(&request, text, gathered, "partial", history.len()));
            }
            if let Err(reason) = (setup.consume_quota)() {
                let text = partial_markdown(&format!("your AI allowance is used up ({reason})"), &ran, &draft);
                gathered.step(
                    "answer",
                    None,
                    "Stopped: AI allowance used up".to_owned(),
                    Vec::new(),
                    Duration::ZERO,
                    None,
                );
                return Ok(finish(&request, text, gathered, "partial", history.len()));
            }
            fit_to_budget(&mut messages, setup.context_tokens);
            let asked = Instant::now();
            let turn = tool_turn(
                provider,
                setup.tool_mode,
                ChatRequest {
                    messages: messages.clone(),
                    tools: specs.clone(),
                    options: json!({ "temperature": 0.2 }),
                },
            )
            .await;
            let turn = match turn {
                Ok(turn) => turn,
                Err(error) if error.downcast_ref::<InvalidToolReply>().is_some() => {
                    let text = partial_markdown(
                        "the model did not answer in the format I need to use my tools",
                        &ran,
                        &draft,
                    );
                    gathered.step(
                        "answer",
                        None,
                        "Stopped: the model's reply could not be read".to_owned(),
                        Vec::new(),
                        asked.elapsed(),
                        Some(format!("{error:#}")),
                    );
                    return Ok(finish(&request, text, gathered, "partial", history.len()));
                }
                Err(error) => {
                    // Provider problems are part of the conversation, as for the router's jobs.
                    gathered.warnings.push(format!("{error:#}"));
                    gathered.step(
                        "answer",
                        None,
                        "The model could not be reached".to_owned(),
                        Vec::new(),
                        asked.elapsed(),
                        Some(shorten(&format!("{error:#}"), 200)),
                    );
                    return Ok(finish(
                        &request,
                        super::trace::FAILED_REPLY.to_owned(),
                        gathered,
                        "failed",
                        history.len(),
                    ));
                }
            };
            if let Some(usage) = turn.usage {
                gathered.usage = Usage {
                    input_tokens: gathered.usage.input_tokens.saturating_add(usage.input_tokens),
                    output_tokens: gathered.usage.output_tokens.saturating_add(usage.output_tokens),
                };
            }
            if turn.tool_calls.is_empty() {
                let answer = turn.content.trim().to_owned();
                if answer.is_empty() {
                    let text = partial_markdown("the model gave no answer", &ran, &draft);
                    gathered.step(
                        "answer",
                        None,
                        "Stopped: the model gave no answer".to_owned(),
                        Vec::new(),
                        asked.elapsed(),
                        None,
                    );
                    return Ok(finish(&request, text, gathered, "partial", history.len()));
                }
                gathered.step(
                    "answer",
                    None,
                    format!(
                        "Wrote the answer after {step} {}",
                        if step == 1 { "model call" } else { "model calls" }
                    ),
                    Vec::new(),
                    asked.elapsed(),
                    None,
                );
                return Ok(finish(&request, answer, gathered, "answered", history.len()));
            }
            if !turn.content.trim().is_empty() {
                draft = turn.content.trim().to_owned();
            }
            messages.push(ChatMessage::assistant_calls(
                turn.content.clone(),
                turn.tool_calls.clone(),
            ));
            let mut proposed = false;
            for call in &turn.tool_calls {
                let called = Instant::now();
                let (for_model, evidence, error, summary) = self
                    .call_loop_tool(&tools, call, &request, &history, &mut ledger, &mut gathered)
                    .await;
                let is_proposal = tools.iter().any(|tool| tool.name == call.name && tool.proposes) && error.is_none();
                gathered.step(
                    if is_proposal { "proposal" } else { "tool" },
                    Some(&call.name),
                    summary,
                    evidence,
                    called.elapsed(),
                    error,
                );
                ran.push(call.name.clone());
                messages.push(ChatMessage::tool_result(call.id.clone(), for_model));
                proposed |= is_proposal;
            }
            if proposed && !gathered.proposals.is_empty() {
                let text = if turn.content.trim().is_empty() {
                    "I prepared this for you. Nothing changes until you confirm it.".to_owned()
                } else {
                    turn.content.trim().to_owned()
                };
                gathered.step(
                    "answer",
                    None,
                    "Waiting for you to confirm what it proposed".to_owned(),
                    Vec::new(),
                    Duration::ZERO,
                    None,
                );
                return Ok(finish(&request, text, gathered, "awaiting_confirmation", history.len()));
            }
        }
        let why = format!("I reached my limit of {} steps for one request", setup.max_steps.max(1));
        let text = partial_markdown(&why, &ran, &draft);
        gathered.step(
            "answer",
            None,
            "Stopped at the step limit".to_owned(),
            Vec::new(),
            Duration::ZERO,
            None,
        );
        Ok(finish(&request, text, gathered, "partial", history.len()))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use repomemo_ai::{ChatResponse, ScriptedProvider};

    use super::*;

    async fn core(label: &str) -> (RepoMemoCore, std::path::PathBuf) {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let data_dir = std::env::temp_dir().join(format!("repomemo-loop-{label}-{nanos}"));
        (RepoMemoCore::boot(data_dir.clone()).await.unwrap(), data_dir)
    }

    async fn workspace_with_a_note(core: &RepoMemoCore) -> String {
        let workspace = core.create_workspace("Loop".to_owned()).await.unwrap();
        let file = core
            .import_text(
                workspace.id.clone(),
                "retry-policy.md".to_owned(),
                "# Retries\n\nRetries wait twice as long each time, at most five times.".to_owned(),
                Some("Markdown".to_owned()),
            )
            .await
            .unwrap();
        core.index_artifact(file.id).await.unwrap();
        workspace.id
    }

    fn request(workspace_id: &str, message: &str) -> AgentRequest {
        AgentRequest {
            workspace_id: workspace_id.to_owned(),
            message: message.to_owned(),
            capability: None,
            artifact_id: None,
            provider_id: Some("scripted".to_owned()),
            ai_unavailable_reason: None,
            history: Vec::new(),
            artifact_ids: Vec::new(),
            exchanges: Vec::new(),
        }
    }

    fn setup(max_steps: usize, can_write: bool) -> AgentLoopSetup {
        AgentLoopSetup {
            tool_mode: ToolMode::Native,
            context_tokens: 8_192,
            max_steps,
            max_seconds: 60,
            can_write,
            consume_quota: Arc::new(|| Ok(())),
        }
    }

    #[tokio::test]
    async fn a_two_step_request_runs_both_tools_and_answers_with_their_results() {
        let (core, data_dir) = core("chain").await;
        let workspace_id = workspace_with_a_note(&core).await;
        let provider = ScriptedProvider::new(vec![
            ChatResponse::call("search_evidence", json!({ "query": "retries" })),
            ChatResponse::call("read_file", json!({ "file": "retry-policy.md" })),
            ChatResponse::text("Retries wait twice as long each time [E1]."),
        ]);
        let outcome = core
            .loop_with(
                &provider,
                request(&workspace_id, "How do retries work?"),
                setup(8, true),
            )
            .await
            .unwrap();
        assert_eq!(
            outcome.reply.reply_markdown,
            "Retries wait twice as long each time [E1]."
        );
        assert_eq!(outcome.reply.routing, AgentRouting::Agent);
        let trace = outcome.trace.unwrap();
        assert_eq!(trace.state, "answered");
        assert_eq!(
            trace.steps.iter().map(|step| step.kind.as_str()).collect::<Vec<_>>(),
            ["tool", "tool", "answer"]
        );
        assert_eq!(trace.steps[0].tool.as_deref(), Some("search_evidence"));
        assert_eq!(trace.steps[0].evidence[0].title, "retry-policy.md");
        // The model was shown both results: the third request holds two tool messages.
        let requests = provider.requests();
        assert_eq!(requests.len(), 3);
        let tool_messages = requests[2]
            .messages
            .iter()
            .filter(|message| message.role == ChatRole::Tool)
            .collect::<Vec<_>>();
        assert_eq!(tool_messages.len(), 2);
        assert!(tool_messages[1].content.contains("twice as long"));
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn an_unknown_tool_is_answered_to_the_model_as_an_error_it_can_recover_from() {
        let (core, data_dir) = core("unknown").await;
        let workspace_id = workspace_with_a_note(&core).await;
        let provider = ScriptedProvider::new(vec![
            ChatResponse::call("delete_everything", json!({})),
            ChatResponse::text("I could not do that."),
        ]);
        let outcome = core
            .loop_with(&provider, request(&workspace_id, "Do something odd"), setup(8, true))
            .await
            .unwrap();
        let requests = provider.requests();
        let result = requests[1]
            .messages
            .iter()
            .find(|message| message.role == ChatRole::Tool)
            .unwrap();
        assert!(
            result.content.contains("There is no tool named \"delete_everything\""),
            "{}",
            result.content
        );
        let trace = outcome.trace.unwrap();
        assert!(trace.steps[0].error.is_some());
        assert_eq!(trace.state, "answered");
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn the_step_budget_ends_the_run_partial_and_says_what_is_missing() {
        let (core, data_dir) = core("budget").await;
        let workspace_id = workspace_with_a_note(&core).await;
        let provider = ScriptedProvider::new(vec![
            ChatResponse::call("workspace_brief", json!({})),
            ChatResponse::call("workspace_brief", json!({})),
            ChatResponse::call("workspace_brief", json!({})),
        ]);
        let outcome = core
            .loop_with(&provider, request(&workspace_id, "Tell me everything"), setup(2, true))
            .await
            .unwrap();
        assert_eq!(provider.requests().len(), 2, "no model call past the budget");
        assert_eq!(outcome.trace.as_ref().unwrap().state, "partial");
        let text = &outcome.reply.reply_markdown;
        assert!(text.contains("I reached my limit of 2 steps"), "{text}");
        assert!(text.contains("What is missing"));
        assert!(text.contains("workspace_brief"));
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn a_viewers_run_is_offered_no_proposing_tool_and_cannot_call_one() {
        let (core, data_dir) = core("viewer").await;
        let workspace_id = workspace_with_a_note(&core).await;
        let provider = ScriptedProvider::new(vec![
            ChatResponse::call("propose_note", json!({ "subject": "create a note titled Plan: do it" })),
            ChatResponse::text("I cannot change the workspace for you."),
        ]);
        let outcome = core
            .loop_with(
                &provider,
                request(&workspace_id, "Create a note titled Plan: do it"),
                setup(8, false),
            )
            .await
            .unwrap();
        let requests = provider.requests();
        assert!(requests[0].tools.iter().all(|tool| !tool.name.starts_with("propose_")));
        assert!(outcome.reply.proposals.is_empty());
        let result = requests[1]
            .messages
            .iter()
            .find(|message| message.role == ChatRole::Tool)
            .unwrap();
        assert!(result.content.contains("There is no tool named \"propose_note\""));
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn a_writers_proposal_ends_the_run_waiting_for_confirmation() {
        let (core, data_dir) = core("proposal").await;
        let workspace_id = workspace_with_a_note(&core).await;
        let provider = ScriptedProvider::new(vec![ChatResponse::call(
            "propose_note",
            json!({ "subject": "create a note titled Pager rota: Alice is on call in March" }),
        )]);
        let outcome = core
            .loop_with(
                &provider,
                request(&workspace_id, "Create a note titled Pager rota"),
                setup(8, true),
            )
            .await
            .unwrap();
        assert_eq!(provider.requests().len(), 1, "the run stops at the proposal");
        assert_eq!(outcome.reply.proposals.len(), 1);
        let trace = outcome.trace.unwrap();
        assert_eq!(trace.state, "awaiting_confirmation");
        assert!(trace.steps.iter().any(|step| step.kind == "proposal"));
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn each_model_call_uses_the_quota_and_an_exhausted_quota_ends_the_run() {
        let (core, data_dir) = core("quota").await;
        let workspace_id = workspace_with_a_note(&core).await;
        let spent = Arc::new(AtomicUsize::new(0));
        let counter = spent.clone();
        let mut limited = setup(8, true);
        limited.consume_quota = Arc::new(move || {
            if counter.fetch_add(1, Ordering::SeqCst) >= 1 {
                Err("2 per hour".to_owned())
            } else {
                Ok(())
            }
        });
        let provider = ScriptedProvider::new(vec![
            ChatResponse::call("workspace_brief", json!({})),
            ChatResponse::text("never reached"),
        ]);
        let outcome = core
            .loop_with(&provider, request(&workspace_id, "Brief me"), limited)
            .await
            .unwrap();
        assert_eq!(provider.requests().len(), 1);
        assert_eq!(spent.load(Ordering::SeqCst), 2);
        assert_eq!(outcome.trace.unwrap().state, "partial");
        assert!(outcome.reply.reply_markdown.contains("AI allowance is used up"));
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn old_tool_results_are_left_out_first_to_fit_the_window() {
        let long = "x".repeat(4_000);
        let mut messages = vec![
            ChatMessage::new(ChatRole::System, "rules"),
            ChatMessage::new(ChatRole::User, "question"),
            ChatMessage::tool_result("a", long.clone()),
            ChatMessage::tool_result("b", long.clone()),
            ChatMessage::tool_result("c", long),
        ];
        fit_to_budget(&mut messages, 2_000);
        assert_eq!(messages[2].content, OMITTED);
        assert_eq!(messages[3].content, OMITTED);
        assert_ne!(messages[4].content, OMITTED, "the newest result stays");
        assert_eq!(messages[1].content, "question");
    }
}
