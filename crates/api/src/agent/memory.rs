//! Recalling what the workspace has kept as memory (V0.9.29): "what do we know about retries", "search memory for pager",
//! "list the memory cards". CAIRN searches the memory cards and shows them in the chat, with what each cites and whether it
//! needs review, so the team's own decisions come before a model's reading of the files.

use super::{clean_subject, find_phrase};

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

#[cfg(test)]
mod tests {
    use super::*;

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
