//! Managing files from the chat (V0.9.11): move a file to a folder, rename it, or send it to the trash. CAIRN reads the
//! request, finds the file and the folder, and proposes the change; it happens only when the person confirms (see
//! `proposals.rs`). Only one file is handled at a time and only when it is clear which one is meant, so a loose request never
//! reaches several files. The trash keeps what it takes, and the workspace's trash can restore it.

use repomemo_domain::{
    AgentProposal, AgentProposalKind, AgentProposalState, AgentReply, AgentRouting, ArtifactSummary, Folder,
};
use serde_json::json;

use super::{clean_subject, reply};

/// What a request to manage a file asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FileCommand {
    Move {
        file: String,
        folder: String,
    },
    Rename {
        file: String,
        title: String,
    },
    Trash {
        file: String,
    },
    /// Change the evidence state of files (V0.9.28): `status` is the id the lifecycle uses.
    SetStatus {
        file: String,
        status: &'static str,
    },
}

/// The evidence state a request names ("verified", "needs review", "outdated", "active"), as the id the lifecycle uses.
pub(crate) fn status_from(words: &str) -> Option<&'static str> {
    let words = words
        .trim()
        .trim_end_matches(|c: char| c == '.' || c == '!' || c.is_whitespace())
        .to_lowercase()
        .replace('_', " ");
    match words.as_str() {
        "verified" | "verified evidence" | "confirmed" | "checked" => Some("verified"),
        "outdated" | "stale" | "obsolete" | "out of date" => Some("outdated"),
        "needs review" | "needing review" | "to review" | "to be reviewed" | "needs a review" => Some("needs_review"),
        "active" | "current" | "up to date" => Some("active"),
        _ => None,
    }
}

/// How a state is said in a sentence.
fn status_label(status: &str) -> &'static str {
    match status {
        "verified" => "verified",
        "outdated" => "outdated",
        "needs_review" => "needing review",
        _ => "active",
    }
}

const TOP_LEVEL: &[&str] = &[
    "top level",
    "the top level",
    "top",
    "the top",
    "root",
    "the root",
    "workspace root",
    "the workspace root",
    "no folder",
    "outside any folder",
];

/// Whether the message talks about a file: it says so, or names one by its extension ("retry-policy.md").
pub(crate) fn mentions_a_file(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    ["file", "document", "note", "folder", "selected"].iter().any(|word| {
        lower
            .split(|c: char| !c.is_alphanumeric())
            .any(|part| part == *word || part == format!("{word}s"))
    }) || text.split_whitespace().any(|word| {
        let word = word.trim_matches(|c: char| !c.is_alphanumeric());
        word.rsplit_once('.').is_some_and(|(stem, ext)| {
            !stem.is_empty()
                && (1..=5).contains(&ext.len())
                && ext.chars().all(|c| c.is_ascii_alphanumeric())
                && ext.chars().any(|c| c.is_ascii_alphabetic())
        })
    })
}

/// The position of the last whole-word `separator` (with spaces around it) in `lower`.
fn last_separator(lower: &str, separator: &str) -> Option<usize> {
    lower.rfind(&format!(" {separator} "))
}

fn unquote(text: &str) -> String {
    text.trim()
        .trim_matches(|c: char| matches!(c, '"' | '\'' | '`' | '“' | '”' | '‘' | '’') || c == '.')
        .trim()
        .to_owned()
}

