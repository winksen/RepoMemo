//! CAIRN abilities (V0.9.104): stored with every version. An ability is the organization's or a workspace's; it names the tools it may
//! use, and the server checks the names against the registry. Nothing here runs an ability.

use serde::{Deserialize, Serialize};

use super::*;

/// The longest instructions, in characters.
pub const ABILITY_INSTRUCTIONS_CHARS: usize = 8_000;
/// The most input fields an ability asks.
pub const ABILITY_INPUTS_MAX: usize = 8;

/// A field an ability asks the person for before it runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbilityInput {
    pub name: String,
    pub label: String,
}

/// An ability with its current version.
#[derive(Debug, Clone, Serialize)]
pub struct CairnAbility {
    pub id: String,
    /// `organization` or `workspace`.
    pub scope: String,
    pub organization_id: Option<String>,
    pub workspace_id: Option<String>,
    pub name: String,
    /// The `/command` that runs it, with the slash.
    pub command: String,
    pub description: String,
    pub enabled: bool,
    pub version: i64,
    pub instructions: String,
    pub tools: Vec<String>,
    pub inputs: Vec<AbilityInput>,
    pub output_template: String,
    pub updated_at: String,
    pub updated_by: Option<String>,
    /// The verified playbook (a file of the workspace) it was made from, when it was (V0.9.108).
    pub source_playbook_id: Option<String>,
}

/// What a person writes to make or publish a version of an ability.
#[derive(Debug, Clone, Deserialize)]
pub struct AbilityDraft {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub description: String,
    pub instructions: String,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub inputs: Vec<AbilityInput>,
    #[serde(default)]
    pub output_template: String,
    /// Set by the server when an ability is made from a playbook; never taken from a request.
    #[serde(skip)]
    pub source_playbook_id: Option<String>,
}

/// A command as written: lower case letters, digits and hyphens, with or without the slash; returns it with the slash.
pub fn checked_command(command: &str) -> Result<String> {
    let bare = command.trim().trim_start_matches('/').to_ascii_lowercase();
    let valid = (2..=30).contains(&bare.len())
        && bare
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && bare.starts_with(|c: char| c.is_ascii_lowercase());
    if !valid {
        repomemo_domain::bail_invalid!(
            "A command is 2 to 30 lower case letters, digits or hyphens, starting with a letter, like /weekly-status."
        );
    }
    Ok(format!("/{bare}"))
}

fn checked(draft: &AbilityDraft) -> Result<()> {
    if draft.name.trim().is_empty() || draft.name.chars().count() > 80 {
        repomemo_domain::bail_invalid!("An ability needs a name of at most 80 characters.");
    }
    if draft.description.chars().count() > 300 {
        repomemo_domain::bail_invalid!("The description is at most 300 characters.");
    }
    if draft.instructions.trim().is_empty() || draft.instructions.chars().count() > ABILITY_INSTRUCTIONS_CHARS {
        repomemo_domain::bail_invalid!(
            "The instructions are required and at most {ABILITY_INSTRUCTIONS_CHARS} characters."
        );
    }
    if draft.inputs.len() > ABILITY_INPUTS_MAX {
        repomemo_domain::bail_invalid!("An ability asks at most {ABILITY_INPUTS_MAX} fields.");
    }
    if draft
        .inputs
        .iter()
        .any(|input| input.name.trim().is_empty() || input.label.trim().is_empty())
    {
        repomemo_domain::bail_invalid!("Each input field needs a name and a label.");
    }
    Ok(())
}

const SELECT: &str = "SELECT a.id, a.scope, a.organization_id, a.workspace_id, a.name, a.command, a.description, a.enabled, a.current_version,
       v.instructions, v.tools_json, v.inputs_json, v.output_template, v.created_at AS updated_at, u.display_name AS updated_by, a.source_playbook_id
     FROM cairn_abilities a
     JOIN cairn_ability_versions v ON v.ability_id = a.id AND v.version = a.current_version
     LEFT JOIN users u ON u.id = v.created_by";

