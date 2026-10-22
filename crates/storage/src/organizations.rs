use repomemo_domain::WorkspaceTemplateRef;

use super::*;

/// What a workspace made from a template starts with: the choice to save and the folders to make.
#[derive(Debug, Clone, PartialEq)]
pub struct StarterSetup {
    pub reference: WorkspaceTemplateRef,
    /// For the activity feed: "Created workspace X from the {name} template."
    pub template_name: String,
    /// Folders, each with the names of the folders one level below it.
    pub folders: Vec<(String, Vec<String>)>,
}

/// Inserts one starter folder inside the transaction that makes the workspace, with the checks a folder made by hand
/// has: a name that is valid and not already taken beside it.
async fn insert_starter_folder(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    workspace_id: &str,
    parent_id: Option<&str>,
    name: &str,
    created_by: &str,
    created_at: &str,
) -> Result<String> {
    let name = validate_folder_name(name)?;
    let duplicate: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM folders WHERE workspace_id = ?1 AND COALESCE(parent_id, '') = COALESCE(?2, '') AND name = ?3 COLLATE NOCASE",
    )
    .bind(workspace_id)
    .bind(parent_id)
    .bind(name)
    .fetch_optional(&mut **tx)
    .await?;
    if duplicate.is_some() {
        repomemo_domain::bail_conflict!("A folder with this name already exists here.");
    }
    let id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO folders (id, workspace_id, parent_id, name, created_by, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)")
        .bind(&id)
        .bind(workspace_id)
        .bind(parent_id)
        .bind(name)
        .bind(created_by)
        .bind(created_at)
        .execute(&mut **tx)
        .await?;
    Ok(id)
}

/// Key of the workspace's template in its settings object.
pub(crate) const TEMPLATE_SETTING: &str = "template";

impl StorageEngine {
    pub async fn workspace_exists(&self, workspace_id: &str) -> Result<bool> {
        let exists = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM workspaces
            WHERE id = ?1
            "#,
        )
        .bind(workspace_id)
        .fetch_one(self.reader())
        .await?;

