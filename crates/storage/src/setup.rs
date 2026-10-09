//! First-run setup of a new server: whether the onboarding still has to run,
//! the creation of its first system administrator, and its completion.
//!
//! The state only moves forward: new → finishing → complete. Completion is
//! permanent; nothing in the application resets it.

use anyhow::Result;
use chrono::Utc;
use repomemo_domain::SharedUser;
use sqlx::Row;
use uuid::Uuid;

use super::{normalize_email, StorageEngine};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetupState {
    /// No account exists yet: the onboarding starts by creating the first
    /// system administrator.
    New,
    /// The first system administrator exists but has not finished the
    /// onboarding.
    Finishing { admin_user_id: Option<String> },
    /// Set up; the onboarding never shows again.
    Complete,
}

impl SetupState {
    pub fn is_complete(&self) -> bool {
        matches!(self, Self::Complete)
    }
}

impl StorageEngine {
    pub async fn setup_state(&self) -> Result<SetupState> {
        let row = sqlx::query("SELECT admin_user_id, completed_at FROM server_setup WHERE id = 1")
            .fetch_optional(&self.pool)
            .await?;
        if let Some(row) = row {
            return Ok(match row.get::<Option<String>, _>("completed_at") {
                Some(_) => SetupState::Complete,
                None => SetupState::Finishing {
                    admin_user_id: row.get("admin_user_id"),
                },
            });
        }
        // Accounts without a setup record were created another way (setup
        // turned off, or an older version): the server is set up.
        if self.count_users().await? > 0 {
            self.complete_setup(None).await?;
            return Ok(SetupState::Complete);
        }
        Ok(SetupState::New)
    }

    /// Creates the first account as a system administrator and starts the
    /// setup record, in one transaction. Returns `None` when an account or a
    /// setup record already exists, so only one first administrator can ever
    /// be created this way.
    pub async fn create_setup_admin(
        &self,
        email: &str,
        display_name: &str,
        password_hash: &str,
    ) -> Result<Option<SharedUser>> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let email = normalize_email(email)?;
        let display_name = display_name.trim().to_owned();
        let mut tx = self.pool.begin().await?;
        let inserted = sqlx::query(
            r#"
            INSERT INTO users (id, email, display_name, password_hash, created_at, updated_at, is_system_admin)
            SELECT ?1, ?2, ?3, ?4, ?5, ?5, 1
            WHERE NOT EXISTS (SELECT 1 FROM users)
              AND NOT EXISTS (SELECT 1 FROM server_setup)
            "#,
        )
        .bind(&id)
        .bind(&email)
        .bind(&display_name)
        .bind(password_hash)
        .bind(&now)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if inserted == 0 {
            return Ok(None);
        }
        sqlx::query("INSERT INTO server_setup (id, admin_user_id, admin_created_at) VALUES (1, ?1, ?2)")
            .bind(&id)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(Some(SharedUser {
            id,
            email: Some(email),
            display_name,
        }))
    }

    /// Marks the server as set up. Returns false when it already was; the
    /// first completion is kept.
    pub async fn complete_setup(&self, completed_by_user_id: Option<&str>) -> Result<bool> {
        let changed = sqlx::query(
            r#"
            INSERT INTO server_setup (id, completed_at, completed_by_user_id) VALUES (1, ?1, ?2)
            ON CONFLICT(id) DO UPDATE SET completed_at = excluded.completed_at,
              completed_by_user_id = excluded.completed_by_user_id
            WHERE server_setup.completed_at IS NULL
            "#,
        )
        .bind(Utc::now().to_rfc3339())
        .bind(completed_by_user_id)
        .execute(&self.pool)
        .await?
        .rows_affected();
        Ok(changed > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::SetupState;
    use crate::{StorageConfig, StorageEngine};

    async fn storage() -> (StorageEngine, std::path::PathBuf) {
        let data_dir = std::env::temp_dir().join(format!("repomemo-setup-{}", uuid::Uuid::new_v4()));
        (StorageEngine::open(StorageConfig::new(data_dir.clone())).await.unwrap(), data_dir)
    }

    #[tokio::test]
    async fn setup_moves_forward_once_and_creates_one_first_admin() {
        let (storage, data_dir) = storage().await;
        assert_eq!(storage.setup_state().await.unwrap(), SetupState::New);

        let admin = storage.create_setup_admin("Admin@Example.com", "Admin", "hash").await.unwrap().unwrap();
        assert_eq!(admin.email.as_deref(), Some("admin@example.com"));
        assert!(storage.user_access(&admin.id).await.unwrap().unwrap().is_system_admin);
        assert_eq!(
            storage.setup_state().await.unwrap(),
            SetupState::Finishing { admin_user_id: Some(admin.id.clone()) }
        );
        assert!(storage.create_setup_admin("second@example.com", "Second", "hash").await.unwrap().is_none(), "only one first admin");

        assert!(storage.complete_setup(Some(&admin.id)).await.unwrap());
        assert_eq!(storage.setup_state().await.unwrap(), SetupState::Complete);
        assert!(!storage.complete_setup(Some(&admin.id)).await.unwrap(), "completion happens once");
        assert!(storage.create_setup_admin("third@example.com", "Third", "hash").await.unwrap().is_none());

        storage.pool.close().await;
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn servers_with_accounts_are_already_set_up() {
        let (storage, data_dir) = storage().await;
        storage.create_user("someone@example.com", "Someone", "hash").await.unwrap();
        assert_eq!(storage.setup_state().await.unwrap(), SetupState::Complete);
        assert!(storage.create_setup_admin("late@example.com", "Late", "hash").await.unwrap().is_none());
        storage.pool.close().await;
        let _ = std::fs::remove_dir_all(data_dir);
    }
}
