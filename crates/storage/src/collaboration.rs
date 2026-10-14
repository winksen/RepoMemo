use super::*;

impl StorageEngine {
    pub async fn list_collaboration_tasks(&self, workspace_id: &str) -> Result<Vec<CollaborationTask>> {
        let rows = sqlx::query_as::<_, CollaborationTaskRow>(
            r#"
            SELECT
              task.id,
              task.workspace_id,
              task.title,
              task.description,
              task.status,
              task.priority,
              assignee.id AS assignee_id,
              assignee.email AS assignee_email,
              assignee.display_name AS assignee_display_name,
              creator.id AS creator_id,
              creator.email AS creator_email,
              creator.display_name AS creator_display_name,
              task.artifact_id,
              task.due_at,
              task.completed_at,
              task.created_at,
              task.updated_at
            FROM workspace_tasks AS task
            JOIN users AS creator ON creator.id = task.created_by_user_id
            LEFT JOIN users AS assignee ON assignee.id = task.assignee_user_id
            WHERE task.workspace_id = ?1
            ORDER BY
              CASE task.status WHEN 'in_progress' THEN 0 WHEN 'blocked' THEN 1 WHEN 'open' THEN 2 ELSE 3 END,
              CASE task.priority WHEN 'urgent' THEN 0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2 ELSE 3 END,
              task.due_at IS NULL,
              task.due_at ASC,
              task.updated_at DESC
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows.into_iter().map(CollaborationTask::from).collect())
    }

    pub async fn list_assigned_collaboration_tasks(&self, user_id: &str) -> Result<Vec<CollaborationTask>> {
        let rows = sqlx::query_as::<_, CollaborationTaskRow>(
            r#"
            SELECT
              task.id,
              task.workspace_id,
              task.title,
              task.description,
              task.status,
              task.priority,
              assignee.id AS assignee_id,
              assignee.email AS assignee_email,
              assignee.display_name AS assignee_display_name,
              creator.id AS creator_id,
              creator.email AS creator_email,
              creator.display_name AS creator_display_name,
              task.artifact_id,
              task.due_at,
              task.completed_at,
              task.created_at,
              task.updated_at
            FROM workspace_tasks AS task
            JOIN users AS creator ON creator.id = task.created_by_user_id
            JOIN users AS assignee ON assignee.id = task.assignee_user_id
            JOIN workspace_memberships AS membership
              ON membership.workspace_id = task.workspace_id AND membership.user_id = ?1
            WHERE task.assignee_user_id = ?1 AND task.status != 'done'
            ORDER BY
              CASE task.priority WHEN 'urgent' THEN 0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2 ELSE 3 END,
              task.due_at IS NULL,
              task.due_at ASC,
              task.updated_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows.into_iter().map(CollaborationTask::from).collect())
    }

    pub async fn get_collaboration_task(&self, task_id: &str) -> Result<CollaborationTask> {
        let row = sqlx::query_as::<_, CollaborationTaskRow>(
            r#"
            SELECT
              task.id,
              task.workspace_id,
              task.title,
              task.description,
              task.status,
              task.priority,
              assignee.id AS assignee_id,
              assignee.email AS assignee_email,
              assignee.display_name AS assignee_display_name,
              creator.id AS creator_id,
              creator.email AS creator_email,
              creator.display_name AS creator_display_name,
              task.artifact_id,
              task.due_at,
              task.completed_at,
              task.created_at,
              task.updated_at
            FROM workspace_tasks AS task
            JOIN users AS creator ON creator.id = task.created_by_user_id
            LEFT JOIN users AS assignee ON assignee.id = task.assignee_user_id
            WHERE task.id = ?1
            "#,
        )
        .bind(task_id)
        .fetch_one(self.reader())
        .await?;
        Ok(CollaborationTask::from(row))
    }

