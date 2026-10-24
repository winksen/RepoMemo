//! Workspace assistant. A message is routed to exactly one capability, either
//! picked by the user, recognised by keyword rules, or classified by the
//! enabled provider, and then run over the existing search, summary and ask
//! flows. Anything that matches no capability gets a plain reply.

use anyhow::Result;
use repomemo_ai::{provider_from_settings, AiProvider, GenerateRequest};
use repomemo_domain::{
    AgentCapability, AgentCapabilityInfo, AgentHistoryFile, AgentHistoryTurn, AgentMessage, AgentReply, AgentRequest,
    AgentRouting, ArtifactSummary, AskRequest, SearchRequest,
};
use serde_json::{json, Value};

use crate::RepoMemoCore;

const MAX_MESSAGE_CHARS: usize = 2_000;
const FILE_RESULT_LIMIT: usize = 20;
const FILE_CHOICE_LIMIT: usize = 8;
const SEARCH_RESULT_LIMIT: i64 = 10;
/// Earlier turns that go with a question: the newest ones that fit this many
/// estimated tokens (about four characters each), at most `HISTORY_MAX_TURNS`.
/// The room left in the model's window stays for the numbered passages.
pub const HISTORY_TOKEN_BUDGET: usize = 1_500;
pub const HISTORY_MAX_TURNS: usize = 8;
/// An earlier answer is cut to this many characters, a question to a third.
const HISTORY_ANSWER_CHARS: usize = 700;
/// Extra words added to the search for a follow-up, from the turn before.
const HISTORY_QUERY_CHARS: usize = 300;

/// Phrases that ask for a summary anywhere in the message, longest first.
const SUMMARY_TRIGGERS: &[&str] = &[
    "give me a summary of",
    "write a summary of",
    "summary of",
    "summarize",
    "summarise",
    "summary",
    "sum up",
    "tl;dr",
    "tldr",
    "recap",
    "résumé",
    "resume",
];
const OVERVIEW_PHRASES: &[&str] = &[
    "overview",
    "what is this workspace",
    "what's this workspace",
    "what is in this workspace",
    "what's in this workspace",
    "what is this project",
    "what's this project",
    "brief me",
    "onboard me",
];
/// Searches inside content. Checked before file lookups because several of
/// them also start with "find".
const SEARCH_PREFIXES: &[&str] = &[
    "find mentions of",
    "find references to",
    "find occurrences of",
    "which files mention",
    "which files reference",
    "which files contain",
    "files that mention",
    "files mentioning",
    "mentions of",
    "occurrences of",
    "search for",
    "search",
    "grep",
    "look for",
    "look up",
];
/// Words that point back at something said or listed earlier.
const REFERENCE_WORDS: &[&str] = &[
    "it", "its", "this", "that", "these", "those", "they", "them", "their", "there", "same", "above", "earlier",
    "previous", "former", "latter", "also", "again", "one", "ones",
];
/// What may follow "summarize" when it means the file talked about before.
const REFERENCE_FILLERS: &[&str] = &["the", "file", "document", "doc", "one", "same", "again", "please", "of"];
/// Lookups of where a code symbol is defined. Checked before file lookups,
/// since "where is" starts both.
const SYMBOL_PREFIXES: &[&str] = &[
    "find the symbol",
    "find symbol",
    "find the function",
    "find function",
    "find the class",
    "find class",
    "find the method",
    "find method",
    "where is the function",
    "where is function",
    "where is the class",
    "where is class",
    "where is the method",
    "where is method",
    "where is the struct",
    "where is struct",
    "definition of",
    "symbol",
];
const FIND_PREFIXES: &[&str] = &[
    "is there a file",
    "which file",
    "where is",
    "where's",
    "where are",
    "show me",
    "find",
    "locate",
    "open",
    "list",
];
const POLITE_PREFIXES: &[&str] = &[
    "please",
    "can you",
    "could you",
    "would you",
    "will you",
    "i want to",
    "i'd like to",
    "i need to",
    "help me",
];
const QUESTION_WORDS: &[&str] = &[
    "what", "why", "how", "when", "who", "whom", "whose", "which", "where", "does", "do", "did", "is", "are", "was",
    "were", "can", "could", "should", "would", "will", "has", "have", "explain", "describe", "tell me",
];
const FILLER_WORDS: &[&str] = &[
    "me",
    "the",
    "a",
    "an",
    "all",
    "my",
    "our",
    "of",
    "for",
    "about",
    "on",
    "to",
    "up",
    "file",
    "files",
    "document",
    "documents",
    "doc",
    "docs",
    "named",
    "called",
    "titled",
    "that",
    "which",
    "with",
    "containing",
    "contain",
    "contains",
    "mention",
    "mentions",
    "mentioning",
    "please",
];
const WORKSPACE_REFERENCES: &[&str] = &[
    "workspace",
    "this workspace",
    "project",
    "this project",
    "repo",
    "this repo",
    "repository",
    "this repository",
    "codebase",
    "this codebase",
    "everything",
];

pub fn agent_capabilities(ai_available: bool) -> Vec<AgentCapabilityInfo> {
    [
        (
            AgentCapability::FindFiles,
            "Find files",
            "Match file names and paths in this workspace.",
            "File name, part of a path, or an extension like .pdf",
            false,
        ),
        (
            AgentCapability::FindSymbol,
            "Find a symbol",
            "Find where a function, class or method is defined in indexed code.",
            "Function, class or method name",
            false,
        ),
        (
            AgentCapability::SearchContent,
            "Search content",
            "Find passages inside indexed files.",
            "Words or a phrase to look for",
            false,
        ),
        (
            AgentCapability::SummarizeFile,
            "Summarize a file",
            "Summarize one indexed file, with the passages it used.",
            "Which file should be summarized?",
            true,
        ),
        (
            AgentCapability::AskQuestion,
            "Ask a question",
            "Answer from indexed evidence, with citations.",
            "What do you want to know?",
            true,
        ),
        (
            AgentCapability::WorkspaceOverview,
            "Workspace overview",
            "Brief the whole workspace from indexed excerpts.",
            "No input needed",
            true,
        ),
    ]
    .into_iter()
    .map(
        |(id, label, description, placeholder, requires_ai)| AgentCapabilityInfo {
            id,
            label: label.to_owned(),
            description: description.to_owned(),
            placeholder: placeholder.to_owned(),
            requires_ai,
            available: !requires_ai || ai_available,
        },
    )
    .collect()
}

const MAX_TITLE_CHARS: usize = 80;

