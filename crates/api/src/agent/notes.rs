//! Creating notes from the chat (V0.9.31): "create a note titled Pager rota: Alice is on call in March", "save that as a note in the
//! Runbooks folder". A note is evidence like any text file: stored in the workspace, indexed so CAIRN and search find it, and
//! cited like any file. CAIRN writes it from the person's sentence or from its last answer with the sources that answer cited,
//! and stores it only when the person confirms.

use repomemo_domain::{
    AgentExchange, AgentProposal, AgentProposalKind, AgentProposalState, AgentReply, AgentRouting, Citation,
};
use serde_json::json;

use super::{find_phrase, reply, shorten};

/// What the person says to have a note made.
const NOTE_PHRASES: &[&str] = &[
    "save that as a note",
    "save this as a note",
    "save it as a note",
    "save as a note",
    "create a note",
    "add a note",
    "make a note",
    "write a note",
    "write down a note",
    "take a note",
];

/// The shortest sentence taken as the text of a note, in characters.
const MIN_NOTE_CHARS: usize = 8;
const NOTE_TITLE_CHARS: usize = 80;
const NOTE_CHARS: usize = 20_000;

/// What a request to create a note says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NoteRequest {
    /// The title, when the person gave one ("titled X", "called X").
    pub title: Option<String>,
    /// The text, when the person wrote it; empty means the last answer.
    pub text: String,
    /// The folder to store it in, as named.
    pub folder: Option<String>,
}

/// Cuts a title at the words that go on to say where the note goes.
fn title_before_place(title: &str) -> String {
    let lower = title.to_ascii_lowercase();
    let cut = [
        " in the ",
        " in folder ",
        " into the ",
        " in ",
        " into ",
        " to the folder ",
    ]
    .iter()
    .filter_map(|marker| lower.find(marker))
    .min()
    .unwrap_or(title.len());
    title[..cut]
        .trim()
        .trim_matches(|c: char| matches!(c, '"' | '“' | '”' | '`' | '\'' | '.'))
        .to_owned()
}

/// The folder a request names: "in the Runbooks folder" or "in folder Runbooks".
fn folder_in(head: &str) -> Option<String> {
    let tokens = head.split_whitespace().collect::<Vec<_>>();
    let is_place = |token: &&str| matches!(token.to_ascii_lowercase().as_str(), "in" | "into" | "to");
    let is_folder = |token: &str| matches!(token.to_ascii_lowercase().as_str(), "folder" | "directory");
    let named = |words: &[&str]| {
        let words = words
            .iter()
            .filter(|word| !word.eq_ignore_ascii_case("the"))
            .copied()
            .collect::<Vec<_>>();
        (!words.is_empty()).then(|| words.join(" "))
    };
    // "in the Runbooks folder"
    if tokens.last().is_some_and(|token| is_folder(token)) {
        let end = tokens.len() - 1;
        if let Some(place) = tokens[..end].iter().rposition(is_place) {
            return named(&tokens[place + 1..end]);
        }
    }
    // "in folder Runbooks"
    let at = tokens.iter().position(|token| is_folder(token))?;
    (at > 0 && is_place(&tokens[at - 1]))
        .then(|| named(&tokens[at + 1..]))
        .flatten()
}

