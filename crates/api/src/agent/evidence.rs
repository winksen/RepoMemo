//! The evidence CAIRN shows under an answer (V0.9.4): the passages the answer relied on, not every passage it was given. The
//! model cites the numbered passages it uses as [1], [2]; those are the ones kept. An answer that says the context is
//! insufficient relied on nothing and shows nothing.

use repomemo_domain::Citation;

/// The numbers of the passages an answer cites, in the order they first appear, each once and each within `1..=total`. It reads
/// `[2]`, `[1][3]` and `[1, 4]` alike.
pub(crate) fn cited_numbers(answer: &str, total: usize) -> Vec<usize> {
    let mut found = Vec::new();
    let mut rest = answer;
    while let Some(open) = rest.find('[') {
        let after = &rest[open + 1..];
        let Some(close) = after.find(']') else { break };
        let inside = &after[..close];
        if !inside.is_empty()
            && inside
                .chars()
                .all(|c| c.is_ascii_digit() || c == ',' || c.is_whitespace())
        {
            for number in inside.split(',').filter_map(|part| part.trim().parse::<usize>().ok()) {
                if (1..=total).contains(&number) && !found.contains(&number) {
                    found.push(number);
                }
            }
        }
        rest = &after[close + 1..];
    }
    found
}

/// Whether the answer says the indexed context could not answer the question.
pub(crate) fn says_insufficient(answer: &str) -> bool {
    answer.to_lowercase().contains("insufficient")
}

/// The citations to show for an answer, given the passages in the order they were numbered: the ones it cites; none when it says
/// the context is insufficient; and all of them when it cites none of its own accord, since then nothing says which it used.
pub(crate) fn evidence_for(answer: &str, passages: Vec<Citation>) -> Vec<Citation> {
    let cited = cited_numbers(answer, passages.len());
    if !cited.is_empty() {
        return cited.into_iter().map(|number| passages[number - 1].clone()).collect();
    }
    if says_insufficient(answer) {
        return Vec::new();
    }
    passages
}

#[cfg(test)]
mod tests {
    use super::*;

    fn passage(title: &str) -> Citation {
        Citation {
            artifact_id: format!("id-{title}"),
            chunk_id: Some(format!("chunk-{title}")),
            title: title.to_owned(),
            path: format!("src/{title}"),
            start_line: Some(1),
            end_line: Some(9),
            confidence: None,
            commit: None,
            last_author: None,
            last_changed_at: None,
        }
    }

    fn titles(citations: &[Citation]) -> Vec<&str> {
        citations.iter().map(|citation| citation.title.as_str()).collect()
    }

    #[test]
    fn markers_are_read_in_every_common_shape_and_only_once() {
        assert_eq!(
            cited_numbers("Retries back off [2]. Deploys wait [1][3].", 4),
            [2, 1, 3]
        );
        assert_eq!(cited_numbers("See [1, 4] and again [1].", 4), [1, 4]);
        assert_eq!(
            cited_numbers("A list [x] and [] and [9] and [0]", 4),
            Vec::<usize>::new()
        );
        assert_eq!(cited_numbers("an unclosed [2", 4), Vec::<usize>::new());
        assert_eq!(cited_numbers("[link](http://x) and arr[1]", 2), [1]);
    }

    #[test]
    fn only_the_cited_passages_are_shown_in_the_order_they_are_cited() {
        let passages = vec![passage("a.rs"), passage("b.rs"), passage("c.rs")];
        let shown = evidence_for("It retries [3] and then stops [1].", passages);
        assert_eq!(titles(&shown), ["c.rs", "a.rs"]);
    }

    #[test]
    fn an_insufficient_answer_shows_no_evidence() {
        let passages = vec![passage("a.rs"), passage("b.rs")];
        let shown = evidence_for("The indexed context is insufficient to answer this question.", passages);
        assert!(shown.is_empty());
    }

    #[test]
    fn an_answer_that_cites_nothing_but_answers_keeps_what_it_was_given() {
        let passages = vec![passage("a.rs"), passage("b.rs")];
        let shown = evidence_for("Deploys run on Fridays.", passages);
        assert_eq!(titles(&shown), ["a.rs", "b.rs"]);
    }

    #[test]
    fn a_citation_wins_over_a_mention_of_insufficient_context() {
        let passages = vec![passage("a.rs"), passage("b.rs")];
        let shown = evidence_for("Part of this is covered [2]; the rest is insufficient.", passages);
        assert_eq!(titles(&shown), ["b.rs"]);
    }
}
