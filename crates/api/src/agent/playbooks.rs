//! Playbooks (V0.9.54): procedures a team keeps as notes in a `Playbooks` folder at the root of the workspace. Each note starts with
//! a header (`name`, `description`, `when`, and optionally `tools`) between two `---` lines. Only a playbook that was verified is
//! offered: its name and description go in the prompt and `load_playbook` returns its steps. A playbook is instructions to follow
//! with the tools the person's role already gives CAIRN; it never grants a tool, and it is not evidence.

use repomemo_domain::{AgentStep, ArtifactSummary, ArtifactType};

use super::shorten;
use crate::RepoMemoCore;

/// The folder the playbooks live in, at the root of the workspace.
pub const PLAYBOOK_FOLDER: &str = "Playbooks";
/// The most playbooks offered, and the most characters of one that CAIRN reads.
pub const PLAYBOOKS_MAX: usize = 20;
const PLAYBOOK_CHARS: usize = 4_000;
const NAME_MAX: usize = 60;
const DESCRIPTION_MAX: usize = 200;
const WHEN_MAX: usize = 300;

/// A playbook's header and steps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::agent) struct Playbook {
    pub name: String,
    pub description: String,
    pub when: String,
    /// The tools it expects, when it names any.
    pub tools: Vec<String>,
    pub body: String,
    /// The note it was read from and when that note last changed; empty for a playbook read from text alone.
    pub artifact_id: String,
    pub updated_at: String,
}

/// A playbook read from a note's text, or why the note is not one. An unknown key is a mistake, not something to ignore.
pub(in crate::agent) fn parse_playbook(raw: &str) -> Result<Playbook, String> {
    let text = raw.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    let Some(rest) = text.strip_prefix("---\n") else {
        return Err("it does not start with a header between two --- lines".to_owned());
    };
    let Some((header, body)) = rest.split_once("\n---") else {
        return Err("the header is not closed by a --- line".to_owned());
    };
    let body = body.trim_start_matches('-').trim().to_owned();
    let (mut name, mut description, mut when, mut tools) = (None, None, None, Vec::new());
    for line in header.lines().filter(|line| !line.trim().is_empty()) {
        let Some((key, value)) = line.split_once(':') else {
            return Err(format!("the header line \"{}\" is not `key: value`", shorten(line, 40)));
        };
        let value = value.trim().trim_matches(['"', '\'']).trim().to_owned();
        match key.trim() {
            "name" => name = Some(value),
            "description" => description = Some(value),
            "when" => when = Some(value),
            "tools" => {
                tools = value
                    .trim_matches(['[', ']'])
                    .split(',')
                    .map(|tool| tool.trim().trim_matches(['"', '\'']).to_owned())
                    .filter(|tool| !tool.is_empty())
                    .collect();
            }
            other => return Err(format!("the header has an unknown key `{other}`")),
        }
    }
    let name = name.filter(|name| !name.is_empty()).ok_or("the header has no `name`")?;
    let description = description
        .filter(|text| !text.is_empty())
        .ok_or("the header has no `description`")?;
    let when = when.filter(|text| !text.is_empty()).ok_or("the header has no `when`")?;
    if name.chars().count() > NAME_MAX
        || !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == ' ' || c == '-' || c == '_')
    {
        return Err(format!(
            "the name must be at most {NAME_MAX} letters, digits, spaces, - or _"
        ));
    }
    if description.chars().count() > DESCRIPTION_MAX || when.chars().count() > WHEN_MAX {
        return Err("the description or the `when` line is too long".to_owned());
    }
    if body.is_empty() {
        return Err("it has no steps after the header".to_owned());
    }
    Ok(Playbook {
        name,
        description,
        when,
        tools,
        body,
        artifact_id: String::new(),
        updated_at: String::new(),
    })
}