/// How a request reads in the transcript. A picked action often carries no
/// typed text, and a file summary picked from a list carries only the title.
pub fn agent_turn_label(request: &AgentMessage) -> String {
    let message = request.message.trim();
    match request.capability {
        Some(AgentCapability::SummarizeFile) if request.artifact_id.is_some() => {
            format!("Summarize {message}")
        }
        Some(capability) if message.is_empty() => agent_capabilities(true)
            .into_iter()
            .find(|info| info.id == capability)
            .map(|info| info.label)
            .unwrap_or_default(),
        _ => message.to_owned(),
    }
}

/// A conversation is named after its first request until the user renames it.
pub fn agent_conversation_title(label: &str) -> String {
    let label = label.split_whitespace().collect::<Vec<_>>().join(" ");
    if label.chars().count() <= MAX_TITLE_CHARS {
        return if label.is_empty() { "New chat".to_owned() } else { label };
    }
    let cut = label.chars().take(MAX_TITLE_CHARS - 1).collect::<String>();
    format!("{}…", cut.trim_end())
}

fn requires_ai(capability: AgentCapability) -> bool {
    matches!(
        capability,
        AgentCapability::SummarizeFile | AgentCapability::AskQuestion | AgentCapability::WorkspaceOverview
    )
}

impl RepoMemoCore {
    pub async fn run_agent(&self, request: AgentRequest) -> Result<AgentReply> {
        if request.workspace_id.trim().is_empty() {
            repomemo_domain::bail_invalid!("Workspace id is required.");
        }
        let message_text = request.message.trim().to_owned();
        let message = message_text.as_str();
        if message.is_empty() && request.capability.is_none() {
            repomemo_domain::bail_invalid!("A message is required.");
        }
        if message.chars().count() > MAX_MESSAGE_CHARS {
            repomemo_domain::bail_invalid!("Messages must be between 1 and {MAX_MESSAGE_CHARS} characters.");
        }

        let history = fit_history(&request.history);
        // "Summarize it" and "summarize that file" mean the file just discussed.
        let earlier_file = if request.capability.is_none() && request.artifact_id.is_none() {
            last_file(&history).filter(|_| asks_for_earlier_file(message))
        } else {
            None
        };
        let mut request = request;
        let mut warnings = Vec::new();
        // The phrases the workspace's template adds after the router's own rules (V0.2.185).
        let phrases = self.assistant_phrases(&request.workspace_id).await;
        let routed = if let Some(file) = earlier_file {
            request.artifact_id = Some(file.id);
            Some((AgentCapability::SummarizeFile, AgentRouting::Rules, file.title))
        } else if let Some(capability) = request.capability {
            let subject = if capability == AgentCapability::AskQuestion {
                message.to_owned()
            } else {
                clean_subject(message)
            };
            Some((capability, AgentRouting::Explicit, subject))
        } else if let Some((capability, subject)) =
            route_by_rules(message).or_else(|| route_by_phrases(message, &phrases))
        {
            Some((capability, AgentRouting::Rules, subject))
        } else if let Some(provider_id) = request.provider_id.as_deref() {
            match self.route_by_model(&request.workspace_id, provider_id, message).await {
                Ok(Some((capability, subject))) => Some((capability, AgentRouting::Model, subject)),
                Ok(None) => None,
                Err(error) => {
                    warnings.push(format!("The AI provider could not interpret this message: {error:#}"));
                    None
                }
            }
        } else {
            None
        };

        let Some((capability, routing, mut subject)) = routed else {
            let mut unmatched = reply("I can't help with that yet. Right now I can find files, search inside indexed files, summarize a file, answer questions from your evidence, and give a workspace overview. Pick one of those or rephrase your request.");
            unmatched.warnings = warnings;
            return Ok(unmatched);
        };

        // "Search for it" after "find the file about login" searches for login (V0.3.11).
        if matches!(
            capability,
            AgentCapability::FindFiles | AgentCapability::FindSymbol | AgentCapability::SearchContent
        ) {
            if let Some(earlier) = earlier_subject(message, &subject, &history) {
                subject = earlier;
            }
        }

        let outcome = if requires_ai(capability) && request.provider_id.is_none() {
            Ok(reply(request.ai_unavailable_reason.as_deref().unwrap_or(
                "This needs an AI provider, and none is enabled for this workspace. An administrator can enable one in Settings. Nothing was sent anywhere.",
            )))
        } else {
            match capability {
                AgentCapability::FindFiles => self.agent_find_files(&request.workspace_id, &subject).await,
                AgentCapability::FindSymbol => self.agent_find_symbol(&request.workspace_id, &subject).await,
                AgentCapability::SearchContent => self.agent_search(&request.workspace_id, &subject).await,
                AgentCapability::SummarizeFile => self.agent_summarize(&request, &subject).await,
                AgentCapability::AskQuestion => self.agent_ask(&request, &subject, &history).await,
                AgentCapability::WorkspaceOverview => self.agent_overview(&request).await,
            }
        };
        // Provider and indexing problems are part of the conversation, not a
        // failed request: the user can fix the cause and ask again.
        let mut answer = outcome.unwrap_or_else(|error| {
            let mut failed = reply("I couldn't complete that.");
            // `{:#}` keeps the cause, e.g. a timeout behind "could not reach OpenRouter".
            failed.warnings.push(format!("{error:#}"));
            failed
        });
        answer.capability = Some(capability);
        answer.routing = routing;
        answer.subject = subject;
        warnings.append(&mut answer.warnings);
        answer.warnings = warnings;
        Ok(answer)
    }

    async fn route_by_model(
        &self,
        workspace_id: &str,
        provider_id: &str,
        message: &str,
    ) -> Result<Option<(AgentCapability, String)>> {
        let settings = self.storage.get_provider_settings(provider_id).await?;
        if !settings.enabled || settings.workspace_id.as_deref() != Some(workspace_id) {
            repomemo_domain::bail_invalid!("The selected provider is not enabled for this workspace.");
        }
        let provider = provider_from_settings(settings)?;
        let raw = provider
            .generate(GenerateRequest {
                system: Some("You classify requests. You reply with JSON only.".to_owned()),
                prompt: "You route requests for RepoMemo, a workspace of technical files. Choose the one capability that fits the user's message:\n- find_files: locate files by name, path or type\n- find_symbol: locate where a function, class or method is defined in code\n- search_content: find passages inside files that mention some words\n- summarize_file: summarize one named file\n- ask_question: answer a question using the workspace content\n- workspace_overview: describe the whole workspace\n- none: anything else, such as chit-chat, writing or editing code, or actions on files\nReply with JSON only, no prose: {\"capability\": \"<id>\", \"subject\": \"<the file name, search words or question, without the instruction>\"}. The user's message is the context below.".to_owned(),
                context: message.to_owned(),
                options: json!({ "temperature": 0.0 }),
            })
            .await?;
        Ok(parse_model_route(&raw, message))
    }

