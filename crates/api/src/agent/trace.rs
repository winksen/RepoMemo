//! What a run did, as a trace (V0.9.39): the steps behind one answer, each a one-line summary with the files it named. Today a
//! message is answered by one tool, so a run is that tool, the changes it proposed and the answer; the loop (V0.9.40) adds a
//! step for each tool it chains. A trace never holds the text of a passage.

use repomemo_domain::{
    AgentMessage, AgentReply, AgentRouting, AgentStep, AgentStepEvidence, ArtifactSummary, Citation,
};

use super::{shorten, tools::tool_for};

/// The most files one step names.
const EVIDENCE_PER_STEP: usize = 12;
const SUMMARY_CHARS: usize = 160;
/// What CAIRN says when a job fails; the run is `failed` then.
pub(crate) const FAILED_REPLY: &str = "I couldn't complete that.";

/// How a message was answered, ready to be stored.
#[derive(Debug, Clone)]
pub struct RunTrace {
    /// `answered`, `awaiting_confirmation` or `failed`.
    pub state: String,
    pub steps: Vec<AgentStep>,
    /// Tokens the model reported for the run; 0 for a run with no model call or a provider that does not say.
    pub input_tokens: i64,
    pub output_tokens: i64,
}

fn named_files(files: &[ArtifactSummary], citations: &[Citation]) -> Vec<AgentStepEvidence> {
    let mut seen = Vec::<String>::new();
    let mut out = Vec::new();
    let from_files = files.iter().map(|file| (&file.id, &file.title));
    let from_citations = citations
        .iter()
        .map(|citation| (&citation.artifact_id, &citation.title));
    for (id, title) in from_citations.chain(from_files) {
        if out.len() == EVIDENCE_PER_STEP {
            break;
        }
        if !seen.contains(id) {
            seen.push(id.clone());
            out.push(AgentStepEvidence {
                artifact_id: id.clone(),
                title: title.clone(),
            });
        }
    }
    out
}

fn how(routing: AgentRouting) -> &'static str {
    match routing {
        AgentRouting::Explicit => "picked",
        AgentRouting::Rules => "recognised",
        AgentRouting::Model => "classified by the model",
        AgentRouting::Agent => "worked out with its tools",
        AgentRouting::Unmatched => "answered in CAIRN's own words",
    }
}

