//! What CAIRN offers to do (V0.9.10). A reply can carry proposals: actions CAIRN describes and does not take. They run only
//! when the person confirms, as that person and through the same checks as when they do it themselves, so CAIRN never gains
//! a way to change the workspace that its user does not have.
//!
//! This part builds the proposals. The first is saving a memory card: from the answer just given, with the evidence it
//! cited, or from a sentence the person writes.

use repomemo_domain::{
    AgentExchange, AgentProposal, AgentProposalKind, AgentProposalState, AgentReply, AgentRouting, Citation,
};
use serde_json::json;

use super::{find_phrase, reply, shorten};

/// What the person says to have something saved as a memory card.
const SAVE_MEMORY_PHRASES: &[&str] = &[
    "save this as a memory card",
    "save that as a memory card",
    "save it as a memory card",
    "save this as a memory",
    "save that as a memory",
    "save it as a memory",
    "save this as memory",
    "save that as memory",
    "save it as memory",
    "save as a memory card",
    "save as memory",
    "save to memory",
    "save this to memory",
    "save that to memory",
    "add this to memory",
    "add that to memory",
    "add it to memory",
    "create a memory card",
    "make a memory card",
    "create a memory",
    "turn this into a memory card",
    "turn that into a memory card",
    "keep this as a memory",
    "keep that as a memory",
    "remember this",
    "remember that",
];

/// The longest text a memory card made here carries, in characters.
const MEMORY_BODY_CHARS: usize = 8_000;
const MEMORY_TITLE_CHARS: usize = 80;
/// The shortest sentence taken as the thing to remember, in characters; less is a leftover word, not a statement.
const MIN_STATEMENT_CHARS: usize = 8;

/// Recognises a request to save a memory card. It answers with the text the person gave to save (empty when they gave none and
/// mean the last answer): what follows the phrase, after any colon or comma, when that is a sentence.
pub(crate) fn save_memory_request(text: &str, lower: &str) -> Option<String> {
    let (start, phrase) = SAVE_MEMORY_PHRASES
        .iter()
        .filter_map(|phrase| find_phrase(lower, phrase).map(|start| (start, *phrase)))
        .min_by_key(|(start, phrase)| (*start, std::cmp::Reverse(phrase.len())))?;
    let rest =
        text[start + phrase.len()..].trim_matches(|c: char| c == ':' || c == ',' || c == '-' || c.is_whitespace());
    Some(if rest.chars().count() >= MIN_STATEMENT_CHARS {
        rest.to_owned()
    } else {
        String::new()
    })
}

/// A card's title from the question it answers or the sentence it states: one line, without markup or a closing question mark.
fn title_of(text: &str) -> String {
    let line = text.lines().find(|line| !line.trim().is_empty()).unwrap_or("");
    let plain = line
        .trim()
        .trim_start_matches(|c: char| c == '#' || c == '*' || c == '-' || c == '>' || c.is_whitespace())
        .trim_end_matches(|c: char| c == '?' || c == '.' || c == ':' || c == '*' || c.is_whitespace());
    let mut title = shorten(plain, MEMORY_TITLE_CHARS);
    if let Some(first) = title.chars().next() {
        title = first.to_uppercase().collect::<String>() + &title[first.len_utf8()..];
    }
    title
}

/// The names of the files behind some citations, each once.
fn file_names(citations: &[Citation]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for citation in citations {
        if !names.contains(&citation.title) {
            names.push(citation.title.clone());
        }
    }
    names
}