/// Recognises a request to create a note, with its title, text and folder when it gives them. The text follows a colon; what comes
/// before the colon may name the title (*titled X*, *called X*) and the folder (*in the X folder*).
pub(crate) fn note_request(text: &str, lower: &str) -> Option<NoteRequest> {
    let (start, phrase) = NOTE_PHRASES
        .iter()
        .filter_map(|phrase| find_phrase(lower, phrase).map(|start| (start, *phrase)))
        .min_by_key(|(start, phrase)| (*start, std::cmp::Reverse(phrase.len())))?;
    let rest = &text[start + phrase.len()..];
    let (head, body) = match rest.split_once(':') {
        Some((head, body)) => (head, body.trim()),
        None => (rest, ""),
    };
    let head_lower = head.to_ascii_lowercase();
    let title = [" titled ", " called ", " named "]
        .iter()
        .find_map(|word| {
            head_lower
                .find(word)
                .map(|at| title_before_place(&head[at + word.len()..]))
        })
        .filter(|title| !title.is_empty());
    let folder = folder_in(head);
    // Without a colon, the words after the phrase are the text only when nothing names a title or a folder.
    let text = if body.is_empty() && title.is_none() && folder.is_none() {
        head.trim()
            .trim_matches(|c: char| c == ',' || c == '-' || c.is_whitespace())
            .to_owned()
    } else {
        body.to_owned()
    };
    let text = if text.chars().count() >= MIN_NOTE_CHARS {
        text
    } else {
        String::new()
    };
    Some(NoteRequest { title, text, folder })
}

/// A note's title from its text or from the question it answers: one plain line.
fn title_from(text: &str) -> String {
    let line = text.lines().find(|line| !line.trim().is_empty()).unwrap_or("");
    let plain = line
        .trim()
        .trim_start_matches(|c: char| c == '#' || c == '*' || c == '-' || c == '>' || c.is_whitespace())
        .trim_end_matches(|c: char| c == '?' || c == '.' || c == ':' || c == '*' || c.is_whitespace());
    shorten(plain, NOTE_TITLE_CHARS)
}

fn sources_section(sources: &[Citation]) -> String {
    if sources.is_empty() {
        return String::new();
    }
    let mut section = String::from("\n\n## Sources\n");
    for source in sources {
        section.push_str(&format!("- {} ({})\n", source.title, source.path));
    }
    section
}

