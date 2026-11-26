//! What CAIRN remembers about one person (V0.9.51). Personal: only the person's own runs read it, the person edits it in their
//! profile, anonymizing the account removes it, and workspace archives never contain it.

use repomemo_domain::{PersonalMemory, PersonalMemoryEntry};

use super::*;

/// The most entries a person has.
pub const PERSONAL_MEMORY_ENTRIES: usize = 40;
/// The most characters of an entry.
pub const PERSONAL_MEMORY_CHARS: usize = 280;

/// What an entry may say: one line of text, not empty and within the bound.
fn clean_entry(text: &str) -> Result<String> {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        repomemo_domain::bail_invalid!("Write what CAIRN should remember.");
    }
    if text.chars().count() > PERSONAL_MEMORY_CHARS {
        repomemo_domain::bail_invalid!("An entry is at most {PERSONAL_MEMORY_CHARS} characters.");
    }
    Ok(text)
}

fn entry_from(row: &SqliteRow) -> PersonalMemoryEntry {
    PersonalMemoryEntry {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        text: row.get("text"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

impl StorageEngine {
    /// A person's entries, newest first, and whether the memory is on.
    pub async fn personal_memory(&self, user_id: &str) -> Result<PersonalMemory> {
        let rows = sqlx::query(
            "SELECT id, workspace_id, text, created_at, updated_at FROM agent_personal_memory WHERE user_id = ?1 ORDER BY updated_at DESC, id",
        )
        .bind(user_id)
        .fetch_all(self.reader())
        .await?;
        let enabled: Option<i64> =
            sqlx::query_scalar("SELECT enabled FROM agent_personal_memory_settings WHERE user_id = ?1")
                .bind(user_id)
                .fetch_optional(self.reader())
                .await?;
        Ok(PersonalMemory {
            enabled: enabled.is_none_or(|value| value != 0),
            entries: rows.iter().map(entry_from).collect(),
        })
    }

    /// Adds an entry: at most 40 a person and 280 characters each. `workspace_id` limits it to one workspace; none means everywhere.
    pub async fn add_personal_memory(
        &self,
        user_id: &str,
        workspace_id: Option<&str>,
        text: &str,
        source_turn_id: Option<&str>,
    ) -> Result<PersonalMemoryEntry> {
        let text = clean_entry(text)?;
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_personal_memory WHERE user_id = ?1")
            .bind(user_id)
            .fetch_one(self.reader())
            .await?;
        let cap = self.personal_memory_cap(user_id).await?;
        if count as usize >= cap {
            repomemo_domain::bail_conflict!(
                "CAIRN remembers at most {cap} things about you. Delete one in your profile first."
            );
        }
        let duplicate: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM agent_personal_memory WHERE user_id = ?1 AND lower(text) = lower(?2)",
        )
        .bind(user_id)
        .bind(&text)
        .fetch_one(self.reader())
        .await?;
        if duplicate > 0 {
            repomemo_domain::bail_conflict!("CAIRN already remembers that about you.");
        }
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO agent_personal_memory (id, user_id, workspace_id, text, source_turn_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
        )
        .bind(&id)
        .bind(user_id)
        .bind(workspace_id)
        .bind(&text)
        .bind(source_turn_id)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(PersonalMemoryEntry {
            id,
            workspace_id: workspace_id.map(str::to_owned),
            text,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    /// Rewrites one of the person's entries.
    pub async fn update_personal_memory(&self, user_id: &str, entry_id: &str, text: &str) -> Result<()> {
        let text = clean_entry(text)?;
        let done =
            sqlx::query("UPDATE agent_personal_memory SET text = ?3, updated_at = ?4 WHERE id = ?1 AND user_id = ?2")
                .bind(entry_id)
                .bind(user_id)
                .bind(text)
                .bind(Utc::now().to_rfc3339())
                .execute(&self.pool)
                .await?;
        if done.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("That entry was not found.");
        }
        Ok(())
    }

    /// Deletes one of the person's entries.
    pub async fn delete_personal_memory(&self, user_id: &str, entry_id: &str) -> Result<()> {
        let done = sqlx::query("DELETE FROM agent_personal_memory WHERE id = ?1 AND user_id = ?2")
            .bind(entry_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        if done.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("That entry was not found.");
        }
        Ok(())
    }

    /// Deletes everything CAIRN remembers about the person, and says how many entries went.
    pub async fn delete_all_personal_memory(&self, user_id: &str) -> Result<u64> {
        let done = sqlx::query("DELETE FROM agent_personal_memory WHERE user_id = ?1")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(done.rows_affected())
    }

    /// Turns the person's memory on or off. Off, it is neither read into their runs nor added to.
    pub async fn set_personal_memory_enabled(&self, user_id: &str, enabled: bool) -> Result<()> {
        sqlx::query(
            "INSERT INTO agent_personal_memory_settings (user_id, enabled) VALUES (?1, ?2) ON CONFLICT(user_id) DO UPDATE SET enabled = excluded.enabled",
        )
        .bind(user_id)
        .bind(i64::from(enabled))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// What goes into one person's run in one workspace: their entries that apply there, newest first, none when the memory is off.
    pub async fn personal_memory_for_run(&self, user_id: &str, workspace_id: &str) -> Result<Vec<String>> {
        let memory = self.personal_memory(user_id).await?;
        if !memory.enabled || !self.personal_memory_policy_for_workspace(workspace_id).await?.enabled {
            return Ok(Vec::new());
        }
        Ok(memory
            .entries
            .into_iter()
            .filter(|entry| entry.workspace_id.as_deref().is_none_or(|only| only == workspace_id))
            .map(|entry| entry.text)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::clean_entry;

    #[test]
    fn an_entry_is_one_line_and_bounded() {
        assert_eq!(
            clean_entry("  I prefer\nanswers in French ").unwrap(),
            "I prefer answers in French"
        );
        assert!(clean_entry("   ").is_err());
        assert!(clean_entry(&"a".repeat(280)).is_ok());
        assert!(clean_entry(&"a".repeat(281)).is_err());
    }
}