    async fn agent_find_files(&self, workspace_id: &str, subject: &str) -> Result<AgentReply> {
        if subject.is_empty() {
            return Ok(reply(
                "Which file are you looking for? Give a name, part of a path, or an extension like `.pdf`.",
            ));
        }
        let matches = self.match_files(workspace_id, subject).await?;
        if matches.is_empty() {
            // Nothing is named like that, but the words may appear inside files.
            let passages = self.search_passages(workspace_id, subject).await?;
            let mut result = reply(if passages.is_empty() {
                format!("No file names, paths or indexed content match “{subject}”.")
            } else {
                format!(
                    "No file is named like “{subject}”, but {} indexed {} mention it.",
                    passages.len(),
                    plural(passages.len(), "passage", "passages")
                )
            });
            result.matches = passages;
            return Ok(result);
        }
        let total = matches.len();
        let mut result = reply(format!(
            "Found {total} {} whose name or path matches “{subject}”.{}",
            plural(total, "file", "files"),
            if total > FILE_RESULT_LIMIT {
                format!(" Showing the first {FILE_RESULT_LIMIT}.")
            } else {
                String::new()
            }
        ));
        result.files = matches
            .into_iter()
            .take(FILE_RESULT_LIMIT)
            .map(|file| file.artifact)
            .collect();
        Ok(result)
    }

    async fn agent_find_symbol(&self, workspace_id: &str, subject: &str) -> Result<AgentReply> {
        let name = subject.trim_matches(|c: char| c == '`' || c == '(' || c == ')' || c.is_whitespace());
        if name.is_empty() {
            return Ok(reply(
                "Which function, class or method are you looking for? Give its name.",
            ));
        }
        let found = self.search_symbols(workspace_id.to_owned(), name.to_owned()).await?;
        if found.is_empty() {
            return Ok(reply(format!(
                "No indexed code defines a symbol named like “{name}”. Only TypeScript, JavaScript, Python, Rust, Go, Java, C# and PHP files are read for symbols, and files still being indexed are not searchable yet."
            )));
        }
        let mut lines = vec![format!(
            "Found {} {} named like “{name}”:",
            found.len(),
            plural(found.len(), "definition", "definitions")
        )];
        let mut artifact_ids = Vec::new();
        for hit in &found {
            let at = hit.symbol.start_line.map(|line| format!(":{line}")).unwrap_or_default();
            lines.push(format!(
                "- `{}` ({:?}) in {}{at}",
                hit.symbol.name, hit.symbol.kind, hit.path
            ));
            if !artifact_ids.contains(&hit.symbol.artifact_id) {
                artifact_ids.push(hit.symbol.artifact_id.clone());
            }
        }
        let mut result = reply(lines.join("\n"));
        for artifact_id in artifact_ids.into_iter().take(FILE_CHOICE_LIMIT) {
            if let Ok(summary) = self.storage.get_artifact_summary(&artifact_id).await {
                result.files.push(summary);
            }
        }
        Ok(result)
    }

    async fn agent_search(&self, workspace_id: &str, subject: &str) -> Result<AgentReply> {
        if subject.is_empty() {
            return Ok(reply("What should I search for? Give a few words or a phrase."));
        }
        let passages = self.search_passages(workspace_id, subject).await?;
        let mut result = reply(if passages.is_empty() {
            format!(
                "No indexed content mentions “{subject}”. Files that are still being indexed are not searchable yet."
            )
        } else {
            format!(
                "{} indexed {} “{subject}”.",
                passages.len(),
                plural(passages.len(), "passage mentions", "passages mention")
            )
        });
        result.matches = passages;
        Ok(result)
    }

    async fn agent_summarize(&self, request: &AgentRequest, subject: &str) -> Result<AgentReply> {
        let provider_id = request.provider_id.clone().unwrap_or_default();
        let target = match request.artifact_id.as_deref() {
            Some(artifact_id) => {
                let detail = self.storage.get_artifact(artifact_id).await?;
                if detail.summary.workspace_id != request.workspace_id {
                    repomemo_domain::bail_invalid!("That file belongs to a different workspace.");
                }
                detail.summary
            }
            None => {
                if subject.is_empty() {
                    return Ok(reply(
                        "Which file should I summarize? Give its name or part of its path.",
                    ));
                }
                let mut matches = self.match_files(&request.workspace_id, subject).await?;
                let clear_winner = matches.len() == 1 || (matches.len() > 1 && matches[0].exact && !matches[1].exact);
                if matches.is_empty() {
                    return Ok(reply(format!(
                        "No file name or path matches “{subject}”. Use Find files to look it up first."
                    )));
                }
                if !clear_winner {
                    let mut result = reply(format!("Several files match “{subject}”. Pick the one to summarize."));
                    result.files = matches
                        .into_iter()
                        .take(FILE_CHOICE_LIMIT)
                        .map(|file| file.artifact)
                        .collect();
                    return Ok(result);
                }
                matches.swap_remove(0).artifact
            }
        };
        let summary = self.summarize_artifact(target.id.clone(), provider_id).await?;
        let mut result = reply(summary.summary_markdown);
        result.generated = true;
        result.files = vec![target];
        result.citations = summary.citations;
        result.warnings = summary.warnings;
        Ok(result)
    }

    async fn agent_ask(
        &self,
        request: &AgentRequest,
        question: &str,
        history: &[AgentHistoryTurn],
    ) -> Result<AgentReply> {
        if question.is_empty() {
            return Ok(reply("What would you like to know?"));
        }
        let answer = self
            .ask_workspace_in_conversation(
                AskRequest {
                    workspace_id: request.workspace_id.clone(),
                    question: question.to_owned(),
                    provider_id: request.provider_id.clone(),
                    limit: Some(8),
                },
                history,
            )
            .await?;
        let mut result = reply(answer.answer_markdown);
        // With no matching context the core answers without calling the provider.
        result.generated = !answer.retrieved_context.is_empty();
        if result.generated {
            result.context_turns = history.len() as i64;
        }
        result.citations = answer.citations;
        result.warnings = answer.warnings;
        Ok(result)
    }

    async fn agent_overview(&self, request: &AgentRequest) -> Result<AgentReply> {
        let summary = self
            .summarize_workspace(
                request.workspace_id.clone(),
                request.provider_id.clone().unwrap_or_default(),
            )
            .await?;
        let mut result = reply(summary.summary_markdown);
        result.generated = true;
        result.citations = summary.citations;
        result.warnings = summary.warnings;
        Ok(result)
    }

