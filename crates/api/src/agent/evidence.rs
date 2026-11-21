//! The evidence CAIRN shows under an answer (V0.9.4): the passages the answer relied on, not every passage it was given. The
//! model cites the numbered passages it uses as [1], [2]; those are the ones kept. An answer that says the context is
//! insufficient relied on nothing and shows nothing.

use repomemo_domain::Citation;

use super::{words_of, FILLER_WORDS, QUESTION_WORDS};
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

/// Whether the text between two brackets is a citation marker: only digits, commas and spaces.
fn is_marker(inside: &str) -> bool {
    !inside.is_empty()
        && inside
            .chars()
            .all(|c| c.is_ascii_digit() || c == ',' || c.is_whitespace())
}

/// The answer with its markers renumbered to the evidence shown (V0.9.8): the passage cited first becomes [1], the next [2], and
/// so on, so a marker in the text and the number beside the evidence name the same passage. A number that names no cited
/// passage is left as it was.
pub(crate) fn renumber_markers(answer: &str, cited: &[usize]) -> String {
    let renumber = |inside: &str| {
        let mut out = String::with_capacity(inside.len());
        let mut digits = String::new();
        let flush = |digits: &mut String, out: &mut String| {
            if digits.is_empty() {
                return;
            }
            let shown = digits
                .parse::<usize>()
                .ok()
                .and_then(|number| cited.iter().position(|&original| original == number))
                .map_or_else(|| digits.clone(), |position| (position + 1).to_string());
            out.push_str(&shown);
            digits.clear();
        };
        for c in inside.chars() {
            if c.is_ascii_digit() {
                digits.push(c);
            } else {
                flush(&mut digits, &mut out);
                out.push(c);
            }
        }
        flush(&mut digits, &mut out);
        out
    };
    let mut out = String::with_capacity(answer.len());
    let mut rest = answer;
    while let Some(open) = rest.find('[') {
        out.push_str(&rest[..=open]);
        let after = &rest[open + 1..];
        let Some(close) = after.find(']') else {
            rest = after;
            break;
        };
        let inside = &after[..close];
        if is_marker(inside) {
            out.push_str(&renumber(inside));
        } else {
            out.push_str(inside);
        }
        out.push(']');
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

/// The answer and the citations to show for it, given the passages in the order they were numbered: the ones it cites, with its
/// markers renumbered to match; none when it says the context is insufficient; and all of them when it cites none of its own
/// accord, since then nothing says which it used.
pub(crate) fn evidence_for(answer: &str, passages: Vec<Citation>) -> (String, Vec<Citation>) {
    let cited = cited_numbers(answer, passages.len());
    if !cited.is_empty() {
        let shown = cited.iter().map(|&number| passages[number - 1].clone()).collect();
        return (renumber_markers(answer, &cited), shown);
    }
    if says_insufficient(answer) {
        return (answer.to_owned(), Vec::new());
    }
    (answer.to_owned(), passages)
}

/// Words too common to look a file up by.
const COMMON_WORDS: &[&str] = &[
    "is", "are", "was", "were", "be", "been", "do", "does", "did", "we", "you", "i", "it", "its", "in", "at", "by",
    "as", "and", "or", "this", "there", "here", "use", "used", "uses", "work", "works", "happen", "happens", "get",
    "gets",
];

/// The few words of a question worth searching for: not question words, fillers or common words.
pub(crate) fn search_words(question: &str) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    for word in words_of(question) {
        let plain = word.as_str();
        if word.chars().count() < 3
            || QUESTION_WORDS.contains(&plain)
            || FILLER_WORDS.contains(&plain)
            || COMMON_WORDS.contains(&plain)
            || words.contains(&word)
        {
            continue;
        }
        words.push(word);
        if words.len() == 3 {
            break;
        }
    }
    words
}

/// What CAIRN offers after an answer that found nothing (V0.9.5): the lookups that need no AI and no sentence-level match,
/// with the question's own words filled in.
pub(crate) fn next_steps(question: &str) -> String {
    let words = search_words(question);
    let try_this = if words.is_empty() {
        "Try **Search content** with a few exact words, or **Find files** if you know the file's name.".to_owned()
    } else {
        let joined = words.join(" ");
        format!(
            "Try `search for {joined}` to see where those words appear, or `find the file about {}` if you know what the file is called.",
            words[0]
        )
    };
    format!("\n\n{try_this} If the files were only just added, they may still be indexing.")
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
    fn the_next_steps_use_the_questions_own_words() {
        assert_eq!(search_words("How does the retry backoff work?"), ["retry", "backoff"]);
        assert_eq!(
            search_words("What is the deployment rollback timeout policy"),
            ["deployment", "rollback", "timeout"]
        );
        assert!(next_steps("How does the retry backoff work?").contains("`search for retry backoff`"));
        assert!(next_steps("how is it?").contains("**Search content**"));
        assert!(next_steps("retry").contains("may still be indexing"));
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
    fn renumbering_rewrites_every_marker_shape_and_leaves_other_brackets_alone() {
        assert_eq!(
            renumber_markers("a [3][1] b [1, 3] c [9] d [x] e [", &[3, 1]),
            "a [1][2] b [2, 1] c [9] d [x] e ["
        );
        assert_eq!(renumber_markers("see [link](u) and [2]", &[2]), "see [link](u) and [1]");
        assert_eq!(renumber_markers("no markers", &[1]), "no markers");
        assert_eq!(renumber_markers("[12] and [2]", &[2, 12]), "[2] and [1]");
    }

    #[test]
    fn only_the_cited_passages_are_shown_in_the_order_they_are_cited() {
        let passages = vec![passage("a.rs"), passage("b.rs"), passage("c.rs")];
        let (text, shown) = evidence_for("It retries [3] and then stops [1].", passages);
        assert_eq!(titles(&shown), ["c.rs", "a.rs"]);
        assert_eq!(
            text, "It retries [1] and then stops [2].",
            "the markers follow the evidence"
        );
    }

    #[test]
    fn an_insufficient_answer_shows_no_evidence() {
        let passages = vec![passage("a.rs"), passage("b.rs")];
        let (text, shown) = evidence_for("The indexed context is insufficient to answer this question.", passages);
        assert!(text.contains("insufficient"));
        assert!(shown.is_empty());
    }

    #[test]
    fn an_answer_that_cites_nothing_but_answers_keeps_what_it_was_given() {
        let passages = vec![passage("a.rs"), passage("b.rs")];
        let (text, shown) = evidence_for("Deploys run on Fridays.", passages);
        assert_eq!(text, "Deploys run on Fridays.");
        assert_eq!(titles(&shown), ["a.rs", "b.rs"]);
    }

    #[test]
    fn a_citation_wins_over_a_mention_of_insufficient_context() {
        let passages = vec![passage("a.rs"), passage("b.rs")];
        let (text, shown) = evidence_for("Part of this is covered [2]; the rest is insufficient.", passages);
        assert_eq!(titles(&shown), ["b.rs"]);
        assert_eq!(text, "Part of this is covered [1]; the rest is insufficient.");
    }
}
