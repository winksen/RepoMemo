//! What an outside agent proposes over MCP (V0.9.67): a memory card or a note, made into a pending proposal for the CAIRN inbox. The
//! agent, the token it used and what it wrote are all outside content, so the proposal says whom it is from, quotes the text as data
//! and names the client among its untrusted sources. It does nothing until the person confirms it.

use repomemo_domain::{AgentProposal, AgentProposalKind, AgentProposalState, AgentReply, AgentRouting};
use serde_json::json;

use super::shorten;

/// The most pending proposals a person's inbox holds.
pub const INBOX_PENDING_MAX: i64 = 50;
const TITLE_CHARS: usize = 120;
/// The most characters of a proposal's body.
pub const BODY_CHARS: usize = 4_000;

/// The inbox turn for one proposal: its label, and what CAIRN says about it with the proposal itself. `note` is a note; otherwise a
/// memory card.
pub fn inbox_turn(client: &str, note: bool, title: &str, body: &str) -> Result<(String, AgentReply), String> {
    let title = shorten(&title.split_whitespace().collect::<Vec<_>>().join(" "), TITLE_CHARS);
    let title = title.trim_end_matches('…').trim().to_owned();
    let body = body.trim();
    if title.is_empty() || body.is_empty() {
        return Err("A proposal needs a title and a body.".to_owned());
    }
    if body.chars().count() > BODY_CHARS {
        return Err(format!("A proposal's body is at most {BODY_CHARS} characters."));
    }
    let client = shorten(client.trim(), 60);
    let quoted = body
        .lines()
        .map(|line| format!("> {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    let what = if note { "a note" } else { "a memory card" };
    let proposal = if note {
        AgentProposal {
            id: "p1".to_owned(),
            kind: AgentProposalKind::SaveNote,
            state: AgentProposalState::Pending,
            title: format!("Save “{title}” as a note"),
            detail: format!(
                "A note named **{title}**, proposed by {client}. It cites no file: it is what the agent wrote."
            ),
            payload: json!({
                "title": title,
                "content": format!("_Proposed by {client} over MCP and confirmed by a person._\n\n{body}"),
                "language": "Note",
            }),
            outcome: None,
            untrusted_sources: vec![client.clone()],
        }
    } else {
        AgentProposal {
            id: "p1".to_owned(),
            kind: AgentProposalKind::SaveMemory,
            state: AgentProposalState::Pending,
            title: format!("Save “{title}” as a memory card"),
            detail: format!(
                "A memory card named **{title}**, proposed by {client}. It cites no file: it is what the agent wrote."
            ),
            payload: json!({
                "title": title,
                "body_markdown": body,
                "source": format!("Outside agent: {client}"),
                "confidence": null,
                "citations": [],
            }),
            outcome: None,
            untrusted_sources: vec![client.clone()],
        }
    };
    let text = format!(
        "**{client}**, an outside agent reading this workspace over MCP, proposes {what} named **{title}**. What it wrote is below as quoted data; check it before you confirm, since it may be wrong or may have been steered by something it read.\n\n{quoted}"
    );
    let reply = AgentReply {
        capability: None,
        routing: AgentRouting::Rules,
        subject: format!("{client}: {title}"),
        reply_markdown: text,
        generated: false,
        files: Vec::new(),
        matches: Vec::new(),
        citations: Vec::new(),
        warnings: Vec::new(),
        context_turns: 0,
        proposals: vec![proposal],
        file_set: None,
        memory_cards: Vec::new(),
        changeset: None,
        trust: None,
        ai_unavailable_reason: None,
    };
    Ok((format!("{client} proposes: {title}"), reply))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_outside_proposal_names_its_client_and_quotes_what_it_wrote_as_data() {
        let (label, reply) =
            inbox_turn("Cursor", false, "  Retry   policy ", "Five attempts.\nDoubling waits.").unwrap();
        assert_eq!(label, "Cursor proposes: Retry policy");
        assert!(reply.reply_markdown.contains("> Five attempts.\n> Doubling waits."));
        assert!(reply.reply_markdown.contains("may have been steered"));
        let proposal = &reply.proposals[0];
        assert_eq!(proposal.kind, AgentProposalKind::SaveMemory);
        assert_eq!(proposal.state, AgentProposalState::Pending);
        assert_eq!(proposal.untrusted_sources, vec!["Cursor".to_owned()]);
        assert_eq!(proposal.payload["source"], "Outside agent: Cursor");
        let note = inbox_turn("Cursor", true, "Runbook", "Steps.").unwrap().1;
        assert_eq!(note.proposals[0].kind, AgentProposalKind::SaveNote);
        assert!(note.proposals[0].payload["content"]
            .as_str()
            .unwrap()
            .contains("Proposed by Cursor over MCP"));
        assert!(inbox_turn("Cursor", false, " ", "x").is_err());
        assert!(inbox_turn("Cursor", false, "t", &"a".repeat(BODY_CHARS + 1)).is_err());
    }
}