fn ability_from(row: &SqliteRow) -> CairnAbility {
    CairnAbility {
        id: row.get("id"),
        scope: row.get("scope"),
        organization_id: row.get("organization_id"),
        workspace_id: row.get("workspace_id"),
        name: row.get("name"),
        command: row.get("command"),
        description: row.get("description"),
        enabled: row.get::<i64, _>("enabled") != 0,
        version: row.get("current_version"),
        instructions: row.get("instructions"),
        tools: serde_json::from_str(&row.get::<String, _>("tools_json")).unwrap_or_default(),
        inputs: serde_json::from_str(&row.get::<String, _>("inputs_json")).unwrap_or_default(),
        output_template: row.get("output_template"),
        updated_at: row.get("updated_at"),
        updated_by: row.get("updated_by"),
        source_playbook_id: row.get("source_playbook_id"),
    }
}

impl StorageEngine {
    /// The abilities of an organization (`organization_id`) or of a workspace, by command.
    pub async fn list_cairn_abilities(
        &self,
        organization_id: Option<&str>,
        workspace_id: Option<&str>,
    ) -> Result<Vec<CairnAbility>> {
        let rows = sqlx::query(&format!(
            "{SELECT} WHERE (?1 IS NOT NULL AND a.organization_id = ?1) OR (?2 IS NOT NULL AND a.workspace_id = ?2) ORDER BY a.command"
        ))
        .bind(organization_id)
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows.iter().map(ability_from).collect())
    }

    pub async fn get_cairn_ability(&self, id: &str) -> Result<CairnAbility> {
        let row = sqlx::query(&format!("{SELECT} WHERE a.id = ?1"))
            .bind(id)
            .fetch_optional(self.reader())
            .await?
            .ok_or_else(|| repomemo_domain::not_found!("The ability was not found."))?;
        Ok(ability_from(&row))
    }

    /// Makes an ability at version 1. The tools must already be checked against the registry.
    pub async fn create_cairn_ability(
        &self,
        organization_id: Option<&str>,
        workspace_id: Option<&str>,
        draft: &AbilityDraft,
        actor: &str,
    ) -> Result<CairnAbility> {
        checked(draft)?;
        let command = checked_command(&draft.command)?;
        // A workspace's command may not repeat one its organization teaches, so a /command means one thing.
        if let Some(workspace_id) = workspace_id {
            if let Some(organization_id) = self.workspace_organization(workspace_id).await? {
                let taken: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM cairn_abilities WHERE organization_id = ?1 AND command = ?2",
                )
                .bind(organization_id)
                .bind(&command)
                .fetch_one(self.reader())
                .await?;
                if taken > 0 {
                    repomemo_domain::bail_conflict!(
                        "{command} is already an ability of this workspace's organization."
                    );
                }
            }
        }
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        let inserted = sqlx::query(
            "INSERT INTO cairn_abilities (id, scope, organization_id, workspace_id, name, command, description, created_at, updated_at, created_by, source_playbook_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8, ?9, ?10)",
        )
        .bind(&id)
        .bind(if organization_id.is_some() { "organization" } else { "workspace" })
        .bind(organization_id)
        .bind(workspace_id)
        .bind(draft.name.trim())
        .bind(&command)
        .bind(draft.description.trim())
        .bind(&now)
        .bind(actor)
        .bind(&draft.source_playbook_id)
        .execute(&mut *tx)
        .await;
        match inserted {
            Ok(_) => {}
            Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
                repomemo_domain::bail_conflict!("{command} is already an ability here.")
            }
            Err(error) => return Err(error.into()),
        }
        insert_version(&mut tx, &id, 1, draft, actor, &now).await?;
        tx.commit().await?;
        self.get_cairn_ability(&id).await
    }

    /// Publishes a new version of an ability that was read at `expected_version`; a stale save returns the current ability.
    pub async fn publish_cairn_ability(
        &self,
        id: &str,
        expected_version: i64,
        draft: &AbilityDraft,
        actor: &str,
    ) -> Result<std::result::Result<CairnAbility, CairnAbility>> {
        checked(draft)?;
        let command = checked_command(&draft.command)?;
        let existing = self.get_cairn_ability(id).await?;
        if existing.version != expected_version {
            return Ok(Err(existing));
        }
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        let updated = sqlx::query(
            "UPDATE cairn_abilities SET name = ?2, command = ?3, description = ?4, current_version = current_version + 1, updated_at = ?5
             WHERE id = ?1 AND current_version = ?6",
        )
        .bind(id)
        .bind(draft.name.trim())
        .bind(&command)
        .bind(draft.description.trim())
        .bind(&now)
        .bind(expected_version)
        .execute(&mut *tx)
        .await;
        match updated {
            Ok(done) if done.rows_affected() == 1 => {}
            Ok(_) => return Ok(Err(self.get_cairn_ability(id).await?)),
            Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
                repomemo_domain::bail_conflict!("{command} is already an ability here.")
            }
            Err(error) => return Err(error.into()),
        }
        insert_version(&mut tx, id, expected_version + 1, draft, actor, &now).await?;
        tx.commit().await?;
        Ok(Ok(self.get_cairn_ability(id).await?))
    }

    /// The abilities a workspace offers: its organization's that are on and that the workspace did not turn off, and its own that are on.
    pub async fn abilities_for_workspace(&self, workspace_id: &str) -> Result<Vec<CairnAbility>> {
        let organization_id = self.workspace_organization(workspace_id).await?;
        let rows = sqlx::query(&format!(
            "{SELECT} WHERE a.enabled = 1 AND ((?1 IS NOT NULL AND a.organization_id = ?1
                 AND a.id NOT IN (SELECT ability_id FROM workspace_ability_off WHERE workspace_id = ?2))
               OR a.workspace_id = ?2) ORDER BY a.command"
        ))
        .bind(organization_id)
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows.iter().map(ability_from).collect())
    }

    /// An organization's abilities as a workspace sees them, each with whether the workspace has it on.
    pub async fn organization_abilities_for_workspace(&self, workspace_id: &str) -> Result<Vec<(CairnAbility, bool)>> {
        let Some(organization_id) = self.workspace_organization(workspace_id).await? else {
            return Ok(Vec::new());
        };
        let off: Vec<String> =
            sqlx::query_scalar("SELECT ability_id FROM workspace_ability_off WHERE workspace_id = ?1")
                .bind(workspace_id)
                .fetch_all(self.reader())
                .await?;
        Ok(self
            .list_cairn_abilities(Some(&organization_id), None)
            .await?
            .into_iter()
            .map(|ability| {
                let on = !off.contains(&ability.id);
                (ability, on)
            })
            .collect())
    }

    /// Turns one of the workspace's organization's abilities off or on for the workspace.
    pub async fn set_workspace_ability(
        &self,
        workspace_id: &str,
        ability_id: &str,
        on: bool,
        actor: &str,
    ) -> Result<()> {
        let ability = self.get_cairn_ability(ability_id).await?;
        let organization_id = self.workspace_organization(workspace_id).await?;
        if ability.organization_id.is_none() || ability.organization_id != organization_id {
            repomemo_domain::bail_invalid!("That ability is not one of this workspace's organization.");
        }
        if on {
            sqlx::query("DELETE FROM workspace_ability_off WHERE workspace_id = ?1 AND ability_id = ?2")
                .bind(workspace_id)
                .bind(ability_id)
                .execute(&self.pool)
                .await?;
        } else {
            sqlx::query(
                "INSERT OR IGNORE INTO workspace_ability_off (workspace_id, ability_id, turned_off_at, turned_off_by) VALUES (?1, ?2, ?3, ?4)",
            )
            .bind(workspace_id)
            .bind(ability_id)
            .bind(Utc::now().to_rfc3339())
            .bind(actor)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    /// The abilities a workspace made from its playbooks, by the playbook's file id.
    pub async fn abilities_from_playbooks(&self, workspace_id: &str) -> Result<Vec<CairnAbility>> {
        let rows = sqlx::query(&format!(
            "{SELECT} WHERE a.workspace_id = ?1 AND a.source_playbook_id IS NOT NULL ORDER BY a.command"
        ))
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows.iter().map(ability_from).collect())
    }

    /// Turns an ability on or off.
    pub async fn set_cairn_ability_enabled(&self, id: &str, enabled: bool) -> Result<CairnAbility> {
        let done = sqlx::query("UPDATE cairn_abilities SET enabled = ?2, updated_at = ?3 WHERE id = ?1")
            .bind(id)
            .bind(enabled)
            .bind(Utc::now().to_rfc3339())
            .execute(&self.pool)
            .await?;
        if done.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("The ability was not found.");
        }
        self.get_cairn_ability(id).await
    }

    pub async fn delete_cairn_ability(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM cairn_abilities WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Every version of an ability, newest first, with who published it.
    pub async fn cairn_ability_versions(&self, id: &str) -> Result<Vec<(i64, String, Option<String>)>> {
        let rows = sqlx::query(
            "SELECT v.version, v.created_at, u.display_name AS by_name FROM cairn_ability_versions v
             LEFT JOIN users u ON u.id = v.created_by WHERE v.ability_id = ?1 ORDER BY v.version DESC",
        )
        .bind(id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows
            .iter()
            .map(|row| (row.get("version"), row.get("created_at"), row.get("by_name")))
            .collect())
    }
}