/// CAIRN's reply to a request to create a note: a proposal to confirm, or what is missing to make one. `folder` is the folder the
/// request named, already found, as its id and path.
pub(crate) fn note_reply(asked: &NoteRequest, exchanges: &[AgentExchange], folder: Option<(&str, &str)>) -> AgentReply {
    let (title, body, sources) = if !asked.text.is_empty() {
        let title = asked.title.clone().unwrap_or_else(|| title_from(&asked.text));
        (title, asked.text.clone(), Vec::new())
    } else if let Some(last) = exchanges.last().filter(|last| !last.answer.trim().is_empty()) {
        if super::evidence::says_insufficient(&last.answer) {
            return reply("My last answer found nothing, so there is nothing worth keeping as a note. Write the note yourself: `create a note titled Pager rota: Alice is on call in March`.");
        }
        let title = asked.title.clone().unwrap_or_else(|| title_from(&last.question));
        (title, last.answer.clone(), last.citations.clone())
    } else {
        return reply("There is nothing to put in a note yet. Write it out: `create a note titled Pager rota: Alice is on call in March`, or ask me something first and then say `save that as a note`.");
    };
    let title = if title.is_empty() { "Note".to_owned() } else { title };
    let content: String = format!("{}{}", body.trim(), sources_section(&sources))
        .chars()
        .take(NOTE_CHARS)
        .collect();
    let place = folder.map_or("the workspace's top level".to_owned(), |(_, path)| {
        format!("the folder **{path}**")
    });
    let proposal = AgentProposal {
        id: "1".to_owned(),
        kind: AgentProposalKind::SaveNote,
        state: AgentProposalState::Pending,
        title: "Create the note".to_owned(),
        detail: format!(
            "Create a note named **{title}** in {place}:\n\n> {}\n\nIt is indexed like any file, so CAIRN and search can find it, and everyone who can read this workspace can read it.",
            shorten(&body, 400)
        ),
        payload: json!({
            "title": title,
            "content": content,
            "language": "Note",
            "folder_id": folder.map(|(id, _)| id),
        }),
        outcome: None,
    };
    let mut answer = reply(format!(
        "I can create a note named **{title}** in {place}. Nothing is stored until you confirm below."
    ));
    answer.routing = AgentRouting::Rules;
    answer.proposals = vec![proposal];
    answer
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(message: &str) -> Option<NoteRequest> {
        note_request(message, &message.to_ascii_lowercase())
    }

    fn exchange(question: &str, answer: &str) -> AgentExchange {
        AgentExchange {
            question: question.to_owned(),
            answer: answer.to_owned(),
            citations: vec![Citation {
                artifact_id: "a".to_owned(),
                chunk_id: None,
                title: "Retry policy".to_owned(),
                path: "docs/retry.md".to_owned(),
                start_line: None,
                end_line: None,
                confidence: None,
                commit: None,
                last_author: None,
                last_changed_at: None,
            }],
            file_ids: Vec::new(),
        }
    }

    #[test]
    fn a_request_to_create_a_note_is_read_with_its_title_text_and_folder() {
        let full = parse("create a note titled Pager rota in the Runbooks folder: Alice is on call in March").unwrap();
        assert_eq!(full.title.as_deref(), Some("Pager rota"));
        assert_eq!(full.text, "Alice is on call in March");
        assert_eq!(full.folder.as_deref(), Some("Runbooks"));

        let plain = parse("Add a note: Deploys need approval from two people").unwrap();
        assert_eq!(
            (plain.title, plain.text.as_str(), plain.folder),
            (None, "Deploys need approval from two people", None)
        );

        let last = parse("save that as a note").unwrap();
        assert_eq!((last.title, last.text.as_str(), last.folder), (None, "", None));

        let last_in = parse("save this as a note in the Decisions folder").unwrap();
        assert_eq!(
            (last_in.text.as_str(), last_in.folder.as_deref()),
            ("", Some("Decisions"))
        );
        assert_eq!(
            parse("write a note called Standup").unwrap().title.as_deref(),
            Some("Standup")
        );
    }

    #[test]
    fn other_requests_are_not_a_note() {
        for message in [
            "what is a note",
            "find the note about login",
            "save that as a memory",
            "notes about retries",
        ] {
            assert_eq!(parse(message), None, "{message}");
        }
    }

    #[test]
    fn a_written_note_is_proposed_as_written_in_the_folder_named() {
        let asked = parse("create a note titled Pager rota: Alice is on call in March").unwrap();
        let answer = note_reply(&asked, &[], Some(("f-9", "Runbooks / Payments")));
        let proposal = &answer.proposals[0];
        assert_eq!(proposal.kind, AgentProposalKind::SaveNote);
        assert_eq!(proposal.payload["title"], "Pager rota");
        assert_eq!(proposal.payload["content"], "Alice is on call in March");
        assert_eq!(proposal.payload["language"], "Note");
        assert_eq!(proposal.payload["folder_id"], "f-9");
        assert!(
            proposal.detail.contains("the folder **Runbooks / Payments**")
                && proposal.detail.contains("everyone who can read this workspace")
        );
        assert!(answer.reply_markdown.contains("Nothing is stored until you confirm"));
    }

    #[test]
    fn the_last_answer_becomes_a_note_with_its_sources() {
        let asked = parse("save that as a note").unwrap();
        let answer = note_reply(
            &asked,
            &[exchange(
                "How does the retry backoff work?",
                "Retries wait twice as long [1].",
            )],
            None,
        );
        let proposal = &answer.proposals[0];
        assert_eq!(proposal.payload["title"], "How does the retry backoff work");
        assert_eq!(
            proposal.payload["content"],
            "Retries wait twice as long [1].\n\n## Sources\n- Retry policy (docs/retry.md)\n"
        );
        assert!(proposal.payload["folder_id"].is_null());
        assert!(proposal.detail.contains("the workspace's top level"));
    }

    #[test]
    fn with_nothing_to_put_in_a_note_there_is_no_proposal() {
        let asked = parse("save that as a note").unwrap();
        assert!(note_reply(&asked, &[], None).proposals.is_empty());
        let nothing = note_reply(
            &asked,
            &[exchange("pager?", "The indexed context is insufficient.")],
            None,
        );
        assert!(nothing.proposals.is_empty() && nothing.reply_markdown.contains("found nothing"));
    }
}
