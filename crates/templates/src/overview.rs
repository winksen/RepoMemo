use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::TemplateIssue;

/// The figures the Overview can show as a card, by id. Every one is already in the metrics the page loads
/// (`web/workspace/pulseCards.tsx` draws them; a test compares the two lists).
pub const PULSE_CARDS: [&str; 11] = [
    "index_coverage",
    "indexed_storage",
    "fresh_evidence",
    "knowledge_density",
    "code_symbols",
    "sources",
    "memory_cards",
    "team_access",
    "active_tasks",
    "at_risk",
    "team_discussion",
];

/// The cards a workspace shows when its template does not choose, in the order they have always had.
pub const DEFAULT_CARDS: [&str; 9] = [
    "index_coverage",
    "indexed_storage",
    "fresh_evidence",
    "knowledge_density",
    "code_symbols",
    "team_access",
    "active_tasks",
    "at_risk",
    "team_discussion",
];

/// The pulse cards the dashboard can show as a workspace's headline figure (V0.2.187): the ones one grouped count over
/// the dashboard's workspaces answers, so the dashboard asks for no workspace one by one.
pub const HEADLINE_CARDS: [&str; 4] = ["memory_cards", "active_tasks", "code_symbols", "sources"];

/// How many cards a template may list.
pub const CARDS_MAX: usize = 8;

/// Which pulse cards the Overview shows and in what order (V0.2.174). A card not listed is not shown; the charts
/// below the cards stay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OverviewBlock {
    #[serde(default)]
    pub cards: Vec<String>,
    /// The figure shown on the workspace's card on the dashboard, one of [`HEADLINE_CARDS`]; none when left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headline: Option<String>,
}

impl OverviewBlock {
    /// The cards to show: the template's list, or the usual nine without a block.
    pub fn resolved(block: Option<&OverviewBlock>) -> Vec<String> {
        match block {
            Some(block) => block.cards.clone(),
            None => DEFAULT_CARDS.map(str::to_owned).to_vec(),
        }
    }
}

pub(crate) fn validate(block: &OverviewBlock, issues: &mut Vec<TemplateIssue>) {
    if let Some(headline) = &block.headline {
        if !HEADLINE_CARDS.contains(&headline.as_str()) {
            issues.push(TemplateIssue::new(
                "overview.headline",
                format!(
                    "`{headline}` cannot be a headline; it is one of {}",
                    HEADLINE_CARDS.join(", ")
                ),
            ));
        }
    }
    if block.cards.is_empty() {
        issues.push(TemplateIssue::new(
            "overview.cards",
            "list at least one card, or leave the block out",
        ));
    }
    if block.cards.len() > CARDS_MAX {
        issues.push(TemplateIssue::new(
            "overview.cards",
            format!(
                "there are {} cards; a template lists at most {CARDS_MAX}",
                block.cards.len()
            ),
        ));
    }
    let mut seen = HashSet::new();
    for (index, card) in block.cards.iter().enumerate() {
        let path = format!("overview.cards[{index}]");
        if !PULSE_CARDS.contains(&card.as_str()) {
            issues.push(TemplateIssue::new(path, format!("`{card}` is not a pulse card")));
        } else if !seen.insert(card.as_str()) {
            issues.push(TemplateIssue::new(path, format!("`{card}` is listed twice")));
        }
    }
}
