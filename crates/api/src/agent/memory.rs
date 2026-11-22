//! Recalling what the workspace has kept as memory (V0.9.29): "what do we know about retries", "search memory for pager",
//! "list the memory cards". CAIRN searches the memory cards and shows them in the chat, with what each cites and whether it
//! needs review, so the team's own decisions come before a model's reading of the files.

use repomemo_domain::{
    AgentProposal, AgentProposalKind, AgentProposalState, AgentReply, AgentRouting, MemoryCardDetail,
};
use serde_json::json;

use super::{clean_subject, find_phrase, reply, shorten};

/// What the person says to look in memory, and whether the words after it are what to look for.
const RECALL_PHRASES: &[&str] = &[
    "what do we know about",
    "what do we know on",
    "what does the team know about",
    "what do we have on",
    "what do we have about",
    "what do we remember about",
    "what did we decide about",
    "what have we decided about",
    "do we have a memory about",
    "do we have a memory card about",
    "do we have a memory on",
    "search memory for",
    "search the memory for",
    "search our memory for",
    "search memories for",
    "search memory cards for",
    "search the memory cards for",
    "find memory cards about",
    "find memory cards for",
    "find memories about",
    "find a memory about",
    "memory cards about",
    "memories about",
    "recall",
];

/// Phrases that ask to see the memory without saying what to look for.
const LIST_PHRASES: &[&str] = &[
    "show me the memory cards",
    "show me my memory cards",
    "show the memory cards",
    "show my memory cards",
    "show our memory cards",
    "list the memory cards",
    "list my memory cards",
    "list our memory cards",
    "list memory cards",
    "list the memories",
    "list memories",
    "what is in memory",
    "what's in memory",
    "what is in the memory",
    "what's in the memory",
    "what memory cards do we have",
    "what memories do we have",
];

/// The longest a query is taken to be, in characters; more is a question, which is Ask's.
const QUERY_CHARS: usize = 120;

/// Recognises a request to recall memory cards. It answers with what to look for, empty to list the newest cards.
pub(crate) fn recall_request(text: &str, lower: &str) -> Option<String> {
    if LIST_PHRASES.iter().any(|phrase| find_phrase(lower, phrase).is_some()) {
        return Some(String::new());
    }
    let (start, phrase) = RECALL_PHRASES
        .iter()
        .filter_map(|phrase| find_phrase(lower, phrase).map(|start| (start, *phrase)))
        .min_by_key(|(start, phrase)| (*start, std::cmp::Reverse(phrase.len())))?;
    let rest = &text[start + phrase.len()..];
    let query = clean_subject(rest);
    // "recall" alone or followed by a long sentence is not a lookup in memory.
    (!query.is_empty() && query.chars().count() <= QUERY_CHARS).then_some(query)
}

/// What a request to change the memory asks for (V0.9.30).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MemoryCommand {
    /// Replace the card's text.
    Replace { card: String, text: String },
    /// Add a paragraph to the card's text.
    Append { card: String, text: String },
    /// Give the card a new title.
    Rename { card: String, title: String },
    /// Delete the card.
    Delete { card: String },
}

const QUOTES: &[(char, char)] = &[('“', '”'), ('"', '"'), ('\'', '\''), ('`', '`'), ('‘', '’')];

/// The card's name and what follows it. A name in quotes ends at the closing quote, so a title with a colon in it is safe;
/// otherwise it ends at the first of the separators that say the text begins.
fn card_and_rest<'a>(after: &'a str, separators: &[&str]) -> Option<(String, &'a str)> {
    let after = after.trim_start();
    if let Some(first) = after.chars().next() {
        if let Some((_, close)) = QUOTES.iter().find(|(open, _)| *open == first) {
            let inner = &after[first.len_utf8()..];
            let end = inner.find(*close)?;
            return Some((inner[..end].trim().to_owned(), &inner[end + close.len_utf8()..]));
        }
    }
    let lower = after.to_ascii_lowercase();
    let (at, length) = separators
        .iter()
        .filter_map(|separator| lower.find(separator).map(|at| (at, separator.len())))
        .min_by_key(|(at, _)| *at)?;
    Some((after[..at].trim().to_owned(), &after[at + length..]))
}

