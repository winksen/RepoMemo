//! The citation ledger (V0.9.41). An answer of the loop cites evidence by handle (`[E3]`). Only handles the run itself was shown
//! count: the ones it read become the evidence list, in the order they are cited and numbered to match the markers in the text;
//! any other is removed from the text and reported. An answer that rests on files the run read and cites none of them says so.

use repomemo_domain::{AgentSourceState, Citation, MemoryCardSummary};

use super::tools::atoms::EvidenceLedger;

/// An answer after the ledger has checked it.
#[derive(Debug, Clone)]
pub(in crate::agent) struct LedgerCheck {
    pub markdown: String,
    pub citations: Vec<Citation>,
    /// The memory cards the answer cites (`[M1]`), in the order they are cited (V0.9.50).
    pub cards: Vec<MemoryCardSummary>,
    pub warnings: Vec<String>,
}

/// Said under an answer that read evidence and cites none of it.
pub(in crate::agent) const UNTIED: &str = "I could not tie this answer to a source.";

/// The handle a token of a marker names, if it is one: `E3` for evidence, `M1` for a memory card (either case).
fn handle_of(token: &str) -> Option<String> {
    let token = token.trim();
    let kind = token.chars().next()?.to_ascii_uppercase();
    let digits = &token[1..];
    (matches!(kind, 'E' | 'M') && !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()))
        .then(|| format!("{kind}{digits}"))
}

/// Whether the run was given what a handle stands for.
fn is_known(ledger: &EvidenceLedger, handle: &str) -> bool {
    if handle.starts_with('M') {
        ledger.card(handle).is_some()
    } else {
        ledger.get(handle).is_some()
    }
}

/// The tokens of a marker's inside when every one of them is a handle (`E3`, `E3, E1`, `E3 E1`), else none.
fn marker_handles(inside: &str) -> Option<Vec<String>> {
    let tokens = inside
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    if tokens.is_empty() {
        return None;
    }
    tokens.iter().map(|token| handle_of(token)).collect()
}

/// Checks an answer against what the run read. See the module's description.
pub(in crate::agent) fn check_answer(ledger: &EvidenceLedger, answer: &str, says_insufficient: bool) -> LedgerCheck {
    // First pass: the handles cited that the run read, in the order they first appear, and the ones it did not.
    let mut cited: Vec<String> = Vec::new();
    let mut invented: Vec<String> = Vec::new();
    let mut rest = answer;
    while let Some(open) = rest.find('[') {
        let after = &rest[open + 1..];
        let Some(close) = after.find(']') else { break };
        if let Some(handles) = marker_handles(&after[..close]) {
            for handle in handles {
                if is_known(ledger, &handle) {
                    if !cited.contains(&handle) {
                        cited.push(handle);
                    }
                } else if !invented.contains(&handle) {
                    invented.push(handle);
                }
            }
        }
        rest = &after[close + 1..];
    }

    // Second pass: the text, with each marker renumbered to the evidence list and unknown handles taken out.
    let mut markdown = String::with_capacity(answer.len());
    let mut rest = answer;
    while let Some(open) = rest.find('[') {
        let after = &rest[open + 1..];
        let Some(close) = after.find(']') else { break };
        match marker_handles(&after[..close]) {
            Some(handles) => {
                let numbers = handles
                    .iter()
                    .filter_map(|handle| {
                        if !is_known(ledger, handle) {
                            None
                        } else if handle.starts_with('M') {
                            Some(handle.clone())
                        } else {
                            cited
                                .iter()
                                .filter(|known| known.starts_with('E'))
                                .position(|known| known == handle)
                                .map(|position| (position + 1).to_string())
                        }
                    })
                    .collect::<Vec<_>>();
                markdown.push_str(&rest[..open]);
                if numbers.is_empty() {
                    // Nothing in the marker is evidence of this run: no marker, and no space left before it.
                    while markdown.ends_with(' ') && after[close + 1..].starts_with([' ', '.', ',', ';', ':']) {
                        markdown.pop();
                    }
                } else {
                    markdown.push('[');
                    markdown.push_str(&numbers.join(", "));
                    markdown.push(']');
                }
            }
            None => markdown.push_str(&rest[..open + 1 + close + 1]),
        }
        rest = &after[close + 1..];
    }
    markdown.push_str(rest);

    let cards = cited
        .iter()
        .filter(|handle| handle.starts_with('M'))
        .filter_map(|handle| ledger.card(handle).cloned())
        .collect::<Vec<_>>();
    let citations = cited
        .iter()
        .filter(|handle| handle.starts_with('E'))
        .filter_map(|handle| ledger.get(handle))
        .map(|item| Citation {
            artifact_id: item.artifact_id.clone(),
            chunk_id: item.chunk_id.clone(),
            title: item.title.clone(),
            path: item.path.clone(),
            start_line: item.start_line,
            end_line: item.end_line,
            confidence: None,
            commit: None,
            last_author: None,
            last_changed_at: None,
        })
        .collect::<Vec<_>>();

    let mut warnings = Vec::new();
    if !invented.is_empty() {
        warnings.push(format!(
            "I removed {} that I had not read: {}.",
            if invented.len() == 1 { "a source" } else { "sources" },
            invented.join(", ")
        ));
    }
    if cited.is_empty() && (!ledger.items().is_empty() || ledger.has_cards()) && !says_insufficient {
        markdown = format!("{}\n\n{UNTIED}", markdown.trim_end());
    }
    LedgerCheck {
        markdown,
        citations,
        cards,
        warnings,
    }
}