/// Recognises "move X to Y", "rename X to Y" and "delete X" as a request about a file. A request that does not speak of a file
/// is left to the other jobs, so "remove duplicates from the list" never becomes a deletion.
pub(crate) fn file_command(text: &str, lower: &str) -> Option<FileCommand> {
    let (verb, rest_start) = [
        "rename ", "move ", "delete ", "trash ", "remove ", "discard ", "mark ", "set ",
    ]
    .iter()
    .find(|verb| lower.starts_with(**verb))
    .map(|verb| (verb.trim(), verb.len()))?;
    let rest = &text[rest_start..];
    let lower_rest = &lower[rest_start..];
    match verb {
        "rename" => {
            let at = last_separator(lower_rest, "to")?;
            let file = clean_subject(&rest[..at]);
            let title = unquote(&rest[at + 4..]);
            (!file.is_empty() && !title.is_empty() && mentions_a_file(text))
                .then_some(FileCommand::Rename { file, title })
        }
        // "mark the word documents as outdated", "set the selected files to verified" (V0.9.28).
        "mark" | "set" => {
            let at = last_separator(lower_rest, if verb == "mark" { "as" } else { "to" })?;
            let file = clean_subject(&rest[..at]);
            let status = status_from(&rest[at + 4..])?;
            (!file.is_empty()
                && (mentions_a_file(text)
                    || is_set_reference(&file)
                    || super::filters::parse_filter(&file).is_structured()))
            .then_some(FileCommand::SetStatus { file, status })
        }
        "move" => {
            let at = last_separator(lower_rest, "into")
                .map(|at| (at, 6))
                .or_else(|| last_separator(lower_rest, "to").map(|at| (at, 4)))?;
            let file = clean_subject(&rest[..at.0]);
            let folder = unquote(&rest[at.0 + at.1..]);
            let folder = folder
                .strip_prefix("the ")
                .unwrap_or(&folder)
                .trim_end_matches(" folder")
                .trim_end_matches(" directory")
                .trim()
                .to_owned();
            (!file.is_empty() && !folder.is_empty() && mentions_a_file(text))
                .then_some(FileCommand::Move { file, folder })
        }
        _ => {
            // "remove X from Y" can mean taking something out of a place, which is not a deletion.
            if lower_rest.contains(" from ") {
                return None;
            }
            let file = clean_subject(rest);
            (!file.is_empty() && mentions_a_file(text)).then_some(FileCommand::Trash { file })
        }
    }
}

/// A folder with the names from the top level down to it ("Runbooks / Payments").
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FolderPath {
    pub id: String,
    pub path: String,
}

fn folder_paths(folders: &[Folder]) -> Vec<FolderPath> {
    folders
        .iter()
        .map(|folder| {
            let mut names = vec![folder.name.clone()];
            let mut parent = folder.parent_id.clone();
            // A depth limit keeps a broken parent chain from looping.
            for _ in 0..32 {
                let Some(id) = parent else { break };
                let Some(above) = folders.iter().find(|candidate| candidate.id == id) else {
                    break;
                };
                names.push(above.name.clone());
                parent = above.parent_id.clone();
            }
            names.reverse();
            FolderPath {
                id: folder.id.clone(),
                path: names.join(" / "),
            }
        })
        .collect()
}

/// What a folder name in a request stands for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FolderChoice {
    TopLevel,
    One(FolderPath),
    Many(Vec<String>),
    Missing(Vec<String>),
}

pub(crate) fn choose_folder(wanted: &str, folders: &[Folder]) -> FolderChoice {
    let wanted = wanted.trim().to_lowercase().replace(['>', '\\'], "/");
    let wanted = wanted.split('/').map(str::trim).collect::<Vec<_>>().join(" / ");
    if TOP_LEVEL.contains(&wanted.as_str()) {
        return FolderChoice::TopLevel;
    }
    let paths = folder_paths(folders);
    let lowered = |path: &FolderPath| path.path.to_lowercase();
    let name_of = |path: &FolderPath| path.path.rsplit(" / ").next().unwrap_or("").to_lowercase();
    for matches in [
        paths.iter().filter(|path| lowered(path) == wanted).collect::<Vec<_>>(),
        paths.iter().filter(|path| name_of(path) == wanted).collect::<Vec<_>>(),
        paths
            .iter()
            .filter(|path| lowered(path).contains(&wanted))
            .collect::<Vec<_>>(),
    ] {
        match matches.len() {
            0 => continue,
            1 => return FolderChoice::One(matches[0].clone()),
            _ => return FolderChoice::Many(matches.iter().map(|path| path.path.clone()).collect()),
        }
    }
    FolderChoice::Missing(paths.into_iter().map(|path| path.path).take(10).collect())
}

fn proposal(kind: AgentProposalKind, title: &str, detail: String, payload: serde_json::Value) -> AgentProposal {
    AgentProposal {
        id: "1".to_owned(),
        kind,
        state: AgentProposalState::Pending,
        title: title.to_owned(),
        detail,
        payload,
        outcome: None,
        untrusted_sources: Vec::new(),
    }
}