async fn insert_version(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    id: &str,
    version: i64,
    draft: &AbilityDraft,
    actor: &str,
    now: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO cairn_ability_versions (ability_id, version, instructions, tools_json, inputs_json, output_template, created_at, created_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )
    .bind(id)
    .bind(version)
    .bind(draft.instructions.trim())
    .bind(serde_json::to_string(&draft.tools)?)
    .bind(serde_json::to_string(&draft.inputs)?)
    .bind(draft.output_template.trim())
    .bind(now)
    .bind(actor)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_is_lower_case_letters_digits_and_hyphens() {
        assert_eq!(checked_command("Weekly-Status").unwrap(), "/weekly-status");
        assert_eq!(checked_command("/ab").unwrap(), "/ab");
        for bad in ["a", "1abc", "has space", "under_score", "/", "x".repeat(31).as_str()] {
            assert!(checked_command(bad).is_err(), "{bad}");
        }
    }

    #[tokio::test]
    async fn every_version_is_kept_and_a_stale_publish_returns_the_current_ability() {
        let dir = std::env::temp_dir().join(format!("repomemo-abilities-{}", Uuid::new_v4()));
        let storage = StorageEngine::open(crate::StorageConfig::new(dir.clone()))
            .await
            .unwrap();
        let actor = storage.create_user("owner@example.com", "Owner", "hash").await.unwrap();
        let organization = storage
            .create_organization_with(&actor.id, "Org", None, None)
            .await
            .unwrap();
        let draft = |instructions: &str| AbilityDraft {
            name: "Weekly status".to_owned(),
            command: "weekly-status".to_owned(),
            description: String::new(),
            instructions: instructions.to_owned(),
            tools: vec!["search_evidence".to_owned()],
            inputs: Vec::new(),
            output_template: String::new(),
            source_playbook_id: None,
        };
        let first = storage
            .create_cairn_ability(Some(&organization.id), None, &draft("Summarize the week."), &actor.id)
            .await
            .unwrap();
        assert_eq!((first.version, first.command.as_str()), (1, "/weekly-status"));
        assert!(
            storage
                .create_cairn_ability(Some(&organization.id), None, &draft("Again."), &actor.id)
                .await
                .is_err(),
            "a command once per scope"
        );

        let second = storage
            .publish_cairn_ability(&first.id, 1, &draft("Summarize the week and the risks."), &actor.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(second.version, 2);
        let stale = storage
            .publish_cairn_ability(&first.id, 1, &draft("Late."), &actor.id)
            .await
            .unwrap();
        assert_eq!(stale.unwrap_err().version, 2);
        assert_eq!(storage.cairn_ability_versions(&first.id).await.unwrap().len(), 2);
        assert!(
            !storage
                .set_cairn_ability_enabled(&first.id, false)
                .await
                .unwrap()
                .enabled
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