/// The line under an answer about its sources (V0.9.45): how many are verified, need review, are outdated or superseded or have not
/// been reviewed, and for each kind of outside content how many there are and how long ago the oldest was synced.
pub(in crate::agent) fn trust_summary(sources: &[AgentSourceState]) -> Option<String> {
    if sources.is_empty() {
        return None;
    }
    let count = |state: &str| {
        sources
            .iter()
            .filter(|source| source.untrusted.is_none() && source.state == state)
            .count()
    };
    let mut parts = Vec::new();
    for (state, label) in [
        ("verified", "verified"),
        ("needs_review", "needing review"),
        ("outdated", "outdated"),
        ("superseded", "superseded"),
        ("active", "not reviewed"),
    ] {
        let found = count(state);
        if found > 0 {
            parts.push(format!("{found} {label}"));
        }
    }
    let mut kinds: Vec<&str> = Vec::new();
    for source in sources {
        if let Some(kind) = source.untrusted.as_deref() {
            if !kinds.contains(&kind) {
                kinds.push(kind);
            }
        }
    }
    for kind in kinds {
        let from = sources
            .iter()
            .filter(|source| source.untrusted.as_deref() == Some(kind))
            .collect::<Vec<_>>();
        let oldest = from.iter().filter_map(|source| source.synced_days_ago).max();
        parts.push(match oldest {
            Some(days) => format!(
                "{} from {kind} (synced {})",
                from.len(),
                if days == 0 {
                    "today".to_owned()
                } else if days == 1 {
                    "1 day ago".to_owned()
                } else {
                    format!("{days} days ago")
                }
            ),
            None => format!("{} from {kind}", from.len()),
        });
    }
    Some(parts.join(" · "))
}

