use super::*;

/// Public configuration only. Credentials are sealed separately and never serialized.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default, PartialEq)]
pub struct IdentityProvider {
    pub name: String,
    pub issuer: String,
    pub client_id: String,
    pub enabled: bool,
    #[serde(default)]
    pub allowed_domains: Vec<String>,
    #[serde(default)]
    pub authorization_endpoint: String,
    #[serde(default)]
    pub token_endpoint: String,
    #[serde(default)]
    pub jwks_uri: String,
    #[serde(default)]
    pub end_session_endpoint: Option<String>,
    #[serde(default)]
    pub checked_at: Option<String>,
    /// Password sign-in is refused for everyone except the break-glass accounts.
    #[serde(default)]
    pub enforced: bool,
    /// User ids of full system administrators who keep password sign-in while it is enforced.
    #[serde(default)]
    pub break_glass: Vec<String>,
    /// Name of the ID-token claim holding the person's groups.
    #[serde(default)]
    pub group_claim: String,
    #[serde(default)]
    pub group_mappings: Vec<GroupMapping>,
}

/// An allowlisted provider group and the one membership it grants. Ownership is never mapped.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct GroupMapping {
    pub group: String,
    /// `organization` or `workspace`.
    pub scope: String,
    pub target_id: String,
    pub role: String,
}

#[derive(Debug, Clone, serde::Serialize, Default, PartialEq)]
pub struct GroupChange {
    pub scope: String,
    pub target_id: String,
    pub role: String,
    pub group: String,
}

/// What a group sync adds, changes, removes or leaves alone for one person.
#[derive(Debug, Clone, serde::Serialize, Default, PartialEq)]
pub struct GroupSyncPlan {
    pub add: Vec<GroupChange>,
    pub change: Vec<GroupChange>,
    pub remove: Vec<GroupChange>,
    /// Wanted by a group, but the person already holds a membership no sync created; it is left as it is.
    pub keep: Vec<GroupChange>,
}

impl GroupSyncPlan {
    pub fn is_empty(&self) -> bool {
        self.add.is_empty() && self.change.is_empty() && self.remove.is_empty()
    }
}

pub fn valid_group_role(scope: &str, role: &str) -> bool {
    match scope {
        "organization" => matches!(role, "admin" | "member"),
        "workspace" => matches!(role, "admin" | "member" | "viewer"),
        _ => false,
    }
}

impl StorageEngine {
    pub async fn link_identity(&self, issuer: &str, subject: &str, user: &str, version: i64) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let inserted=sqlx::query("INSERT INTO linked_identities (issuer,subject,user_id,created_at) SELECT ?1,?2,id,?3 FROM users WHERE id=?4 AND session_version=?5 AND deactivated_at IS NULL ON CONFLICT DO UPDATE SET confirmed=1 WHERE linked_identities.user_id=excluded.user_id AND linked_identities.subject=excluded.subject AND linked_identities.issuer=excluded.issuer")
            .bind(issuer).bind(subject).bind(Utc::now().to_rfc3339()).bind(user).bind(version).execute(&mut *tx).await?;
        if inserted.rows_affected() != 1 {
            repomemo_domain::bail_conflict!(
                "The identity or account is already linked, or the authorizing session ended. No link was changed."
            );
        }
        sqlx::query("INSERT INTO system_audit_events (id,created_at,actor_user_id,action,detail) VALUES (?1,?2,?3,'identity_linked','Provider identity linked after explicit account proof')")
            .bind(Uuid::new_v4().to_string()).bind(Utc::now().to_rfc3339()).bind(user).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    /// New-account enrollment is insert-only. A concurrent email registration cannot be linked.
    pub async fn create_identity_user(
        &self,
        issuer: &str,
        subject: &str,
        email: &str,
        name: &str,
        password_hash: &str,
    ) -> Result<SharedUser> {
        let email = normalize_new_email(email)?;
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        let inserted=sqlx::query("INSERT INTO users (id,email,display_name,password_hash,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?5) ON CONFLICT DO NOTHING")
            .bind(&id).bind(&email).bind(name).bind(password_hash).bind(&now).execute(&mut *tx).await?;
        if inserted.rows_affected() != 1 {
            repomemo_domain::bail_conflict!(
                "An account already uses this email. Sign in to that account and explicitly link the provider."
            );
        }
        sqlx::query(
            "INSERT INTO linked_identities (issuer,subject,user_id,created_at,confirmed) VALUES (?1,?2,?3,?4,0)",
        )
        .bind(issuer)
        .bind(subject)
        .bind(&id)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(SharedUser {
            id,
            email: Some(email),
            display_name: name.into(),
        })
    }

