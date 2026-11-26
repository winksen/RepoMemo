//! CAIRN's tools (V0.9.37). Every job CAIRN offers is one registered tool: a name, what the model reads about it, the
//! schema of its arguments, whether it only reads or proposes a change, the group it belongs to and the handler that runs it.
//! The menu, what CAIRN says it can do, the viewer rule, the routing classifier and the dispatch are all derived from the
//! one table below, so a new job has to be added here and cannot be half added.

use std::future::Future;
use std::pin::Pin;

use anyhow::Result;
use repomemo_domain::{AgentCapability, AgentCapabilityInfo, AgentHistoryTurn, AgentReply, AgentRequest};
use serde_json::{json, Value};

use crate::RepoMemoCore;

// The loop reads the atoms from V0.9.40; until then only their tests do.
#[allow(dead_code)]
pub(in crate::agent) mod atoms;
mod change;
pub(in crate::agent) mod find;
mod understand;
pub(in crate::agent) mod work;

/// What running a tool does. `External` (sending something out of RepoMemo) is not representable in this line on purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::agent) enum ToolEffect {
    /// Looks at or writes about what is there, with the person's rights, and changes nothing.
    Read,
    /// Builds a proposal the person confirms; only the confirmation changes the workspace.
    Propose,
}

/// What a handler is given: the request, what the router took the subject of the message to be, and the earlier turns.
pub(in crate::agent) struct ToolRun<'a> {
    pub request: &'a AgentRequest,
    pub subject: &'a str,
    pub history: &'a [AgentHistoryTurn],
}

type ToolFuture<'a> = Pin<Box<dyn Future<Output = Result<AgentReply>> + Send + 'a>>;
type Handler = for<'a> fn(&'a RepoMemoCore, ToolRun<'a>) -> ToolFuture<'a>;

pub(in crate::agent) struct CairnTool {
    /// Snake case and stable: the name the model calls and the evaluation set refers to.
    #[cfg_attr(not(test), allow(dead_code))]
    pub name: &'static str,
    /// The job the + menu offers for this tool.
    pub capability: AgentCapability,
    pub label: &'static str,
    /// What the person and the model read about the job.
    pub description: &'static str,
    pub placeholder: &'static str,
    /// One line for the routing classifier: when this job fits a message.
    pub routing_hint: &'static str,
    /// JSON Schema of the arguments, for the loop (V0.9.40) and the MCP surface.
    #[cfg_attr(not(test), allow(dead_code))]
    pub parameters: fn() -> Value,
    pub effect: ToolEffect,
    pub requires_ai: bool,
    /// Find, Understand or Change.
    pub group: &'static str,
    /// Whether an explicitly picked job takes the whole typed message as its subject, not just the part after an instruction.
    pub whole_message_subject: bool,
    pub handler: Handler,
}

impl CairnTool {
    /// The id the job has in the API, such as `find_files`.
    pub fn capability_id(&self) -> String {
        serde_json::to_value(self.capability)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_default()
    }

    /// Whether the tool changes the workspace once the person confirms it, so a viewer is not offered it.
    pub fn writes(&self) -> bool {
        self.effect == ToolEffect::Propose
    }
}

fn subject_schema(what: &'static str) -> Value {
    json!({
        "type": "object",
        "properties": { "subject": { "type": "string", "description": what } },
        "required": ["subject"],
    })
}

fn find_files_schema() -> Value {
    subject_schema("A file name, part of a path, or an extension like .pdf")
}
fn find_symbol_schema() -> Value {
    subject_schema("The function, class or method name")
}
fn search_content_schema() -> Value {
    subject_schema("The words or phrase to look for inside files")
}
fn recall_memory_schema() -> Value {
    json!({
        "type": "object",
        "properties": { "subject": { "type": "string", "description": "What to look for in memory; empty lists the newest cards" } },
    })
}
fn summarize_file_schema() -> Value {
    subject_schema("The file to summarize")
}
fn ask_question_schema() -> Value {
    subject_schema("The question to answer from the workspace")
}
fn no_arguments_schema() -> Value {
    json!({ "type": "object", "properties": {} })
}
fn analyze_files_schema() -> Value {
    subject_schema("Which files to read together, and what to look for")
}
fn propose_memory_schema() -> Value {
    json!({
        "type": "object",
        "properties": { "subject": { "type": "string", "description": "What to remember; empty saves the last answer" } },
    })
}
fn propose_file_changes_schema() -> Value {
    subject_schema("The change, for example: move retry-policy.md to Runbooks")
}
fn propose_memory_change_schema() -> Value {
    subject_schema("The change, for example: rename the memory card \"Retry policy\" to Backoff policy")
}
fn propose_note_schema() -> Value {
    subject_schema("The note, for example: create a note titled Pager rota: Alice is on call in March")
}