    async fn search_passages(&self, workspace_id: &str, query: &str) -> Result<Vec<repomemo_domain::SearchResult>> {
        self.search_workspace(SearchRequest {
            workspace_id: workspace_id.to_owned(),
            query: query.to_owned(),
            artifact_types: Vec::new(),
            languages: Vec::new(),
            source_ids: Vec::new(),
            limit: Some(SEARCH_RESULT_LIMIT),
            ..Default::default()
        })
        .await
    }

    /// Files whose title or path contains every word of `subject`, best first:
    /// exact names, then names starting with or containing the phrase.
    async fn match_files(&self, workspace_id: &str, subject: &str) -> Result<Vec<FileMatch>> {
        let phrase = subject.to_lowercase();
        let terms = phrase
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|term| !term.is_empty())
            .collect::<Vec<_>>();
        if terms.is_empty() {
            return Ok(Vec::new());
        }
        let mut scored = self
            .storage
            .list_artifacts(workspace_id)
            .await?
            .into_iter()
            .filter_map(|artifact| {
                let title = artifact.title.to_lowercase();
                let path = artifact.path.to_lowercase();
                let matches_term = |term: &&str| {
                    title.contains(term)
                        || path.contains(term)
                        || extensions_for(term).iter().any(|ext| path.ends_with(ext))
                };
                if !terms.iter().all(matches_term) {
                    return None;
                }
                let stem = title.rsplit_once('.').map_or(title.as_str(), |(stem, _)| stem);
                let rank = if title == phrase || stem == phrase {
                    0
                } else if title.starts_with(&phrase) {
                    1
                } else if title.contains(&phrase) {
                    2
                } else if terms.iter().all(|term| title.contains(term)) {
                    3
                } else {
                    4
                };
                Some((rank, artifact))
            })
            .collect::<Vec<_>>();
        scored.sort_by(|(rank_a, a), (rank_b, b)| {
            rank_a
                .cmp(rank_b)
                .then_with(|| a.path.len().cmp(&b.path.len()))
                .then_with(|| a.title.cmp(&b.title))
        });
        Ok(scored
            .into_iter()
            .map(|(rank, artifact)| FileMatch {
                exact: rank == 0,
                artifact,
            })
            .collect())
    }
}

struct FileMatch {
    exact: bool,
    artifact: ArtifactSummary,
}

/// File extensions meant by a kind word such as "pdfs" or "markdown".
fn extensions_for(term: &str) -> &'static [&'static str] {
    match term.trim_end_matches('s') {
        "markdown" | "md" => &[".md", ".mdx"],
        "word" => &[".doc", ".docx"],
        "excel" | "spreadsheet" => &[".xls", ".xlsx", ".csv"],
        "powerpoint" | "slide" | "presentation" => &[".ppt", ".pptx"],
        "pdf" => &[".pdf"],
        "image" | "picture" | "screenshot" => &[".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg", ".bmp"],
        "email" | "mail" => &[".msg", ".eml"],
        _ => &[],
    }
}

fn reply(markdown: impl Into<String>) -> AgentReply {
    AgentReply {
        capability: None,
        routing: AgentRouting::Unmatched,
        subject: String::new(),
        reply_markdown: markdown.into(),
        generated: false,
        files: Vec::new(),
        matches: Vec::new(),
        citations: Vec::new(),
        warnings: Vec::new(),
        context_turns: 0,
    }
}

fn plural<'a>(count: usize, one: &'a str, many: &'a str) -> &'a str {
    if count == 1 {
        one
    } else {
        many
    }
}

/// Rough size of a text in model tokens: four characters each, rounded up.
fn estimate_tokens(text: &str) -> usize {
    text.chars().count().div_ceil(4)
}

fn shorten(text: &str, limit: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= limit {
        return flat;
    }
    let cut = flat.chars().take(limit.saturating_sub(1)).collect::<String>();
    format!("{}…", cut.trim_end())
}

/// The earlier turns that go with a question, oldest first: the newest ones
/// whose questions and (shortened) answers fit `HISTORY_TOKEN_BUDGET`. A turn
/// is kept whole or left out, so the model never reads half an exchange.
pub fn fit_history(history: &[AgentHistoryTurn]) -> Vec<AgentHistoryTurn> {
    let mut kept = Vec::new();
    let mut spent = 0;
    for turn in history.iter().rev().take(HISTORY_MAX_TURNS) {
        let question = shorten(&turn.question, HISTORY_ANSWER_CHARS / 3);
        let answer = shorten(&turn.answer, HISTORY_ANSWER_CHARS);
        let cost = estimate_tokens(&question) + estimate_tokens(&answer);
        if spent + cost > HISTORY_TOKEN_BUDGET {
            break;
        }
        spent += cost;
        kept.push(AgentHistoryTurn {
            question,
            answer,
            files: turn.files.clone(),
        });
    }
    kept.reverse();
    kept
}

/// The one file the latest turn that had any was about. None when that turn
/// listed several, since "it" would not say which.
fn last_file(history: &[AgentHistoryTurn]) -> Option<AgentHistoryFile> {
    let turn = history.iter().rev().find(|turn| !turn.files.is_empty())?;
    (turn.files.len() == 1).then(|| turn.files[0].clone())
}

fn words_of(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect()
}

/// "Summarize it", "summarize that file", "give me a summary of this": a
/// summary request whose only subject is a reference.
fn asks_for_earlier_file(message: &str) -> bool {
    let text = strip_polite_prefix(message.trim());
    let lower = text.to_ascii_lowercase();
    let Some(rest) = after_phrase(text, &lower, SUMMARY_TRIGGERS, false) else {
        return false;
    };
    let words = words_of(rest);
    let has_reference = words
        .iter()
        .any(|word| matches!(word.as_str(), "it" | "this" | "that" | "same"));
    has_reference
        && words
            .iter()
            .all(|word| REFERENCE_WORDS.contains(&word.as_str()) || REFERENCE_FILLERS.contains(&word.as_str()))
}

/// Words that make a lookup lean on what was said before: "search for it", "find that", "where is it defined".
const LOOKUP_REFERENCES: &[&str] = &["it", "this", "that", "these", "those", "them", "same"];
/// Characters of an earlier question that stand in for what it was about when it named nothing shorter.
const EARLIER_SUBJECT_CHARS: usize = 120;