    pub async fn identity_links(&self, user: &str) -> Result<Vec<(String, String)>> {
        Ok(
            sqlx::query_as("SELECT issuer,created_at FROM linked_identities WHERE user_id=?1 AND confirmed=1")
                .bind(user)
                .fetch_all(self.reader())
                .await?,
        )
    }

    pub async fn confirm_identity_enrollment(&self, user: &str) -> Result<()> {
        sqlx::query("UPDATE linked_identities SET confirmed=1 WHERE user_id=?1")
            .bind(user)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    pub async fn linked_identity_user(&self, issuer: &str, subject: &str) -> Result<Option<String>> {
        Ok(sqlx::query_scalar("SELECT i.user_id FROM linked_identities i JOIN users u ON u.id=i.user_id WHERE issuer=?1 AND subject=?2 AND confirmed=1 AND u.deactivated_at IS NULL")
            .bind(issuer).bind(subject).fetch_optional(self.reader()).await?)
    }

    pub async fn create_identity_flow(&self, state: &str, proof: &str, payload: &Value) -> Result<()> {
        let key = identity_hash(state);
        let sealed = self
            .secrets
            .seal(&payload.to_string(), &format!("identity_flow:{key}"))?;
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM identity_flows WHERE expires_at<=?1")
            .bind(Utc::now().timestamp())
            .execute(&mut *tx)
            .await?;
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM identity_flows")
            .fetch_one(&mut *tx)
            .await?;
        if count >= 1000 {
            repomemo_domain::bail_invalid!("Too many sign-ins are pending. Try again shortly.");
        }
        sqlx::query("INSERT INTO identity_flows VALUES (?1,?2,?3,'pending',?4)")
            .bind(key)
            .bind(identity_hash(proof))
            .bind(sealed)
            .bind(Utc::now().timestamp() + 300)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Claim the callback atomically before any network call; a replay cannot exchange twice.
    pub async fn take_identity_callback(&self, state: &str) -> Result<Option<Value>> {
        let key = identity_hash(state);
        let sealed: Option<String> = sqlx::query_scalar("UPDATE identity_flows SET phase='exchanging' WHERE state_hash=?1 AND phase='pending' AND expires_at>?2 RETURNING payload")
            .bind(&key).bind(Utc::now().timestamp()).fetch_optional(&self.pool).await?;
        sealed
            .map(|s| {
                Ok(serde_json::from_str(
                    &self.secrets.open(&s, &format!("identity_flow:{key}"))?,
                )?)
            })
            .transpose()
    }

    pub async fn finish_identity_flow(&self, state: &str, result: &Value, success: bool) -> Result<()> {
        let key = identity_hash(state);
        let sealed = self
            .secrets
            .seal(&result.to_string(), &format!("identity_flow:{key}"))?;
        sqlx::query("UPDATE identity_flows SET payload=?1, phase=?2 WHERE state_hash=?3 AND phase='exchanging'")
            .bind(sealed)
            .bind(if success { "done" } else { "failed" })
            .bind(key)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Only the initiating client knows the proof. Terminal results are single-use.
    pub async fn complete_identity_flow(&self, state: &str, proof: &str) -> Result<Option<(String, Value)>> {
        let key = identity_hash(state);
        let mut tx = self.pool.begin().await?;
        let row: Option<(String,String)> = sqlx::query_as("DELETE FROM identity_flows WHERE state_hash=?1 AND proof_hash=?2 AND phase IN ('done','failed') AND expires_at>?3 RETURNING phase,payload")
            .bind(&key).bind(identity_hash(proof)).bind(Utc::now().timestamp()).fetch_optional(&mut *tx).await?;
        if let Some((phase, payload)) = row {
            tx.commit().await?;
            return Ok(Some((
                phase,
                serde_json::from_str(&self.secrets.open(&payload, &format!("identity_flow:{key}"))?)?,
            )));
        }
        let pending: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM identity_flows WHERE state_hash=?1 AND proof_hash=?2 AND phase IN ('pending','exchanging') AND expires_at>?3")
            .bind(key).bind(identity_hash(proof)).bind(Utc::now().timestamp()).fetch_one(&mut *tx).await?;
        tx.commit().await?;
        Ok((pending == 1).then(|| ("pending".into(), Value::Null)))
    }
    /// The highest wanted role per target for the groups a person holds; ownership is never granted.
    fn wanted_grants(mappings: &[GroupMapping], groups: &[String]) -> Vec<GroupChange> {
        let rank = |r: &str| match r {
            "admin" => 3,
            "member" => 2,
            _ => 1,
        };
        let mut wanted: Vec<GroupChange> = Vec::new();
        for m in mappings
            .iter()
            .filter(|m| valid_group_role(&m.scope, &m.role) && groups.contains(&m.group))
        {
            match wanted
                .iter_mut()
                .find(|w| w.scope == m.scope && w.target_id == m.target_id)
            {
                Some(w) if rank(&m.role) > rank(&w.role) => {
                    w.role = m.role.clone();
                    w.group = m.group.clone();
                }
                Some(_) => {}
                None => wanted.push(GroupChange {
                    scope: m.scope.clone(),
                    target_id: m.target_id.clone(),
                    role: m.role.clone(),
                    group: m.group.clone(),
                }),
            }
        }
        wanted
    }

    async fn membership_role(&self, user: &str, scope: &str, target: &str) -> Result<Option<String>> {
        let sql = if scope == "organization" {
            "SELECT role FROM organization_memberships WHERE user_id=?1 AND organization_id=?2"
        } else {
            "SELECT role FROM workspace_memberships WHERE user_id=?1 AND workspace_id=?2"
        };
        Ok(sqlx::query_scalar(sql)
            .bind(user)
            .bind(target)
            .fetch_optional(&self.pool)
            .await?)
    }

    /// Compares what the person's groups ask for with what earlier syncs granted; with `apply` it also does it.
    /// Direct grants are never touched, owners are never added, changed or removed, and a membership whose role
    /// someone changed after a sync granted it is released from the sync instead of being overwritten.
    pub async fn sync_group_grants(
        &self,
        user: &str,
        mappings: &[GroupMapping],
        groups: &[String],
        apply: bool,
    ) -> Result<GroupSyncPlan> {
        let email = self
            .find_user(user)
            .await?
            .and_then(|u| u.email)
            .ok_or_else(|| repomemo_domain::invalid!("The person has no email address."))?;
        let wanted = Self::wanted_grants(mappings, groups);
        let held: Vec<(String, String, String, String)> =
            sqlx::query_as("SELECT scope,target_id,role,group_name FROM identity_group_grants WHERE user_id=?1")
                .bind(user)
                .fetch_all(&self.pool)
                .await?;
        let mut plan = GroupSyncPlan::default();
        for w in &wanted {
            let current = self.membership_role(user, &w.scope, &w.target_id).await?;
            let grant = held.iter().find(|g| g.0 == w.scope && g.1 == w.target_id);
            match (current.as_deref(), grant) {
                (Some("owner"), _) => plan.keep.push(w.clone()),
                (None, _) => plan.add.push(w.clone()),
                (Some(role), Some(g)) if role == g.2 && role != w.role => plan.change.push(w.clone()),
                (Some(role), Some(g)) if role == g.2 => {}
                (Some(_), _) => plan.keep.push(w.clone()),
            }
        }
        for (scope, target, role, group) in &held {
            if wanted.iter().any(|w| &w.scope == scope && &w.target_id == target) {
                continue;
            }
            let current = self.membership_role(user, scope, target).await?;
            if current.as_deref() == Some(role.as_str()) {
                plan.remove.push(GroupChange {
                    scope: scope.clone(),
                    target_id: target.clone(),
                    role: role.clone(),
                    group: group.clone(),
                });
            }
        }
        if !apply {
            return Ok(plan);
        }
        for w in plan.add.iter().chain(plan.change.iter()) {
            let done = if w.scope == "organization" {
                let role = if w.role == "admin" {
                    OrganizationRole::Admin
                } else {
                    OrganizationRole::Member
                };
                self.upsert_organization_member(&w.target_id, &email, role)
                    .await
                    .is_ok()
            } else {
                let role = match w.role.as_str() {
                    "admin" => WorkspaceRole::Admin,
                    "viewer" => WorkspaceRole::Viewer,
                    _ => WorkspaceRole::Member,
                };
                self.upsert_workspace_member(&w.target_id, &email, role).await.is_ok()
            };
            if done {
                sqlx::query("INSERT INTO identity_group_grants (user_id,scope,target_id,role,group_name,created_at) VALUES (?1,?2,?3,?4,?5,?6) ON CONFLICT(user_id,scope,target_id) DO UPDATE SET role=excluded.role,group_name=excluded.group_name")
                    .bind(user).bind(&w.scope).bind(&w.target_id).bind(&w.role).bind(&w.group).bind(Utc::now().to_rfc3339())
                    .execute(&self.pool).await?;
            }
        }
        for r in &plan.remove {
            if r.scope == "organization" {
                let _ = self.remove_organization_member(&r.target_id, user).await;
            } else {
                let _ = self.remove_workspace_member(&r.target_id, user).await;
            }
        }
        // A row is kept only while its membership still exists with the role the sync gave it and a group still wants it.
        for (scope, target, role, _) in &held {
            let wanted_here = wanted.iter().any(|w| &w.scope == scope && &w.target_id == target);
            let current = self.membership_role(user, scope, target).await?;
            let changed_here = plan.change.iter().any(|c| &c.scope == scope && &c.target_id == target);
            if !wanted_here || (current.as_deref() != Some(role.as_str()) && !changed_here) {
                sqlx::query("DELETE FROM identity_group_grants WHERE user_id=?1 AND scope=?2 AND target_id=?3")
                    .bind(user)
                    .bind(scope)
                    .bind(target)
                    .execute(&self.pool)
                    .await?;
            }
        }
        Ok(plan)
    }

    pub async fn identity_provider(&self) -> Result<Option<IdentityProvider>> {
        let row: Option<String> = sqlx::query_scalar("SELECT configuration FROM identity_provider WHERE id=1")
            .fetch_optional(self.reader())
            .await?;
        row.map(|s| serde_json::from_str(&s).map_err(Into::into)).transpose()
    }

    pub async fn identity_secret(&self) -> Result<Option<String>> {
        let row: Option<String> = sqlx::query_scalar("SELECT client_secret FROM identity_provider WHERE id=1")
            .fetch_optional(self.reader())
            .await?;
        row.map(|s| self.secrets.open(&s, "identity_provider:1:client_secret"))
            .transpose()
    }

    pub async fn save_identity_provider(&self, provider: &IdentityProvider, secret: &str, actor: &str) -> Result<()> {
        let sealed = self.secrets.seal(secret, "identity_provider:1:client_secret")?;
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO identity_provider (id,configuration,client_secret,updated_at) VALUES (1,?1,?2,?3) ON CONFLICT(id) DO UPDATE SET configuration=excluded.configuration,client_secret=excluded.client_secret,updated_at=excluded.updated_at")
            .bind(serde_json::to_string(provider)?).bind(sealed).bind(Utc::now().to_rfc3339())
            .execute(&mut *tx).await?;
        sqlx::query("INSERT INTO system_audit_events (id,actor_user_id,action,detail,created_at) VALUES (?1,?2,'identity_provider_updated','Identity provider configuration changed',?3)")
            .bind(Uuid::new_v4().to_string()).bind(actor).bind(Utc::now().to_rfc3339())
            .execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
}

fn identity_hash(value: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(value.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[tokio::test]
    async fn group_sync_previews_keeps_direct_grants_and_never_touches_owners() {
        let dir = std::env::temp_dir().join(format!("repomemo-groups-{}", Uuid::new_v4()));
        let storage = StorageEngine::open(StorageConfig::new(dir)).await.unwrap();
        let owner = storage.create_user("owner@example.com", "O", "hash").await.unwrap();
        let b = storage.create_user("b@example.com", "B", "hash").await.unwrap();
        let c = storage.create_user("c@example.com", "C", "hash").await.unwrap();
        let org = storage.create_organization(&owner.id, "Org").await.unwrap();
        storage
            .upsert_organization_member(&org.id, "c@example.com", OrganizationRole::Admin)
            .await
            .unwrap();
        let map = |role: &str| {
            vec![GroupMapping {
                group: "eng".into(),
                scope: "organization".into(),
                target_id: org.id.clone(),
                role: role.into(),
            }]
        };
        let eng = vec!["eng".to_string()];
        let role = |user: &str| {
            let storage = storage.clone();
            let (user, org) = (user.to_owned(), org.id.clone());
            async move { storage.organization_role_for_user(&user, &org).await.unwrap() }
        };
        let preview = storage
            .sync_group_grants(&b.id, &map("member"), &eng, false)
            .await
            .unwrap();
        assert_eq!(preview.add.len(), 1);
        assert!(role(&b.id).await.is_none());
        storage
            .sync_group_grants(&b.id, &map("member"), &eng, true)
            .await
            .unwrap();
        assert_eq!(role(&b.id).await, Some(OrganizationRole::Member));
        let removal = storage
            .sync_group_grants(&b.id, &map("member"), &[], false)
            .await
            .unwrap();
        assert_eq!(removal.remove.len(), 1);
        storage
            .sync_group_grants(&b.id, &map("member"), &[], true)
            .await
            .unwrap();
        assert!(role(&b.id).await.is_none());
        let kept = storage
            .sync_group_grants(&c.id, &map("member"), &eng, true)
            .await
            .unwrap();
        assert_eq!(kept.keep.len(), 1);
        storage
            .sync_group_grants(&c.id, &map("member"), &[], true)
            .await
            .unwrap();
        assert_eq!(role(&c.id).await, Some(OrganizationRole::Admin));
        let owned = storage
            .sync_group_grants(&owner.id, &map("member"), &eng, true)
            .await
            .unwrap();
        assert_eq!(owned.keep.len(), 1);
        storage
            .sync_group_grants(&owner.id, &map("member"), &[], true)
            .await
            .unwrap();
        assert_eq!(role(&owner.id).await, Some(OrganizationRole::Owner));
    }
    #[tokio::test]
    async fn identity_linking_never_reassigns_email_or_subject() {
        let dir = std::env::temp_dir().join(format!("repomemo-link-{}", Uuid::new_v4()));
        let storage = StorageEngine::open(StorageConfig::new(dir)).await.unwrap();
        let a = storage.create_user("a@example.com", "A", "hash").await.unwrap();
        let b = storage.create_user("b@example.com", "B", "hash").await.unwrap();
        storage
            .link_identity("https://id.example.com", "subject-a", &a.id, 0)
            .await
            .unwrap();
        assert!(storage
            .link_identity("https://id.example.com", "subject-a", &b.id, 0)
            .await
            .is_err());
        assert!(storage
            .link_identity("https://id.example.com", "subject-b", &a.id, 0)
            .await
            .is_err());
        assert!(storage
            .create_identity_user(
                "https://id.example.com",
                "new-subject",
                "a@example.com",
                "Attacker",
                "hash"
            )
            .await
            .is_err());
        assert_eq!(
            storage
                .linked_identity_user("https://id.example.com", "subject-a")
                .await
                .unwrap(),
            Some(a.id)
        );
        storage.end_user_sessions(&b.id).await.unwrap();
        assert!(storage
            .link_identity("https://id.example.com", "subject-b", &b.id, 0)
            .await
            .is_err());
        let c = storage
            .create_identity_user("https://id.example.com", "subject-c", "c@example.com", "C", "hash")
            .await
            .unwrap();
        assert!(storage
            .linked_identity_user("https://id.example.com", "subject-c")
            .await
            .unwrap()
            .is_none());
        storage.confirm_identity_enrollment(&c.id).await.unwrap();
        assert_eq!(
            storage
                .linked_identity_user("https://id.example.com", "subject-c")
                .await
                .unwrap(),
            Some(c.id)
        );
        assert_eq!(storage.count_users().await.unwrap(), 3);
    }
    #[tokio::test]
    async fn identity_handoffs_refuse_replay_wrong_proof_and_expiry() {
        let dir = std::env::temp_dir().join(format!("repomemo-flow-{}", Uuid::new_v4()));
        let storage = StorageEngine::open(StorageConfig::new(dir)).await.unwrap();
        storage
            .create_identity_flow("state", "proof", &json!({"nonce":"nonce"}))
            .await
            .unwrap();
        assert!(storage
            .complete_identity_flow("state", "wrong")
            .await
            .unwrap()
            .is_none());
        assert_eq!(
            storage
                .complete_identity_flow("state", "proof")
                .await
                .unwrap()
                .unwrap()
                .0,
            "pending"
        );
        assert!(storage.take_identity_callback("wrong").await.unwrap().is_none());
        assert!(storage.take_identity_callback("state").await.unwrap().is_some());
        assert!(storage.take_identity_callback("state").await.unwrap().is_none());
        storage
            .finish_identity_flow("state", &json!({"user_id":"user"}), true)
            .await
            .unwrap();
        assert!(storage
            .complete_identity_flow("state", "wrong")
            .await
            .unwrap()
            .is_none());
        assert_eq!(
            storage
                .complete_identity_flow("state", "proof")
                .await
                .unwrap()
                .unwrap()
                .1["user_id"],
            "user"
        );
        assert!(storage
            .complete_identity_flow("state", "proof")
            .await
            .unwrap()
            .is_none());
        storage
            .create_identity_flow("expired", "proof", &Value::Null)
            .await
            .unwrap();
        sqlx::query("UPDATE identity_flows SET expires_at=0")
            .execute(&storage.pool)
            .await
            .unwrap();
        assert!(storage.take_identity_callback("expired").await.unwrap().is_none());
        assert!(storage
            .complete_identity_flow("expired", "proof")
            .await
            .unwrap()
            .is_none());
    }
    #[tokio::test]
    async fn identity_secret_is_sealed_and_configuration_survives_reopen() {
        let dir = std::env::temp_dir().join(format!("repomemo-identity-{}", Uuid::new_v4()));
        let storage = StorageEngine::open(StorageConfig::new(dir.clone())).await.unwrap();
        let user = storage
            .create_user("admin@example.com", "Admin", "unused")
            .await
            .unwrap();
        let provider = IdentityProvider {
            issuer: "https://id.example.com".into(),
            ..Default::default()
        };
        storage
            .save_identity_provider(&provider, "secret-to-seal", &user.id)
            .await
            .unwrap();
        let stored: String = sqlx::query_scalar("SELECT client_secret FROM identity_provider")
            .fetch_one(storage.reader())
            .await
            .unwrap();
        assert!(SecretBox::is_sealed(&stored));
        assert!(!stored.contains("secret-to-seal"));
        assert_eq!(
            storage.identity_secret().await.unwrap().as_deref(),
            Some("secret-to-seal")
        );
        assert_eq!(storage.identity_provider().await.unwrap(), Some(provider));
    }
}