/// What to say next to an answer that rests on a source that may be wrong: one warning naming each such source.
pub(in crate::agent) fn trust_warnings(citations: &[Citation], sources: &[AgentSourceState]) -> Vec<String> {
    let mut warnings = Vec::new();
    for source in sources {
        let title = citations
            .iter()
            .find(|citation| citation.artifact_id == source.artifact_id)
            .map_or("a source", |citation| citation.title.as_str());
        match (source.state.as_str(), source.untrusted.as_deref()) {
            ("outdated", _) => warnings.push(format!("This answer rests on {title}, which is marked outdated.")),
            ("superseded", _) => warnings.push(format!(
                "This answer rests on {title}, which a newer file has replaced."
            )),
            ("needs_review", _) => warnings.push(format!("This answer rests on {title}, which is waiting for review.")),
            (_, Some(kind)) => warnings.push(format!(
                "{title} is {kind} from outside the workspace; check it before you rely on this answer."
            )),
            _ => {}
        }
    }
    warnings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::tools::atoms::EvidenceKind;

    fn ledger() -> EvidenceLedger {
        let mut ledger = EvidenceLedger::new("w1");
        for (file, chunk, title) in [
            ("a1", "c1", "retries.md"),
            ("a2", "c2", "pager.md"),
            ("a3", "c3", "backups.md"),
        ] {
            ledger.handle_for(EvidenceKind::Passage, file, Some(chunk), title, title, Some("active"));
        }
        ledger
    }

    #[test]
    fn a_handle_the_run_never_read_is_removed_and_reported() {
        let check = check_answer(
            &ledger(),
            "Retries wait twice as long [E9]. They stop after five [E1].",
            false,
        );
        assert_eq!(check.markdown, "Retries wait twice as long. They stop after five [1].");
        assert_eq!(check.citations.len(), 1);
        assert_eq!(check.citations[0].title, "retries.md");
        assert_eq!(
            check.warnings,
            vec!["I removed a source that I had not read: E9.".to_owned()]
        );
    }

    #[test]
    fn handles_become_citations_in_the_order_they_are_cited() {
        let check = check_answer(&ledger(), "Pager first [E2][E1], then backups [E3, E2].", false);
        assert_eq!(check.markdown, "Pager first [1][2], then backups [3, 1].");
        let titles = check.citations.iter().map(|c| c.title.as_str()).collect::<Vec<_>>();
        assert_eq!(titles, ["pager.md", "retries.md", "backups.md"]);
        assert!(check.warnings.is_empty());
    }

    #[test]
    fn an_answer_resting_on_nothing_it_read_says_so_and_one_that_read_nothing_passes() {
        let untied = check_answer(&ledger(), "Retries wait twice as long.", false);
        assert!(untied.markdown.ends_with(UNTIED));
        assert!(untied.citations.is_empty());
        let insufficient = check_answer(&ledger(), "The indexed context is insufficient to answer.", true);
        assert!(!insufficient.markdown.contains(UNTIED));
        let nothing_read = check_answer(&EvidenceLedger::new("w1"), "Hello! How can I help?", false);
        assert_eq!(nothing_read.markdown, "Hello! How can I help?");
        assert!(nothing_read.warnings.is_empty());
    }

    fn source(id: &str, state: &str, untrusted: Option<&str>, synced: Option<i64>) -> AgentSourceState {
        AgentSourceState {
            artifact_id: id.to_owned(),
            state: state.to_owned(),
            untrusted: untrusted.map(str::to_owned),
            synced_days_ago: synced,
        }
    }

    #[test]
    fn the_summary_counts_states_and_outside_content_with_how_stale_it_is() {
        let sources = [
            source("a", "verified", None, None),
            source("b", "verified", None, None),
            source("c", "verified", None, None),
            source("d", "outdated", None, None),
            source("e", "active", Some("an issue"), Some(9)),
        ];
        assert_eq!(
            trust_summary(&sources).as_deref(),
            Some("3 verified · 1 outdated · 1 from an issue (synced 9 days ago)")
        );
        assert_eq!(trust_summary(&[]), None);
        assert_eq!(
            trust_summary(&[source("a", "needs_review", None, None)]).as_deref(),
            Some("1 needing review")
        );
    }

    #[test]
    fn an_answer_resting_on_an_outdated_file_says_so() {
        let citations = ledger_citations();
        let warnings = trust_warnings(&citations, &[source("a1", "outdated", None, None)]);
        assert_eq!(
            warnings,
            vec!["This answer rests on retries.md, which is marked outdated.".to_owned()]
        );
        assert!(trust_warnings(&citations, &[source("a1", "verified", None, None)]).is_empty());
        let outside = trust_warnings(&citations, &[source("a1", "active", Some("an issue"), None)]);
        assert!(outside[0].contains("from outside the workspace"));
    }

    fn ledger_citations() -> Vec<Citation> {
        check_answer(&ledger(), "See [E1].", false).citations
    }

    #[test]
    fn a_memory_card_is_cited_by_its_handle_and_an_invented_one_is_removed() {
        let mut ledger = ledger();
        let card = repomemo_domain::MemoryCardSummary {
            id: "card-1".to_owned(),
            workspace_id: "w1".to_owned(),
            title: "Retry decision".to_owned(),
            body_excerpt: "Capped at five.".to_owned(),
            source: "chat".to_owned(),
            evidence_count: 1,
            created_at: String::new(),
            updated_at: String::new(),
            needs_review: false,
        };
        assert_eq!(ledger.card_handle_for(&card), "M1");
        assert_eq!(ledger.card_handle_for(&card), "M1", "the same card keeps its handle");
        let check = check_answer(
            &ledger,
            "The team capped retries [M1] and the file agrees [E2]. See also [M7].",
            false,
        );
        assert_eq!(
            check.markdown,
            "The team capped retries [M1] and the file agrees [1]. See also."
        );
        assert_eq!(check.cards.len(), 1);
        assert_eq!(check.cards[0].title, "Retry decision");
        assert_eq!(check.citations.len(), 1);
        assert_eq!(
            check.warnings,
            vec!["I removed a source that I had not read: M7.".to_owned()]
        );
        // An answer that cites only a card is tied to a source.
        assert!(!check_answer(&ledger, "Capped at five [M1].", false)
            .markdown
            .contains(UNTIED));
    }

    #[test]
    fn ordinary_brackets_are_left_alone() {
        let check = check_answer(&ledger(), "Use arr[0] and [a link](x.md) and see [E1].", false);
        assert_eq!(check.markdown, "Use arr[0] and [a link](x.md) and see [1].");
    }
}
