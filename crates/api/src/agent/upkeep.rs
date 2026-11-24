//! Memory upkeep (V0.9.56): a weekly pass over a workspace's memory, on when an administrator turned it on. It looks for three things
//! and edits nothing: pairs of current memory cards on the same topic that give different figures, questions several people asked
//! that were answered with "I found nothing", and verified playbooks no run has read for ninety days. Each finding goes to the review
//! queue with the evidence it rests on. Cards whose evidence changed are already in the queue and are not repeated. No model is
//! asked, so the pass spends no AI allowance; it is bounded by the number of findings instead.

use std::collections::BTreeSet;

use repomemo_domain::MemoryCardSummary;
use repomemo_storage::{AskedQuestion, NewUpkeepFinding, UPKEEP_PER_PASS_MAX};

use super::evidence::says_insufficient;
use super::shorten;
use super::team_memory::significant_words;
use crate::RepoMemoCore;

/// How far back the pass reads questions, and how long a playbook may go unread.
const QUESTIONS_DAYS: i64 = 30;
const PLAYBOOK_UNUSED_DAYS: i64 = 90;
/// A question has to be asked this many times by at least two people before it is a finding.
const ASKED_TIMES: usize = 3;
const ASKED_PEOPLE: usize = 2;
/// The most pairs of cards compared in one pass.
const PAIRS_MAX: usize = 10;

/// The figures in a text: its numbers, as written.
fn figures(text: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut current = String::new();
    for character in text.chars().chain(std::iter::once(' ')) {
        if character.is_ascii_digit() || (character == '.' && !current.is_empty()) {
            current.push(character);
        } else if !current.is_empty() {
            let number = current.trim_end_matches('.').to_owned();
            if !number.is_empty() {
                found.insert(number);
            }
            current.clear();
        }
    }
    found
}

/// Pairs of current cards that share at least two significant words of their titles and each give figures that the other does not.
pub(in crate::agent) fn possible_contradictions(
    cards: &[MemoryCardSummary],
) -> Vec<(&MemoryCardSummary, &MemoryCardSummary)> {
    let current = cards.iter().filter(|card| !card.needs_review).collect::<Vec<_>>();
    let mut pairs = Vec::new();
    for (index, first) in current.iter().enumerate() {
        let first_words = significant_words(&first.title);
        let first_figures = figures(&format!("{} {}", first.title, first.body_excerpt));
        for second in &current[index + 1..] {
            let shared = significant_words(&second.title)
                .iter()
                .filter(|word| first_words.contains(word))
                .count();
            if shared < 2 {
                continue;
            }
            let second_figures = figures(&format!("{} {}", second.title, second.body_excerpt));
            if first_figures.is_empty() || second_figures.is_empty() || first_figures == second_figures {
                continue;
            }
            if first_figures.is_subset(&second_figures) || second_figures.is_subset(&first_figures) {
                continue;
            }
            pairs.push((*first, *second));
            if pairs.len() >= PAIRS_MAX {
                return pairs;
            }
        }
    }
    pairs
}

/// Whether an answer says nothing was found, in the words the router and the loop use for it.
fn found_nothing(answer: &str) -> bool {
    let lower = answer.to_lowercase();
    says_insufficient(answer)
        || [
            "could not find",
            "couldn't find",
            "found nothing",
            "no indexed passage",
            "did not find",
        ]
        .iter()
        .any(|phrase| lower.contains(phrase))
}

/// A question as people word it differently: lower case, one line, without a closing question mark.
fn normal_question(label: &str) -> String {
    label
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_end_matches(['?', '!', '.', ' '])
        .to_lowercase()
}

/// The questions asked at least three times by at least two people whose answer was that nothing was found, with how many times.
pub(in crate::agent) fn unanswered_repeats(questions: &[AskedQuestion]) -> Vec<(String, usize)> {
    let mut groups: Vec<(String, usize, BTreeSet<&str>)> = Vec::new();
    for question in questions.iter().filter(|question| found_nothing(&question.answer)) {
        let key = normal_question(&question.label);
        if key.is_empty() {
            continue;
        }
        match groups.iter_mut().find(|group| group.0 == key) {
            Some(group) => {
                group.1 += 1;
                group.2.insert(question.user_id.as_str());
            }
            None => groups.push((key, 1, BTreeSet::from([question.user_id.as_str()]))),
        }
    }
    groups
        .into_iter()
        .filter(|group| group.1 >= ASKED_TIMES && group.2.len() >= ASKED_PEOPLE)
        .map(|group| (group.0, group.1))
        .collect()
}

