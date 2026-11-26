//! What can be turned off in CAIRN (V0.9.91): its jobs, which a person asks for from the + menu or in words, and its tools, which it
//! uses in a loop. An organization turns off any of them for its workspaces and a workspace turns off more; a workspace cannot turn on
//! what its organization turned off. The names are the jobs' ids and the tools' names, in one list.

use axum::Json;
use serde::Serialize;

use crate::{ApiError, AuthenticatedSubject};

#[derive(Debug, Serialize)]
pub(crate) struct SwitchableTool {
    /// The job's id or the tool's name, as the policies keep it.
    name: String,
    label: String,
    /// `job` or `tool`.
    kind: &'static str,
    description: String,
    /// The job changes the workspace once the person confirms it.
    writes: bool,
}

/// Every job and tool that can be turned off.
pub(crate) fn switchable() -> Vec<SwitchableTool> {
    let mut all: Vec<SwitchableTool> = repomemo_api::agent_capabilities(true)
        .into_iter()
        .map(|job| SwitchableTool {
            name: job_id(job.id),
            label: job.label,
            kind: "job",
            description: job.description,
            writes: job.writes,
        })
        .collect();
    for (name, label) in repomemo_api::cairn_tool_catalog() {
        if !all.iter().any(|existing| existing.name == name) {
            all.push(SwitchableTool {
                name: name.to_owned(),
                label: label.to_owned(),
                kind: "tool",
                description: String::new(),
                writes: false,
            });
        }
    }
    all
}

/// A job's id as the policies keep it.
pub(crate) fn job_id(capability: repomemo_domain::AgentCapability) -> String {
    serde_json::to_value(capability)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// The names of a request, checked against what can be turned off, without repeats.
pub(crate) fn checked_names(names: Vec<String>) -> Result<Vec<String>, ApiError> {
    let known = switchable();
    let mut checked: Vec<String> = Vec::new();
    for name in names {
        if !known.iter().any(|tool| tool.name == name) {
            return Err(ApiError::bad_request(format!("{name} is not a CAIRN job or tool.")));
        }
        if !checked.contains(&name) {
            checked.push(name);
        }
    }
    Ok(checked)
}

/// The label of a job or tool, for what is said to a person.
pub(crate) fn label_of(name: &str) -> String {
    switchable()
        .into_iter()
        .find(|tool| tool.name == name)
        .map_or_else(|| name.to_owned(), |tool| tool.label)
}

/// `GET /v1/cairn/tools`: what an organization or a workspace can turn off.
pub(crate) async fn list_tools(_subject: AuthenticatedSubject) -> Json<Vec<SwitchableTool>> {
    Json(switchable())
}