fn proposed(markdown: String, proposal: AgentProposal, files: Vec<ArtifactSummary>) -> AgentReply {
    let mut answer = reply(markdown);
    answer.routing = AgentRouting::Rules;
    answer.files = files.into_iter().take(PROPOSAL_FILES_SHOWN).collect();
    answer.proposals = vec![proposal];
    answer
}

/// How many files of a set a proposal shows as rows; the card lists the rest by count.
const PROPOSAL_FILES_SHOWN: usize = 10;

/// Whether the words that name the files point at files already in view: the ones the person selected, or those a reply listed.
pub(crate) fn is_set_reference(wanted: &str) -> bool {
    wanted
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .any(|word| matches!(word, "selected" | "these" | "those" | "them" | "above" | "previous"))
}

/// One file by name, or several as a count: "**a.md**" or "**12 files**".
fn named(files: &[ArtifactSummary]) -> String {
    match files {
        [one] => format!("**{}**", one.title),
        many => format!("**{} files**", many.len()),
    }
}

/// The files of a proposal, one line each, up to ten and then the number left.
fn listing(files: &[ArtifactSummary]) -> String {
    let mut lines = files
        .iter()
        .take(PROPOSAL_FILES_SHOWN)
        .map(|file| format!("- **{}** (`{}`)", file.title, file.path))
        .collect::<Vec<_>>();
    if files.len() > PROPOSAL_FILES_SHOWN {
        lines.push(format!("- …and {} more", files.len() - PROPOSAL_FILES_SHOWN));
    }
    lines.join("\n")
}

fn title_of_set(files: &[ArtifactSummary]) -> String {
    match files {
        [one] => one.title.clone(),
        many => format!("{} files", many.len()),
    }
}

/// The proposal to move files to a folder, or to the top level when `folder` is `None`.
pub(crate) fn move_reply(files: Vec<ArtifactSummary>, folder: Option<&FolderPath>) -> AgentReply {
    let target = folder.map_or("the top level".to_owned(), |folder| {
        format!("the folder **{}**", folder.path)
    });
    let what = named(&files);
    let detail = match files.as_slice() {
        [one] => format!(
            "Move **{}** (`{}`) to {target}. The file keeps its identity, so everything that cites it stays attached.",
            one.title, one.path
        ),
        _ => format!(
            "Move {what} to {target}:\n{}\n\nThe files keep their identity, so everything that cites them stays attached.",
            listing(&files)
        ),
    };
    let payload = json!({
        "action": "move",
        "artifact_ids": files.iter().map(|file| file.id.clone()).collect::<Vec<_>>(),
        "count": files.len(),
        "folder_id": folder.map(|folder| folder.id.clone()),
        "folder_name": folder.map_or("the top level".to_owned(), |folder| folder.path.clone()),
        "title": title_of_set(&files),
    });
    let title = if files.len() == 1 {
        "Move the file"
    } else {
        "Move the files"
    };
    let proposal = proposal(AgentProposalKind::MoveFiles, title, detail, payload);
    proposed(
        format!("I can move {what} to {target}. Nothing moves until you confirm below."),
        proposal,
        files,
    )
}

/// The proposal to give one file a new title.
pub(crate) fn rename_reply(file: ArtifactSummary, title: &str) -> AgentReply {
    if file.title == title {
        return reply(format!("**{}** already has that name.", file.title));
    }
    let detail = format!(
        "Rename **{}** (`{}`) to **{title}**. Its path and its history stay as they are.",
        file.title, file.path
    );
    let payload = json!({ "artifact_id": file.id, "title": title, "was": file.title });
    let proposal = proposal(AgentProposalKind::RenameFile, "Rename the file", detail, payload);
    proposed(
        format!(
            "I can rename **{}** to **{title}**. Nothing changes until you confirm below.",
            file.title
        ),
        proposal,
        vec![file],
    )
}