        Ok(exists > 0)
    }

    pub async fn create_workspace(&self, name: &str) -> Result<Workspace> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let settings_json = "{}";

        sqlx::query(
            r#"
            INSERT INTO workspaces (id, name, created_at, updated_at, settings_json)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
        )
        .bind(&id)
        .bind(name.trim())
        .bind(&now)
        .bind(&now)
        .bind(settings_json)
        .execute(&self.pool)
        .await?;

        Ok(Workspace {
            id,
            name: name.trim().to_owned(),
            created_at: now.clone(),
            updated_at: now,
            settings: Value::Object(Default::default()),
        })
    }

    pub async fn list_workspaces(&self) -> Result<Vec<Workspace>> {
        let rows = sqlx::query_as::<_, WorkspaceRow>(
            r#"
            SELECT id, name, created_at, updated_at, settings_json
            FROM workspaces
            ORDER BY updated_at DESC, name ASC
            "#,
        )
        .fetch_all(self.reader())
        .await?;

        Ok(rows.into_iter().map(Workspace::from).collect())
    }

    pub async fn update_workspace_name(&self, workspace_id: &str, name: &str) -> Result<Workspace> {
        let name = require_name(name, "Workspace name")?;
        let now = Utc::now().to_rfc3339();
        let changed = sqlx::query("UPDATE workspaces SET name = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(&name)
            .bind(&now)
            .bind(workspace_id)
            .execute(&self.pool)
            .await?;
        if changed.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("Workspace was not found.");
        }
        let row = sqlx::query_as::<_, WorkspaceRow>(
            "SELECT id, name, created_at, updated_at, settings_json FROM workspaces WHERE id = ?1",
        )
        .bind(workspace_id)
        .fetch_one(self.reader())
        .await?;
        Ok(Workspace::from(row))
    }

    pub async fn delete_workspace(&self, workspace_id: &str) -> Result<()> {
        let changed = sqlx::query("DELETE FROM workspaces WHERE id = ?1")
            .bind(workspace_id)
            .execute(&self.pool)
            .await?;
        if changed.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("Workspace was not found.");
        }
        Ok(())
    }

    /// The workspace's settings object (`{}` when none were saved).
    pub async fn workspace_settings(&self, workspace_id: &str) -> Result<Value> {
        let settings_json = sqlx::query_scalar::<_, String>("SELECT settings_json FROM workspaces WHERE id = ?1")
            .bind(workspace_id)
            .fetch_optional(self.reader())
            .await?
            .ok_or_else(|| repomemo_domain::not_found!("Workspace was not found."))?;
        Ok(serde_json::from_str::<Value>(&settings_json)
            .ok()
            .filter(Value::is_object)
            .unwrap_or_else(|| Value::Object(Default::default())))
    }

    /// Sets one top-level key of the workspace's settings object, keeping the
    /// others. The read and write happen in one transaction.
    pub async fn set_workspace_setting(&self, workspace_id: &str, key: &str, value: Value) -> Result<Value> {
        let mut tx = self.pool.begin().await?;
        // Take the write lock first so a concurrent change is not lost.
        sqlx::query("UPDATE workspaces SET settings_json = settings_json WHERE id = ?1")
            .bind(workspace_id)
            .execute(&mut *tx)
            .await?;
        let settings_json = sqlx::query_scalar::<_, String>("SELECT settings_json FROM workspaces WHERE id = ?1")
            .bind(workspace_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| repomemo_domain::not_found!("Workspace was not found."))?;
        let mut settings = serde_json::from_str::<Value>(&settings_json)
            .ok()
            .filter(Value::is_object)
            .unwrap_or_else(|| Value::Object(Default::default()));
        settings[key] = value;
        sqlx::query("UPDATE workspaces SET settings_json = ?1 WHERE id = ?2")
            .bind(serde_json::to_string(&settings)?)
            .bind(workspace_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(settings)
    }

    /// The template the workspace was given (V0.2.166), or `None` when it has none, or one that is not shaped like a
    /// reference: such a workspace is on `general`.
    pub async fn workspace_template_ref(&self, workspace_id: &str) -> Result<Option<WorkspaceTemplateRef>> {
        let settings = self.workspace_settings(workspace_id).await?;
        Ok(settings
            .get(TEMPLATE_SETTING)
            .and_then(WorkspaceTemplateRef::from_setting))
    }

    /// Saves the workspace's template, or clears it (`None`), keeping the other settings.
    pub async fn set_workspace_template_ref(
        &self,
        workspace_id: &str,
        reference: Option<&WorkspaceTemplateRef>,
    ) -> Result<()> {
        let value = match reference {
            Some(reference) => serde_json::to_value(reference)?,
            None => Value::Null,
        };
        self.set_workspace_setting(workspace_id, TEMPLATE_SETTING, value)
            .await
            .map(|_| ())
    }

    pub async fn create_organization(&self, owner_id: &str, name: &str) -> Result<Organization> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let name = require_name(name, "Organization name")?;
        let mut tx = self.pool.begin().await?;

        sqlx::query("INSERT INTO organizations (id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)")
            .bind(&id)
            .bind(&name)
            .bind(&now)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO organization_memberships (organization_id, user_id, role, created_at) VALUES (?1, ?2, 'owner', ?3)",
        )
        .bind(&id)
        .bind(owner_id)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        Ok(Organization {
            id,
            name,
            role: OrganizationRole::Owner,
            created_at: now.clone(),
            updated_at: now,
            guest: false,
        })
    }

    /// The organizations a user is a member of, plus, as guests, those where
    /// they only belong to some workspaces, so those workspaces can be found.
    pub async fn list_organizations_for_user(&self, user_id: &str) -> Result<Vec<Organization>> {
        let rows = sqlx::query_as::<_, OrganizationRow>(
            "SELECT o.id, o.name, m.role, o.created_at, o.updated_at, 0 AS guest FROM organizations o INNER JOIN organization_memberships m ON m.organization_id = o.id WHERE m.user_id = ?1 \
             UNION ALL \
             SELECT o.id, o.name, 'member' AS role, o.created_at, o.updated_at, 1 AS guest FROM organizations o \
             WHERE EXISTS (SELECT 1 FROM workspace_organizations wo INNER JOIN workspace_memberships wm ON wm.workspace_id = wo.workspace_id WHERE wo.organization_id = o.id AND wm.user_id = ?1) \
             AND NOT EXISTS (SELECT 1 FROM organization_memberships m WHERE m.organization_id = o.id AND m.user_id = ?1) \
             ORDER BY 2 ASC",
        )
        .bind(user_id)
        .fetch_all(self.reader())
        .await?;

        Ok(rows.into_iter().map(Organization::from).collect())
    }

    pub async fn organization_role_for_user(
        &self,
        user_id: &str,
        organization_id: &str,
    ) -> Result<Option<OrganizationRole>> {
        let role = sqlx::query_scalar::<_, String>(
            "SELECT role FROM organization_memberships WHERE organization_id = ?1 AND user_id = ?2",
        )
        .bind(organization_id)
        .bind(user_id)
        .fetch_optional(self.reader())
        .await?;
        role.map(|role| organization_role_from_db(&role)).transpose()
    }

    pub async fn update_organization(&self, organization_id: &str, user_id: &str, name: &str) -> Result<Organization> {
        let name = require_name(name, "Organization name")?;
        let now = Utc::now().to_rfc3339();
        let updated = sqlx::query("UPDATE organizations SET name = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(&name)
            .bind(&now)
            .bind(organization_id)
            .execute(&self.pool)
            .await?;
        if updated.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("Organization was not found.");
        }
        // A system administrator may rename an organization they are not a
        // member of; they act as its administrator.
        let row = sqlx::query_as::<_, OrganizationRow>(
            "SELECT o.id, o.name, COALESCE(m.role, 'admin') AS role, o.created_at, o.updated_at FROM organizations o LEFT JOIN organization_memberships m ON m.organization_id = o.id AND m.user_id = ?2 WHERE o.id = ?1",
        )
        .bind(organization_id)
        .bind(user_id)
        .fetch_one(self.reader())
        .await?;
        Ok(row.into())
    }

    pub async fn list_organization_members(&self, organization_id: &str) -> Result<Vec<OrganizationMember>> {
        let rows = sqlx::query_as::<_, OrganizationMemberRow>(
            "SELECT u.id, u.email, u.display_name, m.role, m.created_at FROM organization_memberships m INNER JOIN users u ON u.id = m.user_id WHERE m.organization_id = ?1 ORDER BY CASE m.role WHEN 'owner' THEN 0 WHEN 'admin' THEN 1 ELSE 2 END, u.display_name ASC",
        )
        .bind(organization_id)
        .fetch_all(self.reader())
        .await?;
        rows.into_iter().map(OrganizationMember::try_from).collect()
    }

    pub async fn upsert_organization_member(
        &self,
        organization_id: &str,
        email: &str,
        role: OrganizationRole,
    ) -> Result<OrganizationMember> {
        if matches!(role, OrganizationRole::Owner) {
            repomemo_domain::bail_invalid!(
                "The owner role cannot be assigned through organization membership management."
            );
        }
        let user = self
            .find_user_by_email(email)
            .await?
            .ok_or_else(|| repomemo_domain::invalid!("No RepoMemo account exists for this email address."))?;
        if self.is_deactivated(&user.id).await? {
            repomemo_domain::bail_invalid!("This account is deactivated. Ask a system administrator to reactivate it.");
        }
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        let organization_exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM organizations WHERE id = ?1")
            .bind(organization_id)
            .fetch_one(&mut *tx)
            .await?;
        if organization_exists == 0 {
            repomemo_domain::bail_not_found!("Organization was not found.");
        }
        let current = sqlx::query_scalar::<_, String>(
            "SELECT role FROM organization_memberships WHERE organization_id = ?1 AND user_id = ?2",
        )
        .bind(organization_id)
        .bind(&user.id)
        .fetch_optional(&mut *tx)
        .await?;
        if current.as_deref() == Some("owner") {
            repomemo_domain::bail_invalid!(
                "The owner role stays with the organization owner; it cannot be changed through membership management."
            );
        }
        sqlx::query("INSERT INTO organization_memberships (organization_id, user_id, role, created_at) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(organization_id, user_id) DO UPDATE SET role = excluded.role")
            .bind(organization_id)
            .bind(&user.id)
            .bind(organization_role_to_db(&role))
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        // The organization role sets the role in the workspaces whose access came from the
        // organization: it never takes a workspace away from its owner, does not undo a role a
        // workspace set itself (`origin = 'direct'`), and does not bring back a person a
        // workspace administrator removed (V0.2.142, review E9).
        sqlx::query("INSERT INTO workspace_memberships (workspace_id, user_id, role, created_at, updated_at, origin) SELECT wo.workspace_id, ?1, CASE ?2 WHEN 'admin' THEN 'admin' ELSE 'member' END, ?3, ?3, 'organization' FROM workspace_organizations wo WHERE wo.organization_id = ?4 AND NOT EXISTS (SELECT 1 FROM workspace_membership_removals r WHERE r.workspace_id = wo.workspace_id AND r.user_id = ?1) ON CONFLICT(workspace_id, user_id) DO UPDATE SET role = excluded.role, updated_at = excluded.updated_at WHERE workspace_memberships.origin = 'organization' AND workspace_memberships.role <> 'owner'")
            .bind(&user.id)
            .bind(organization_role_to_db(&role))
            .bind(&now)
            .bind(organization_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        self.list_organization_members(organization_id)
            .await?
            .into_iter()
            .find(|member| member.user.id == user.id)
            .ok_or_else(|| anyhow::anyhow!("Organization member could not be loaded."))
    }

    pub async fn remove_organization_member(&self, organization_id: &str, user_id: &str) -> Result<()> {
        let role = self.organization_role_for_user(user_id, organization_id).await?;
        if matches!(role, Some(OrganizationRole::Owner)) {
            repomemo_domain::bail_invalid!("The organization owner cannot be removed.");
        }
        // Someone who still owns a workspace of the organization cannot leave
        // it: the workspace would be left with nobody in charge (review N8).
        let owned = sqlx::query_scalar::<_, String>(
            "SELECT w.name FROM workspace_memberships wm \
             JOIN workspace_organizations wo ON wo.workspace_id = wm.workspace_id \
             JOIN workspaces w ON w.id = wm.workspace_id \
             WHERE wo.organization_id = ?1 AND wm.user_id = ?2 AND wm.role = 'owner' ORDER BY w.name",
        )
        .bind(organization_id)
        .bind(user_id)
        .fetch_all(self.reader())
        .await?;
        if !owned.is_empty() {
            repomemo_domain::bail_conflict!(
                "This person still owns {}. Transfer ownership to someone else first.",
                owned.join(", ")
            );
        }
        let mut tx = self.pool.begin().await?;
        let deleted = sqlx::query("DELETE FROM organization_memberships WHERE organization_id = ?1 AND user_id = ?2")
            .bind(organization_id)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        if deleted.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("Organization membership was not found.");
        }
        sqlx::query("DELETE FROM workspace_memberships WHERE user_id = ?1 AND workspace_id IN (SELECT workspace_id FROM workspace_organizations WHERE organization_id = ?2)")
            .bind(user_id)
            .bind(organization_id)
            .execute(&mut *tx)
            .await?;
        // Someone who leaves the organization starts afresh if they return.
        sqlx::query("DELETE FROM workspace_membership_removals WHERE user_id = ?1 AND workspace_id IN (SELECT workspace_id FROM workspace_organizations WHERE organization_id = ?2)")
            .bind(user_id)
            .bind(organization_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Hands a workspace to another member: they become its owner and the
    /// previous owner stays on as an administrator. Someone of the workspace's
    /// organization who has no workspace role yet may receive it too.
    pub async fn transfer_workspace_ownership(&self, workspace_id: &str, new_owner_id: &str) -> Result<SharedUser> {
        let Some(user) = self.find_user(new_owner_id).await? else {
            repomemo_domain::bail_not_found!("User was not found.");
        };
        let has_role = self
            .workspace_role_for_user(new_owner_id, workspace_id)
            .await?
            .is_some();
        if !has_role && !self.user_in_workspace_organization(new_owner_id, workspace_id).await? {
            repomemo_domain::bail_invalid!("The new owner must be a member of this workspace or its organization.");
        }
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "UPDATE workspace_memberships SET role = 'admin', updated_at = ?2 WHERE workspace_id = ?1 AND role = 'owner'",
        )
        .bind(workspace_id)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"INSERT INTO workspace_memberships (workspace_id, user_id, role, created_at, updated_at)
               VALUES (?1, ?2, 'owner', ?3, ?3)
               ON CONFLICT(workspace_id, user_id) DO UPDATE SET role = 'owner', updated_at = excluded.updated_at"#,
        )
        .bind(workspace_id)
        .bind(new_owner_id)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(user)
    }

    /// Hands an organization to another of its members; the previous owner
    /// stays on as an administrator.
    pub async fn transfer_organization_ownership(
        &self,
        organization_id: &str,
        new_owner_id: &str,
    ) -> Result<SharedUser> {
        let Some(user) = self.find_user(new_owner_id).await? else {
            repomemo_domain::bail_not_found!("User was not found.");
        };
        if !self.user_belongs_to_organization(new_owner_id, organization_id).await? {
            repomemo_domain::bail_invalid!("The new owner must be a member of this organization.");
        }
        let mut tx = self.pool.begin().await?;
        sqlx::query("UPDATE organization_memberships SET role = 'admin' WHERE organization_id = ?1 AND role = 'owner'")
            .bind(organization_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE organization_memberships SET role = 'owner' WHERE organization_id = ?1 AND user_id = ?2")
            .bind(organization_id)
            .bind(new_owner_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(user)
    }

    /// Whether the user is a member of the organization the workspace belongs to.
    pub async fn user_in_workspace_organization(&self, user_id: &str, workspace_id: &str) -> Result<bool> {
        Ok(sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS (SELECT 1 FROM organization_memberships om INNER JOIN workspace_organizations wo ON wo.organization_id = om.organization_id WHERE wo.workspace_id = ?1 AND om.user_id = ?2)",
        )
        .bind(workspace_id)
        .bind(user_id)
        .fetch_one(self.reader())
        .await?
            > 0)
    }

    pub async fn user_belongs_to_organization(&self, user_id: &str, organization_id: &str) -> Result<bool> {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM organization_memberships WHERE organization_id = ?1 AND user_id = ?2",
        )
        .bind(organization_id)
        .bind(user_id)
        .fetch_one(self.reader())
        .await?;
        Ok(count > 0)
    }

    /// Fails unless `creator_id` may create workspaces in the organization:
    /// owners and administrators, and system administrators anywhere.
    pub(crate) async fn require_workspace_creator(&self, creator_id: &str, organization_id: &str) -> Result<OrganizationRole> {
        let membership = self.organization_role_for_user(creator_id, organization_id).await?;
        let is_system_admin = self
            .user_access(creator_id)
            .await?
            .is_some_and(|access| access.is_system_admin);
        // System administrators act as administrators of every organization.
        let creator_role = match membership {
            Some(OrganizationRole::Owner) => OrganizationRole::Owner,
            Some(_) | None if is_system_admin => {
                if !self.organization_exists(organization_id).await? {
                    repomemo_domain::bail_not_found!("Organization was not found.");
                }
                OrganizationRole::Admin
            }
            Some(role) => role,
            None => repomemo_domain::bail_invalid!("User is not a member of this organization."),
        };
        if matches!(creator_role, OrganizationRole::Member) {
            repomemo_domain::bail_invalid!("Only organization owners and admins can create workspaces.");
        }
        Ok(creator_role)
    }

    pub async fn create_shared_workspace(
        &self,
        creator_id: &str,
        organization_id: &str,
        name: &str,
    ) -> Result<SharedWorkspace> {
        self.create_shared_workspace_from(creator_id, organization_id, name, None)
            .await
    }

    /// Creates a workspace that starts with a template (V0.2.168): its choice saved in the settings and its starter
    /// folders made, all in the transaction that makes the workspace, so a folder that cannot be made leaves no
    /// workspace behind.
    pub async fn create_shared_workspace_from(
        &self,
        creator_id: &str,
        organization_id: &str,
        name: &str,
        setup: Option<&StarterSetup>,
    ) -> Result<SharedWorkspace> {
        let creator_role = self.require_workspace_creator(creator_id, organization_id).await?;
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let name = require_name(name, "Workspace name")?;
        let settings = match setup {
            Some(setup) => serde_json::json!({ TEMPLATE_SETTING: serde_json::to_value(&setup.reference)? }),
            None => Value::Object(Default::default()),
        };
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            "INSERT INTO workspaces (id, name, created_at, updated_at, settings_json) VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(&id)
        .bind(&name)
        .bind(&now)
        .bind(&now)
        .bind(serde_json::to_string(&settings)?)
        .execute(&mut *tx)
        .await?;
        if let Some(setup) = setup {
            for (folder, children) in &setup.folders {
                let parent = insert_starter_folder(&mut tx, &id, None, folder, creator_id, &now).await?;
                for child in children {
                    insert_starter_folder(&mut tx, &id, Some(&parent), child, creator_id, &now).await?;
                }
            }
        }
        sqlx::query("INSERT INTO workspace_organizations (workspace_id, organization_id) VALUES (?1, ?2)")
            .bind(&id)
            .bind(organization_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO workspace_memberships (workspace_id, user_id, role, created_at, updated_at, origin) SELECT ?1, om.user_id, CASE om.role WHEN 'owner' THEN 'owner' WHEN 'admin' THEN 'admin' ELSE 'member' END, ?2, ?2, 'organization' FROM organization_memberships om WHERE om.organization_id = ?3")
            .bind(&id)
            .bind(&now)
            .bind(organization_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO workspace_activity (id, workspace_id, actor_user_id, action, subject_type, subject_id, summary, created_at) VALUES (?1, ?2, ?3, 'workspace_created', 'workspace', ?2, ?4, ?5)")
            .bind(Uuid::new_v4().to_string())
            .bind(&id)
            .bind(creator_id)
            .bind(match setup {
                Some(setup) => format!("Created workspace {name} from the {} template.", setup.template_name),
                None => format!("Created workspace {name}."),
            })
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;

        Ok(SharedWorkspace {
            workspace: Workspace {
                id,
                name,
                created_at: now.clone(),
                updated_at: now,
                settings,
            },
            organization_id: organization_id.to_owned(),
            role: organization_role_to_workspace_role(&creator_role),
        })
    }

    pub async fn list_shared_workspaces_for_user(&self, user_id: &str) -> Result<Vec<SharedWorkspace>> {
        let rows = sqlx::query_as::<_, SharedWorkspaceRow>(
            "SELECT w.id, w.name, w.created_at, w.updated_at, w.settings_json, wo.organization_id, wm.role FROM workspaces w INNER JOIN workspace_organizations wo ON wo.workspace_id = w.id INNER JOIN workspace_memberships wm ON wm.workspace_id = w.id WHERE wm.user_id = ?1 ORDER BY w.updated_at DESC, w.name ASC",
        )
        .bind(user_id)
        .fetch_all(self.reader())
        .await?;

        rows.into_iter().map(SharedWorkspace::try_from).collect()
    }

    pub async fn workspace_memberships_for_user(&self, user_id: &str) -> Result<Vec<WorkspaceMembership>> {
        let rows = sqlx::query_as::<_, (String, String)>(
            "SELECT workspace_id, role FROM workspace_memberships WHERE user_id = ?1",
        )
        .bind(user_id)
        .fetch_all(self.reader())
        .await?;
        rows.into_iter()
            .map(|(workspace_id, role)| {
                Ok(WorkspaceMembership {
                    workspace_id,
                    role: workspace_role_from_db(&role)?,
                })
            })
            .collect()
    }

    pub async fn workspace_role_for_user(&self, user_id: &str, workspace_id: &str) -> Result<Option<WorkspaceRole>> {
        let role = sqlx::query_scalar::<_, String>(
            "SELECT role FROM workspace_memberships WHERE workspace_id = ?1 AND user_id = ?2",
        )
        .bind(workspace_id)
        .bind(user_id)
        .fetch_optional(self.reader())
        .await?;
        role.map(|role| workspace_role_from_db(&role)).transpose()
    }

    pub async fn list_workspace_members(&self, workspace_id: &str) -> Result<Vec<WorkspaceMember>> {
        let rows = sqlx::query_as::<_, WorkspaceMemberRow>(
            "SELECT u.id, u.email, u.display_name, wm.role, wm.created_at, wm.updated_at FROM workspace_memberships wm INNER JOIN users u ON u.id = wm.user_id WHERE wm.workspace_id = ?1 ORDER BY CASE wm.role WHEN 'owner' THEN 0 WHEN 'admin' THEN 1 WHEN 'member' THEN 2 ELSE 3 END, u.display_name ASC",
        )
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?;
        rows.into_iter().map(WorkspaceMember::try_from).collect()
    }

    pub async fn upsert_workspace_member(
        &self,
        workspace_id: &str,
        email: &str,
        role: WorkspaceRole,
    ) -> Result<WorkspaceMember> {
        if matches!(role, WorkspaceRole::Owner) {
            repomemo_domain::bail_invalid!("The owner role cannot be assigned through membership management.");
        }
        let user = self
            .find_user_by_email(email)
            .await?
            .ok_or_else(|| repomemo_domain::invalid!("No RepoMemo account exists for this email address."))?;
        if self.is_deactivated(&user.id).await? {
            repomemo_domain::bail_invalid!("This account is deactivated. Ask a system administrator to reactivate it.");
        }
        // Membership of one workspace is just that: the person does not join
        // the organization, so nothing flows to its other workspaces (they
        // see the organization as a guest). Organization membership, which
        // does flow, is granted by organization administrators only.
        if !self.workspace_exists(workspace_id).await? {
            repomemo_domain::bail_not_found!("Workspace was not found.");
        }
        // Not even the owner may change their own role here: a workspace
        // without an owner can no longer be renamed or deleted.
        if matches!(
            self.workspace_role_for_user(&user.id, workspace_id).await?,
            Some(WorkspaceRole::Owner)
        ) {
            repomemo_domain::bail_invalid!(
                "The owner role stays with the workspace owner; it cannot be changed through membership management."
            );
        }
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        // A role set here is the workspace's own: an organization role change leaves it alone.
        sqlx::query("INSERT INTO workspace_memberships (workspace_id, user_id, role, created_at, updated_at, origin) VALUES (?1, ?2, ?3, ?4, ?5, 'direct') ON CONFLICT(workspace_id, user_id) DO UPDATE SET role = excluded.role, updated_at = excluded.updated_at, origin = 'direct'")
            .bind(workspace_id)
            .bind(&user.id)
            .bind(workspace_role_to_db(&role))
            .bind(&now)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM workspace_membership_removals WHERE workspace_id = ?1 AND user_id = ?2")
            .bind(workspace_id)
            .bind(&user.id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        self.list_workspace_members(workspace_id)
            .await?
            .into_iter()
            .find(|member| member.user.id == user.id)
            .ok_or_else(|| anyhow::anyhow!("Workspace member could not be loaded."))
    }

    pub async fn remove_workspace_member(&self, workspace_id: &str, user_id: &str) -> Result<()> {
        let role = self.workspace_role_for_user(user_id, workspace_id).await?;
        if matches!(role, Some(WorkspaceRole::Owner)) {
            repomemo_domain::bail_invalid!("The workspace owner cannot be removed.");
        }
        let mut tx = self.pool.begin().await?;
        let result = sqlx::query("DELETE FROM workspace_memberships WHERE workspace_id = ?1 AND user_id = ?2")
            .bind(workspace_id)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        if result.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("Workspace membership was not found.");
        }
        // A member of the workspace's organization comes back with the next organization role
        // change unless this removal is remembered (V0.2.142, review E9).
        sqlx::query("INSERT OR REPLACE INTO workspace_membership_removals (workspace_id, user_id, removed_at) SELECT ?1, ?2, ?3 WHERE EXISTS (SELECT 1 FROM organization_memberships om JOIN workspace_organizations wo ON wo.organization_id = om.organization_id WHERE wo.workspace_id = ?1 AND om.user_id = ?2)")
            .bind(workspace_id)
            .bind(user_id)
            .bind(Utc::now().to_rfc3339())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
}