/// The text of a change, without the words that introduce it.
fn change_text(rest: &str) -> String {
    let rest = rest.trim_start_matches(|c: char| c == ':' || c == ',' || c == '-' || c.is_whitespace());
    let lower = rest.to_ascii_lowercase();
    for lead in ["to say ", "to read ", "to be ", "to: ", "to ", "so it says ", "with "] {
        if lower.starts_with(lead) {
            return rest[lead.len()..].trim().to_owned();
        }
    }
    rest.trim().to_owned()
}

/// Recognises "update the memory card X to say Y", "append to the memory card X: Y", "rename the memory card X to Y" and "delete
/// the memory card X". The message must speak of a memory card, so such words about files or anything else are not taken.
pub(crate) fn memory_command(text: &str, lower: &str) -> Option<MemoryCommand> {
    let start = find_phrase(lower, "memory card")?;
    let verb = lower[..start].trim();
    let after = &text[start + "memory card".len()..];
    let verb_is = |words: &[&str]| {
        words
            .iter()
            .any(|word| verb == *word || verb.starts_with(&format!("{word} ")))
    };
    if verb_is(&["delete", "remove", "discard", "forget"]) && !verb.contains(" from ") {
        let (card, _) = card_and_rest(after, &["\n"]).or_else(|| Some((after.trim().to_owned(), "")))?;
        let card = clean_subject(&card);
        return (!card.is_empty()).then_some(MemoryCommand::Delete { card });
    }
    if verb_is(&["rename"]) {
        let (card, rest) = card_and_rest(after, &[" to "])?;
        let title = change_text(rest)
            .trim_matches(|c: char| matches!(c, '"' | '“' | '”' | '`' | '\'' | '.'))
            .to_owned();
        return (!card.is_empty() && !title.is_empty()).then_some(MemoryCommand::Rename { card, title });
    }
    let append = verb_is(&["append to", "add to", "append"]);
    if append || verb_is(&["update", "change", "edit", "rewrite", "revise", "correct"]) {
        let (card, rest) = card_and_rest(after, &[": ", " to say ", " to read ", " to be ", " so it says "])?;
        let body = change_text(rest);
        if card.is_empty() || body.chars().count() < MIN_CHANGE_CHARS {
            return None;
        }
        return Some(if append {
            MemoryCommand::Append { card, text: body }
        } else {
            MemoryCommand::Replace { card, text: body }
        });
    }
    None
}

/// The shortest text taken as a change to a card, in characters.
const MIN_CHANGE_CHARS: usize = 8;
/// The longest a card's text is kept, as when it is saved from an answer.
const MEMORY_BODY_CHARS: usize = 8_000;

fn card_proposal(kind: AgentProposalKind, title: &str, detail: String, payload: serde_json::Value) -> AgentProposal {
    AgentProposal {
        id: "1".to_owned(),
        kind,
        state: AgentProposalState::Pending,
        title: title.to_owned(),
        detail,
        payload,
        outcome: None,
    }
}