/// The proposal to move files to the trash.
pub(crate) fn trash_reply(files: Vec<ArtifactSummary>) -> AgentReply {
    let what = named(&files);
    let detail = match files.as_slice() {
        [one] => format!(
            "Move **{}** (`{}`) to the trash. It is no longer searched or cited while it is there, and the workspace's trash can restore it.",
            one.title, one.path
        ),
        _ => format!(
            "Move {what} to the trash:\n{}\n\nThey are no longer searched or cited while they are there, and the workspace's trash can restore them.",
            listing(&files)
        ),
    };
    let payload = json!({
        "action": "delete",
        "artifact_ids": files.iter().map(|file| file.id.clone()).collect::<Vec<_>>(),
        "count": files.len(),
        "title": title_of_set(&files),
    });
    let title = if files.len() == 1 {
        "Move the file to the trash"
    } else {
        "Move the files to the trash"
    };
    let proposal = proposal(AgentProposalKind::TrashFiles, title, detail, payload);
    proposed(
        format!("I can move {what} to the trash. Nothing is removed until you confirm below."),
        proposal,
        files,
    )
}

/// The proposal to change the evidence state of files.
pub(crate) fn status_reply(files: Vec<ArtifactSummary>, status: &str) -> AgentReply {
    let what = named(&files);
    let state = status_label(status);
    let detail = match files.as_slice() {
        [one] => format!(
            "Mark **{}** (`{}`) as **{state}**. The state shows in the evidence list, and its review note and owner stay as they are.",
            one.title, one.path
        ),
        _ => format!(
            "Mark {what} as **{state}**:\n{}\n\nThe state shows in the evidence list, and each file's review note and owner stay as they are.",
            listing(&files)
        ),
    };
    let payload = json!({
        "action": "lifecycle",
        "artifact_ids": files.iter().map(|file| file.id.clone()).collect::<Vec<_>>(),
        "status": status,
        "status_label": state,
        "count": files.len(),
        "title": title_of_set(&files),
    });
    let title = if files.len() == 1 {
        "Change the file's state"
    } else {
        "Change the files' state"
    };
    let proposal = proposal(AgentProposalKind::SetStatus, title, detail, payload);
    proposed(
        format!("I can mark {what} as **{state}**. Nothing changes until you confirm below."),
        proposal,
        files,
    )
}

