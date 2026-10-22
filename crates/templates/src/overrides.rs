use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{assistant, sections, AssistantBlock, SectionsBlock, TemplateDefinition, TemplateIssue};

/// The few adjustments an administrator may save on top of a workspace's template (V0.2.177). They use the same
/// vocabularies and limits as the template's own blocks, and each is optional: a missing key keeps what the
/// template says.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Overrides {
    /// The section the workspace opens on (not People or Settings).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub landing: Option<String>,
    /// Exactly the sections kept under *More*; the others are tabs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub more: Option<Vec<String>>,
    /// The questions the assistant suggests, replacing the template's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggestions: Option<Vec<String>>,
}

impl Overrides {
    pub fn is_empty(&self) -> bool {
        self.landing.is_none() && self.more.is_none() && self.suggestions.is_none()
    }

    /// Reads adjustments that arrive in a request: every problem is returned, with its key under `overrides`.
    pub fn parse(value: &Value) -> Result<Self, Vec<TemplateIssue>> {
        let overrides: Self = serde_json::from_value(value.clone())
            .map_err(|error| vec![TemplateIssue::new("overrides", error.to_string())])?;
        let issues = overrides.issues();
        if issues.is_empty() {
            Ok(overrides)
        } else {
            Err(issues)
        }
    }

    /// Reads the adjustments saved with a workspace. A key that is no longer valid (a section this server lacks) is
    /// dropped and the others are kept, so a stale adjustment never breaks a page.
    pub fn from_stored(value: &Value) -> Self {
        let Value::Object(map) = value else {
            return Self::default();
        };
        let mut kept = Self::default();
        for (key, entry) in map {
            if let Ok(one) = Self::parse(&json!({ key: entry })) {
                kept.landing = kept.landing.or(one.landing);
                kept.more = kept.more.or(one.more);
                kept.suggestions = kept.suggestions.or(one.suggestions);
            }
        }
        kept
    }

    fn issues(&self) -> Vec<TemplateIssue> {
        let mut found = Vec::new();
        if self.landing.is_some() || self.more.is_some() {
            let block = SectionsBlock {
                order: Vec::new(),
                more: self.more.clone().unwrap_or_default(),
                landing: self.landing.clone(),
            };
            sections::validate(&block, &mut found);
        }
        if let Some(suggestions) = &self.suggestions {
            let block = AssistantBlock {
                order: Vec::new(),
                suggestions: suggestions.clone(),
            };
            assistant::validate(&block, &mut found);
        }
        found
            .into_iter()
            .map(|mut issue| {
                issue.path = issue
                    .path
                    .replacen("sections.", "overrides.", 1)
                    .replacen("assistant.", "overrides.", 1);
                issue
            })
            .collect()
    }
}

impl TemplateDefinition {
    /// The template with a workspace's adjustments laid over it: the landing section, the sections under *More*
    /// (taken out of the tabs) and the suggested questions. Nothing else of the template changes.
    pub fn with_overrides(&self, overrides: &Overrides) -> TemplateDefinition {
        let mut adjusted = self.clone();
        if overrides.landing.is_some() || overrides.more.is_some() {
            let mut block = adjusted.sections.take().unwrap_or_default();
            if let Some(landing) = &overrides.landing {
                block.landing = Some(landing.clone());
            }
            if let Some(more) = &overrides.more {
                block.order.retain(|section| !more.contains(section));
                block.more = more.clone();
            }
            adjusted.sections = Some(block);
        }
        if let Some(suggestions) = &overrides.suggestions {
            let mut block = adjusted.assistant.take().unwrap_or_default();
            block.suggestions = suggestions.clone();
            adjusted.assistant = Some(block);
        }
        adjusted
    }
}
