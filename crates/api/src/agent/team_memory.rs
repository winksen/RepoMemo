//! Team memory first (V0.9.50). At the start of a run of the loop CAIRN is given the memory cards that match the message, the ones
//! whose evidence still holds first, as handles `M1`, `M2`, … in a *Team memory* block. The team wrote those cards down on purpose,
//! so CAIRN is told to prefer a current card to a passage that disagrees with it, to say when they disagree, and to treat a card marked
//! as needing review as possibly out of date. A card it cites is shown in the chat with its review state.

use repomemo_domain::MemoryCardSummary;

use super::tools::atoms::EvidenceLedger;
use super::{words_of, FILLER_WORDS, QUESTION_WORDS};
use crate::RepoMemoCore;

/// The most cards a run starts with.
pub const TEAM_MEMORY_CARDS: usize = 5;
/// The most characters of a card's text given to the model.
const CARD_CHARS: usize = 220;

/// The words of a message that can find a card: not filler, not a question word, at least three letters, each once.
pub(in crate::agent) fn significant_words(message: &str) -> Vec<String> {
    let mut words = Vec::new();
    for word in words_of(message) {
        if word.chars().count() >= 3
            && !FILLER_WORDS.contains(&word.as_str())
            && !QUESTION_WORDS.contains(&word.as_str())
            && !words.contains(&word)
        {
            words.push(word);
        }
        if words.len() == 8 {
            break;
        }
    }
    words
}

/// The cards that match the message, at most [`TEAM_MEMORY_CARDS`]: a card whose evidence still holds before one that needs review,
/// then the card matching more of the words, then the one with more evidence.
pub(in crate::agent) fn rank_cards(cards: Vec<MemoryCardSummary>, words: &[String]) -> Vec<MemoryCardSummary> {
    if words.is_empty() {
        return Vec::new();
    }
    let mut scored = cards
        .into_iter()
        .filter_map(|card| {
            let text = format!("{} {}", card.title, card.body_excerpt).to_lowercase();
            let score = words.iter().filter(|word| text.contains(word.as_str())).count();
            (score > 0).then_some((score, card))
        })
        .collect::<Vec<_>>();
    scored.sort_by(|(score_a, a), (score_b, b)| {
        a.needs_review
            .cmp(&b.needs_review)
            .then(score_b.cmp(score_a))
            .then(b.evidence_count.cmp(&a.evidence_count))
            .then(b.updated_at.cmp(&a.updated_at))
    });
    scored
        .into_iter()
        .take(TEAM_MEMORY_CARDS)
        .map(|(_, card)| card)
        .collect()
}

/// The *Team memory* block of the system message, one line a card.
pub(in crate::agent) fn team_memory_block(cards: &[(String, MemoryCardSummary)]) -> String {
    let lines = cards
        .iter()
        .map(|(handle, card)| {
            let state = if card.needs_review { "needs review" } else { "current" };
            let text = card.body_excerpt.split_whitespace().collect::<Vec<_>>().join(" ");
            let text: String = text.chars().take(CARD_CHARS).collect();
            format!("[{handle}] {} ({state}): {text}", card.title)
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Team memory: cards the team saved on purpose. Prefer a card that is current to a passage that disagrees with it, and say when they disagree. A card marked needs review may be out of date. Cite a card by its handle, such as [M1].\n{lines}"
    )
}

impl RepoMemoCore {
    /// The memory cards that match a message, given handles in the run's ledger, as the block for the prompt; none when no card
    /// matches or the cards cannot be read.
    pub(in crate::agent) async fn team_memory(
        &self,
        ledger: &mut EvidenceLedger,
        workspace_id: &str,
        message: &str,
    ) -> Option<String> {
        let cards = self.storage.list_memory_cards(workspace_id).await.ok()?;
        let found = rank_cards(cards, &significant_words(message));
        if found.is_empty() {
            return None;
        }
        let with_handles = found
            .into_iter()
            .map(|card| (ledger.card_handle_for(&card), card))
            .collect::<Vec<_>>();
        Some(team_memory_block(&with_handles))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(id: &str, title: &str, body: &str, needs_review: bool, evidence: i64) -> MemoryCardSummary {
        MemoryCardSummary {
            id: id.to_owned(),
            workspace_id: "w1".to_owned(),
            title: title.to_owned(),
            body_excerpt: body.to_owned(),
            source: "chat".to_owned(),
            evidence_count: evidence,
            created_at: "2026-01-01".to_owned(),
            updated_at: "2026-01-01".to_owned(),
            needs_review,
        }
    }

    #[test]
    fn the_words_that_can_find_a_card_leave_out_filler() {
        let words = significant_words("What did we decide about the retries policy?");
        assert!(words.contains(&"retries".to_owned()) && words.contains(&"policy".to_owned()));
        assert!(!words.contains(&"what".to_owned()) && !words.contains(&"the".to_owned()));
        assert!(significant_words("? ! a an").is_empty());
    }

    #[test]
    fn current_cards_come_first_then_the_better_match_and_at_most_five() {
        let words = vec!["retries".to_owned(), "policy".to_owned()];
        let cards = vec![
            card("old", "Retry policy", "retries policy from last year", true, 5),
            card(
                "best",
                "Retries policy",
                "Retries are capped at five by policy",
                false,
                1,
            ),
            card("some", "Policy", "Escalation policy", false, 9),
            card("none", "Billing", "Invoices go out monthly", false, 3),
        ];
        let ranked = rank_cards(cards, &words);
        assert_eq!(
            ranked.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            ["best", "some", "old"]
        );
        let many = (0..9)
            .map(|n| card(&format!("c{n}"), "Retries", "retries", false, n))
            .collect();
        assert_eq!(rank_cards(many, &words).len(), TEAM_MEMORY_CARDS);
        assert!(rank_cards(Vec::new(), &words).is_empty());
        assert!(rank_cards(vec![card("a", "Retries", "x", false, 0)], &[]).is_empty());
    }

    #[test]
    fn the_block_marks_a_card_that_needs_review_and_tells_cairn_to_prefer_a_current_one() {
        let block = team_memory_block(&[
            (
                "M1".to_owned(),
                card("a", "Retry decision", "Capped at five.", false, 1),
            ),
            ("M2".to_owned(), card("b", "Old rule", "Three retries.", true, 1)),
        ]);
        assert!(block.contains("[M1] Retry decision (current): Capped at five."));
        assert!(block.contains("[M2] Old rule (needs review): Three retries."));
        assert!(block.contains("Prefer a card that is current"));
        assert!(block.contains("say when they disagree"));
    }
}