macro_rules! handler {
    ($name:ident, |$core:ident, $run:ident| $body:expr) => {
        fn $name<'a>($core: &'a RepoMemoCore, $run: ToolRun<'a>) -> ToolFuture<'a> {
            Box::pin($body)
        }
    };
}

handler!(run_find_files, |core, run| core
    .agent_find_files(&run.request.workspace_id, run.subject));
handler!(run_find_symbol, |core, run| core
    .agent_find_symbol(&run.request.workspace_id, run.subject));
handler!(run_search_content, |core, run| core
    .agent_search(&run.request.workspace_id, run.subject));
handler!(run_recall_memory, |core, run| core.agent_recall_memory(
    run.request,
    run.subject,
    run.history
));
handler!(run_summarize_file, |core, run| core
    .agent_summarize(run.request, run.subject));
handler!(run_ask_question, |core, run| core.agent_ask(
    run.request,
    run.subject,
    run.history
));
handler!(run_workspace_overview, |core, run| core.agent_overview(run.request));
handler!(run_summarize_chat, |core, run| core.agent_summarize_chat(run.request));
handler!(run_analyze_files, |core, run| core
    .agent_analyze_files(run.request, run.subject));
handler!(run_propose_memory, |core, run| async move {
    Ok(core.agent_save_memory(run.request, run.subject))
});
handler!(run_propose_memory_change, |core, run| core
    .agent_manage_memory(run.request, run.subject));
handler!(run_propose_note, |core, run| core
    .agent_create_note(run.request, run.subject));
handler!(run_propose_file_changes, |core, run| core
    .agent_manage_files(run.request, run.subject));