/// The playbooks among a folder's notes that count: verified and not superseded, a text kind of file, newest first, with the first of
/// two of the same name kept.
pub(in crate::agent) fn offered<'a>(
    files: &'a [ArtifactSummary],
    in_playbooks: &dyn Fn(&str) -> bool,
    verified: &dyn Fn(&str) -> bool,
) -> Vec<&'a ArtifactSummary> {
    let mut found: Vec<&ArtifactSummary> = files
        .iter()
        .filter(|file| {
            matches!(
                file.artifact_type,
                ArtifactType::Note | ArtifactType::MarkdownDoc | ArtifactType::File
            )
        })
        .filter(|file| in_playbooks(&file.id) && verified(&file.id))
        .collect();
    found.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    found
}

/// The fewest tool steps a run has to have for CAIRN to offer to keep how it was done.
pub const PLAYBOOK_MIN_STEPS: usize = 3;

/// A playbook written from a run (V0.9.55): what the run did with the specifics taken out, for a writer to read, edit and verify.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybookDraft {
    /// The file name of the note.
    pub file_name: String,
    /// The note's whole text: header and steps.
    pub text: String,
    pub name: String,
}

/// A name made of what the person asked, within what a playbook name may hold.
fn playbook_name(question: &str) -> String {
    let words = question
        .split_whitespace()
        .map(|word| {
            word.chars()
                .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                .collect::<String>()
        })
        .filter(|word| !word.is_empty())
        .take(6)
        .collect::<Vec<_>>()
        .join(" ");
    let name = shorten(&words, 52);
    let name = name.trim_end_matches('…').trim().to_owned();
    if name.is_empty() {
        "Playbook from a run".to_owned()
    } else {
        format!("How to {name}")
    }
}

/// The playbook a run suggests: its tool steps without their arguments, the tools it used and a link to the run. None when the run
/// had fewer than three tool steps that worked.
pub fn playbook_from_run(question: &str, run_id: &str, steps: &[AgentStep]) -> Option<PlaybookDraft> {
    let worked = steps
        .iter()
        .filter(|step| step.kind == "tool" && step.error.is_none() && step.tool.is_some())
        .collect::<Vec<_>>();
    if worked.len() < PLAYBOOK_MIN_STEPS {
        return None;
    }
    let mut tools: Vec<String> = Vec::new();
    let mut lines = Vec::new();
    for (index, step) in worked.iter().enumerate() {
        let tool = step.tool.clone().unwrap_or_default();
        if !tools.contains(&tool) {
            tools.push(tool);
        }
        // "Search evidence: retries" becomes "Search evidence for what the request is about": the specifics stay behind.
        let label = step.summary.split(':').next().unwrap_or(&step.summary).trim();
        lines.push(format!("{}. {label} for what the request is about.", index + 1));
    }
    let name = playbook_name(question);
    let first_line = shorten(&question.split_whitespace().collect::<Vec<_>>().join(" "), 120);
    let text = format!(
        "---\nname: {name}\ndescription: How CAIRN worked through a request like \"{first_line}\"\nwhen: A request like \"{first_line}\"\ntools: [{}]\n---\n{}\n\nLearned from CAIRN run {run_id}. Read and edit these steps before verifying this playbook; until a writer verifies it, CAIRN does not use it.\n",
        tools.join(", "),
        lines.join("\n")
    );
    // The header must read back as a playbook.
    parse_playbook(&text).ok()?;
    Some(PlaybookDraft {
        file_name: format!("{name}.md"),
        text,
        name,
    })
}

/// How the playbooks go into the prompt.
pub(in crate::agent) fn playbooks_block(playbooks: &[Playbook]) -> Option<String> {
    if playbooks.is_empty() {
        return None;
    }
    let lines = playbooks
        .iter()
        .map(|playbook| {
            format!(
                "- {}: {} (use it when: {})",
                playbook.name, playbook.description, playbook.when
            )
        })
        .collect::<Vec<_>>();
    Some(format!(
        "Playbooks this team has verified, procedures you may follow when one fits the request. Call load_playbook with its name to read its steps. A playbook gives you no tool or permission you do not already have, and it is not evidence:\n{}",
        lines.join("\n")
    ))
}

