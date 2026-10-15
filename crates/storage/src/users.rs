use super::*;

impl StorageEngine {
    /// Ends every session of every user (access tokens stop being accepted,
    /// refresh tokens are revoked). Returns how many accounts were affected.
    pub async fn end_all_sessions(&self) -> Result<u64> {
        let mut tx = self.pool.begin().await?;
        let accounts = sqlx::query("UPDATE users SET session_version = session_version + 1")
            .execute(&mut *tx)
            .await?
            .rows_affected();
        sqlx::query("UPDATE refresh_tokens SET revoked_at = ?1 WHERE revoked_at IS NULL")
            .bind(Utc::now().to_rfc3339())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(accounts)
    }

    pub async fn create_user(&self, email: &str, display_name: &str, password_hash: &str) -> Result<SharedUser> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let email = normalize_email(email)?;

        sqlx::query(
            "INSERT INTO users (id, email, display_name, password_hash, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(&id)
        .bind(&email)
        .bind(display_name.trim())
        .bind(password_hash)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(SharedUser {
            id,
            email: Some(email),
            display_name: display_name.trim().to_owned(),
        })
    }

    pub async fn find_user_for_auth(&self, email: &str) -> Result<Option<StoredUser>> {
        let email = normalize_email(email)?;
        let row =
            sqlx::query_as::<_, UserRow>("SELECT id, email, display_name, password_hash FROM users WHERE email = ?1")
                .bind(email)
                .fetch_optional(self.reader())
                .await?;

        Ok(row.map(StoredUser::from))
    }

    pub async fn find_user(&self, user_id: &str) -> Result<Option<SharedUser>> {
        let row =
            sqlx::query_as::<_, UserRow>("SELECT id, email, display_name, password_hash FROM users WHERE id = ?1")
                .bind(user_id)
                .fetch_optional(self.reader())
                .await?;

        Ok(row.map(|row| StoredUser::from(row).user))
    }

    pub async fn find_user_by_email(&self, email: &str) -> Result<Option<SharedUser>> {
        let email = normalize_email(email)?;
        let row =
            sqlx::query_as::<_, UserRow>("SELECT id, email, display_name, password_hash FROM users WHERE email = ?1")
                .bind(email)
                .fetch_optional(self.reader())
                .await?;
        Ok(row.map(|row| StoredUser::from(row).user))
    }

    pub async fn get_user_profile(&self, user_id: &str) -> Result<Option<UserProfile>> {
        let row = sqlx::query_as::<_, UserProfileRow>(
            "SELECT id, email, display_name, created_at, updated_at, last_connected_at FROM users WHERE id = ?1",
        )
        .bind(user_id)
        .fetch_optional(self.reader())
        .await?;
        Ok(row.map(UserProfile::from))
    }

    /// Stores (or replaces) the user's profile picture. The caller has already
    /// validated the image.
    pub async fn set_user_avatar(&self, user_id: &str, content_type: &str, data: &[u8]) -> Result<()> {
        sqlx::query(
            "INSERT INTO user_avatars (user_id, content_type, data, updated_at) VALUES (?1, ?2, ?3, ?4)              ON CONFLICT(user_id) DO UPDATE SET content_type = excluded.content_type, data = excluded.data, updated_at = excluded.updated_at",
        )
        .bind(user_id)
        .bind(content_type)
        .bind(data)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// The user's profile picture as `(content type, bytes)`, if they have one.
    pub async fn get_user_avatar(&self, user_id: &str) -> Result<Option<(String, Vec<u8>)>> {
        let row =
            sqlx::query_as::<_, (String, Vec<u8>)>("SELECT content_type, data FROM user_avatars WHERE user_id = ?1")
                .bind(user_id)
                .fetch_optional(self.reader())
                .await?;
        Ok(row)
    }

    pub async fn delete_user_avatar(&self, user_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM user_avatars WHERE user_id = ?1")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Whether `viewer` may see `target`'s picture: themselves, or someone who
    /// shares an organization or a workspace with them.
    pub async fn users_share_a_group(&self, viewer: &str, target: &str) -> Result<bool> {
        let shared: i64 = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM organization_memberships a JOIN organization_memberships b ON b.organization_id = a.organization_id WHERE a.user_id = ?1 AND b.user_id = ?2)                  OR EXISTS (SELECT 1 FROM workspace_memberships a JOIN workspace_memberships b ON b.workspace_id = a.workspace_id WHERE a.user_id = ?1 AND b.user_id = ?2)",
        )
        .bind(viewer)
        .bind(target)
        .fetch_one(self.reader())
        .await?;
        Ok(shared != 0)
    }

    pub async fn update_user_display_name(&self, user_id: &str, display_name: &str) -> Result<SharedUser> {
        let display_name = require_name(display_name, "Display name")?;
        let now = Utc::now().to_rfc3339();
        let changed = sqlx::query("UPDATE users SET display_name = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(&display_name)
            .bind(now)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        if changed.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("User was not found.");
        }
        self.find_user(user_id)
            .await?
            .ok_or_else(|| repomemo_domain::not_found!("User was not found."))
    }

    pub async fn update_user_password(&self, user_id: &str, password_hash: &str) -> Result<()> {
        let changed = sqlx::query("UPDATE users SET password_hash = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(password_hash)
            .bind(Utc::now().to_rfc3339())
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        if changed.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("User was not found.");
        }
        Ok(())
    }

