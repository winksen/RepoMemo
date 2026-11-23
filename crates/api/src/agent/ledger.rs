//! The citation ledger (V0.9.41). An answer of the loop cites evidence by handle (`[E3]`). Only handles the run itself was shown
//! count: the ones it read become the evidence list, in the order they are cited and numbered to match the markers in the text;
//! any other is removed from the text and reported. An answer that rests on files the run read and cites none of them says so.

use repomemo_domain::Citation;

use super::tools::atoms::EvidenceLedger;

/// An answer after the ledger has checked it.
#[derive(Debug, Clone)]
pub(in crate::agent) struct LedgerCheck {
    pub markdown: String,
    pub citations: Vec<Citation>,
    pub warnings: Vec<String>,
}

/// Said under an answer that read evidence and cites none of it.
pub(in crate::agent) const UNTIED: &str = "I could not tie this answer to a source.";

/// The handle a token of a marker names, if it is one: `E3`, `e3`.
fn handle_of(token: &str) -> Option<String> {
    let token = token.trim();
    let digits = token.strip_prefix(['E', 'e'])?;
    (!digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())).then(|| format!("E{digits}"))
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
                if ledger.get(&handle).is_some() {
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
                    .filter_map(|handle| cited.iter().position(|known| known == handle))
                    .map(|position| (position + 1).to_string())
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

    let citations = cited
        .iter()
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
    if cited.is_empty() && !ledger.items().is_empty() && !says_insufficient {
        markdown = format!("{}\n\n{UNTIED}", markdown.trim_end());
    }
    LedgerCheck {
        markdown,
        citations,
        warnings,
    }
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

    #[test]
    fn ordinary_brackets_are_left_alone() {
        let check = check_answer(&ledger(), "Use arr[0] and [a link](x.md) and see [E1].", false);
        assert_eq!(check.markdown, "Use arr[0] and [a link](x.md) and see [1].");
    }
}
