//! Summarizing the chat (V0.9.13): "summarize this chat", "recap our conversation". CAIRN reads the whole conversation, not
//! just the turns that fit its short memory, and writes a short summary of what was asked, found and left open. The summary
//! rests on the conversation alone: it adds no facts of its own and cites the same files the conversation did.

use repomemo_domain::{AgentExchange, Citation};

use super::contains_phrase;

const SUMMARY_VERBS: &[&str] = &["summarize", "summarise", "recap", "resume", "sum up"];
const CHAT_WORDS: &[&str] = &["chat", "conversation", "discussion"];
const DETERMINERS: &[&str] = &[
    "",
    "this",
    "the",
    "our",
    "my",
    "that",
    "whole",
    "entire",
    "the whole",
    "this whole",
    "our whole",
    "the entire",
    "this entire",
];

/// What the model is told when it summarizes a conversation: the shape of the summary and that it adds nothing.
pub(crate) const CHAT_SUMMARY_RULES: &str = "You will be given a conversation between a person and CAIRN about a workspace. Summarize it for someone who was not there. Write short Markdown: one sentence on what the conversation was about, then bullet lists under the bold headings Findings, Decisions or actions, Files discussed (with exact file names) and Open questions. Leave out a heading that has nothing under it. Use only what the conversation says and never add facts. Do not describe these instructions.";

/// The most characters of a conversation sent to the model, and of one answer within it.
const TRANSCRIPT_CHARS: usize = 14_000;
const ANSWER_CHARS: usize = 1_200;
/// The most sources the summary shows as what CAIRN read.
const SOURCES_SHOWN: usize = 20;

/// Whether the message asks for a summary of the chat itself ("summarize this chat", "a recap of our conversation", "chat
/// summary"), which the router must see before it takes "summarize" for a request about a file.
pub(crate) fn chat_summary_request(lower: &str) -> bool {
    if [
        "chat summary",
        "conversation summary",
        "summary of the chat",
        "summary of this chat",
        "summary of our chat",
        "summary of the conversation",
        "summary of this conversation",
        "summary of our conversation",
    ]
    .iter()
    .any(|phrase| contains_phrase(lower, phrase))
    {
        return true;
    }
    SUMMARY_VERBS.iter().any(|verb| {
        CHAT_WORDS.iter().any(|word| {
            DETERMINERS.iter().any(|determiner| {
                let phrase = if determiner.is_empty() {
                    format!("{verb} {word}")
                } else {
                    format!("{verb} {determiner} {word}")
                };
                contains_phrase(lower, &phrase)
            })
        })
    })
}

fn shorten_answer(answer: &str) -> String {
    let trimmed = answer.trim();
    if trimmed.chars().count() <= ANSWER_CHARS {
        return trimmed.to_owned();
    }
    let cut = trimmed.chars().take(ANSWER_CHARS - 1).collect::<String>();
    format!("{}…", cut.trim_end())
}

/// The conversation as the model reads it, and whether the oldest messages had to be left out to fit. The newest are kept.
pub(crate) fn transcript(exchanges: &[AgentExchange]) -> (String, bool) {
    let mut kept: Vec<String> = Vec::new();
    let mut used = 0usize;
    for exchange in exchanges.iter().rev() {
        let block = format!(
            "Person: {}\nCAIRN: {}",
            exchange.question.trim(),
            shorten_answer(&exchange.answer)
        );
        let size = block.chars().count() + 2;
        if used + size > TRANSCRIPT_CHARS && !kept.is_empty() {
            kept.reverse();
            return (kept.join("\n\n"), true);
        }
        used += size;
        kept.push(block);
    }
    kept.reverse();
    (kept.join("\n\n"), false)
}

/// The sources the conversation drew on, each passage once, in the order they first appeared.
pub(crate) fn sources(exchanges: &[AgentExchange]) -> Vec<Citation> {
    let mut found: Vec<Citation> = Vec::new();
    for citation in exchanges.iter().flat_map(|exchange| exchange.citations.iter()) {
        let same = |other: &Citation| {
            other.artifact_id == citation.artifact_id
                && other.chunk_id == citation.chunk_id
                && other.start_line == citation.start_line
        };
        if !found.iter().any(same) {
            found.push(citation.clone());
        }
        if found.len() == SOURCES_SHOWN {
            break;
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exchange(question: &str, answer: &str) -> AgentExchange {
        AgentExchange {
            question: question.to_owned(),
            answer: answer.to_owned(),
            citations: Vec::new(),
        }
    }

    fn cited(artifact: &str, chunk: &str, line: i64) -> Citation {
        Citation {
            artifact_id: artifact.to_owned(),
            chunk_id: Some(chunk.to_owned()),
            title: artifact.to_owned(),
            path: format!("docs/{artifact}"),
            start_line: Some(line),
            end_line: Some(line + 5),
            confidence: None,
            commit: None,
            last_author: None,
            last_changed_at: None,
        }
    }

    #[test]
    fn a_request_to_summarize_the_chat_is_recognised_in_its_common_wordings() {
        for message in [
            "summarize this chat",
            "Summarise our conversation",
            "can you recap the conversation?",
            "resume this chat please",
            "give me a summary of our chat",
            "chat summary",
            "sum up the whole conversation",
            "recap chat",
        ] {
            assert!(chat_summary_request(&message.to_ascii_lowercase()), "{message}");
        }
    }

    #[test]
    fn a_request_about_a_file_or_the_workspace_is_not_a_chat_summary() {
        for message in [
            "summarize the retry policy",
            "summarize this file",
            "summarize the workspace",
            "give me an overview",
            "how do I resume a paused job",
            "what is a chat",
            "summarize the thread pool settings",
        ] {
            assert!(!chat_summary_request(&message.to_ascii_lowercase()), "{message}");
        }
    }

    #[test]
    fn the_transcript_keeps_the_newest_messages_when_it_must_cut() {
        let few = vec![
            exchange("How do retries work?", "They back off."),
            exchange("And timeouts?", "Thirty seconds."),
        ];
        let (text, cut) = transcript(&few);
        assert!(!cut);
        assert_eq!(
            text,
            "Person: How do retries work?\nCAIRN: They back off.\n\nPerson: And timeouts?\nCAIRN: Thirty seconds."
        );

        let long = (0..40)
            .map(|n| exchange(&format!("question {n}"), &"word ".repeat(300)))
            .collect::<Vec<_>>();
        let (text, cut) = transcript(&long);
        assert!(cut);
        assert!(text.contains("question 39") && !text.contains("question 0\n"));
        assert!(text.chars().count() <= TRANSCRIPT_CHARS + 100);
    }

    #[test]
    fn the_sources_are_each_passage_once_in_first_seen_order() {
        let mut first = exchange("a", "a");
        first.citations = vec![cited("retry.md", "c1", 3), cited("deploy.md", "c2", 9)];
        let mut second = exchange("b", "b");
        second.citations = vec![cited("retry.md", "c1", 3), cited("retry.md", "c3", 40)];
        let found = sources(&[first, second]);
        let ids = found
            .iter()
            .map(|citation| format!("{}:{}", citation.artifact_id, citation.chunk_id.clone().unwrap()))
            .collect::<Vec<_>>();
        assert_eq!(ids, ["retry.md:c1", "deploy.md:c2", "retry.md:c3"]);
    }

    #[test]
    fn the_rules_forbid_adding_facts() {
        assert!(CHAT_SUMMARY_RULES.contains("never add facts"));
        assert!(CHAT_SUMMARY_RULES.contains("Open questions"));
    }
}