/// What a lookup or a search that only points back ("search for it", "find that file") is about: what the latest
/// earlier turn asked for, so "find the file about login" followed by "search for it" searches for `login`.
/// `None` when the message names something itself, points at nothing, or no earlier turn names anything.
pub(crate) fn earlier_subject(message: &str, subject: &str, history: &[AgentHistoryTurn]) -> Option<String> {
    let words = words_of(message);
    if !words.iter().any(|word| LOOKUP_REFERENCES.contains(&word.as_str())) {
        return None;
    }
    let named = words_of(subject);
    if !named
        .iter()
        .all(|word| REFERENCE_WORDS.contains(&word.as_str()) || REFERENCE_FILLERS.contains(&word.as_str()))
    {
        return None;
    }
    history.iter().rev().find_map(|turn| {
        let from_question = match route_by_rules(&turn.question) {
            Some((AgentCapability::AskQuestion, _)) => Some(shorten(turn.question.trim(), EARLIER_SUBJECT_CHARS)),
            Some((AgentCapability::WorkspaceOverview, _)) => None,
            Some((_, earlier)) => Some(earlier),
            None => None,
        };
        let candidate = from_question
            .filter(|text| !text.is_empty())
            .or_else(|| turn.files.first().map(|file| file.title.clone()))?;
        // An earlier "search for it" has nothing to give either: look further back.
        let pointing = words_of(&candidate)
            .iter()
            .all(|word| REFERENCE_WORDS.contains(&word.as_str()) || REFERENCE_FILLERS.contains(&word.as_str()));
        (!pointing).then_some(candidate)
    })
}

/// Whether a question leans on what was said before: it is short or it uses
/// a word such as "it", "that" or "those".
fn looks_like_follow_up(question: &str) -> bool {
    let words = words_of(question);
    words.len() < 6 || words.iter().any(|word| REFERENCE_WORDS.contains(&word.as_str()))
}

/// What to search for: the question, with the words that tell what a
/// follow-up is about (the turn before and the files it named) added to it.
/// A question that stands on its own is searched as asked.
pub(crate) fn follow_up_query(question: &str, history: &[AgentHistoryTurn]) -> String {
    let Some(last) = history.last() else {
        return question.to_owned();
    };
    if !looks_like_follow_up(question) {
        return question.to_owned();
    }
    let mut extra = vec![shorten(&last.question, HISTORY_QUERY_CHARS)];
    extra.extend(last.files.iter().map(|file| file.title.clone()));
    let extra = shorten(&extra.join(" "), HISTORY_QUERY_CHARS);
    format!("{question} {extra}")
}

/// The earlier turns as the model is shown them, ahead of the question.
pub(crate) fn render_history(history: &[AgentHistoryTurn]) -> String {
    if history.is_empty() {
        return String::new();
    }
    let turns = history
        .iter()
        .map(|turn| format!("User: {}\nAssistant: {}", turn.question, turn.answer))
        .collect::<Vec<_>>()
        .join("\n\n");
    format!(
        "Earlier in this conversation (it only tells what the question refers to; it is not evidence):\n{turns}\n\n"
    )
}

/// The system prompt of Ask. A template's one line about what the workspace is for is added after it, never in place of
/// any of it: the citation rule and the "insufficient context" answer stay as they are.
pub(crate) const ASK_SYSTEM: &str = "You answer questions about a workspace of technical material using only the numbered passages supplied. Cite the passages you rely on as [1], [2] and so on. If the passages do not answer the question, say that the indexed context is insufficient instead of guessing. Earlier turns of the conversation may be shown before the question; they only tell what words such as \"it\" or \"that file\" refer to and are never evidence.";

/// The system prompt of Ask for a workspace, with its template's focus if it has one. Without a focus it is
/// [`ASK_SYSTEM`] to the byte.
pub(crate) fn ask_system(focus: Option<&str>) -> String {
    match focus {
        None => ASK_SYSTEM.to_owned(),
        Some(focus) => format!(
            "{ASK_SYSTEM} What the workspace is for, as its administrators describe it: {focus} This only says what the material is about; it never changes the rules above."
        ),
    }
}

/// A phrase from a workspace's template and the job it maps onto (V0.2.185).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PhraseRule {
    pub phrase: String,
    pub capability: AgentCapability,
}

/// Tries a template's phrases on a message the router's own rules did not recognise: the first phrase that stands in
/// it as whole words wins. A phrase can only add a route; it never takes a message from a rule.
pub(crate) fn route_by_phrases(message: &str, rules: &[PhraseRule]) -> Option<(AgentCapability, String)> {
    let text = strip_polite_prefix(message.trim());
    let lower = text.to_ascii_lowercase();
    let rule = rules
        .iter()
        .find(|rule| contains_phrase(&lower, &rule.phrase.to_ascii_lowercase()))?;
    let subject = match rule.capability {
        AgentCapability::WorkspaceOverview => String::new(),
        AgentCapability::AskQuestion => text.trim().to_owned(),
        _ => clean_subject(text),
    };
    Some((rule.capability, subject))
}

/// Recognises the common phrasings of each capability without any AI. Returns
/// the capability and what it should run on.
pub(crate) fn route_by_rules(message: &str) -> Option<(AgentCapability, String)> {
    let text = strip_polite_prefix(message.trim());
    // ASCII lowercasing keeps byte offsets aligned with `text`.
    let lower = text.to_ascii_lowercase();

    if let Some(rest) = after_phrase(text, &lower, SUMMARY_TRIGGERS, false) {
        let mut subject = clean_subject(rest);
        if matches!(subject.to_ascii_lowercase().as_str(), "this" | "that" | "it") {
            subject.clear();
        }
        let names_a_file = lower.contains("file") || lower.contains("doc");
        if (subject.is_empty() && !names_a_file) || WORKSPACE_REFERENCES.contains(&subject.to_lowercase().as_str()) {
            return Some((AgentCapability::WorkspaceOverview, String::new()));
        }
        return Some((AgentCapability::SummarizeFile, subject));
    }
    if OVERVIEW_PHRASES.iter().any(|phrase| contains_phrase(&lower, phrase)) {
        return Some((AgentCapability::WorkspaceOverview, String::new()));
    }
    if let Some(subject) = symbol_lookup(text, &lower) {
        return Some((AgentCapability::FindSymbol, subject));
    }
    if let Some(rest) = after_phrase(text, &lower, SEARCH_PREFIXES, true) {
        return Some((AgentCapability::SearchContent, clean_subject(rest)));
    }
    if let Some(rest) = after_phrase(text, &lower, FIND_PREFIXES, true) {
        return Some((AgentCapability::FindFiles, clean_subject(rest)));
    }
    let is_question = lower.ends_with('?') || QUESTION_WORDS.iter().any(|word| starts_with_phrase(&lower, word));
    is_question.then(|| (AgentCapability::AskQuestion, text.trim().to_owned()))
}