/// CAIRN's reply to a request to save a memory card: a proposal to confirm, or what is missing to make one.
pub(crate) fn save_memory_reply(statement: &str, exchanges: &[AgentExchange]) -> AgentReply {
    let (title, body, citations) = if !statement.is_empty() {
        (title_of(statement), statement.to_owned(), Vec::new())
    } else if let Some(last) = exchanges.last() {
        if last.answer.trim().is_empty() || super::evidence::says_insufficient(&last.answer) {
            return reply("My last answer found nothing, so there is nothing worth saving. Ask me something I can answer from the files, or write the memory out yourself: \"remember this: Deploys need approval\".");
        }
        (title_of(&last.question), last.answer.clone(), last.citations.clone())
    } else {
        return reply("There is nothing to save yet. Ask me something first and then say \"save that as a memory\", or write it out: \"remember this: Deploys need approval\".");
    };
    let body = shorten_keeping_lines(&body, MEMORY_BODY_CHARS);
    let names = file_names(&citations);
    let evidence = if citations.is_empty() {
        "No evidence is attached to it.".to_owned()
    } else {
        format!(
            "It cites {} {} from {}.",
            citations.len(),
            if citations.len() == 1 { "passage" } else { "passages" },
            names.join(", ")
        )
    };
    let preview = shorten(&body, 500);
    let proposal = AgentProposal {
        id: "1".to_owned(),
        kind: AgentProposalKind::SaveMemory,
        state: AgentProposalState::Pending,
        title: "Save as a memory card".to_owned(),
        detail: format!("**{title}**\n\n{preview}\n\n{evidence}"),
        payload: json!({
            "title": title,
            "body_markdown": body,
            "source": "CAIRN chat",
            "confidence": null,
            "citations": citations,
        }),
        outcome: None,
        untrusted_sources: Vec::new(),
    };
    let mut answer = reply(format!(
        "I can save this as a memory card named **{title}**. {evidence} Nothing is saved until you confirm below."
    ));
    answer.routing = AgentRouting::Rules;
    answer.proposals = vec![proposal];
    answer
}

/// A memory card proposed from a correction the person gave (V0.9.53): the corrected fact as they stated it, with what they said
/// was wrong. It waits for their confirmation like any card, and it carries no evidence of its own.
pub(crate) fn correction_card(title: &str, fact: &str, statement: &str) -> AgentProposal {
    let body = shorten_keeping_lines(
        &format!("{fact}\n\nCorrected after a wrong answer: {statement}"),
        MEMORY_BODY_CHARS,
    );
    AgentProposal {
        id: "p1".to_owned(),
        kind: AgentProposalKind::SaveMemory,
        state: AgentProposalState::Pending,
        title: "Save the correction as a memory card".to_owned(),
        detail: format!(
            "**{title}**\n\n{}\n\nIt cites no file: it is what you told me.",
            shorten(&body, 500)
        ),
        payload: json!({ "title": title, "body_markdown": body, "source": "CAIRN chat correction", "confidence": null, "citations": [], "offer": "correction" }),
        outcome: None,
        untrusted_sources: Vec::new(),
    }
}

/// A decision card offered when the person stated a decision in the chat (V0.9.53). Dismissing it stops these offers in that chat.
pub(crate) fn decision_card(title: &str, decision: &str) -> AgentProposal {
    let body = shorten_keeping_lines(decision, MEMORY_BODY_CHARS);
    AgentProposal {
        id: "p1".to_owned(),
        kind: AgentProposalKind::SaveMemory,
        state: AgentProposalState::Pending,
        title: "Save the decision as a memory card".to_owned(),
        detail: format!(
            "**{title}**\n\n{}\n\nIt cites no file: it is what was decided in this chat.",
            shorten(&body, 500)
        ),
        payload: json!({ "title": title, "body_markdown": body, "source": "CAIRN chat decision", "confidence": null, "citations": [], "offer": "decision" }),
        outcome: None,
        untrusted_sources: Vec::new(),
    }
}

