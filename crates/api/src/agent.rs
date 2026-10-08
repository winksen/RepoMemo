//! Workspace assistant. A message is routed to exactly one capability, either
//! picked by the user, recognised by keyword rules, or classified by the
//! enabled provider, and then run over the existing search, summary and ask
//! flows. Anything that matches no capability gets a plain reply.

use anyhow::{bail, Result};
use repomemo_ai::{provider_from_settings, AiProvider, GenerateRequest};
use repomemo_domain::{
    AgentCapability, AgentCapabilityInfo, AgentMessage, AgentReply, AgentRequest, AgentRouting,
    ArtifactSummary, AskRequest, SearchRequest,
};
use serde_json::{json, Value};

use crate::RepoMemoCore;

const MAX_MESSAGE_CHARS: usize = 2_000;
const FILE_RESULT_LIMIT: usize = 20;
const FILE_CHOICE_LIMIT: usize = 8;
const SEARCH_RESULT_LIMIT: i64 = 10;

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
    "what", "why", "how", "when", "who", "whom", "whose", "which", "where", "does", "do", "did",
    "is", "are", "was", "were", "can", "could", "should", "would", "will", "has", "have",
    "explain", "describe", "tell me",
];
const FILLER_WORDS: &[&str] = &[
    "me", "the", "a", "an", "all", "my", "our", "of", "for", "about", "on", "to", "up", "file",
    "files", "document", "documents", "doc", "docs", "named", "called", "titled", "that",
    "which", "with", "containing", "contain", "contains", "mention", "mentions", "mentioning",
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
        AgentCapability::SummarizeFile
            | AgentCapability::AskQuestion
            | AgentCapability::WorkspaceOverview
    )
}