/// "find the function parse_config", "definition of Config", "where is
/// parse_config defined": the name of the symbol asked for.
fn symbol_lookup(text: &str, lower: &str) -> Option<String> {
    if let Some(rest) = after_phrase(text, lower, SYMBOL_PREFIXES, true) {
        let name = clean_subject(rest);
        return (!name.is_empty()).then_some(name);
    }
    // "where is <name> defined|declared": one word between the two.
    let trimmed = lower.trim_end_matches(['?', '.', '!', ' ']);
    let body = trimmed
        .strip_suffix("defined")
        .or_else(|| trimmed.strip_suffix("declared"))?
        .trim_end();
    let name_start = body.strip_prefix("where is").or_else(|| body.strip_prefix("where's"))?;
    let name = clean_subject(&text[body.len() - name_start.len()..body.len()]);
    (!name.is_empty() && !name.contains(' ')).then_some(name)
}

fn strip_polite_prefix(text: &str) -> &str {
    let mut text = text.trim_start();
    loop {
        let lower = text.to_ascii_lowercase();
        match POLITE_PREFIXES.iter().find(|prefix| starts_with_phrase(&lower, prefix)) {
            Some(prefix) => text = text[prefix.len()..].trim_start_matches([' ', ',']),
            None => return text,
        }
    }
}

/// The text after the first phrase found, at the start only when `anchored`.
fn after_phrase<'a>(text: &'a str, lower: &str, phrases: &[&str], anchored: bool) -> Option<&'a str> {
    phrases.iter().find_map(|phrase| {
        let start = if anchored {
            starts_with_phrase(lower, phrase).then_some(0)?
        } else {
            find_phrase(lower, phrase)?
        };
        Some(&text[start + phrase.len()..])
    })
}

fn starts_with_phrase(lower: &str, phrase: &str) -> bool {
    lower.starts_with(phrase) && is_boundary(lower, phrase.len())
}

fn contains_phrase(lower: &str, phrase: &str) -> bool {
    find_phrase(lower, phrase).is_some()
}

/// Byte offset of `phrase` in `lower` where it stands as whole words.
fn find_phrase(lower: &str, phrase: &str) -> Option<usize> {
    lower.match_indices(phrase).map(|(index, _)| index).find(|&index| {
        let before = lower[..index].chars().next_back();
        before.is_none_or(|c| !c.is_alphanumeric()) && is_boundary(lower, index + phrase.len())
    })
}

fn is_boundary(lower: &str, index: usize) -> bool {
    lower[index..].chars().next().is_none_or(|c| !c.is_alphanumeric())
}

/// Drops quotes, trailing punctuation and filler words around the part of a
/// request that names a file or search terms.
pub(crate) fn clean_subject(raw: &str) -> String {
    const EDGE: &[char] = &[' ', ':', ',', '.', '?', '!', '"', '\'', '`', '“', '”', '‘', '’'];
    let mut words = raw.trim_matches(EDGE).split_whitespace().collect::<Vec<_>>();
    let is_filler = |word: &str| FILLER_WORDS.contains(&word.to_ascii_lowercase().as_str());
    while words.first().is_some_and(|word| is_filler(word)) {
        words.remove(0);
    }
    while words.len() > 1 && words.last().is_some_and(|word| is_filler(word)) {
        words.pop();
    }
    words.join(" ").trim_matches(EDGE).to_owned()
}

