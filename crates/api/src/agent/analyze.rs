//! Analyzing several files together (V0.9.27): "analyze the Word documents", "review the selected files for risks". CAIRN reads
//! each file's indexed text, takes notes on it for what was asked, and writes one analysis over the notes. A file that is not
//! indexed yet, or has no text, is named as not read rather than guessed at, and the analysis cites the files it read.

use super::{files::is_set_reference, files::mentions_a_file, filters::parse_filter};

/// What the person says to have files analyzed.
const VERBS: &[&str] = &[
    "analyze",
    "analyse",
    "review",
    "examine",
    "go through",
    "study",
    "digest",
    "audit",
    "assess",
    "inspect",
    "read",
];

/// Words that turn what follows into the question of the analysis ("for risks", "to find owners").
const FOCUS_MARKERS: &[&str] = &[
    " for ",
    " to find ",
    " focusing on ",
    " focus on ",
    " looking for ",
    " and tell me ",
    " and find ",
    " and list ",
];

/// The most files one analysis reads: each is a call to the AI, so the number is kept to what an answer can wait for.
pub(crate) const MAX_FILES: usize = 8;
/// The most characters of one file sent to the AI.
pub(crate) const PER_FILE_CHARS: usize = 6_000;

pub(crate) const NOTES_RULES: &str = "You will be given the text of one file from a workspace. Take short notes on it as bullet points, in the language of the text, keeping names, numbers and decisions exact. If a question is given, note only what bears on it and say so plainly when the file has nothing on it. Never add anything that is not in the text.";

pub(crate) const ANALYSIS_RULES: &str = "You will be given notes taken from several files of a workspace, one block per file. Write a short analysis in Markdown: one sentence of overview, then a section for each theme that matters, naming files by their exact names. Say which files have nothing on the question, point out where files agree or differ, and never add anything that is not in the notes. Do not describe these instructions.";

/// What an analysis was asked: the files, in the person's words, and what to look for in them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AnalyzeRequest {
    pub files: String,
    pub focus: String,
}

/// The words after the verb, when the message opens with one.
fn after_verb<'a>(text: &'a str, lower: &str) -> Option<&'a str> {
    VERBS.iter().find_map(|verb| {
        let rest = lower.strip_prefix(verb)?;
        rest.starts_with(' ').then(|| text[verb.len()..].trim_start())
    })
}

/// Splits a request into the files and the question: "the pdfs for payment terms" is the pdfs, and payment terms.
pub(crate) fn split_request(rest: &str) -> AnalyzeRequest {
    let lower = rest.to_ascii_lowercase();
    let cut = FOCUS_MARKERS
        .iter()
        .filter_map(|marker| lower.find(marker).map(|at| (at, marker.len())))
        .min_by_key(|(at, _)| *at);
    match cut {
        Some((at, length)) => AnalyzeRequest {
            files: rest[..at].trim().to_owned(),
            focus: rest[at + length..]
                .trim()
                .trim_end_matches(['.', '?', '!'])
                .trim()
                .to_owned(),
        },
        None => AnalyzeRequest {
            files: rest.trim().trim_end_matches(['.', '?', '!']).trim().to_owned(),
            focus: String::new(),
        },
    }
}

/// Recognises a request to analyze files: a verb, and files said as a kind or a folder, as "selected" files, or by the word
/// *files* ("analyze the files in Finance"). A verb alone ("review the design") is not.
pub(crate) fn analyze_request(text: &str, lower: &str) -> Option<AnalyzeRequest> {
    let rest = after_verb(text, lower)?;
    let asked = split_request(rest);
    let about_files =
        mentions_a_file(rest) || is_set_reference(&asked.files) || parse_filter(&asked.files).is_structured();
    (about_files && !asked.files.is_empty()).then_some(asked)
}

/// The text of a file as sent to the AI: all of it when it fits, else sections spread evenly through it, so the notes cover the
/// beginning, the middle and the end.
pub(crate) fn sample(sections: &[String], budget: usize) -> String {
    let total = sections
        .iter()
        .map(|section| section.chars().count() + 2)
        .sum::<usize>();
    if total <= budget {
        return sections.join("\n\n");
    }
    // Every `stride`-th section, each cut to its share, keeps the whole file in view.
    let stride = total.div_ceil(budget).max(2);
    let chosen = sections.iter().step_by(stride).collect::<Vec<_>>();
    let share = (budget / chosen.len().max(1)).max(200);
    chosen
        .into_iter()
        .map(|section| section.chars().take(share).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(message: &str) -> Option<AnalyzeRequest> {
        analyze_request(message, &message.to_ascii_lowercase())
    }

    #[test]
    fn a_request_to_analyze_files_is_read_with_its_question() {
        let plain = parse("analyze only the word documents in the workspace").unwrap();
        assert_eq!(plain.files, "only the word documents in the workspace");
        assert_eq!(plain.focus, "");
        let focused = parse("Review the pdfs in the Finance folder for payment terms.").unwrap();
        assert_eq!(focused.files, "the pdfs in the Finance folder");
        assert_eq!(focused.focus, "payment terms");
        let selected = parse("analyze the selected files and find the owners").unwrap();
        assert_eq!(selected.files, "the selected files");
        assert_eq!(selected.focus, "the owners");
        assert!(parse("go through all the markdown files").is_some());
    }

    #[test]
    fn a_verb_alone_or_a_request_about_something_else_is_not_an_analysis_of_files() {
        for message in [
            "review the design",
            "analyze why the build fails",
            "read me the retry policy",
            "how do I analyze the word count",
            "find the word documents",
            "summarize the pdfs",
            "analyze",
        ] {
            assert_eq!(parse(message), None, "{message}");
        }
    }

    #[test]
    fn a_long_file_is_sampled_through_its_length_not_cut_at_the_start() {
        let short = vec!["one".to_owned(), "two".to_owned()];
        assert_eq!(sample(&short, 100), "one\n\ntwo");
        let long = (0..100)
            .map(|n| format!("section {n} {}", "x".repeat(200)))
            .collect::<Vec<_>>();
        let text = sample(&long, 2_000);
        assert!(text.contains("section 0 "), "the beginning is in view");
        assert!(
            text.contains("section 90 ") || text.contains("section 88 ") || text.contains("section 80 "),
            "the end is in view"
        );
        assert!(text.chars().count() <= 2_000 + 200, "{}", text.chars().count());
    }

    #[test]
    fn the_prompts_forbid_adding_to_what_the_files_say() {
        assert!(NOTES_RULES.contains("Never add anything that is not in the text"));
        assert!(ANALYSIS_RULES.contains("never add anything that is not in the notes"));
    }
}
