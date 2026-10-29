//! Local multi-factor sign-in: the sealed TOTP secret, the replay guard and the single-use recovery codes.
use super::*;

impl StorageEngine {
    /// Whether the account has confirmed MFA, and how many recovery codes are still unused.
    pub async fn mfa_status(&self, user_id: &str) -> Result<(bool, i64)> {
        let enabled: Option<i64> = sqlx::query_scalar("SELECT enabled FROM user_mfa WHERE user_id = ?1")
            .bind(user_id)
            .fetch_optional(self.reader())
            .await?;
        let remaining: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM user_recovery_codes WHERE user_id = ?1 AND used_at IS NULL")
                .bind(user_id)
                .fetch_one(self.reader())
                .await?;
        Ok((enabled == Some(1), remaining))
    }

    /// Begins (or restarts) an enrollment with a new secret. An account that already has MFA must disable it first.
    pub async fn begin_mfa(&self, user_id: &str, secret: &str) -> Result<()> {
        let sealed = self.secrets.seal(secret, &format!("user_mfa:{user_id}"))?;
        let changed = sqlx::query("INSERT INTO user_mfa (user_id, secret, enabled, last_step, created_at) VALUES (?1, ?2, 0, 0, ?3) ON CONFLICT(user_id) DO UPDATE SET secret = excluded.secret, last_step = 0, created_at = excluded.created_at WHERE user_mfa.enabled = 0")
            .bind(user_id)
            .bind(sealed)
            .bind(Utc::now().to_rfc3339())
            .execute(&self.pool)
            .await?;
        if changed.rows_affected() == 0 {
            repomemo_domain::bail_conflict!("Two-step sign-in is already on. Turn it off before setting it up again.");
        }
        Ok(())
    }

    /// The secret, whether it is confirmed and the newest accepted step.
    pub async fn mfa_secret(&self, user_id: &str) -> Result<Option<(String, bool, i64)>> {
        let row: Option<(String, i64, i64)> =
            sqlx::query_as("SELECT secret, enabled, last_step FROM user_mfa WHERE user_id = ?1")
                .bind(user_id)
                .fetch_optional(self.reader())
                .await?;
        row.map(|(sealed, enabled, step)| {
            Ok((
                self.secrets.open(&sealed, &format!("user_mfa:{user_id}"))?,
                enabled == 1,
                step,
            ))
        })
        .transpose()
    }

    /// Accepts a code's time step once: false when that step, or a later one, was already used.
    pub async fn accept_mfa_step(&self, user_id: &str, step: i64) -> Result<bool> {
        Ok(
            sqlx::query("UPDATE user_mfa SET last_step = ?1 WHERE user_id = ?2 AND last_step < ?1")
                .bind(step)
                .bind(user_id)
                .execute(&self.pool)
                .await?
                .rows_affected()
                == 1,
        )
    }

    /// Turns MFA on after a first valid code, with these recovery code hashes. Returns false if there was no pending enrollment.
    pub async fn enable_mfa(&self, user_id: &str, step: i64, code_hashes: &[String]) -> Result<bool> {
        let mut tx = self.pool.begin().await?;
        let changed = sqlx::query("UPDATE user_mfa SET enabled = 1, last_step = ?1, confirmed_at = ?2 WHERE user_id = ?3 AND enabled = 0 AND last_step < ?1")
            .bind(step)
            .bind(Utc::now().to_rfc3339())
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        if changed.rows_affected() == 0 {
            return Ok(false);
        }
        replace_codes(&mut tx, user_id, code_hashes).await?;
        tx.commit().await?;
        Ok(true)
    }

    /// Replaces every recovery code: the old ones, used or not, stop working.
    pub async fn replace_recovery_codes(&self, user_id: &str, code_hashes: &[String]) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        replace_codes(&mut tx, user_id, code_hashes).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Uses a recovery code: true once, false for an unknown or already used one.
    pub async fn use_recovery_code(&self, user_id: &str, code_hash: &str) -> Result<bool> {
        Ok(sqlx::query(
            "UPDATE user_recovery_codes SET used_at = ?1 WHERE user_id = ?2 AND code_hash = ?3 AND used_at IS NULL",
        )
        .bind(Utc::now().to_rfc3339())
        .bind(user_id)
        .bind(code_hash)
        .execute(&self.pool)
        .await?
        .rows_affected()
            == 1)
    }