    pub async fn create_collaboration_task(
        &self,
        workspace_id: &str,
        created_by_user_id: &str,
        task: NewCollaborationTask,
    ) -> Result<CollaborationTask> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let completed_at = (task.status == "done").then(|| now.clone());
        sqlx::query(
            r#"
            INSERT INTO workspace_tasks (
              id, workspace_id, title, description, status, priority,
              assignee_user_id, created_by_user_id, artifact_id, due_at,
              completed_at, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            "#,
        )
        .bind(&id)
        .bind(workspace_id)
        .bind(task.title)
        .bind(task.description)
        .bind(task.status)
        .bind(task.priority)
        .bind(task.assignee_user_id)
        .bind(created_by_user_id)
        .bind(task.artifact_id)
        .bind(task.due_at)
        .bind(completed_at)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.get_collaboration_task(&id).await
    }

    pub async fn update_collaboration_task(
        &self,
        task_id: &str,
        task: NewCollaborationTask,
    ) -> Result<CollaborationTask> {
        let current = self.get_collaboration_task(task_id).await?;
        let now = Utc::now().to_rfc3339();
        let completed_at = if task.status == "done" {
            current.completed_at.or_else(|| Some(now.clone()))
        } else {
            None
        };
        sqlx::query(
            r#"
            UPDATE workspace_tasks
            SET title = ?1, description = ?2, status = ?3, priority = ?4,
                assignee_user_id = ?5, artifact_id = ?6, due_at = ?7,
                completed_at = ?8, updated_at = ?9
            WHERE id = ?10
            "#,
        )
        .bind(task.title)
        .bind(task.description)
        .bind(task.status)
        .bind(task.priority)
        .bind(task.assignee_user_id)
        .bind(task.artifact_id)
        .bind(task.due_at)
        .bind(completed_at)
        .bind(now)
        .bind(task_id)
        .execute(&self.pool)
        .await?;
        self.get_collaboration_task(task_id).await
    }

    pub async fn delete_collaboration_task(&self, task_id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM workspace_tasks WHERE id = ?1")
            .bind(task_id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("Collaboration task was not found.");
        }
        Ok(())
    }

    pub async fn list_saved_searches(&self, workspace_id: &str) -> Result<Vec<SavedSearch>> {
        let rows = sqlx::query_as::<_, SavedSearchRow>("SELECT search.id, search.workspace_id, search.name, search.query, search.artifact_types_json, search.languages_json, search.source_ids_json, search.result_limit, creator.id AS creator_id, creator.email AS creator_email, creator.display_name AS creator_display_name, search.created_at, search.updated_at FROM workspace_saved_searches search JOIN users creator ON creator.id = search.created_by_user_id WHERE search.workspace_id = ?1 ORDER BY search.updated_at DESC, search.name ASC")
            .bind(workspace_id).fetch_all(self.reader()).await?;
        Ok(rows.into_iter().map(SavedSearch::from).collect())
    }

