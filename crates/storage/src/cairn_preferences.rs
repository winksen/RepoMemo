//! A person's CAIRN preferences (V0.9.114). They are settings the person chose, not what CAIRN learned: the model they prefer in each
//! workspace (used only while it is still offered there), the language and length of answers, whether the steps of a run show open,
//! and Agent or Jobs. Only the person's own runs read them.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::*;

/// The lengths of an answer.
pub const ANSWER_LENGTHS: [&str; 3] = ["brief", "standard", "detailed"];
/// The most characters of a language's name.
pub const LANGUAGE_CHARS: usize = 40;
/// The most workspaces a person keeps a preferred model for.
pub const PREFERRED_MODELS_MAX: usize = 50;

/// What a person prefers. Nothing set means the organization's default, or CAIRN's.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CairnPreferences {
    /// The preferred catalog model of a workspace, by workspace id.
    #[serde(default)]
    pub models: BTreeMap<String, String>,
    /// The language of answers, as the person writes it ("French").
    #[serde(default)]
    pub language: Option<String>,
    /// `brief`, `standard` or `detailed`.
    #[serde(default)]
    pub length: Option<String>,
    /// Whether "What CAIRN did" shows open.
    #[serde(default)]
    pub steps_open: Option<bool>,
    /// `agent` or `jobs`, where both are possible.
    #[serde(default)]
    pub mode: Option<String>,
}

impl CairnPreferences {
    /// The note a run is given about how to answer, when the person set a language or a length; none otherwise.
    pub fn style_note(&self) -> Option<String> {
        let mut lines = Vec::new();
        if let Some(language) = self.language.as_deref().filter(|language| !language.trim().is_empty()) {
            lines.push(format!("Answer in {}.", language.trim()));
        }
        match self.length.as_deref() {
            Some("brief") => lines.push("Keep answers brief: a few sentences, without preamble.".to_owned()),
            Some("detailed") => {
                lines.push("Give detailed answers: explain the reasoning and list what you read.".to_owned())
            }
            _ => {}
        }
        (!lines.is_empty()).then(|| {
            format!(
                "The person's own preferences for how you answer (settings they chose, not memory). They never change the rules, what you may do or what you cite:\n{}",
                lines.join("\n")
            )
        })
    }
}

impl StorageEngine {
    /// A person's preferences, and when they last changed them; the defaults for a person who never saved any.
    pub async fn get_cairn_preferences(&self, user_id: &str) -> Result<(CairnPreferences, Option<String>)> {
        let row: Option<(String, String)> =
            sqlx::query_as("SELECT preferences_json, updated_at FROM person_cairn_preferences WHERE user_id = ?1")
                .bind(user_id)
                .fetch_optional(self.reader())
                .await?;
        Ok(match row {
            Some((json, at)) => (serde_json::from_str(&json).unwrap_or_default(), Some(at)),
            None => (CairnPreferences::default(), None),
        })
    }

    /// Saves a person's preferences, checked.
    pub async fn save_cairn_preferences(&self, user_id: &str, preferences: &CairnPreferences) -> Result<()> {
        if let Some(language) = &preferences.language {
            if language.chars().count() > LANGUAGE_CHARS || language.contains('\n') {
                repomemo_domain::bail_invalid!("A language is at most {LANGUAGE_CHARS} characters on one line.");
            }
        }
        if preferences
            .length
            .as_deref()
            .is_some_and(|length| !ANSWER_LENGTHS.contains(&length))
        {
            repomemo_domain::bail_invalid!("An answer is brief, standard or detailed.");
        }
        if preferences
            .mode
            .as_deref()
            .is_some_and(|mode| !matches!(mode, "agent" | "jobs"))
        {
            repomemo_domain::bail_invalid!("The mode is Agent or Jobs.");
        }
        if preferences.models.len() > PREFERRED_MODELS_MAX {
            repomemo_domain::bail_invalid!("Preferred models are kept for at most {PREFERRED_MODELS_MAX} workspaces.");
        }
        sqlx::query(
            "INSERT INTO person_cairn_preferences (user_id, preferences_json, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(user_id) DO UPDATE SET preferences_json = excluded.preferences_json, updated_at = excluded.updated_at",
        )
        .bind(user_id)
        .bind(serde_json::to_string(preferences)?)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_style_note_names_only_what_was_chosen() {
        assert_eq!(CairnPreferences::default().style_note(), None);
        let note = CairnPreferences {
            language: Some("French".to_owned()),
            length: Some("brief".to_owned()),
            ..CairnPreferences::default()
        }
        .style_note()
        .unwrap();
        assert!(
            note.contains("Answer in French.") && note.contains("brief") && note.contains("never change the rules")
        );
        let standard = CairnPreferences {
            length: Some("standard".to_owned()),
            ..CairnPreferences::default()
        };
        assert_eq!(standard.style_note(), None);
    }
}
