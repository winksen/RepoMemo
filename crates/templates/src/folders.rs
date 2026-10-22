use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{limits, TemplateIssue};

/// A folder a template creates in a workspace made from it. It has a name and,
/// if wanted, folders of its own one level below: two levels at most.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StarterFolder {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<String>,
}

/// Checks the `starter_folders` block: how many folders in all, the name of each
/// and that no two folders beside each other share a name (case aside), because a
/// workspace refuses that too.
pub(crate) fn validate(folders: &[StarterFolder], issues: &mut Vec<TemplateIssue>) {
    let total = folders.len() + folders.iter().map(|folder| folder.children.len()).sum::<usize>();
    if total > limits::STARTER_FOLDERS_MAX {
        issues.push(TemplateIssue::new(
            "starter_folders",
            format!(
                "there are {total} folders; a template creates at most {}",
                limits::STARTER_FOLDERS_MAX
            ),
        ));
    }
    let mut top = HashSet::new();
    for (index, folder) in folders.iter().enumerate() {
        let path = format!("starter_folders[{index}]");
        check_name(&folder.name, &format!("{path}.name"), &mut top, issues);
        let mut below = HashSet::new();
        for (child_index, child) in folder.children.iter().enumerate() {
            check_name(child, &format!("{path}.children[{child_index}]"), &mut below, issues);
        }
    }
}

fn check_name(name: &str, path: &str, seen: &mut HashSet<String>, issues: &mut Vec<TemplateIssue>) {
    let length = name.chars().count();
    let problem = if name.trim().is_empty() {
        Some("the name is empty".to_owned())
    } else if length > limits::FOLDER_NAME_MAX {
        Some(format!(
            "the name is {length} characters; the most is {}",
            limits::FOLDER_NAME_MAX
        ))
    } else if name.trim() != name {
        Some("the name starts or ends with a space".to_owned())
    } else if name.chars().any(|c| c.is_control() || c == '/' || c == '\\') {
        Some("the name has a slash or a control character".to_owned())
    } else if !seen.insert(name.to_lowercase()) {
        Some(format!("another folder beside this one is already named {name}"))
    } else {
        None
    };
    if let Some(problem) = problem {
        issues.push(TemplateIssue::new(path, problem));
    }
}