/// What CAIRN says to a request about files it cannot act on: how to word one.
pub(crate) fn files_help() -> AgentReply {
    reply("I can move, rename or trash one file at a time, and only after you confirm. Try `move retry-policy.md to Runbooks`, `rename the deploy runbook to Release runbook` or `delete the file old-notes.md`.")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(message: &str) -> Option<FileCommand> {
        file_command(message, &message.to_ascii_lowercase())
    }

    fn folder(id: &str, name: &str, parent: Option<&str>) -> Folder {
        Folder {
            id: id.to_owned(),
            workspace_id: "w".to_owned(),
            parent_id: parent.map(str::to_owned),
            name: name.to_owned(),
            created_at: String::new(),
            file_count: 0,
        }
    }

    fn file(title: &str) -> ArtifactSummary {
        ArtifactSummary {
            id: format!("id-{title}"),
            workspace_id: "w".to_owned(),
            source_id: "s".to_owned(),
            source_name: "src".to_owned(),
            artifact_type: repomemo_domain::ArtifactType::File,
            title: title.to_owned(),
            path: format!("docs/{title}"),
            content_hash: String::new(),
            mime_type: None,
            language: None,
            size_bytes: 1,
            created_at: String::new(),
            updated_at: String::new(),
            indexed_at: None,
        }
    }

    /// V0.9.12: an uploaded file is recorded as CAIRN having it, with when it can be asked about.
    #[test]
    fn an_uploaded_file_is_recorded_and_says_when_it_can_be_asked_about() {
        let waiting = crate::agent::attachment_reply(file("report.pdf"));
        assert!(waiting
            .reply_markdown
            .starts_with("I added **report.pdf** to the workspace."));
        assert!(
            waiting.reply_markdown.contains("being indexed")
                && waiting.reply_markdown.contains("`summarize report.pdf`")
        );
        assert_eq!(waiting.files.len(), 1);
        assert_eq!(waiting.routing, AgentRouting::Rules);

        let mut done = file("report.pdf");
        done.indexed_at = Some("2026-11-22T10:00:00Z".to_owned());
        let ready = crate::agent::attachment_reply(done);
        assert!(ready.reply_markdown.contains("already indexed") && !ready.reply_markdown.contains("being indexed"));
    }

    #[test]
    fn move_rename_and_trash_requests_are_read() {
        assert_eq!(
            command("move retry-policy.md to Runbooks"),
            Some(FileCommand::Move {
                file: "retry-policy.md".into(),
                folder: "Runbooks".into()
            })
        );
        assert_eq!(
            command("Move the file deploy runbook into the Release folder"),
            Some(FileCommand::Move {
                file: "deploy runbook".into(),
                folder: "Release".into()
            })
        );
        assert_eq!(
            command("rename the deploy runbook file to \"Release runbook\""),
            Some(FileCommand::Rename {
                file: "deploy runbook".into(),
                title: "Release runbook".into()
            })
        );
        assert_eq!(
            command("delete the file old-notes.md"),
            Some(FileCommand::Trash {
                file: "old-notes.md".into()
            })
        );
        assert_eq!(
            command("trash old-notes.md"),
            Some(FileCommand::Trash {
                file: "old-notes.md".into()
            })
        );
    }

    #[test]
    fn a_request_that_is_not_about_a_file_is_left_to_the_other_jobs() {
        for message in [
            "remove duplicates from the list",
            "delete the cache",
            "move on to the next question",
            "rename the variable to count",
            "remove the file old-notes.md from the Runbooks folder",
            "how do I move a file to a folder",
            "find the file about login",
        ] {
            assert_eq!(command(message), None, "{message}");
        }
    }

    #[test]
    fn a_folder_is_found_by_path_name_or_part_of_a_name() {
        let folders = vec![
            folder("1", "Runbooks", None),
            folder("2", "Payments", Some("1")),
            folder("3", "Payments", Some("4")),
            folder("4", "Archive", None),
        ];
        assert_eq!(
            choose_folder("runbooks", &folders),
            FolderChoice::One(FolderPath {
                id: "1".into(),
                path: "Runbooks".into()
            })
        );
        assert_eq!(
            choose_folder("Runbooks / Payments", &folders),
            FolderChoice::One(FolderPath {
                id: "2".into(),
                path: "Runbooks / Payments".into()
            })
        );
        assert_eq!(
            choose_folder("archive/payments", &folders),
            FolderChoice::One(FolderPath {
                id: "3".into(),
                path: "Archive / Payments".into()
            })
        );
        assert_eq!(
            choose_folder("payments", &folders),
            FolderChoice::Many(vec!["Runbooks / Payments".into(), "Archive / Payments".into()])
        );
        assert_eq!(
            choose_folder("run", &folders),
            FolderChoice::Many(vec!["Runbooks".into(), "Runbooks / Payments".into()])
        );
        assert_eq!(
            choose_folder("arch", &folders),
            FolderChoice::Many(vec!["Archive / Payments".into(), "Archive".into()])
        );
        assert_eq!(choose_folder("the top level", &folders), FolderChoice::TopLevel);
        assert_eq!(
            choose_folder("Reports", &folders),
            FolderChoice::Missing(vec![
                "Runbooks".into(),
                "Runbooks / Payments".into(),
                "Archive / Payments".into(),
                "Archive".into()
            ])
        );
    }

    #[test]
    fn a_state_change_is_read_with_its_state_and_needs_a_file_or_a_selection() {
        assert_eq!(
            command("mark the word documents as outdated"),
            Some(FileCommand::SetStatus {
                file: "word".into(),
                status: "outdated"
            })
        );
        assert_eq!(
            command("Mark the selected files as needs review"),
            Some(FileCommand::SetStatus {
                file: "selected".into(),
                status: "needs_review"
            })
        );
        assert_eq!(
            command("mark them as verified"),
            Some(FileCommand::SetStatus {
                file: "them".into(),
                status: "verified"
            })
        );
        assert_eq!(
            command("set the pdfs to stale"),
            Some(FileCommand::SetStatus {
                file: "pdfs".into(),
                status: "outdated"
            })
        );
        for message in [
            "mark my calendar as busy",
            "mark the files as done",
            "set up the files to run",
            "mark it as outdated",
        ] {
            assert_eq!(command(message), None, "{message}");
        }
        assert_eq!(status_from("Out of date."), Some("outdated"));
        assert_eq!(status_from("needs_review"), Some("needs_review"));
        assert_eq!(status_from("fine"), None);
    }

    #[test]
    fn a_state_change_is_one_proposal_over_the_files_with_the_lifecycle_payload() {
        let files = vec![file("a.md"), file("b.md")];
        let answer = status_reply(files, "needs_review");
        let proposal = &answer.proposals[0];
        assert_eq!(proposal.kind, AgentProposalKind::SetStatus);
        assert_eq!(proposal.payload["action"], "lifecycle");
        assert_eq!(proposal.payload["status"], "needs_review");
        assert_eq!(proposal.payload["count"], 2);
        assert!(
            proposal.detail.starts_with("Mark **2 files** as **needing review**:"),
            "{}",
            proposal.detail
        );
        assert!(answer.reply_markdown.contains("Nothing changes until you confirm"));
    }

    #[test]
    fn files_in_view_are_pointed_at_by_selected_these_or_them() {
        for wanted in ["selected", "these", "all of them", "those files", "the files above"] {
            assert!(is_set_reference(wanted), "{wanted}");
        }
        for wanted in ["retry-policy.md", "deploy runbook", "word", "the theme file"] {
            assert!(!is_set_reference(wanted), "{wanted}");
        }
    }

    #[test]
    fn many_files_make_one_proposal_that_lists_them_and_counts_the_rest() {
        let files = (0..14).map(|n| file(&format!("f{n}.md"))).collect::<Vec<_>>();
        let moved = move_reply(
            files.clone(),
            Some(&FolderPath {
                id: "9".into(),
                path: "Runbooks".into(),
            }),
        );
        let proposal = &moved.proposals[0];
        assert_eq!(proposal.payload["artifact_ids"].as_array().unwrap().len(), 14);
        assert_eq!(proposal.payload["count"], 14);
        assert_eq!(proposal.payload["title"], "14 files");
        assert!(
            proposal
                .detail
                .starts_with("Move **14 files** to the folder **Runbooks**:"),
            "{}",
            proposal.detail
        );
        assert!(proposal.detail.contains("- …and 4 more"));
        assert!(moved.reply_markdown.contains("I can move **14 files**"));
        assert_eq!(moved.files.len(), PROPOSAL_FILES_SHOWN);

        let trashed = trash_reply(files);
        assert_eq!(trashed.proposals[0].payload["action"], "delete");
        assert!(trashed.proposals[0].detail.contains("trash can restore them"));
        assert_eq!(trashed.proposals[0].title, "Move the files to the trash");
    }

    #[test]
    fn each_request_becomes_one_proposal_for_one_file() {
        let moved = move_reply(
            vec![file("a.md")],
            Some(&FolderPath {
                id: "9".into(),
                path: "Runbooks".into(),
            }),
        );
        assert_eq!(moved.proposals[0].kind, AgentProposalKind::MoveFiles);
        assert_eq!(moved.proposals[0].payload["artifact_ids"], json!(["id-a.md"]));
        assert_eq!(moved.proposals[0].payload["folder_id"], "9");
        assert_eq!(moved.proposals[0].payload["action"], "move");
        assert!(moved.reply_markdown.contains("Nothing moves until you confirm"));

        let top = move_reply(vec![file("a.md")], None);
        assert!(top.proposals[0].payload["folder_id"].is_null());

        let renamed = rename_reply(file("a.md"), "b.md");
        assert_eq!(renamed.proposals[0].kind, AgentProposalKind::RenameFile);
        assert_eq!(renamed.proposals[0].payload["title"], "b.md");
        assert!(rename_reply(file("a.md"), "a.md").proposals.is_empty());

        let trashed = trash_reply(vec![file("a.md")]);
        assert_eq!(trashed.proposals[0].kind, AgentProposalKind::TrashFiles);
        assert_eq!(trashed.proposals[0].payload["action"], "delete");
        assert!(trashed.proposals[0].detail.contains("trash can restore it"));
        assert_eq!(trashed.files.len(), 1);
    }
}