/// The proposal to change a card's title or text. The revision the change started from goes with it, so a card saved by someone
/// else in the meantime is not overwritten.
pub(crate) fn update_reply(
    detail: &MemoryCardDetail,
    new_title: Option<&str>,
    new_body: Option<String>,
    how: &str,
) -> AgentReply {
    let card = &detail.card;
    let title = new_title.unwrap_or(&card.title).to_owned();
    let body = new_body.unwrap_or_else(|| card.body_markdown.clone());
    let body: String = body.chars().take(MEMORY_BODY_CHARS).collect();
    let mut lines = vec![format!("{how} the memory card **{}**.", card.title)];
    if title != card.title {
        lines.push(format!("New title: **{title}**"));
    }
    if body != card.body_markdown {
        lines.push(format!("Before:\n> {}", shorten(&card.body_markdown, 300)));
        lines.push(format!("After:\n> {}", shorten(&body, 300)));
    }
    lines.push(
        "The card keeps the evidence it cites, and the change is saved as a new revision you can restore.".to_owned(),
    );
    let payload = json!({
        "card_id": card.id,
        "title": title,
        "body_markdown": body,
        "source": card.source,
        "confidence": card.confidence,
        "expected_revision": detail.revision,
        "was": card.title,
    });
    let proposal = card_proposal(
        AgentProposalKind::UpdateMemory,
        "Change the memory card",
        lines.join("\n\n"),
        payload,
    );
    let mut answer = reply(format!(
        "I can change the memory card **{}**. Nothing changes until you confirm below.",
        card.title
    ));
    answer.routing = AgentRouting::Rules;
    answer.proposals = vec![proposal];
    answer
}

/// The proposal to delete a card.
pub(crate) fn delete_reply(detail: &MemoryCardDetail) -> AgentReply {
    let card = &detail.card;
    let detail_text = format!(
        "Delete the memory card **{}**:\n> {}\n\nThe evidence it cites is not touched. A deleted card cannot be restored.",
        card.title,
        shorten(&card.body_markdown, 300)
    );
    let payload = json!({ "card_id": card.id, "title": card.title });
    let proposal = card_proposal(
        AgentProposalKind::DeleteMemory,
        "Delete the memory card",
        detail_text,
        payload,
    );
    let mut answer = reply(format!(
        "I can delete the memory card **{}**. It is not deleted until you confirm below.",
        card.title
    ));
    answer.routing = AgentRouting::Rules;
    answer.proposals = vec![proposal];
    answer
}

/// The body after a paragraph was added to it.
pub(crate) fn appended(body: &str, text: &str) -> String {
    format!("{}\n\n{}", body.trim_end(), text.trim())
}

#[cfg(test)]
mod tests {
    use super::*;
    use repomemo_domain::MemoryCard;

    fn command(message: &str) -> Option<MemoryCommand> {
        memory_command(message, &message.to_ascii_lowercase())
    }

    fn detail() -> MemoryCardDetail {
        MemoryCardDetail {
            card: MemoryCard {
                id: "card-1".to_owned(),
                workspace_id: "w".to_owned(),
                title: "Retry policy".to_owned(),
                body_markdown: "Retries wait twice as long.".to_owned(),
                source: "Team note".to_owned(),
                confidence: Some(0.8),
                created_at: String::new(),
                updated_at: String::new(),
                metadata: serde_json::Value::Null,
            },
            evidence: Vec::new(),
            review: Vec::new(),
            revision: 3,
        }
    }

    #[test]
    fn changes_to_a_memory_card_are_read_with_the_card_and_the_text() {
        assert_eq!(
            command("Update the memory card “Retry policy”: Retries wait twice as long."),
            Some(MemoryCommand::Replace {
                card: "Retry policy".into(),
                text: "Retries wait twice as long.".into()
            })
        );
        assert_eq!(
            command("update the memory card Retry policy to say Retries wait three times as long"),
            Some(MemoryCommand::Replace {
                card: "Retry policy".into(),
                text: "Retries wait three times as long".into()
            })
        );
        assert_eq!(
            command("append to the memory card Pager policy: Escalate after 15 minutes."),
            Some(MemoryCommand::Append {
                card: "Pager policy".into(),
                text: "Escalate after 15 minutes.".into()
            })
        );
        assert_eq!(
            command("rename the memory card Retry policy to Backoff policy"),
            Some(MemoryCommand::Rename {
                card: "Retry policy".into(),
                title: "Backoff policy".into()
            })
        );
        assert_eq!(
            command("delete the memory card “Retry policy”"),
            Some(MemoryCommand::Delete {
                card: "Retry policy".into()
            })
        );
        assert_eq!(
            command("update the memory card “A: B” to say the new text is here"),
            Some(MemoryCommand::Replace {
                card: "A: B".into(),
                text: "the new text is here".into()
            })
        );
    }