impl RepoMemoCore {
    /// Runs the pass for one workspace and returns how many findings it added.
    pub async fn run_upkeep(&self, workspace_id: &str) -> anyhow::Result<usize> {
        let mut added = 0;
        let cards = self.storage.list_memory_cards(workspace_id).await?;
        for (first, second) in possible_contradictions(&cards) {
            if added >= UPKEEP_PER_PASS_MAX {
                break;
            }
            let mut ids = [first.id.as_str(), second.id.as_str()];
            ids.sort_unstable();
            let finding = NewUpkeepFinding {
                kind: "possible_contradiction",
                fingerprint: format!("contradiction:{}:{}", ids[0], ids[1]),
                title: format!("Two cards may disagree: {} and {}", shorten(&first.title, 50), shorten(&second.title, 50)),
                detail: "These current memory cards are about the same topic and give different figures. One may be out of date; this is a hint, not a finding that either is wrong.".to_owned(),
                evidence: format!(
                    "{}: {}\n{}: {}",
                    first.title,
                    shorten(&first.body_excerpt, 200),
                    second.title,
                    shorten(&second.body_excerpt, 200)
                ),
                artifact_id: None,
            };
            if self.storage.add_upkeep_finding(workspace_id, &finding).await? {
                added += 1;
            }
        }
        let since = (chrono::Utc::now() - chrono::Duration::days(QUESTIONS_DAYS)).to_rfc3339();
        let asked = self.storage.asked_questions_since(workspace_id, &since, 2_000).await?;
        for (question, times) in unanswered_repeats(&asked) {
            if added >= UPKEEP_PER_PASS_MAX {
                break;
            }
            let finding = NewUpkeepFinding {
                kind: "unanswered_question",
                fingerprint: format!("unanswered:{question}"),
                title: format!("Asked {times} times and never answered: {}", shorten(&question, 80)),
                detail: format!(
                    "At least two people asked this in the last {QUESTIONS_DAYS} days and CAIRN found nothing each time. The workspace may be missing a file or a memory card that answers it."
                ),
                evidence: shorten(&question, 200),
                artifact_id: None,
            };
            if self.storage.add_upkeep_finding(workspace_id, &finding).await? {
                added += 1;
            }
        }
        let unused_since = (chrono::Utc::now() - chrono::Duration::days(PLAYBOOK_UNUSED_DAYS)).to_rfc3339();
        for playbook in self.workspace_playbooks(workspace_id).await {
            if added >= UPKEEP_PER_PASS_MAX {
                break;
            }
            if playbook.updated_at.as_str() >= unused_since.as_str()
                || self
                    .storage
                    .playbook_read_since(workspace_id, &playbook.name, &unused_since)
                    .await?
            {
                continue;
            }
            let finding = NewUpkeepFinding {
                kind: "unused_playbook",
                fingerprint: format!("unused-playbook:{}", playbook.name.to_lowercase()),
                title: format!("Playbook not used in {PLAYBOOK_UNUSED_DAYS} days: {}", playbook.name),
                detail: "No run has read this verified playbook for ninety days. It may no longer fit how the team works, or CAIRN may not know when to use it.".to_owned(),
                evidence: format!("{}: {} (use it when: {})", playbook.name, playbook.description, playbook.when),
                artifact_id: Some(playbook.artifact_id.clone()),
            };
            if self.storage.add_upkeep_finding(workspace_id, &finding).await? {
                added += 1;
            }
        }
        self.storage.mark_upkeep_run(workspace_id).await?;
        Ok(added)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(id: &str, title: &str, body: &str, needs_review: bool) -> MemoryCardSummary {
        MemoryCardSummary {
            id: id.to_owned(),
            workspace_id: "w1".to_owned(),
            title: title.to_owned(),
            body_excerpt: body.to_owned(),
            source: "chat".to_owned(),
            evidence_count: 1,
            created_at: String::new(),
            updated_at: String::new(),
            needs_review,
        }
    }

    #[test]
    fn two_current_cards_on_a_topic_with_different_figures_may_disagree() {
        let cards = [
            card("a", "Upload size limit", "The upload limit is 20 MB.", false),
            card("b", "Upload size limit policy", "Uploads are capped at 50 MB.", false),
            card("c", "Upload size limit old", "The limit was 10 MB.", true),
            card("d", "Retry policy", "Five attempts.", false),
            card(
                "e",
                "Upload size limit note",
                "The upload limit is 20 MB and uploads work.",
                false,
            ),
        ];
        let pairs = possible_contradictions(&cards);
        let ids = pairs
            .iter()
            .map(|(a, b)| (a.id.as_str(), b.id.as_str()))
            .collect::<Vec<_>>();
        assert!(ids.contains(&("a", "b")));
        assert!(
            !ids.iter().any(|(a, b)| *a == "c" || *b == "c"),
            "a card needing review is already in the queue"
        );
        assert!(!ids.iter().any(|(a, b)| *a == "d" || *b == "d"));
        assert!(!ids.contains(&("a", "e")), "the same figure is no disagreement");
    }

    #[test]
    fn a_question_is_a_finding_when_several_people_got_no_answer_three_times() {
        let asked = |label: &str, user: &str, answer: &str| AskedQuestion {
            label: label.to_owned(),
            answer: answer.to_owned(),
            user_id: user.to_owned(),
        };
        let nothing = "I could not find anything about that in the files.";
        let questions = [
            asked("Where is the SLA?", "u1", nothing),
            asked("where is the sla", "u2", nothing),
            asked("Where is the SLA", "u1", nothing),
            asked("Who owns billing?", "u1", nothing),
            asked("Who owns billing?", "u1", nothing),
            asked("Who owns billing?", "u1", nothing),
            asked("What is retry?", "u1", "Retries wait twice as long."),
            asked("What is retry?", "u2", "Retries wait twice as long."),
            asked("What is retry?", "u3", "Retries wait twice as long."),
        ];
        assert_eq!(unanswered_repeats(&questions), vec![("where is the sla".to_owned(), 3)]);
    }

    #[test]
    fn figures_are_the_numbers_as_written() {
        assert_eq!(
            figures("Limit 20 MB, 1.5 retries and v2.")
                .into_iter()
                .collect::<Vec<_>>(),
            vec!["1.5".to_owned(), "2".to_owned(), "20".to_owned()]
        );
    }
}