impl RepoMemoCore {
    pub async fn run_agent(&self, request: AgentRequest) -> Result<AgentReply> {
        if request.workspace_id.trim().is_empty() {
            bail!("Workspace id is required.");
        }
        let message = request.message.trim();
        if message.is_empty() && request.capability.is_none() {
            bail!("A message is required.");
        }
        if message.chars().count() > MAX_MESSAGE_CHARS {
            bail!("Messages must be between 1 and {MAX_MESSAGE_CHARS} characters.");
        }

        let mut warnings = Vec::new();
        let routed = if let Some(capability) = request.capability {
            let subject = if capability == AgentCapability::AskQuestion {
                message.to_owned()
            } else {
                clean_subject(message)
            };
            Some((capability, AgentRouting::Explicit, subject))
        } else if let Some((capability, subject)) = route_by_rules(message) {
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

        let Some((capability, routing, subject)) = routed else {
            let mut unmatched = reply("I can't help with that yet. Right now I can find files, search inside indexed files, summarize a file, answer questions from your evidence, and give a workspace overview. Pick one of those or rephrase your request.");
            unmatched.warnings = warnings;
            return Ok(unmatched);
        };

        let outcome = if requires_ai(capability) && request.provider_id.is_none() {
            Ok(reply(request.ai_unavailable_reason.as_deref().unwrap_or(
                "This needs an AI provider, and none is enabled for this workspace. An administrator can enable one in Settings. Nothing was sent anywhere.",
            )))
        } else {
            match capability {
                AgentCapability::FindFiles => self.agent_find_files(&request.workspace_id, &subject).await,
                AgentCapability::SearchContent => self.agent_search(&request.workspace_id, &subject).await,
                AgentCapability::SummarizeFile => self.agent_summarize(&request, &subject).await,
                AgentCapability::AskQuestion => self.agent_ask(&request, &subject).await,
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
            bail!("The selected provider is not enabled for this workspace.");
        }
        let provider = provider_from_settings(settings)?;
        let raw = provider
            .generate(GenerateRequest {
                system: Some("You classify requests. You reply with JSON only.".to_owned()),
                prompt: "You route requests for RepoMemo, a workspace of technical files. Choose the one capability that fits the user's message:\n- find_files: locate files by name, path or type\n- search_content: find passages inside files that mention some words\n- summarize_file: summarize one named file\n- ask_question: answer a question using the workspace content\n- workspace_overview: describe the whole workspace\n- none: anything else, such as chit-chat, writing or editing code, or actions on files\nReply with JSON only, no prose: {\"capability\": \"<id>\", \"subject\": \"<the file name, search words or question, without the instruction>\"}. The user's message is the context below.".to_owned(),
                context: message.to_owned(),
                options: json!({ "temperature": 0.0 }),
            })
            .await?;
        Ok(parse_model_route(&raw, message))
    }

    async fn agent_find_files(&self, workspace_id: &str, subject: &str) -> Result<AgentReply> {
        if subject.is_empty() {
            return Ok(reply("Which file are you looking for? Give a name, part of a path, or an extension like `.pdf`."));
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

    async fn agent_search(&self, workspace_id: &str, subject: &str) -> Result<AgentReply> {
        if subject.is_empty() {
            return Ok(reply("What should I search for? Give a few words or a phrase."));
        }
        let passages = self.search_passages(workspace_id, subject).await?;
        let mut result = reply(if passages.is_empty() {
            format!("No indexed content mentions “{subject}”. Files that are still being indexed are not searchable yet.")
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
                    bail!("That file belongs to a different workspace.");
                }
                detail.summary
            }
            None => {
                if subject.is_empty() {
                    return Ok(reply("Which file should I summarize? Give its name or part of its path."));
                }
                let mut matches = self.match_files(&request.workspace_id, subject).await?;
                let clear_winner = matches.len() == 1
                    || (matches.len() > 1 && matches[0].exact && !matches[1].exact);
                if matches.is_empty() {
                    return Ok(reply(format!("No file name or path matches “{subject}”. Use Find files to look it up first.")));
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

    async fn agent_ask(&self, request: &AgentRequest, question: &str) -> Result<AgentReply> {
        if question.is_empty() {
            return Ok(reply("What would you like to know?"));
        }
        let answer = self
            .ask_workspace(AskRequest {
                workspace_id: request.workspace_id.clone(),
                question: question.to_owned(),
                provider_id: request.provider_id.clone(),
                limit: Some(8),
            })
            .await?;
        let mut result = reply(answer.answer_markdown);
        // With no matching context the core answers without calling the provider.
        result.generated = !answer.retrieved_context.is_empty();
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

    async fn search_passages(
        &self,
        workspace_id: &str,
        query: &str,
    ) -> Result<Vec<repomemo_domain::SearchResult>> {
        self.search_workspace(SearchRequest {
            workspace_id: workspace_id.to_owned(),
            query: query.to_owned(),
            artifact_types: Vec::new(),
            languages: Vec::new(),
            source_ids: Vec::new(),
            limit: Some(SEARCH_RESULT_LIMIT),
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
        "image" | "picture" | "screenshot" => {
            &[".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg", ".bmp"]
        }
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
    }
}

fn plural<'a>(count: usize, one: &'a str, many: &'a str) -> &'a str {
    if count == 1 {
        one
    } else {
        many
    }
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
        if (subject.is_empty() && !names_a_file)
            || WORKSPACE_REFERENCES.contains(&subject.to_lowercase().as_str())
        {
            return Some((AgentCapability::WorkspaceOverview, String::new()));
        }
        return Some((AgentCapability::SummarizeFile, subject));
    }
    if OVERVIEW_PHRASES.iter().any(|phrase| contains_phrase(&lower, phrase)) {
        return Some((AgentCapability::WorkspaceOverview, String::new()));
    }
    if let Some(rest) = after_phrase(text, &lower, SEARCH_PREFIXES, true) {
        return Some((AgentCapability::SearchContent, clean_subject(rest)));
    }
    if let Some(rest) = after_phrase(text, &lower, FIND_PREFIXES, true) {
        return Some((AgentCapability::FindFiles, clean_subject(rest)));
    }
    let is_question = lower.ends_with('?')
        || QUESTION_WORDS
            .iter()
            .any(|word| starts_with_phrase(&lower, word));
    is_question.then(|| (AgentCapability::AskQuestion, text.trim().to_owned()))
}

fn strip_polite_prefix(text: &str) -> &str {
    let mut text = text.trim_start();
    loop {
        let lower = text.to_ascii_lowercase();
        match POLITE_PREFIXES
            .iter()
            .find(|prefix| starts_with_phrase(&lower, prefix))
        {
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
        before.map_or(true, |c| !c.is_alphanumeric()) && is_boundary(lower, index + phrase.len())
    })
}

fn is_boundary(lower: &str, index: usize) -> bool {
    lower[index..]
        .chars()
        .next()
        .map_or(true, |c| !c.is_alphanumeric())
}

/// Drops quotes, trailing punctuation and filler words around the part of a
/// request that names a file or search terms.
pub(crate) fn clean_subject(raw: &str) -> String {
    const EDGE: &[char] = &[' ', ':', ',', '.', '?', '!', '"', '\'', '`', '“', '”', '‘', '’'];
    let mut words = raw
        .trim_matches(EDGE)
        .split_whitespace()
        .collect::<Vec<_>>();
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

    #[test]
    fn routes_file_lookups() {
        assert_eq!(route("find the readme file"), Some((AgentCapability::FindFiles, "readme".to_owned())));
        assert_eq!(route("Please locate all pdf files"), Some((AgentCapability::FindFiles, "pdf".to_owned())));
        assert_eq!(route("where is config.yaml?"), Some((AgentCapability::FindFiles, "config.yaml".to_owned())));
    }

    #[test]
    fn routes_content_searches_before_lookups() {
        assert_eq!(route("find mentions of retry budget"), Some((AgentCapability::SearchContent, "retry budget".to_owned())));
        assert_eq!(route("search for \"JWT_ISSUER\""), Some((AgentCapability::SearchContent, "JWT_ISSUER".to_owned())));
        assert_eq!(route("Which files mention LibreOffice?"), Some((AgentCapability::SearchContent, "LibreOffice".to_owned())));
    }

    #[test]
    fn routes_summaries_and_overviews() {
        assert_eq!(route("Summarize ARCHITECTURE.md"), Some((AgentCapability::SummarizeFile, "ARCHITECTURE.md".to_owned())));
        assert_eq!(route("can you give me a summary of the onboarding doc"), Some((AgentCapability::SummarizeFile, "onboarding".to_owned())));
        assert_eq!(route("summarize this project"), Some((AgentCapability::WorkspaceOverview, String::new())));
        assert_eq!(route("tl;dr"), Some((AgentCapability::WorkspaceOverview, String::new())));
        assert_eq!(route("give me an overview"), Some((AgentCapability::WorkspaceOverview, String::new())));
        assert_eq!(route("summarize this file"), Some((AgentCapability::SummarizeFile, String::new())));
    }

    #[test]
    fn routes_questions_and_leaves_the_rest() {
        assert_eq!(route("Why do uploads fail over 10 MiB?"), Some((AgentCapability::AskQuestion, "Why do uploads fail over 10 MiB?".to_owned())));
        assert_eq!(route("the auth flow uses JWT?"), Some((AgentCapability::AskQuestion, "the auth flow uses JWT?".to_owned())));
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
            parse_model_route("Sure! {\"capability\": \"find_files\", \"subject\": \"the deploy files\"}", "x"),
            Some((AgentCapability::FindFiles, "deploy".to_owned()))
        );
        assert_eq!(
            parse_model_route("{\"capability\": \"ask_question\", \"subject\": \"\"}", "How is auth done?"),
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
        assert_eq!(agent_turn_label(&picked(Some(AgentCapability::WorkspaceOverview), "", None)), "Workspace overview");
        assert_eq!(agent_turn_label(&picked(Some(AgentCapability::SummarizeFile), "README.md", Some("a1"))), "Summarize README.md");
        assert_eq!(agent_conversation_title("Why do\n uploads   fail?"), "Why do uploads fail?");
        let long = agent_conversation_title(&"é".repeat(200));
        assert_eq!(long.chars().count(), MAX_TITLE_CHARS);
        assert!(long.ends_with('…'));
    }

    #[test]
    fn ai_capabilities_are_unavailable_without_a_provider() {
        let offline = agent_capabilities(false);
        assert!(offline.iter().all(|capability| capability.available != capability.requires_ai));
        assert!(agent_capabilities(true).iter().all(|capability| capability.available));
    }
}