fn parse_model_route(raw: &str, message: &str) -> Option<(AgentCapability, String)> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    let value: Value = serde_json::from_str(raw.get(start..=end)?).ok()?;
    let capability = match value.get("capability")?.as_str()? {
        "find_files" => AgentCapability::FindFiles,
        "find_symbol" => AgentCapability::FindSymbol,
        "search_content" => AgentCapability::SearchContent,
        "summarize_file" => AgentCapability::SummarizeFile,
        "ask_question" => AgentCapability::AskQuestion,
        "workspace_overview" => AgentCapability::WorkspaceOverview,
        _ => return None,
    };
    let subject = value
        .get("subject")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    let subject = match capability {
        AgentCapability::AskQuestion if subject.is_empty() => message.trim().to_owned(),
        AgentCapability::AskQuestion => subject.to_owned(),
        AgentCapability::WorkspaceOverview => String::new(),
        _ => clean_subject(subject),
    };
    Some((capability, subject))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route(message: &str) -> Option<(AgentCapability, String)> {
        route_by_rules(message)
    }

    fn rule(phrase: &str, capability: AgentCapability) -> PhraseRule {
        PhraseRule {
            phrase: phrase.to_owned(),
            capability,
        }
    }

    /// V0.2.185: "runbook for payments" is not recognised by the router's own rules, so in a workspace whose template
    /// has the phrase it finds files, and in one without it it is not routed by rules at all.
    #[test]
    fn a_templates_phrase_routes_what_the_rules_do_not_recognise_and_never_takes_a_message_from_a_rule() {
        let operations = [
            rule("runbook", AgentCapability::FindFiles),
            rule("on-call", AgentCapability::SearchContent),
        ];
        assert_eq!(route("runbook for payments"), None);
        assert_eq!(
            route_by_phrases("runbook for payments", &operations),
            Some((AgentCapability::FindFiles, "runbook for payments".to_owned()))
        );
        assert_eq!(route_by_phrases("runbook for payments", &[]), None);
        // Whole words, in any case, with a polite opening.
        assert!(route_by_phrases("Please show the Runbook.", &operations).is_some());
        assert_eq!(route_by_phrases("runbooks", &operations), None);
        assert_eq!(route_by_phrases("the rebooked trip", &operations), None);
        // The first phrase that stands in the message wins, in the template's order.
        assert_eq!(
            route_by_phrases("on-call runbook", &operations).unwrap().0,
            AgentCapability::FindFiles
        );
        // A message a rule already recognises is the rule's: the phrase is only tried after them.
        let message = "what is the runbook for payments?";
        assert_eq!(route(message).unwrap().0, AgentCapability::AskQuestion);
        assert_eq!(
            route_by_rules(message)
                .or_else(|| route_by_phrases(message, &operations))
                .unwrap()
                .0,
            AgentCapability::AskQuestion
        );
        // What a phrase runs on follows the job: an overview and a question differ from a lookup.
        let overview = [
            rule("state of things", AgentCapability::WorkspaceOverview),
            rule("tell me", AgentCapability::AskQuestion),
        ];
        assert_eq!(
            route_by_phrases("the state of things", &overview),
            Some((AgentCapability::WorkspaceOverview, String::new()))
        );
        assert_eq!(
            route_by_phrases("tell me about payments", &overview).unwrap().1,
            "tell me about payments"
        );
    }

    /// The Ask prompt of a workspace without a focus is what V0.2.162 sent, byte for byte; a focus is added after it.
    #[test]
    fn the_ask_prompt_is_unchanged_without_a_focus_and_a_focus_only_adds_to_it() {
        let before = "You answer questions about a workspace of technical material using only the numbered passages supplied. Cite the passages you rely on as [1], [2] and so on. If the passages do not answer the question, say that the indexed context is insufficient instead of guessing. Earlier turns of the conversation may be shown before the question; they only tell what words such as \"it\" or \"that file\" refer to and are never evidence.";
        assert_eq!(ask_system(None), before);
        assert_eq!(ASK_SYSTEM, before);
        let with = ask_system(Some("This workspace holds runbooks."));
        assert!(
            with.starts_with(before),
            "nothing of the prompt is replaced or loosened"
        );
        assert!(with.contains("This workspace holds runbooks.") && with.contains("never changes the rules above"));
        assert_eq!(with.matches("insufficient").count(), 1);
    }

    #[test]
    fn routes_symbol_lookups_before_file_lookups() {
        for (message, name) in [
            ("find the function parse_config", "parse_config"),
            ("Where is the class UserStore", "UserStore"),
            ("definition of Config", "Config"),
            ("where is parse_config defined?", "parse_config"),
            ("find symbol `load_settings`", "load_settings"),
        ] {
            assert_eq!(
                route(message),
                Some((AgentCapability::FindSymbol, name.to_owned())),
                "{message}"
            );
        }
        // A file lookup stays a file lookup.
        assert_eq!(
            route("where is the readme"),
            Some((AgentCapability::FindFiles, "readme".to_owned()))
        );
    }

    #[test]
    fn routes_file_lookups() {
        assert_eq!(
            route("find the readme file"),
            Some((AgentCapability::FindFiles, "readme".to_owned()))
        );
        assert_eq!(
            route("Please locate all pdf files"),
            Some((AgentCapability::FindFiles, "pdf".to_owned()))
        );
        assert_eq!(
            route("where is config.yaml?"),
            Some((AgentCapability::FindFiles, "config.yaml".to_owned()))
        );
    }

    #[test]
    fn routes_content_searches_before_lookups() {
        assert_eq!(
            route("find mentions of retry budget"),
            Some((AgentCapability::SearchContent, "retry budget".to_owned()))
        );
        assert_eq!(
            route("search for \"JWT_ISSUER\""),
            Some((AgentCapability::SearchContent, "JWT_ISSUER".to_owned()))
        );
        assert_eq!(
            route("Which files mention LibreOffice?"),
            Some((AgentCapability::SearchContent, "LibreOffice".to_owned()))
        );
    }

    #[test]
    fn routes_summaries_and_overviews() {
        assert_eq!(
            route("Summarize ARCHITECTURE.md"),
            Some((AgentCapability::SummarizeFile, "ARCHITECTURE.md".to_owned()))
        );
        assert_eq!(
            route("can you give me a summary of the onboarding doc"),
            Some((AgentCapability::SummarizeFile, "onboarding".to_owned()))
        );
        assert_eq!(
            route("summarize this project"),
            Some((AgentCapability::WorkspaceOverview, String::new()))
        );
        assert_eq!(
            route("tl;dr"),
            Some((AgentCapability::WorkspaceOverview, String::new()))
        );
        assert_eq!(
            route("give me an overview"),
            Some((AgentCapability::WorkspaceOverview, String::new()))
        );
        assert_eq!(
            route("summarize this file"),
            Some((AgentCapability::SummarizeFile, String::new()))
        );
    }

    #[test]
    fn routes_questions_and_leaves_the_rest() {
        assert_eq!(
            route("Why do uploads fail over 10 MiB?"),
            Some((
                AgentCapability::AskQuestion,
                "Why do uploads fail over 10 MiB?".to_owned()
            ))
        );
        assert_eq!(
            route("the auth flow uses JWT?"),
            Some((AgentCapability::AskQuestion, "the auth flow uses JWT?".to_owned()))
        );
        assert_eq!(route("write me a poem"), None);
        assert_eq!(route("hello"), None);
    }

    #[test]
    fn keywords_need_word_boundaries() {
        // "research" contains "search" and "resumed" contains "resume".
        assert_eq!(route("research notes from the spike"), None);
        assert_eq!(route("indexing resumed overnight"), None);
    }

    #[test]
    fn parses_model_routes_leniently() {
        assert_eq!(
            parse_model_route(
                "Sure! {\"capability\": \"find_files\", \"subject\": \"the deploy files\"}",
                "x"
            ),
            Some((AgentCapability::FindFiles, "deploy".to_owned()))
        );
        assert_eq!(
            parse_model_route(
                "{\"capability\": \"ask_question\", \"subject\": \"\"}",
                "How is auth done?"
            ),
            Some((AgentCapability::AskQuestion, "How is auth done?".to_owned()))
        );
        assert_eq!(parse_model_route("{\"capability\": \"none\"}", "x"), None);
        assert_eq!(parse_model_route("no json here", "x"), None);
    }

    #[test]
    fn labels_and_titles_read_like_the_request() {
        let picked = |capability, message: &str, artifact: Option<&str>| AgentMessage {
            message: message.to_owned(),
            capability,
            artifact_id: artifact.map(str::to_owned),
        };
        assert_eq!(agent_turn_label(&picked(None, "  find readme ", None)), "find readme");
        assert_eq!(
            agent_turn_label(&picked(Some(AgentCapability::WorkspaceOverview), "", None)),
            "Workspace overview"
        );
        assert_eq!(
            agent_turn_label(&picked(Some(AgentCapability::SummarizeFile), "README.md", Some("a1"))),
            "Summarize README.md"
        );
        assert_eq!(
            agent_conversation_title("Why do\n uploads   fail?"),
            "Why do uploads fail?"
        );
        let long = agent_conversation_title(&"é".repeat(200));
        assert_eq!(long.chars().count(), MAX_TITLE_CHARS);
        assert!(long.ends_with('…'));
    }

    fn turn(question: &str, answer: &str, files: &[(&str, &str)]) -> AgentHistoryTurn {
        AgentHistoryTurn {
            question: question.to_owned(),
            answer: answer.to_owned(),
            files: files
                .iter()
                .map(|(id, title)| AgentHistoryFile {
                    id: (*id).to_owned(),
                    title: (*title).to_owned(),
                })
                .collect(),
        }
    }

    #[test]
    fn history_keeps_the_newest_turns_that_fit_the_budget() {
        assert!(fit_history(&[]).is_empty());
        let turns = (0..20)
            .map(|index| turn(&format!("question {index}"), &"a long answer ".repeat(200), &[]))
            .collect::<Vec<_>>();
        let kept = fit_history(&turns);
        assert!(!kept.is_empty() && kept.len() <= HISTORY_MAX_TURNS);
        // The newest turn is the last one kept, in the order it was said.
        assert_eq!(kept.last().map(|turn| turn.question.as_str()), Some("question 19"));
        let spent: usize = kept
            .iter()
            .map(|turn| estimate_tokens(&turn.question) + estimate_tokens(&turn.answer))
            .sum();
        assert!(spent <= HISTORY_TOKEN_BUDGET);
        assert!(kept
            .iter()
            .all(|turn| turn.answer.chars().count() <= HISTORY_ANSWER_CHARS));
    }

    #[test]
    fn short_turns_all_go_along_up_to_the_turn_limit() {
        let turns = (0..12)
            .map(|index| turn(&format!("q{index}"), "ok", &[]))
            .collect::<Vec<_>>();
        let kept = fit_history(&turns);
        assert_eq!(kept.len(), HISTORY_MAX_TURNS);
        assert_eq!(kept[0].question, "q4");
    }

    #[test]
    fn it_and_that_file_mean_the_file_just_discussed() {
        for message in [
            "summarize it",
            "Summarize that file",
            "can you summarize this?",
            "give me a summary of the same file",
            "tl;dr of that",
        ] {
            assert!(asks_for_earlier_file(message), "{message}");
        }
        for message in [
            "summarize ARCHITECTURE.md",
            "summarize this project",
            "summarize",
            "tl;dr",
            "why does it fail?",
        ] {
            assert!(!asks_for_earlier_file(message), "{message}");
        }
        let one = [turn("find readme", "Found 1 file", &[("a1", "README.md")])];
        assert_eq!(last_file(&one).map(|file| file.id), Some("a1".to_owned()));
        let several = [turn("find docs", "Found 2 files", &[("a1", "a.md"), ("a2", "b.md")])];
        assert!(last_file(&several).is_none());
        // The latest turn that had files decides, even after plain turns.
        let later = [
            turn("find readme", "Found 1 file", &[("a1", "README.md")]),
            turn("hello", "I can't help with that yet.", &[]),
        ];
        assert_eq!(last_file(&later).map(|file| file.id), Some("a1".to_owned()));
    }

    #[test]
    fn a_search_that_only_points_back_is_about_what_the_turn_before_looked_for() {
        let history = [turn(
            "Find the file about login",
            "Found 2 files whose name matches “login”.",
            &[("f1", "login.rs"), ("f2", "login.md")],
        )];
        // The rules route it to a search for "it"; the earlier turn says what that is.
        let (capability, subject) = route("search for it").unwrap();
        assert_eq!((capability, subject.as_str()), (AgentCapability::SearchContent, "it"));
        assert_eq!(
            earlier_subject("search for it", &subject, &history).as_deref(),
            Some("login")
        );
        // "find that file" cleans down to nothing, and still points back.
        let (_, nothing) = route("find that file").unwrap();
        assert_eq!(
            earlier_subject("find that file", &nothing, &history).as_deref(),
            Some("login")
        );
        assert_eq!(
            earlier_subject("Where is it defined?", "it", &history).as_deref(),
            Some("login")
        );
        // A symbol, a summary of a named file and a question each leave their own subject.
        let symbol = [turn("find the function parse_config", "Found 1 definition.", &[])];
        assert_eq!(
            earlier_subject("search for that", "that", &symbol).as_deref(),
            Some("parse_config")
        );
        let summary = [turn(
            "summarize runbook.md",
            "It covers restarts.",
            &[("f9", "runbook.md")],
        )];
        assert_eq!(
            earlier_subject("look for it", "it", &summary).as_deref(),
            Some("runbook.md")
        );
        let question = [turn("How do uploads back off?", "With a retry budget [1].", &[])];
        assert_eq!(
            earlier_subject("search for it", "it", &question).as_deref(),
            Some("How do uploads back off?")
        );
        // Turns that name nothing are skipped, and an earlier pointing turn too.
        let layered = [
            turn("Find the file about billing", "Found 1 file.", &[("f3", "billing.md")]),
            turn("search for it", "1 passage mentions “billing”.", &[]),
            turn("give me an overview", "This workspace holds…", &[]),
        ];
        assert_eq!(
            earlier_subject("search for it again", "it again", &layered).as_deref(),
            Some("billing")
        );
    }

    #[test]
    fn a_lookup_that_names_its_subject_or_points_at_nothing_is_left_as_typed() {
        let history = [turn(
            "Find the file about login",
            "Found 2 files.",
            &[("f1", "login.rs")],
        )];
        // It names something itself.
        assert_eq!(earlier_subject("search for deploy keys", "deploy keys", &history), None);
        assert_eq!(
            earlier_subject("search for it in deploy.md", "it in deploy.md", &history),
            None
        );
        // It points at nothing: no reference word, so the empty subject still asks which file.
        assert_eq!(earlier_subject("find", "", &history), None);
        // Nothing earlier to point at.
        assert_eq!(earlier_subject("search for it", "it", &[]), None);
        assert_eq!(
            earlier_subject("search for it", "it", &[turn("give me an overview", "…", &[])]),
            None
        );
    }

    #[test]
    fn a_follow_up_is_searched_with_what_it_is_about() {
        let history = [turn(
            "How is the retry budget configured?",
            "Set it in config.",
            &[("a1", "retry.md")],
        )];
        let query = follow_up_query("and what about its timeout?", &history);
        assert!(query.starts_with("and what about its timeout?"), "{query}");
        assert!(query.contains("retry budget") && query.contains("retry.md"), "{query}");
        // A question that stands on its own is searched as asked.
        let standalone = "Why do uploads over ten megabytes fail on the shared server?";
        assert_eq!(follow_up_query(standalone, &history), standalone);
        assert_eq!(follow_up_query("and its timeout?", &[]), "and its timeout?");
    }

    #[test]
    fn history_is_shown_to_the_model_as_context_not_evidence() {
        assert_eq!(render_history(&[]), "");
        let shown = render_history(&[turn("What is X?", "X is a thing [1].", &[])]);
        assert!(shown.contains("User: What is X?") && shown.contains("Assistant: X is a thing [1]."));
        assert!(shown.contains("not evidence"));
    }

    #[test]
    fn ai_capabilities_are_unavailable_without_a_provider() {
        let offline = agent_capabilities(false);
        assert!(offline
            .iter()
            .all(|capability| capability.available != capability.requires_ai));
        assert!(agent_capabilities(true).iter().all(|capability| capability.available));
    }
}