    /// Removes MFA and its recovery codes. Returns whether the account had any.
    pub async fn disable_mfa(&self, user_id: &str) -> Result<bool> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM user_recovery_codes WHERE user_id = ?1")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        let had = sqlx::query("DELETE FROM user_mfa WHERE user_id = ?1")
            .bind(user_id)
            .execute(&mut *tx)
            .await?
            .rows_affected()
            > 0;
        tx.commit().await?;
        Ok(had)
    }
}

async fn replace_codes(tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>, user_id: &str, hashes: &[String]) -> Result<()> {
    sqlx::query("DELETE FROM user_recovery_codes WHERE user_id = ?1")
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
    for hash in hashes {
        sqlx::query("INSERT INTO user_recovery_codes (user_id, code_hash) VALUES (?1, ?2)")
            .bind(user_id)
            .bind(hash)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

/// The stored form of a recovery code: case and dashes do not matter.
pub fn recovery_code_hash(code: &str) -> String {
    let normalized: String = code
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .collect();
    hash_refresh_token(&normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mfa_secrets_are_sealed_steps_and_recovery_codes_work_once() {
        let dir = std::env::temp_dir().join(format!("repomemo-mfa-{}", Uuid::new_v4()));
        let storage = StorageEngine::open(StorageConfig::new(dir)).await.unwrap();
        let user = storage.create_user("mfa@example.com", "M", "hash").await.unwrap();
        assert_eq!(storage.mfa_status(&user.id).await.unwrap(), (false, 0));
        storage.begin_mfa(&user.id, "JBSWY3DPEHPK3PXP").await.unwrap();
        // Pending enrollment is not enforcement.
        assert_eq!(storage.mfa_status(&user.id).await.unwrap(), (false, 0));
        let (secret, enabled, _) = storage.mfa_secret(&user.id).await.unwrap().unwrap();
        assert_eq!((secret.as_str(), enabled), ("JBSWY3DPEHPK3PXP", false));
        let raw: String = sqlx::query_scalar("SELECT secret FROM user_mfa WHERE user_id = ?1")
            .bind(&user.id)
            .fetch_one(&storage.pool)
            .await
            .unwrap();
        assert!(!raw.contains("JBSWY3DPEHPK3PXP"));
        let hashes = vec![recovery_code_hash("AAAAA-BBBBB"), recovery_code_hash("ccccc-ddddd")];
        assert!(storage.enable_mfa(&user.id, 100, &hashes).await.unwrap());
        assert!(!storage.enable_mfa(&user.id, 101, &hashes).await.unwrap());
        assert_eq!(storage.mfa_status(&user.id).await.unwrap(), (true, 2));
        assert!(storage.begin_mfa(&user.id, "OTHER").await.is_err());
        // A step is accepted once, and never an older one.
        assert!(!storage.accept_mfa_step(&user.id, 100).await.unwrap());
        assert!(storage.accept_mfa_step(&user.id, 101).await.unwrap());
        assert!(!storage.accept_mfa_step(&user.id, 101).await.unwrap());
        assert!(!storage.accept_mfa_step(&user.id, 99).await.unwrap());
        // A recovery code works once, whatever its case or dashes.
        assert!(storage
            .use_recovery_code(&user.id, &recovery_code_hash("aaaaabbbbb"))
            .await
            .unwrap());
        assert!(!storage
            .use_recovery_code(&user.id, &recovery_code_hash("AAAAA-BBBBB"))
            .await
            .unwrap());
        assert!(!storage
            .use_recovery_code(&user.id, &recovery_code_hash("ZZZZZ-ZZZZZ"))
            .await
            .unwrap());
        assert_eq!(storage.mfa_status(&user.id).await.unwrap(), (true, 1));
        storage
            .replace_recovery_codes(&user.id, &[recovery_code_hash("NEWCO-DE123")])
            .await
            .unwrap();
        assert!(!storage
            .use_recovery_code(&user.id, &recovery_code_hash("ccccc-ddddd"))
            .await
            .unwrap());
        assert!(storage.disable_mfa(&user.id).await.unwrap());
        assert_eq!(storage.mfa_status(&user.id).await.unwrap(), (false, 0));
        assert!(!storage.disable_mfa(&user.id).await.unwrap());
    }
}
