//! CAIRN chats shared with a workspace (V0.9.57). The owner shares and unshares; readers see the chat read-only and may copy it.

use repomemo_domain::{AgentConversation, AgentMessage, AgentProposalState, AgentReply, SharedConversation};

use super::*;

/// A reply as it is when copied into someone else's chat: what was waiting to be confirmed is not, since only the owner of the
/// original may confirm it.
pub fn neutralize_for_copy(reply: &AgentReply) -> AgentReply {
    let mut copy = reply.clone();
    for proposal in &mut copy.proposals {
        if proposal.state == AgentProposalState::Pending {
            proposal.state = AgentProposalState::Dismissed;
            proposal.outcome = Some("Not applied: this chat was copied from a shared chat.".to_owned());
        }
    }
    if let Some(set) = copy.changeset.as_mut() {
        if set.state == "pending" {
            set.state = "dismissed".to_owned();
            set.outcome = Some("Not applied: this chat was copied from a shared chat.".to_owned());
        }
    }
    copy
}

impl StorageEngine {
    /// Makes a chat one that looks across all the person's workspaces (V0.9.60); set when the chat is made.
    /// Keeps the catalog model a person chose for a chat, or clears it (V0.9.86).
    pub async fn set_agent_conversation_model(&self, conversation_id: &str, model_id: Option<&str>) -> Result<()> {
        sqlx::query("UPDATE assistant_conversations SET cairn_model_id = ?2 WHERE id = ?1")
            .bind(conversation_id)
            .bind(model_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_agent_conversation_all_workspaces(&self, conversation_id: &str) -> Result<()> {
        sqlx::query("UPDATE assistant_conversations SET all_workspaces = 1 WHERE id = ?1")
            .bind(conversation_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The person's inbox chat in a workspace, made when there is none (V0.9.67).
    pub async fn inbox_conversation(&self, workspace_id: &str, user_id: &str) -> Result<AgentConversation> {
        let found: Option<String> = sqlx::query_scalar(
            "SELECT id FROM assistant_conversations WHERE workspace_id = ?1 AND user_id = ?2 AND origin = 'inbox' LIMIT 1",
        )
        .bind(workspace_id)
        .bind(user_id)
        .fetch_optional(self.reader())
        .await?;
        if let Some(id) = found {
            return self.get_agent_conversation(&id, user_id).await;
        }
        let created = self
            .create_agent_conversation(workspace_id, user_id, "CAIRN inbox")
            .await?;
        sqlx::query("UPDATE assistant_conversations SET origin = 'inbox' WHERE id = ?1")
            .bind(&created.id)
            .execute(&self.pool)
            .await?;
        self.get_agent_conversation(&created.id, user_id).await
    }

    /// How many proposals wait for the person across their inbox chats.
    pub async fn pending_inbox_proposals(&self, user_id: &str) -> Result<i64> {
        Ok(sqlx::query_scalar(
            "SELECT COUNT(*) FROM assistant_conversations c JOIN assistant_turns t ON t.conversation_id = c.id, json_each(t.reply_json, '$.proposals') p WHERE c.user_id = ?1 AND c.origin = 'inbox' AND json_extract(p.value, '$.state') = 'pending'",
        )
        .bind(user_id)
        .fetch_one(self.reader())
        .await?)
    }

    /// The name a person gave a token, which is how an outside agent is named in their inbox.
    pub async fn personal_token_name(&self, token_id: &str) -> Result<String> {
        Ok(
            sqlx::query_scalar("SELECT name FROM personal_access_tokens WHERE id = ?1")
                .bind(token_id)
                .fetch_optional(self.reader())
                .await?
                .unwrap_or_else(|| "An outside agent".to_owned()),
        )
    }

    /// Shares one of the caller's chats with its workspace.
    pub async fn share_agent_conversation(&self, conversation_id: &str, user_id: &str) -> Result<()> {
        let conversation = self.get_agent_conversation(conversation_id, user_id).await?;
        // A chat across workspaces holds what other workspaces returned; the readers of this one may not read those (CR-1).
        if conversation.all_workspaces {
            repomemo_domain::bail_conflict!(
                "A chat across all your workspaces cannot be shared: it holds files of other workspaces."
            );
        }
        if conversation.origin == "inbox" {
            repomemo_domain::bail_conflict!("The CAIRN inbox is yours alone and cannot be shared.");
        }
        // A reply that used a card of the organization's knowledge workspace holds that card's text; the readers of this workspace may
        // not read that workspace (review V0.9.117, CR-2).
        let foreign: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM assistant_turns turn, json_each(turn.reply_json, '$.memory_cards') card              WHERE turn.conversation_id = ?1 AND json_extract(card.value, '$.workspace_id') != ?2",
        )
        .bind(conversation_id)
        .bind(&conversation.workspace_id)
        .fetch_one(self.reader())
        .await?;
        if foreign > 0 {
            repomemo_domain::bail_conflict!(
                "This chat used cards of your organization's knowledge workspace, so it cannot be shared: the people it would be shared with may not read them."
            );
        }
        sqlx::query(
            "INSERT OR IGNORE INTO agent_conversation_shares (conversation_id, workspace_id, shared_by, shared_at) VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(conversation_id)
        .bind(&conversation.workspace_id)
        .bind(user_id)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Stops sharing one of the caller's chats; nobody else can open it from then on.
    pub async fn unshare_agent_conversation(&self, conversation_id: &str, user_id: &str) -> Result<()> {
        self.get_agent_conversation(conversation_id, user_id).await?;
        sqlx::query("DELETE FROM agent_conversation_shares WHERE conversation_id = ?1")
            .bind(conversation_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The chats shared with a workspace, newest first, with who shared each.
    pub async fn list_shared_agent_conversations(&self, workspace_id: &str) -> Result<Vec<SharedConversation>> {
        let rows = sqlx::query(
            "SELECT conversation.id, conversation.title, share.shared_at, owner.display_name AS shared_by_name, share.shared_by AS shared_by_id, \
                    (SELECT COUNT(*) FROM assistant_turns turn WHERE turn.conversation_id = conversation.id) AS turn_count \
             FROM agent_conversation_shares share \
             JOIN assistant_conversations conversation ON conversation.id = share.conversation_id \
             JOIN users owner ON owner.id = share.shared_by \
             WHERE share.workspace_id = ?1 ORDER BY share.shared_at DESC LIMIT 100",
        )
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows
            .iter()
            .map(|row| SharedConversation {
                id: row.get("id"),
                title: row.get("title"),
                shared_at: row.get("shared_at"),
                shared_by_name: row.get("shared_by_name"),
                shared_by_id: row.get("shared_by_id"),
                turn_count: row.get("turn_count"),
            })
            .collect())
    }

    /// A chat that is shared with the workspace, whoever owns it; not found when it is not shared (or no longer).
    pub async fn get_shared_agent_conversation(
        &self,
        conversation_id: &str,
        workspace_id: &str,
    ) -> Result<AgentConversation> {
        sqlx::query_as::<_, super::AgentConversationRow>(&format!(
            "{} WHERE conversation.id = ?1 AND conversation.workspace_id = ?2 AND EXISTS (SELECT 1 FROM agent_conversation_shares share WHERE share.conversation_id = conversation.id)",
            super::AGENT_CONVERSATION_SELECT
        ))
        .bind(conversation_id)
        .bind(workspace_id)
        .fetch_optional(self.reader())
        .await?
        .map(AgentConversation::from)
        .ok_or_else(|| repomemo_domain::not_found!("That shared chat was not found."))
    }

    /// Copies a shared chat into a new chat of the caller's, turn by turn, with nothing left waiting for confirmation.
    pub async fn copy_shared_agent_conversation(
        &self,
        conversation_id: &str,
        workspace_id: &str,
        user_id: &str,
    ) -> Result<AgentConversation> {
        let original = self
            .get_shared_agent_conversation(conversation_id, workspace_id)
            .await?;
        let copy = self
            .create_agent_conversation(workspace_id, user_id, &original.title)
            .await?;
        for turn in self.list_agent_turns(&original.id).await? {
            let request: AgentMessage = turn.request;
            self.append_agent_turn(&copy.id, &turn.label, &request, &neutralize_for_copy(&turn.reply))
                .await?;
        }
        self.get_agent_conversation(&copy.id, user_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::neutralize_for_copy;
    use repomemo_domain::{
        AgentChangeset, AgentProposal, AgentProposalKind, AgentProposalState, AgentReply, AgentRouting,
    };

    #[test]
    fn a_copied_reply_has_nothing_waiting_to_be_confirmed() {
        let mut reply = AgentReply {
            capability: None,
            routing: AgentRouting::Agent,
            subject: String::new(),
            reply_markdown: "Done".to_owned(),
            generated: true,
            files: Vec::new(),
            matches: Vec::new(),
            citations: Vec::new(),
            warnings: Vec::new(),
            context_turns: 0,
            proposals: vec![AgentProposal {
                id: "p1".to_owned(),
                kind: AgentProposalKind::SaveMemory,
                state: AgentProposalState::Pending,
                title: "Save".to_owned(),
                detail: String::new(),
                payload: serde_json::json!({}),
                outcome: None,
                untrusted_sources: Vec::new(),
            }],
            file_set: None,
            memory_cards: Vec::new(),
            changeset: Some(AgentChangeset {
                state: "pending".to_owned(),
                outcome: None,
            }),
            trust: None,
        };
        let copy = neutralize_for_copy(&reply);
        assert_eq!(copy.proposals[0].state, AgentProposalState::Dismissed);
        assert_eq!(copy.changeset.unwrap().state, "dismissed");
        // What was already applied stays as it was.
        reply.proposals[0].state = AgentProposalState::Applied;
        assert_eq!(
            neutralize_for_copy(&reply).proposals[0].state,
            AgentProposalState::Applied
        );
    }
}