/// The trace of one message: the tool that ran (when one did), a step for each change proposed, and the answer. `duration_ms` is
/// the time the whole message took and is given to the tool step, which is where it went.
pub fn agent_run_trace(_message: &AgentMessage, reply: &AgentReply, duration_ms: i64) -> RunTrace {
    let failed = reply.reply_markdown.trim() == FAILED_REPLY;
    let mut steps = Vec::new();
    let mut push = |kind: &str,
                    tool: Option<&str>,
                    summary: String,
                    evidence: Vec<AgentStepEvidence>,
                    duration: i64,
                    error: Option<String>| {
        steps.push(AgentStep {
            position: steps.len() as i64 + 1,
            kind: kind.to_owned(),
            tool: tool.map(str::to_owned),
            summary: shorten(&summary, SUMMARY_CHARS),
            evidence,
            duration_ms: duration,
            error,
        });
    };
    match reply.capability {
        Some(capability) => {
            let tool = tool_for(capability);
            let subject = reply.subject.split_whitespace().collect::<Vec<_>>().join(" ");
            let summary = if subject.is_empty() {
                format!("{} ({})", tool.label, how(reply.routing))
            } else {
                format!("{}: {subject} ({})", tool.label, how(reply.routing))
            };
            push(
                "tool",
                Some(tool.name),
                summary,
                named_files(&reply.files, &reply.citations),
                duration_ms,
                failed.then(|| reply.warnings.first().cloned()).flatten(),
            );
        }
        None => push(
            "note",
            None,
            format!("No job fits; {}", how(AgentRouting::Unmatched)),
            Vec::new(),
            duration_ms,
            None,
        ),
    }
    for proposal in &reply.proposals {
        push(
            "proposal",
            None,
            format!("Proposed: {}", proposal.title),
            Vec::new(),
            0,
            None,
        );
    }
    let (state, summary) = if failed {
        ("failed", "Could not complete the request".to_owned())
    } else if !reply.proposals.is_empty() {
        (
            "awaiting_confirmation",
            format!(
                "Waiting for you to confirm {} {}",
                reply.proposals.len(),
                if reply.proposals.len() == 1 {
                    "change"
                } else {
                    "changes"
                }
            ),
        )
    } else if reply.generated {
        ("answered", "Answered, written by the AI".to_owned())
    } else {
        ("answered", "Answered from stored facts".to_owned())
    };
    push("answer", None, summary, Vec::new(), 0, None);
    RunTrace {
        state: state.to_owned(),
        steps,
        input_tokens: 0,
        output_tokens: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use repomemo_domain::{AgentCapability, AgentProposal, AgentProposalKind, AgentProposalState};
    use serde_json::json;

    fn reply(capability: Option<AgentCapability>, routing: AgentRouting, subject: &str, text: &str) -> AgentReply {
        AgentReply {
            capability,
            routing,
            subject: subject.to_owned(),
            reply_markdown: text.to_owned(),
            generated: false,
            files: Vec::new(),
            matches: Vec::new(),
            citations: Vec::new(),
            warnings: Vec::new(),
            context_turns: 0,
            proposals: Vec::new(),
            file_set: None,
            memory_cards: Vec::new(),
            changeset: None,
            trust: None,
            ai_unavailable_reason: None,
        }
    }

    fn message() -> AgentMessage {
        AgentMessage::default()
    }

    fn citation(id: &str, title: &str) -> Citation {
        Citation {
            artifact_id: id.to_owned(),
            chunk_id: Some(format!("{id}-c")),
            title: title.to_owned(),
            path: format!("{title}.md"),
            start_line: None,
            end_line: None,
            confidence: None,
            commit: None,
            last_author: None,
            last_changed_at: None,
        }
    }

    #[test]
    fn a_routed_job_is_one_tool_step_and_an_answer() {
        let mut found = reply(
            Some(AgentCapability::SearchContent),
            AgentRouting::Rules,
            "retry policy",
            "2 passages mention it.",
        );
        found.citations = vec![
            citation("a1", "retries"),
            citation("a1", "retries"),
            citation("a2", "pager"),
        ];
        let trace = agent_run_trace(&message(), &found, 840);
        assert_eq!(trace.state, "answered");
        assert_eq!(trace.steps.len(), 2);
        let tool = &trace.steps[0];
        assert_eq!(
            (tool.position, tool.kind.as_str(), tool.tool.as_deref()),
            (1, "tool", Some("search_content"))
        );
        assert_eq!(tool.summary, "Search content: retry policy (recognised)");
        assert_eq!(tool.duration_ms, 840);
        assert_eq!(tool.evidence.len(), 2, "a file is named once");
        assert_eq!(trace.steps[1].kind, "answer");
        assert_eq!(trace.steps[1].position, 2);
    }

    #[test]
    fn a_message_no_job_fits_is_a_note_and_an_answer() {
        let trace = agent_run_trace(&message(), &reply(None, AgentRouting::Unmatched, "", "Hello!"), 12);
        assert_eq!(trace.state, "answered");
        assert_eq!(trace.steps[0].kind, "note");
        assert!(trace.steps[0].tool.is_none());
    }

    #[test]
    fn proposals_make_steps_and_the_run_waits_for_the_person() {
        let mut proposing = reply(
            Some(AgentCapability::CreateNote),
            AgentRouting::Explicit,
            "Pager rota",
            "Here is the note.",
        );
        proposing.proposals = vec![AgentProposal {
            id: "p1".to_owned(),
            kind: AgentProposalKind::SaveSummary,
            state: AgentProposalState::Pending,
            title: "Create the note Pager rota".to_owned(),
            detail: String::new(),
            payload: json!({}),
            outcome: None,
            untrusted_sources: Vec::new(),
        }];
        let trace = agent_run_trace(&message(), &proposing, 5);
        assert_eq!(trace.state, "awaiting_confirmation");
        assert_eq!(
            trace.steps.iter().map(|step| step.kind.as_str()).collect::<Vec<_>>(),
            ["tool", "proposal", "answer"]
        );
        assert_eq!(trace.steps[2].summary, "Waiting for you to confirm 1 change");
    }

    #[test]
    fn a_failed_job_carries_its_reason_and_a_trace_never_holds_passage_text() {
        let mut failed = reply(
            Some(AgentCapability::AskQuestion),
            AgentRouting::Rules,
            "why",
            FAILED_REPLY,
        );
        failed.warnings = vec!["could not reach the provider".to_owned()];
        failed.matches = Vec::new();
        let trace = agent_run_trace(&message(), &failed, 90);
        assert_eq!(trace.state, "failed");
        assert_eq!(trace.steps[0].error.as_deref(), Some("could not reach the provider"));
        let long = reply(
            Some(AgentCapability::FindFiles),
            AgentRouting::Explicit,
            &"word ".repeat(200),
            "x",
        );
        let summary = &agent_run_trace(&message(), &long, 1).steps[0].summary;
        assert!(summary.chars().count() <= SUMMARY_CHARS);
    }
}