/// Every job CAIRN offers, in the order the menu and the classifier list them.
pub(in crate::agent) static TOOLS: &[CairnTool] = &[
    CairnTool {
        name: "find_files",
        capability: AgentCapability::FindFiles,
        label: "Find files",
        description: "Match file names and paths in this workspace.",
        placeholder: "File name, part of a path, or an extension like .pdf",
        routing_hint: "locate files by name, path or type",
        parameters: find_files_schema,
        effect: ToolEffect::Read,
        requires_ai: false,
        group: "Find",
        whole_message_subject: false,
        handler: run_find_files,
    },
    CairnTool {
        name: "find_symbol",
        capability: AgentCapability::FindSymbol,
        label: "Find a symbol",
        description: "Find where a function, class or method is defined in indexed code.",
        placeholder: "Function, class or method name",
        routing_hint: "locate where a function, class or method is defined in code",
        parameters: find_symbol_schema,
        effect: ToolEffect::Read,
        requires_ai: false,
        group: "Find",
        whole_message_subject: false,
        handler: run_find_symbol,
    },
    CairnTool {
        name: "search_content",
        capability: AgentCapability::SearchContent,
        label: "Search content",
        description: "Find passages inside indexed files.",
        placeholder: "Words or a phrase to look for",
        routing_hint: "find passages inside files that mention some words",
        parameters: search_content_schema,
        effect: ToolEffect::Read,
        requires_ai: false,
        group: "Find",
        whole_message_subject: false,
        handler: run_search_content,
    },
    CairnTool {
        name: "summarize_file",
        capability: AgentCapability::SummarizeFile,
        label: "Summarize a file",
        description: "Summarize one indexed file, with the passages it used.",
        placeholder: "Which file should be summarized?",
        routing_hint: "summarize one named file",
        parameters: summarize_file_schema,
        effect: ToolEffect::Read,
        requires_ai: true,
        group: "Understand",
        whole_message_subject: false,
        handler: run_summarize_file,
    },
    CairnTool {
        name: "ask_question",
        capability: AgentCapability::AskQuestion,
        label: "Ask a question",
        description: "Answer from indexed evidence, with citations.",
        placeholder: "What do you want to know?",
        routing_hint: "answer a question using the workspace content",
        parameters: ask_question_schema,
        effect: ToolEffect::Read,
        requires_ai: true,
        group: "Understand",
        whole_message_subject: true,
        handler: run_ask_question,
    },
    CairnTool {
        name: "workspace_overview",
        capability: AgentCapability::WorkspaceOverview,
        label: "Workspace overview",
        description: "Brief the whole workspace from indexed excerpts.",
        placeholder: "No input needed",
        routing_hint: "describe the whole workspace",
        parameters: no_arguments_schema,
        effect: ToolEffect::Read,
        requires_ai: true,
        group: "Understand",
        whole_message_subject: false,
        handler: run_workspace_overview,
    },
    CairnTool {
        name: "propose_memory",
        capability: AgentCapability::SaveMemory,
        label: "Save as memory",
        description: "Turn my last answer, or a sentence you write, into a memory card that cites its evidence. I save it only when you confirm.",
        placeholder: "What should I remember? Leave it empty to save my last answer",
        routing_hint: "save the last answer, or a fact the user states, as a memory card",
        parameters: propose_memory_schema,
        effect: ToolEffect::Propose,
        requires_ai: false,
        group: "Change",
        whole_message_subject: true,
        handler: run_propose_memory,
    },
    CairnTool {
        name: "propose_file_changes",
        capability: AgentCapability::ManageFiles,
        label: "Manage files",
        description: "Move files to a folder, rename one, send them to the trash or mark them verified, outdated or needing review. I do it only when you confirm.",
        placeholder: "For example: move retry-policy.md to Runbooks",
        routing_hint: "move a named file to a folder, rename it, or move it to the trash",
        parameters: propose_file_changes_schema,
        effect: ToolEffect::Propose,
        requires_ai: false,
        group: "Change",
        whole_message_subject: true,
        handler: run_propose_file_changes,
    },
    CairnTool {
        name: "summarize_chat",
        capability: AgentCapability::SummarizeChat,
        label: "Summarize chat",
        description: "Summarize this whole conversation: what was asked, found and left open.",
        placeholder: "No input needed",
        routing_hint: "summarize this whole conversation",
        parameters: no_arguments_schema,
        effect: ToolEffect::Read,
        requires_ai: true,
        group: "Understand",
        whole_message_subject: false,
        handler: run_summarize_chat,
    },
    CairnTool {
        name: "analyze_files",
        capability: AgentCapability::AnalyzeFiles,
        label: "Analyze files",
        description: "Read several files together, such as all the Word documents or the files you selected, and write one analysis, for a question if you give one.",
        placeholder: "Which files, and what to look for? For example: the pdfs in Finance for payment terms",
        routing_hint: "analyze, review or go through several files together, such as all the Word documents, the pdfs in a folder or the selected files; the subject is the files and what to look for",
        parameters: analyze_files_schema,
        effect: ToolEffect::Read,
        requires_ai: true,
        group: "Understand",
        whole_message_subject: true,
        handler: run_analyze_files,
    },
    CairnTool {
        name: "recall_memory",
        capability: AgentCapability::RecallMemory,
        label: "Recall memory",
        description: "Search the team's memory cards, or list the newest, and show what each says and cites.",
        placeholder: "What to look for in memory? Leave it empty to list the newest cards",
        routing_hint: "look in the team's saved memory cards, such as what was decided about a topic, or list them",
        parameters: recall_memory_schema,
        effect: ToolEffect::Read,
        requires_ai: false,
        group: "Find",
        whole_message_subject: true,
        handler: run_recall_memory,
    },
    CairnTool {
        name: "propose_memory_change",
        capability: AgentCapability::ManageMemory,
        label: "Manage memory",
        description: "Change, add to, rename or delete a memory card. I do it only when you confirm.",
        placeholder: "For example: rename the memory card “Retry policy” to Backoff policy",
        routing_hint: "change, add to, rename or delete a named memory card",
        parameters: propose_memory_change_schema,
        effect: ToolEffect::Propose,
        requires_ai: false,
        group: "Change",
        whole_message_subject: true,
        handler: run_propose_memory_change,
    },
    CairnTool {
        name: "propose_note",
        capability: AgentCapability::CreateNote,
        label: "Create note",
        description: "Write a note into the workspace from a sentence you give, or from my last answer with its sources. It is indexed like any file, and I create it only when you confirm.",
        placeholder: "For example: create a note titled Pager rota: Alice is on call in March",
        routing_hint: "write a new note into the workspace from a sentence or from the last answer",
        parameters: propose_note_schema,
        effect: ToolEffect::Propose,
        requires_ai: false,
        group: "Change",
        whole_message_subject: true,
        handler: run_propose_note,
    },
];

/// The tool behind a job of the + menu.
pub(in crate::agent) fn tool_for(capability: AgentCapability) -> &'static CairnTool {
    TOOLS
        .iter()
        .find(|tool| tool.capability == capability)
        .expect("every capability has a tool")
}

/// The tool for a capability id such as `find_files`, as the routing classifier names it.
pub(in crate::agent) fn tool_for_capability_id(id: &str) -> Option<&'static CairnTool> {
    TOOLS.iter().find(|tool| tool.capability_id() == id)
}

