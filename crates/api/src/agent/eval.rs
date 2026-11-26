//! The CAIRN evaluation set (V0.9.46). `tests/cairn_eval/` holds a fixture workspace (36 documents, a connector issue and a mail
//! that carry instructions, review states and two memory cards) and 40 cases. Each case is a message, what a model that does
//! this well would do (a script of tool calls and an answer) and what must be true of the run: the tools called, the files
//! cited, the tools and files that must not appear, the end state.
//!
//! Here the cases run against a scripted provider, so the harness, the tools, the ledger and the budgets are checked
//! deterministically. `npm run cairn:eval` (scripts/cairn-eval.mjs) runs the same messages against a real model and measures
//! how often it does what the scripts do.

use std::sync::Arc;

use repomemo_ai::{ChatResponse, ScriptedProvider, ToolMode};
use repomemo_domain::{AgentRequest, ArtifactType, CreateMemoryCardRequest, SourceType};
use repomemo_storage::{NewArtifact, SaveArtifactLifecycle, StorageEngine};
use serde::Deserialize;
use serde_json::Value;

use super::AgentLoopSetup;
use crate::RepoMemoCore;

const DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/cairn_eval");

#[derive(Deserialize)]
struct Set {
    thresholds: Thresholds,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Thresholds {
    citation_precision: f64,
    forbidden_actions: usize,
    tool_accuracy: f64,
}

#[derive(Deserialize)]
struct Case {
    id: String,
    message: String,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    max_steps: Option<usize>,
    script: Vec<Move>,
    expect: Expect,
}

#[derive(Deserialize)]
struct Move {
    tool: Option<String>,
    args: Option<Value>,
    answer: Option<String>,
}

#[derive(Deserialize, Default)]
struct Expect {
    #[serde(default)]
    tools: Vec<String>,
    #[serde(default)]
    ordered: bool,
    #[serde(default)]
    cited: Vec<String>,
    #[serde(default)]
    forbidden_tools: Vec<String>,
    #[serde(default)]
    forbidden_files: Vec<String>,
    state: String,
    #[serde(default)]
    proposals: Option<usize>,
    #[serde(default)]
    warnings_contain: Vec<String>,
}

#[derive(Deserialize)]
struct Manifest {
    files: Vec<FixtureFile>,
    cards: Vec<FixtureCard>,
}

#[derive(Deserialize)]
struct FixtureFile {
    file: String,
    title: String,
    kind: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    state: Option<String>,
}

#[derive(Deserialize)]
struct FixtureCard {
    title: String,
    body: String,
}

fn read(path: &str) -> String {
    std::fs::read_to_string(format!("{DIRECTORY}/{path}")).unwrap_or_else(|error| panic!("{path}: {error}"))
}

async fn fixture_workspace(core: &RepoMemoCore) -> String {
    let manifest: Manifest = serde_json::from_str(&read("manifest.json")).unwrap();
    let workspace = core.create_workspace("CAIRN evaluation".to_owned()).await.unwrap();
    let actor = core
        .storage()
        .create_user("eval@example.com", "Evaluator", "hash")
        .await
        .unwrap();
    let connector = core
        .storage()
        .create_or_get_source(&workspace.id, SourceType::Connector, "github", Some("github"))
        .await
        .unwrap();
    for file in &manifest.files {
        let text = read(&format!("fixture/{}", file.file));
        let artifact_id = match file.kind.as_str() {
            "markdown" => {
                core.import_text(
                    workspace.id.clone(),
                    file.title.clone(),
                    text,
                    Some("Markdown".to_owned()),
                )
                .await
                .unwrap()
                .id
            }
            kind => {
                let bytes = text.into_bytes();
                let content_hash = StorageEngine::content_hash(&bytes);
                core.storage()
                    .store_blob(&content_hash, &bytes, Some("text/plain"))
                    .await
                    .unwrap();
                core.storage()
                    .store_artifact(NewArtifact {
                        workspace_id: workspace.id.clone(),
                        source_id: connector.id.clone(),
                        artifact_type: if kind == "issue" {
                            ArtifactType::Issue
                        } else {
                            ArtifactType::File
                        },
                        title: file.title.clone(),
                        path: file.path.clone().unwrap_or_else(|| file.file.clone()),
                        content_hash,
                        mime_type: Some("text/plain".to_owned()),
                        language: None,
                        size_bytes: bytes.len() as i64,
                        metadata: serde_json::json!({}),
                    })
                    .await
                    .unwrap()
                    .artifact
                    .id
            }
        };
        core.index_artifact(artifact_id.clone()).await.unwrap();
        if let Some(state) = &file.state {
            core.storage()
                .save_artifact_lifecycle(
                    &artifact_id,
                    &actor.id,
                    SaveArtifactLifecycle {
                        status: state.clone(),
                        owner_user_id: None,
                        review_note: String::new(),
                        superseded_by_artifact_id: None,
                    },
                )
                .await
                .unwrap();
        }
    }
    for card in &manifest.cards {
        core.create_memory_card(CreateMemoryCardRequest {
            workspace_id: workspace.id.clone(),
            title: card.title.clone(),
            body_markdown: card.body.clone(),
            source: "evaluation".to_owned(),
            confidence: None,
            citations: Vec::new(),
            author_user_id: None,
        })
        .await
        .unwrap();
    }
    workspace.id
}

fn replies(script: &[Move]) -> Vec<ChatResponse> {
    script
        .iter()
        .map(|step| match (&step.tool, &step.answer) {
            (Some(tool), _) => ChatResponse::call(tool, step.args.clone().unwrap_or(Value::Null)),
            (None, Some(answer)) => ChatResponse::text(answer.clone()),
            (None, None) => panic!("a move needs a tool or an answer"),
        })
        .collect()
}

/// Whether `wanted` appears in `called` in that order, not necessarily next to each other.
fn is_subsequence(wanted: &[String], called: &[String]) -> bool {
    let mut remaining = called.iter();
    wanted.iter().all(|tool| remaining.any(|other| other == tool))
}

/// Whether every tool in `wanted` was called at least as often as it is wanted.
fn covers(wanted: &[String], called: &[String]) -> bool {
    wanted.iter().all(|tool| {
        called.iter().filter(|other| *other == tool).count() >= wanted.iter().filter(|w| *w == tool).count()
    })
}

#[derive(Default)]
struct Totals {
    cases: usize,
    tool_ok: usize,
    cited: usize,
    cited_expected: usize,
    forbidden_actions: usize,
    failures: Vec<String>,
}

#[tokio::test]
async fn the_scripted_cases_pass_and_meet_the_thresholds() {
    let set: Set = serde_json::from_str(&read("cases.json")).unwrap();
    assert_eq!(set.cases.len(), 40);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let data_dir = std::env::temp_dir().join(format!("repomemo-cairn-eval-{nanos}"));
    let core = RepoMemoCore::boot(data_dir.clone()).await.unwrap();
    let workspace_id = fixture_workspace(&core).await;
    let mut totals = Totals::default();

    for case in &set.cases {
        let provider = ScriptedProvider::new(replies(&case.script));
        let outcome = core
            .loop_with(
                &provider,
                AgentRequest {
                    workspace_id: workspace_id.clone(),
                    message: case.message.clone(),
                    capability: None,
                    artifact_id: None,
                    provider_id: Some("scripted".to_owned()),
                    ai_unavailable_reason: None,
                    history: Vec::new(),
                    artifact_ids: Vec::new(),
                    summary: None,
                    personal_memory: Vec::new(),
                    corrections: Vec::new(),
                    decision_offers_off: false,
                    history_tokens: None,
                    exchanges: Vec::new(),
                },
                AgentLoopSetup {
                    tool_mode: ToolMode::Native,
                    context_tokens: 8_192,
                    max_steps: case.max_steps.unwrap_or(8),
                    max_seconds: 60,
                    max_run_tokens: 0,
                    disabled_tools: Vec::new(),
                    ability: None,
                    organization_instructions: None,
                    knowledge_workspace: None,
                    fallback: None,
                    can_write: case.role.as_deref() != Some("viewer"),
                    user_id: String::new(),
                    background_max_files: 8,
                    progress: None,
                    cancel: None,
                    consume_quota: Arc::new(|| Ok(())),
                },
            )
            .await
            .unwrap();
        let trace = outcome.trace.expect("the loop records a trace");
        let called = trace
            .steps
            .iter()
            .filter(|step| step.error.is_none())
            .filter_map(|step| step.tool.clone())
            .collect::<Vec<_>>();
        let cited = outcome
            .reply
            .citations
            .iter()
            .map(|citation| citation.title.clone())
            .collect::<Vec<_>>();
        let expect = &case.expect;
        totals.cases += 1;

        let tools_right = if expect.ordered {
            is_subsequence(&expect.tools, &called)
        } else {
            covers(&expect.tools, &called)
        };
        if tools_right {
            totals.tool_ok += 1;
        } else {
            totals
                .failures
                .push(format!("{}: tools {called:?}, wanted {:?}", case.id, expect.tools));
        }
        if !expect.cited.is_empty() {
            totals.cited += cited.len();
            totals.cited_expected += cited.iter().filter(|title| expect.cited.contains(title)).count();
            for wanted in &expect.cited {
                if !cited.contains(wanted) {
                    totals
                        .failures
                        .push(format!("{}: {wanted} was not cited ({cited:?})", case.id));
                }
            }
        }
        let forbidden_tools = called
            .iter()
            .filter(|tool| expect.forbidden_tools.contains(tool))
            .count();
        let forbidden_files = cited
            .iter()
            .filter(|title| expect.forbidden_files.contains(title))
            .count();
        let unwanted_proposals = usize::from(expect.proposals == Some(0) && !outcome.reply.proposals.is_empty());
        totals.forbidden_actions += forbidden_tools + forbidden_files + unwanted_proposals;
        if trace.state != expect.state {
            totals
                .failures
                .push(format!("{}: ended {}, wanted {}", case.id, trace.state, expect.state));
        }
        if let Some(count) = expect.proposals {
            if outcome.reply.proposals.len() != count {
                totals.failures.push(format!(
                    "{}: {} proposals, wanted {count}",
                    case.id,
                    outcome.reply.proposals.len()
                ));
            }
        }
        for phrase in &expect.warnings_contain {
            if !outcome
                .reply
                .warnings
                .iter()
                .any(|warning| warning.contains(phrase.as_str()))
            {
                totals.failures.push(format!(
                    "{}: no warning says \"{phrase}\" ({:?})",
                    case.id, outcome.reply.warnings
                ));
            }
        }
    }

    let precision = if totals.cited == 0 {
        1.0
    } else {
        totals.cited_expected as f64 / totals.cited as f64
    };
    let accuracy = totals.tool_ok as f64 / totals.cases as f64;
    assert!(totals.failures.is_empty(), "{}", totals.failures.join("\n"));
    assert!(
        precision >= set.thresholds.citation_precision,
        "citation precision {precision}"
    );
    assert!(accuracy >= set.thresholds.tool_accuracy, "tool accuracy {accuracy}");
    assert!(
        totals.forbidden_actions <= set.thresholds.forbidden_actions,
        "{} forbidden actions",
        totals.forbidden_actions
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn the_helpers_compare_what_was_called_with_what_was_wanted() {
    let called = ["search_evidence", "read_file", "read_file"]
        .map(str::to_owned)
        .to_vec();
    assert!(is_subsequence(
        &["search_evidence".to_owned(), "read_file".to_owned()],
        &called
    ));
    assert!(!is_subsequence(
        &["read_file".to_owned(), "search_evidence".to_owned()],
        &called
    ));
    assert!(covers(&["read_file".to_owned(), "read_file".to_owned()], &called));
    assert!(!covers(
        &["read_file".to_owned(), "read_file".to_owned(), "read_file".to_owned()],
        &called
    ));
}

#[test]
fn the_set_has_forty_cases_with_unique_ids_and_a_script_each() {
    let set: Set = serde_json::from_str(&read("cases.json")).unwrap();
    assert_eq!(set.cases.len(), 40);
    let mut ids = std::collections::HashSet::new();
    for case in &set.cases {
        assert!(ids.insert(case.id.clone()), "{} is repeated", case.id);
        assert!(!case.script.is_empty(), "{} has no script", case.id);
    }
    let manifest: Manifest = serde_json::from_str(&read("manifest.json")).unwrap();
    assert!(manifest.files.len() >= 38);
    assert_eq!(manifest.cards.len(), 2);
    for file in &manifest.files {
        assert!(!read(&format!("fixture/{}", file.file)).trim().is_empty());
    }
}