    #[test]
    fn a_request_that_is_not_about_a_memory_card_is_left_alone() {
        for message in [
            "delete the file readme.md",
            "what is a memory card",
            "update the memory card",
            "change the memory card Retry policy",
            "save that as a memory card",
            "remove the memory leak from the code",
            "update the readme to say hello there",
        ] {
            assert_eq!(command(message), None, "{message}");
        }
    }

    #[test]
    fn a_change_is_one_proposal_that_carries_the_revision_it_started_from() {
        let card = detail();
        let changed = update_reply(
            &card,
            None,
            Some("Retries wait three times as long.".to_owned()),
            "Replace the text of",
        );
        let proposal = &changed.proposals[0];
        assert_eq!(proposal.kind, AgentProposalKind::UpdateMemory);
        assert_eq!(proposal.payload["card_id"], "card-1");
        assert_eq!(proposal.payload["expected_revision"], 3);
        assert_eq!(proposal.payload["title"], "Retry policy");
        assert_eq!(proposal.payload["source"], "Team note");
        assert_eq!(proposal.payload["body_markdown"], "Retries wait three times as long.");
        assert!(proposal.detail.contains("Before:\n> Retries wait twice as long."));
        assert!(proposal.detail.contains("After:\n> Retries wait three times as long."));
        assert!(proposal.detail.contains("new revision you can restore"));

        let renamed = update_reply(&card, Some("Backoff policy"), None, "Rename");
        assert_eq!(renamed.proposals[0].payload["title"], "Backoff policy");
        assert_eq!(
            renamed.proposals[0].payload["body_markdown"],
            "Retries wait twice as long."
        );
        assert!(renamed.proposals[0].detail.contains("New title: **Backoff policy**"));

        assert_eq!(
            appended("First paragraph.\n", " Second. "),
            "First paragraph.\n\nSecond."
        );
    }

    #[test]
    fn a_deletion_is_one_proposal_that_says_it_cannot_be_restored() {
        let answer = delete_reply(&detail());
        let proposal = &answer.proposals[0];
        assert_eq!(proposal.kind, AgentProposalKind::DeleteMemory);
        assert_eq!(proposal.payload["card_id"], "card-1");
        assert!(
            proposal.detail.contains("cannot be restored")
                && proposal.detail.contains("evidence it cites is not touched")
        );
        assert!(answer.reply_markdown.contains("not deleted until you confirm"));
    }

    fn parse(message: &str) -> Option<String> {
        recall_request(message, &message.to_ascii_lowercase())
    }

    #[test]
    fn a_request_to_recall_memory_is_read_with_what_to_look_for() {
        assert_eq!(parse("what do we know about retries?"), Some("retries".to_owned()));
        assert_eq!(
            parse("What did we decide about the pager policy"),
            Some("pager policy".to_owned())
        );
        assert_eq!(
            parse("search memory for deploy approval"),
            Some("deploy approval".to_owned())
        );
        assert_eq!(
            parse("find memory cards about onboarding."),
            Some("onboarding".to_owned())
        );
        assert_eq!(parse("recall the SQLite decision"), Some("SQLite decision".to_owned()));
        assert_eq!(parse("do we have a memory about backups?"), Some("backups".to_owned()));
    }

    #[test]
    fn asking_to_see_the_memory_lists_the_newest_cards() {
        for message in [
            "show me the memory cards",
            "List memory cards",
            "what's in memory?",
            "what memory cards do we have",
        ] {
            assert_eq!(parse(message), Some(String::new()), "{message}");
        }
    }

    #[test]
    fn other_requests_are_not_a_lookup_in_memory() {
        for message in [
            "what do we know",
            "recall",
            "how does the retry backoff work",
            "save that as a memory card",
            "find the file about login",
            "what do we have for lunch",
        ] {
            assert_eq!(parse(message), None, "{message}");
        }
    }
}