/// The jobs the + menu and CAIRN's greeting list, from the registry.
pub fn agent_capabilities(ai_available: bool) -> Vec<AgentCapabilityInfo> {
    TOOLS
        .iter()
        .map(|tool| AgentCapabilityInfo {
            id: tool.capability,
            label: tool.label.to_owned(),
            description: tool.description.to_owned(),
            placeholder: tool.placeholder.to_owned(),
            requires_ai: tool.requires_ai,
            available: !tool.requires_ai || ai_available,
            group: tool.group.to_owned(),
            writes: tool.writes(),
            switched_off_by: None,
        })
        .collect()
}

/// Whether a job changes the workspace once the person confirms it, so a viewer cannot be offered it (V0.9.10).
pub fn capability_needs_write(capability: AgentCapability) -> bool {
    tool_for(capability).writes()
}

pub(in crate::agent) fn requires_ai(capability: AgentCapability) -> bool {
    tool_for(capability).requires_ai
}

/// The listing of jobs the routing classifier chooses from, one line each, ending with the choice of none.
pub(in crate::agent) fn classifier_catalog() -> String {
    let mut lines = TOOLS
        .iter()
        .map(|tool| format!("- {}: {}", tool.capability_id(), tool.routing_hint))
        .collect::<Vec<_>>();
    lines.push("- none: anything else, such as chit-chat, writing or editing code, or actions on files".to_owned());
    lines.join("\n")
}

impl RepoMemoCore {
    /// Runs the routed job through its registered tool.
    pub(in crate::agent) async fn run_tool(
        &self,
        capability: AgentCapability,
        request: &AgentRequest,
        subject: &str,
        history: &[AgentHistoryTurn],
    ) -> Result<AgentReply> {
        (tool_for(capability).handler)(
            self,
            ToolRun {
                request,
                subject,
                history,
            },
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    const EVERY_CAPABILITY: [AgentCapability; 13] = [
        AgentCapability::FindFiles,
        AgentCapability::FindSymbol,
        AgentCapability::SearchContent,
        AgentCapability::SummarizeFile,
        AgentCapability::AskQuestion,
        AgentCapability::WorkspaceOverview,
        AgentCapability::SaveMemory,
        AgentCapability::ManageFiles,
        AgentCapability::SummarizeChat,
        AgentCapability::AnalyzeFiles,
        AgentCapability::RecallMemory,
        AgentCapability::ManageMemory,
        AgentCapability::CreateNote,
    ];

    #[test]
    fn every_capability_has_exactly_one_tool() {
        for capability in EVERY_CAPABILITY {
            let count = TOOLS.iter().filter(|tool| tool.capability == capability).count();
            assert_eq!(count, 1, "{capability:?}");
        }
        assert_eq!(TOOLS.len(), EVERY_CAPABILITY.len());
    }

    #[test]
    fn tool_names_are_unique_snake_case_and_the_change_tools_are_proposals() {
        let mut names = HashSet::new();
        for tool in TOOLS {
            assert!(names.insert(tool.name), "{} is listed twice", tool.name);
            assert!(
                tool.name
                    .chars()
                    .all(|character| character.is_ascii_lowercase() || character == '_'),
                "{}",
                tool.name
            );
            assert_eq!(
                tool.group == "Change",
                tool.effect == ToolEffect::Propose,
                "{}",
                tool.name
            );
            assert_eq!(tool.name.starts_with("propose_"), tool.effect == ToolEffect::Propose);
            let schema = (tool.parameters)();
            assert_eq!(schema["type"], "object", "{}", tool.name);
        }
    }

    #[test]
    fn the_menu_and_the_rules_are_read_from_the_registry() {
        let capabilities = agent_capabilities(false);
        assert_eq!(capabilities.len(), TOOLS.len());
        for (info, tool) in capabilities.iter().zip(TOOLS) {
            assert_eq!(info.id, tool.capability);
            assert_eq!(info.group, tool.group);
            assert_eq!(info.writes, tool.writes());
            assert_eq!(info.available, !tool.requires_ai);
        }
        assert!(capability_needs_write(AgentCapability::CreateNote));
        assert!(!capability_needs_write(AgentCapability::RecallMemory));
        assert_eq!(tool_for(AgentCapability::AnalyzeFiles).group, "Understand");
        assert!(requires_ai(AgentCapability::SummarizeChat));
    }

    #[test]
    fn the_classifier_lists_every_job_by_its_api_id_and_offers_none() {
        let catalog = classifier_catalog();
        for tool in TOOLS {
            assert!(catalog.contains(&format!("- {}: {}", tool.capability_id(), tool.routing_hint)));
        }
        assert!(catalog.ends_with("or actions on files"));
        assert_eq!(
            tool_for_capability_id("manage_memory").map(|tool| tool.name),
            Some("propose_memory_change")
        );
        assert!(tool_for_capability_id("none").is_none());
    }
}