    pub async fn create_saved_search(
        &self,
        workspace_id: &str,
        user_id: &str,
        search: NewSavedSearch,
    ) -> Result<SavedSearch> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO workspace_saved_searches (id, workspace_id, created_by_user_id, name, query, artifact_types_json, languages_json, source_ids_json, result_limit, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?10)")
            .bind(&id).bind(workspace_id).bind(user_id).bind(search.name).bind(search.query)
            .bind(serde_json::to_string(&search.artifact_types)?).bind(serde_json::to_string(&search.languages)?).bind(serde_json::to_string(&search.source_ids)?).bind(search.result_limit).bind(now).execute(&self.pool).await?;
        self.get_saved_search(&id).await
    }

    pub async fn get_saved_search(&self, search_id: &str) -> Result<SavedSearch> {
        let row = sqlx::query_as::<_, SavedSearchRow>("SELECT search.id, search.workspace_id, search.name, search.query, search.artifact_types_json, search.languages_json, search.source_ids_json, search.result_limit, creator.id AS creator_id, creator.email AS creator_email, creator.display_name AS creator_display_name, search.created_at, search.updated_at FROM workspace_saved_searches search JOIN users creator ON creator.id = search.created_by_user_id WHERE search.id = ?1")
            .bind(search_id).fetch_one(self.reader()).await?;
        Ok(SavedSearch::from(row))
    }

    pub async fn delete_saved_search(&self, search_id: &str) -> Result<()> {
        let changed = sqlx::query("DELETE FROM workspace_saved_searches WHERE id = ?1")
            .bind(search_id)
            .execute(&self.pool)
            .await?;
        if changed.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("Saved search was not found.");
        }
        Ok(())
    }

    pub async fn list_agent_conversations(&self, workspace_id: &str, user_id: &str) -> Result<Vec<AgentConversation>> {
        let rows = sqlx::query_as::<_, AgentConversationRow>(&format!("{AGENT_CONVERSATION_SELECT} WHERE conversation.workspace_id = ?1 AND conversation.user_id = ?2 ORDER BY conversation.updated_at DESC"))
            .bind(workspace_id)
            .bind(user_id)
            .fetch_all(self.reader())
            .await?;
        Ok(rows.into_iter().map(AgentConversation::from).collect())
    }

    /// The conversation when `user_id` owns it; anyone else gets "not found".
    pub async fn get_agent_conversation(&self, conversation_id: &str, user_id: &str) -> Result<AgentConversation> {
        sqlx::query_as::<_, AgentConversationRow>(&format!(
            "{AGENT_CONVERSATION_SELECT} WHERE conversation.id = ?1 AND conversation.user_id = ?2"
        ))
        .bind(conversation_id)
        .bind(user_id)
        .fetch_optional(self.reader())
        .await?
        .map(AgentConversation::from)
        .ok_or_else(|| repomemo_domain::not_found!("Assistant conversation was not found."))
    }

    pub async fn create_agent_conversation(
        &self,
        workspace_id: &str,
        user_id: &str,
        title: &str,
    ) -> Result<AgentConversation> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO assistant_conversations (id, workspace_id, user_id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)")
            .bind(&id)
            .bind(workspace_id)
            .bind(user_id)
            .bind(title)
            .bind(now)
            .execute(&self.pool)
            .await?;
        self.get_agent_conversation(&id, user_id).await
    }

    pub async fn rename_agent_conversation(
        &self,
        conversation_id: &str,
        user_id: &str,
        title: &str,
    ) -> Result<AgentConversation> {
        let changed = sqlx::query("UPDATE assistant_conversations SET title = ?3 WHERE id = ?1 AND user_id = ?2")
            .bind(conversation_id)
            .bind(user_id)
            .bind(title)
            .execute(&self.pool)
            .await?;
        if changed.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("Assistant conversation was not found.");
        }
        self.get_agent_conversation(conversation_id, user_id).await
    }

    pub async fn delete_agent_conversation(&self, conversation_id: &str, user_id: &str) -> Result<()> {
        let changed = sqlx::query("DELETE FROM assistant_conversations WHERE id = ?1 AND user_id = ?2")
            .bind(conversation_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        if changed.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("Assistant conversation was not found.");
        }
        Ok(())
    }

    pub async fn list_agent_turns(&self, conversation_id: &str) -> Result<Vec<AgentTurn>> {
        let rows = sqlx::query_as::<_, AgentTurnRow>("SELECT id, conversation_id, position, label, request_json, reply_json, created_at FROM assistant_turns WHERE conversation_id = ?1 ORDER BY position")
            .bind(conversation_id)
            .fetch_all(self.reader())
            .await?;
        rows.into_iter().map(AgentTurn::try_from).collect()
    }

    /// Stores a finished exchange at the end of the conversation and marks the
    /// conversation as recently used.
    pub async fn append_agent_turn(
        &self,
        conversation_id: &str,
        label: &str,
        request: &AgentMessage,
        reply: &AgentReply,
    ) -> Result<AgentTurn> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let mut transaction = self.pool.begin().await?;
        sqlx::query("INSERT INTO assistant_turns (id, conversation_id, position, label, request_json, reply_json, created_at) VALUES (?1, ?2, (SELECT COALESCE(MAX(position), 0) + 1 FROM assistant_turns WHERE conversation_id = ?2), ?3, ?4, ?5, ?6)")
            .bind(&id)
            .bind(conversation_id)
            .bind(label)
            .bind(serde_json::to_string(request)?)
            .bind(serde_json::to_string(reply)?)
            .bind(&now)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("UPDATE assistant_conversations SET updated_at = ?2 WHERE id = ?1")
            .bind(conversation_id)
            .bind(&now)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        let row = sqlx::query_as::<_, AgentTurnRow>("SELECT id, conversation_id, position, label, request_json, reply_json, created_at FROM assistant_turns WHERE id = ?1")
            .bind(&id)
            .fetch_one(self.reader())
            .await?;
        AgentTurn::try_from(row)
    }

    pub async fn list_task_checklist_items(&self, task_id: &str) -> Result<Vec<TaskChecklistItem>> {
        let rows = sqlx::query_as::<_, TaskChecklistItemRow>("SELECT item.id,item.task_id,item.workspace_id,item.body,item.position,item.completed_at,completed.id AS completed_by_id,completed.email AS completed_by_email,completed.display_name AS completed_by_display_name,creator.id AS creator_id,creator.email AS creator_email,creator.display_name AS creator_display_name,item.created_at,item.updated_at FROM workspace_task_checklist_items item JOIN users creator ON creator.id=item.created_by_user_id LEFT JOIN users completed ON completed.id=item.completed_by_user_id WHERE item.task_id=?1 ORDER BY item.position,item.created_at")
            .bind(task_id).fetch_all(self.reader()).await?;
        Ok(rows.into_iter().map(TaskChecklistItem::from).collect())
    }

    pub async fn create_task_checklist_item(
        &self,
        task_id: &str,
        workspace_id: &str,
        user_id: &str,
        item: NewTaskChecklistItem,
    ) -> Result<TaskChecklistItem> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let position: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM workspace_task_checklist_items WHERE task_id=?1",
        )
        .bind(task_id)
        .fetch_one(self.reader())
        .await?;
        sqlx::query("INSERT INTO workspace_task_checklist_items (id,task_id,workspace_id,created_by_user_id,body,position,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?7)").bind(&id).bind(task_id).bind(workspace_id).bind(user_id).bind(item.body).bind(position).bind(now).execute(&self.pool).await?;
        self.get_task_checklist_item(&id).await
    }

    pub async fn get_task_checklist_item(&self, item_id: &str) -> Result<TaskChecklistItem> {
        let row=sqlx::query_as::<_,TaskChecklistItemRow>("SELECT item.id,item.task_id,item.workspace_id,item.body,item.position,item.completed_at,completed.id AS completed_by_id,completed.email AS completed_by_email,completed.display_name AS completed_by_display_name,creator.id AS creator_id,creator.email AS creator_email,creator.display_name AS creator_display_name,item.created_at,item.updated_at FROM workspace_task_checklist_items item JOIN users creator ON creator.id=item.created_by_user_id LEFT JOIN users completed ON completed.id=item.completed_by_user_id WHERE item.id=?1").bind(item_id).fetch_one(self.reader()).await?;
        Ok(TaskChecklistItem::from(row))
    }

    pub async fn toggle_task_checklist_item(
        &self,
        item_id: &str,
        user_id: &str,
        completed: bool,
    ) -> Result<TaskChecklistItem> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE workspace_task_checklist_items SET completed_at=?1, completed_by_user_id=?2, updated_at=?3 WHERE id=?4").bind(if completed { Some(now.clone()) } else { None }).bind(if completed { Some(user_id) } else { None }).bind(now).bind(item_id).execute(&self.pool).await?;
        self.get_task_checklist_item(item_id).await
    }

    pub async fn delete_task_checklist_item(&self, item_id: &str) -> Result<()> {
        let changed = sqlx::query("DELETE FROM workspace_task_checklist_items WHERE id=?1")
            .bind(item_id)
            .execute(&self.pool)
            .await?;
        if changed.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("Checklist item was not found.");
        }
        Ok(())
    }

    pub async fn list_artifact_comments(&self, artifact_id: &str) -> Result<Vec<ArtifactComment>> {
        let rows = sqlx::query_as::<_, ArtifactCommentRow>(
            r#"
            SELECT
              comment.id,
              comment.workspace_id,
              comment.artifact_id,
              author.id AS author_id,
              author.email AS author_email,
              author.display_name AS author_display_name,
              comment.body,
              comment.created_at,
              comment.updated_at
            FROM artifact_comments AS comment
            JOIN users AS author ON author.id = comment.author_user_id
            WHERE comment.artifact_id = ?1
            ORDER BY comment.created_at ASC, comment.id ASC
            "#,
        )
        .bind(artifact_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows.into_iter().map(ArtifactComment::from).collect())
    }

    pub async fn get_artifact_comment(&self, comment_id: &str) -> Result<ArtifactComment> {
        let row = sqlx::query_as::<_, ArtifactCommentRow>(
            r#"
            SELECT
              comment.id,
              comment.workspace_id,
              comment.artifact_id,
              author.id AS author_id,
              author.email AS author_email,
              author.display_name AS author_display_name,
              comment.body,
              comment.created_at,
              comment.updated_at
            FROM artifact_comments AS comment
            JOIN users AS author ON author.id = comment.author_user_id
            WHERE comment.id = ?1
            "#,
        )
        .bind(comment_id)
        .fetch_one(self.reader())
        .await?;
        Ok(ArtifactComment::from(row))
    }

    pub async fn create_artifact_comment(
        &self,
        workspace_id: &str,
        artifact_id: &str,
        author_user_id: &str,
        body: &str,
    ) -> Result<ArtifactComment> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO artifact_comments (
              id, workspace_id, artifact_id, author_user_id, body, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(&id)
        .bind(workspace_id)
        .bind(artifact_id)
        .bind(author_user_id)
        .bind(body)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.get_artifact_comment(&id).await
    }

    pub async fn update_artifact_comment(&self, comment_id: &str, body: &str) -> Result<ArtifactComment> {
        sqlx::query("UPDATE artifact_comments SET body = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(body)
            .bind(Utc::now().to_rfc3339())
            .bind(comment_id)
            .execute(&self.pool)
            .await?;
        self.get_artifact_comment(comment_id).await
    }

    pub async fn delete_artifact_comment(&self, comment_id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM artifact_comments WHERE id = ?1")
            .bind(comment_id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("Artifact comment was not found.");
        }
        Ok(())
    }

    pub async fn get_artifact_lifecycle(&self, artifact_id: &str) -> Result<ArtifactLifecycle> {
        let artifact = self.get_artifact_summary(artifact_id).await?;
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO artifact_lifecycle (
              artifact_id, workspace_id, status, owner_user_id, review_note,
              reviewed_by_user_id, reviewed_at, superseded_by_artifact_id, created_at, updated_at
            ) VALUES (?1, ?2, 'active', NULL, '', NULL, NULL, NULL, ?3, ?3)
            "#,
        )
        .bind(artifact_id)
        .bind(&artifact.workspace_id)
        .bind(now)
        .execute(&self.pool)
        .await?;

        let row = sqlx::query_as::<_, ArtifactLifecycleRow>(
            r#"
            SELECT
              lifecycle.artifact_id,
              lifecycle.workspace_id,
              lifecycle.status,
              owner.id AS owner_id,
              owner.email AS owner_email,
              owner.display_name AS owner_display_name,
              lifecycle.review_note,
              reviewer.id AS reviewer_id,
              reviewer.email AS reviewer_email,
              reviewer.display_name AS reviewer_display_name,
              lifecycle.reviewed_at,
              lifecycle.superseded_by_artifact_id,
              lifecycle.created_at,
              lifecycle.updated_at
            FROM artifact_lifecycle AS lifecycle
            LEFT JOIN users AS owner ON owner.id = lifecycle.owner_user_id
            LEFT JOIN users AS reviewer ON reviewer.id = lifecycle.reviewed_by_user_id
            WHERE lifecycle.artifact_id = ?1
            "#,
        )
        .bind(artifact_id)
        .fetch_one(self.reader())
        .await?;
        Ok(ArtifactLifecycle::from(row))
    }

    pub async fn save_artifact_lifecycle(
        &self,
        artifact_id: &str,
        actor_user_id: &str,
        lifecycle: SaveArtifactLifecycle,
    ) -> Result<ArtifactLifecycle> {
        let artifact = self.get_artifact_summary(artifact_id).await?;
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO artifact_lifecycle (
              artifact_id, workspace_id, status, owner_user_id, review_note,
              reviewed_by_user_id, reviewed_at, superseded_by_artifact_id, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?7, ?7)
            ON CONFLICT(artifact_id) DO UPDATE SET
              status = excluded.status,
              owner_user_id = excluded.owner_user_id,
              review_note = excluded.review_note,
              reviewed_by_user_id = excluded.reviewed_by_user_id,
              reviewed_at = excluded.reviewed_at,
              superseded_by_artifact_id = excluded.superseded_by_artifact_id,
              updated_at = excluded.updated_at
            "#,
        )
        .bind(artifact_id)
        .bind(&artifact.workspace_id)
        .bind(&lifecycle.status)
        .bind(&lifecycle.owner_user_id)
        .bind(&lifecycle.review_note)
        .bind(actor_user_id)
        .bind(&now)
        .bind(&lifecycle.superseded_by_artifact_id)
        .execute(&self.pool)
        .await?;

        let action = if lifecycle.status == "superseded" {
            "artifact_superseded"
        } else {
            "lifecycle_updated"
        };
        let detail = format!("Evidence lifecycle changed to {}.", lifecycle.status);
        sqlx::query("INSERT INTO artifact_lifecycle_events (id, artifact_id, actor_user_id, action, detail, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)")
            .bind(Uuid::new_v4().to_string())
            .bind(artifact_id)
            .bind(actor_user_id)
            .bind(action)
            .bind(detail)
            .bind(now)
            .execute(&self.pool)
            .await?;
        self.get_artifact_lifecycle(artifact_id).await
    }

    pub async fn list_artifact_lifecycle_events(&self, artifact_id: &str) -> Result<Vec<ArtifactLifecycleEvent>> {
        let rows = sqlx::query_as::<_, ArtifactLifecycleEventRow>(
            r#"
            SELECT event.id, event.artifact_id,
              actor.id AS actor_id, actor.email AS actor_email, actor.display_name AS actor_display_name,
              event.action, event.detail, event.created_at
            FROM artifact_lifecycle_events AS event
            LEFT JOIN users AS actor ON actor.id = event.actor_user_id
            WHERE event.artifact_id = ?1
            ORDER BY event.created_at DESC, event.id DESC
            "#,
        )
        .bind(artifact_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows.into_iter().map(ArtifactLifecycleEvent::from).collect())
    }

    pub async fn create_shared_notification(&self, notification: NewSharedNotification) -> Result<SharedNotification> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO workspace_notifications (
              id, user_id, workspace_id, notification_type, title, body, href, read_at, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8)
            "#,
        )
        .bind(&id)
        .bind(notification.user_id)
        .bind(notification.workspace_id)
        .bind(notification.notification_type)
        .bind(notification.title)
        .bind(notification.body)
        .bind(notification.href)
        .bind(now)
        .execute(&self.pool)
        .await?;
        self.get_shared_notification(&id).await
    }

    pub async fn list_shared_notifications(&self, user_id: &str) -> Result<Vec<SharedNotification>> {
        let rows = sqlx::query_as::<_, SharedNotificationRow>(
            r#"
            SELECT id, workspace_id, notification_type, title, body, href, read_at, created_at
            FROM workspace_notifications
            WHERE user_id = ?1
            ORDER BY CASE WHEN read_at IS NULL THEN 0 ELSE 1 END, created_at DESC, id DESC
            LIMIT 100
            "#,
        )
        .bind(user_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows.into_iter().map(SharedNotification::from).collect())
    }

    pub async fn mark_shared_notification_read(
        &self,
        notification_id: &str,
        user_id: &str,
    ) -> Result<SharedNotification> {
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            "UPDATE workspace_notifications SET read_at = COALESCE(read_at, ?1) WHERE id = ?2 AND user_id = ?3",
        )
        .bind(now)
        .bind(notification_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("Notification was not found.");
        }
        self.get_shared_notification(notification_id).await
    }

    pub async fn mark_all_shared_notifications_read(&self, user_id: &str) -> Result<()> {
        sqlx::query("UPDATE workspace_notifications SET read_at = ?1 WHERE user_id = ?2 AND read_at IS NULL")
            .bind(Utc::now().to_rfc3339())
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub(crate) async fn get_shared_notification(&self, notification_id: &str) -> Result<SharedNotification> {
        let row = sqlx::query_as::<_, SharedNotificationRow>(
            "SELECT id, workspace_id, notification_type, title, body, href, read_at, created_at FROM workspace_notifications WHERE id = ?1",
        )
        .bind(notification_id)
        .fetch_one(self.reader())
        .await?;
        Ok(SharedNotification::from(row))
    }
}