/// A text cut to a length without losing its line breaks, which `shorten` flattens.
fn shorten_keeping_lines(text: &str, limit: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= limit {
        return trimmed.to_owned();
    }
    let cut = trimmed.chars().take(limit.saturating_sub(1)).collect::<String>();
    format!("{}…", cut.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exchange(question: &str, answer: &str, cited: &[&str]) -> AgentExchange {
        AgentExchange {
            question: question.to_owned(),
            answer: answer.to_owned(),
            citations: cited
                .iter()
                .map(|title| Citation {
                    artifact_id: format!("id-{title}"),
                    chunk_id: None,
                    title: (*title).to_owned(),
                    path: format!("docs/{title}"),
                    start_line: Some(3),
                    end_line: Some(9),
                    confidence: None,
                    commit: None,
                    last_author: None,
                    last_changed_at: None,
                })
                .collect(),
            file_ids: Vec::new(),
        }
    }

    fn parse(message: &str) -> Option<String> {
        save_memory_request(message, &message.to_ascii_lowercase())
    }

    #[test]
    fn the_phrases_for_saving_a_memory_are_recognised_with_or_without_text() {
        assert_eq!(parse("save that as a memory card"), Some(String::new()));
        assert_eq!(parse("Please save this as memory"), Some(String::new()));
        assert_eq!(parse("remember this"), Some(String::new()));
        assert_eq!(
            parse("remember this: deploys need approval"),
            Some("deploys need approval".to_owned())
        );
        assert_eq!(
            parse("remember that releases go out on Fridays"),
            Some("releases go out on Fridays".to_owned())
        );
        assert_eq!(
            parse("create a memory card: retries use exponential backoff"),
            Some("retries use exponential backoff".to_owned())
        );
        assert_eq!(parse("what is a memory card"), None);
        assert_eq!(parse("how do I save a file"), None);
    }

    #[test]
    fn the_last_answer_becomes_a_proposal_with_its_evidence() {
        let last = exchange(
            "How does the retry backoff work?",
            "Retries wait twice as long each time [1].",
            &["Retry policy", "Retry policy", "Runbook"],
        );
        let answer = save_memory_reply("", &[exchange("hello", "Salam!", &[]), last]);
        assert_eq!(answer.proposals.len(), 1);
        let proposal = &answer.proposals[0];
        assert_eq!(proposal.kind, AgentProposalKind::SaveMemory);
        assert_eq!(proposal.state, AgentProposalState::Pending);
        assert_eq!(proposal.payload["title"], "How does the retry backoff work");
        assert_eq!(
            proposal.payload["body_markdown"],
            "Retries wait twice as long each time [1]."
        );
        assert_eq!(proposal.payload["source"], "CAIRN chat");
        assert_eq!(proposal.payload["citations"].as_array().unwrap().len(), 3);
        assert!(
            proposal.detail.contains("cites 3 passages from Retry policy, Runbook"),
            "{}",
            proposal.detail
        );
        assert!(answer.reply_markdown.contains("Nothing is saved until you confirm"));
        assert_eq!(answer.routing, AgentRouting::Rules);
    }

    #[test]
    fn a_stated_sentence_is_saved_as_written_with_no_evidence() {
        let answer = save_memory_reply("deploys need approval from two people", &[]);
        let proposal = &answer.proposals[0];
        assert_eq!(proposal.payload["title"], "Deploys need approval from two people");
        assert_eq!(
            proposal.payload["body_markdown"],
            "deploys need approval from two people"
        );
        assert_eq!(proposal.payload["citations"].as_array().unwrap().len(), 0);
        assert!(proposal.detail.contains("No evidence is attached"));
    }

    #[test]
    fn with_nothing_to_save_there_is_no_proposal_and_the_reply_says_what_to_do() {
        let none = save_memory_reply("", &[]);
        assert!(none.proposals.is_empty() && none.reply_markdown.contains("nothing to save yet"));
        let nothing_found = save_memory_reply(
            "",
            &[exchange(
                "what is the pager policy",
                "The indexed context is insufficient to answer this question.",
                &[],
            )],
        );
        assert!(nothing_found.proposals.is_empty() && nothing_found.reply_markdown.contains("found nothing"));
    }

    #[test]
    fn a_title_is_one_plain_line() {
        assert_eq!(title_of("## how do retries work?"), "How do retries work");
        assert_eq!(
            title_of("\n\n**Deploys need approval.**\nmore"),
            "Deploys need approval"
        );
        assert!(title_of(&"x".repeat(300)).chars().count() <= MEMORY_TITLE_CHARS);
    }
}