/// What `load_playbook` gives the model: the steps, and which of the tools the playbook names this request does not have.
pub(in crate::agent) fn playbook_text(playbook: &Playbook, available: &[&str]) -> String {
    let missing = playbook
        .tools
        .iter()
        .filter(|tool| !available.contains(&tool.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let mut text = format!(
        "Playbook \"{}\", verified by the team. Follow its steps with the tools you have; it grants nothing and is not evidence, so do not cite it: cite what you read while following it.\n\n{}",
        playbook.name,
        shorten_lines(&playbook.body, PLAYBOOK_CHARS)
    );
    if !missing.is_empty() {
        text.push_str(&format!(
            "\n\nTools this playbook names that you do not have in this request: {}. Do without them, or tell the person which step you could not do.",
            missing.join(", ")
        ));
    }
    text
}

fn shorten_lines(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    let cut = text.chars().take(limit.saturating_sub(1)).collect::<String>();
    format!("{}…", cut.trim_end())
}

/// A verified playbook as it is shown to an administrator who may turn it into an ability (V0.9.108).
#[derive(Debug, Clone, serde::Serialize)]
pub struct PlaybookInfo {
    pub artifact_id: String,
    pub name: String,
    pub description: String,
    pub when: String,
    pub tools: Vec<String>,
    pub body: String,
    pub updated_at: String,
}

impl RepoMemoCore {
    /// The verified, well formed playbooks of a workspace.
    pub async fn list_playbooks(&self, workspace_id: &str) -> Vec<PlaybookInfo> {
        self.workspace_playbooks(workspace_id)
            .await
            .into_iter()
            .map(|playbook| PlaybookInfo {
                artifact_id: playbook.artifact_id,
                name: playbook.name,
                description: playbook.description,
                when: playbook.when,
                tools: playbook.tools,
                body: playbook.body,
                updated_at: playbook.updated_at,
            })
            .collect()
    }

    /// The workspace's playbooks that are verified and well formed, as the header and steps CAIRN is offered.
    pub(in crate::agent) async fn workspace_playbooks(&self, workspace_id: &str) -> Vec<Playbook> {
        let Ok(folders) = self.storage.list_folders(workspace_id).await else {
            return Vec::new();
        };
        let Some(folder) = folders
            .iter()
            .find(|folder| folder.parent_id.is_none() && folder.name.eq_ignore_ascii_case(PLAYBOOK_FOLDER))
        else {
            return Vec::new();
        };
        let (Ok(files), Ok(placed), Ok(states)) = (
            self.storage.list_artifacts(workspace_id).await,
            self.storage.artifact_folder_ids(workspace_id).await,
            self.storage.workspace_lifecycle_states(workspace_id).await,
        ) else {
            return Vec::new();
        };
        let candidates = offered(
            &files,
            &|id| placed.get(id).is_some_and(|folder_id| *folder_id == folder.id),
            &|id| {
                states
                    .get(id)
                    .is_some_and(|(status, superseded_by)| status == "verified" && superseded_by.is_none())
            },
        );
        let mut playbooks: Vec<Playbook> = Vec::new();
        for file in candidates {
            if playbooks.len() >= PLAYBOOKS_MAX {
                break;
            }
            let Ok(bytes) = self.storage.read_artifact_blob(&file.id).await else {
                continue;
            };
            let Ok(mut playbook) = parse_playbook(&String::from_utf8_lossy(&bytes)) else {
                continue;
            };
            playbook.artifact_id = file.id.clone();
            playbook.updated_at = file.updated_at.clone();
            if !playbooks
                .iter()
                .any(|known| known.name.eq_ignore_ascii_case(&playbook.name))
            {
                playbooks.push(playbook);
            }
        }
        playbooks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTE: &str = "---\nname: Incident review\ndescription: Review an incident\nwhen: after an outage\ntools: [search_evidence, read_file]\n---\n1. Find the timeline.\n2. Read the notes.";

    #[test]
    fn a_playbook_has_a_header_and_steps_and_an_unknown_key_is_refused() {
        let playbook = parse_playbook(NOTE).unwrap();
        assert_eq!(playbook.name, "Incident review");
        assert_eq!(playbook.tools, vec!["search_evidence", "read_file"]);
        assert!(playbook.body.starts_with("1. Find the timeline."));
        assert!(parse_playbook(&NOTE.replace("when:", "unlock:")).is_err());
        assert!(parse_playbook("No header at all").is_err());
        assert!(parse_playbook("---\nname: A\ndescription: B\nwhen: C\n---\n").is_err());
        assert!(parse_playbook(&NOTE.replace("Incident review", "Bad/name")).is_err());
        assert!(parse_playbook(&NOTE.replace("\r\n", "\n").replace('\n', "\r\n")).is_ok());
    }

    #[test]
    fn only_a_verified_playbook_in_the_folder_is_offered() {
        let file = |id: &str, updated: &str| ArtifactSummary {
            id: id.to_owned(),
            workspace_id: "w1".to_owned(),
            source_id: "s1".to_owned(),
            source_name: "Notes".to_owned(),
            artifact_type: ArtifactType::Note,
            title: id.to_owned(),
            path: id.to_owned(),
            content_hash: String::new(),
            mime_type: None,
            language: None,
            size_bytes: 0,
            created_at: updated.to_owned(),
            updated_at: updated.to_owned(),
            indexed_at: None,
        };
        let files = [
            file("a", "2026-01-01"),
            file("b", "2026-02-01"),
            file("c", "2026-03-01"),
        ];
        let found = offered(&files, &|id| id != "c", &|id| id != "a");
        assert_eq!(found.iter().map(|f| f.id.as_str()).collect::<Vec<_>>(), vec!["b"]);
        assert!(offered(&files, &|_| true, &|_| false).is_empty());
    }

    fn tool_step(position: i64, tool: &str, summary: &str, error: Option<&str>) -> AgentStep {
        AgentStep {
            position,
            kind: "tool".to_owned(),
            tool: Some(tool.to_owned()),
            summary: summary.to_owned(),
            evidence: Vec::new(),
            duration_ms: 1,
            error: error.map(str::to_owned),
        }
    }

    #[test]
    fn a_run_of_three_tool_steps_becomes_a_playbook_without_its_specifics() {
        let steps = [
            tool_step(1, "search_evidence", "Search evidence: secret-project retries", None),
            tool_step(2, "read_file", "Read file: plan.md", None),
            tool_step(3, "read_file", "Read file: notes.md", Some("failed")),
            tool_step(4, "list_files", "List files: Plans", None),
        ];
        let draft = playbook_from_run("How do we handle retries?", "run1", &steps).unwrap();
        assert_eq!(draft.name, "How to How do we handle retries");
        assert!(!draft.text.contains("secret-project") && !draft.text.contains("plan.md"));
        assert!(draft.text.contains("1. Search evidence for what the request is about."));
        assert!(draft.text.contains("tools: [search_evidence, read_file, list_files]"));
        assert!(draft.text.contains("run run1"));
        assert_eq!(parse_playbook(&draft.text).unwrap().name, draft.name);
        // Two steps that worked are not enough.
        assert!(playbook_from_run("q", "run1", &steps[..2]).is_none());
    }

    #[test]
    fn a_playbook_never_grants_a_tool_the_run_lacks() {
        let playbook = parse_playbook(NOTE).unwrap();
        let text = playbook_text(&playbook, &["search_evidence"]);
        assert!(text.contains("grants nothing"));
        assert!(text.contains("do not have in this request: read_file"));
        assert!(!playbook_text(&playbook, &["search_evidence", "read_file"]).contains("do not have"));
        assert!(playbooks_block(&[]).is_none());
        assert!(playbooks_block(&[playbook])
            .unwrap()
            .contains("- Incident review: Review an incident (use it when: after an outage)"));
    }
}