    /// Issues a new opaque refresh token for the user and returns the raw value (only its hash is stored).
    pub async fn create_refresh_token(&self, user_id: &str, ttl_days: i64) -> Result<String> {
        let raw = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let now = Utc::now();
        sqlx::query("INSERT INTO refresh_tokens (token_hash, user_id, created_at, expires_at) VALUES (?1, ?2, ?3, ?4)")
            .bind(hash_refresh_token(&raw))
            .bind(user_id)
            .bind(now.to_rfc3339())
            .bind((now + chrono::Duration::days(ttl_days)).to_rfc3339())
            .execute(&self.pool)
            .await?;
        Ok(raw)
    }

    /// Consumes a refresh token and returns its user id, or `None` if it is unknown, expired or revoked.
    /// Reusing an already-revoked token revokes every refresh token of that user.
    pub async fn consume_refresh_token(&self, raw: &str) -> Result<Option<String>> {
        let hash = hash_refresh_token(raw);
        let now = Utc::now().to_rfc3339();
        let row: Option<(String, String, Option<String>)> =
            sqlx::query_as("SELECT user_id, expires_at, revoked_at FROM refresh_tokens WHERE token_hash = ?1")
                .bind(&hash)
                .fetch_optional(self.reader())
                .await?;
        let Some((user_id, expires_at, revoked_at)) = row else {
            return Ok(None);
        };
        if revoked_at.is_some() {
            self.revoke_user_refresh_tokens(&user_id).await?;
            return Ok(None);
        }
        // Conditional update makes concurrent rotation of the same token race-safe.
        let changed =
            sqlx::query("UPDATE refresh_tokens SET revoked_at = ?1 WHERE token_hash = ?2 AND revoked_at IS NULL")
                .bind(&now)
                .bind(&hash)
                .execute(&self.pool)
                .await?;
        if changed.rows_affected() == 0 || expires_at < now {
            return Ok(None);
        }
        Ok(Some(user_id))
    }

    /// The user a refresh token belongs to while it is still usable, without
    /// using it up. For telling the account's own device apart at sign-in.
    pub async fn refresh_token_user(&self, raw: &str) -> Result<Option<String>> {
        let now = Utc::now().to_rfc3339();
        Ok(sqlx::query_scalar::<_, String>(
            "SELECT user_id FROM refresh_tokens WHERE token_hash = ?1 AND revoked_at IS NULL AND expires_at > ?2",
        )
        .bind(hash_refresh_token(raw))
        .bind(now)
        .fetch_optional(self.reader())
        .await?)
    }

    pub async fn revoke_refresh_token(&self, raw: &str) -> Result<()> {
        sqlx::query("UPDATE refresh_tokens SET revoked_at = ?1 WHERE token_hash = ?2 AND revoked_at IS NULL")
            .bind(Utc::now().to_rfc3339())
            .bind(hash_refresh_token(raw))
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn revoke_user_refresh_tokens(&self, user_id: &str) -> Result<()> {
        sqlx::query("UPDATE refresh_tokens SET revoked_at = ?1 WHERE user_id = ?2 AND revoked_at IS NULL")
            .bind(Utc::now().to_rfc3339())
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The version stamped on the user's access tokens, or `None` when the
    /// user no longer exists or is deactivated.
    pub async fn user_session_version(&self, user_id: &str) -> Result<Option<i64>> {
        Ok(
            sqlx::query_scalar::<_, i64>("SELECT session_version FROM users WHERE id = ?1 AND deactivated_at IS NULL")
                .bind(user_id)
                .fetch_optional(self.reader())
                .await?,
        )
    }

    /// Ends every session of a user: access tokens issued so far stop being
    /// accepted and every refresh token is revoked. Returns the new session
    /// version, to stamp on a session issued right after.
    pub async fn end_user_sessions(&self, user_id: &str) -> Result<i64> {
        let mut tx = self.pool.begin().await?;
        let version = sqlx::query_scalar::<_, i64>(
            "UPDATE users SET session_version = session_version + 1 WHERE id = ?1 RETURNING session_version",
        )
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| repomemo_domain::not_found!("User was not found."))?;
        sqlx::query("UPDATE refresh_tokens SET revoked_at = ?1 WHERE user_id = ?2 AND revoked_at IS NULL")
            .bind(Utc::now().to_rfc3339())
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(version)
    }

    pub async fn count_users(&self) -> Result<i64> {
        Ok(sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users")
            .fetch_one(self.reader())
            .await?)
    }

    /// Deletes refresh tokens that expired before `cutoff`. Revoked tokens are
    /// kept until they expire, because presenting one again is how a stolen
    /// token is detected.
    pub async fn purge_expired_refresh_tokens(&self, cutoff: &str) -> Result<u64> {
        Ok(sqlx::query("DELETE FROM refresh_tokens WHERE expires_at < ?1")
            .bind(cutoff)
            .execute(&self.pool)
            .await?
            .rows_affected())
    }

    pub async fn touch_user_connection(&self, user_id: &str) -> Result<()> {
        sqlx::query("UPDATE users SET last_connected_at = ?1 WHERE id = ?2")
            .bind(Utc::now().to_rfc3339())
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
