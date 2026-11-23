//! The Change jobs: each only builds a proposal that the person confirms; nothing here changes the workspace.

use crate::agent::*;

impl RepoMemoCore {
    /// Saving a memory card (V0.9.10): CAIRN proposes it and the person confirms. What it saves is a sentence the person wrote,
    /// else the last answer with the evidence it cited.
    pub(in crate::agent) fn agent_save_memory(&self, request: &AgentRequest, subject: &str) -> AgentReply {
        let lower = subject.to_ascii_lowercase();
        let statement = save_memory_request(subject, &lower).unwrap_or_else(|| subject.trim().to_owned());
        let statement = if statement.chars().count() >= 8 {
            statement
        } else {
            String::new()
        };
        save_memory_reply(&statement, &request.exchanges)
    }

    /// Moving, renaming or trashing one file (V0.9.11): CAIRN finds the file, and the folder for a move, and proposes the
    /// change. It acts only when the file is clearly the one meant; with several candidates it lists them and waits.
    pub(in crate::agent) async fn agent_manage_files(&self, request: &AgentRequest, text: &str) -> Result<AgentReply> {
        let Some(command) = file_command(text, &text.to_ascii_lowercase()) else {
            return Ok(files_help());
        };
        let wanted = match &command {
            files::FileCommand::Move { file, .. }
            | files::FileCommand::Rename { file, .. }
            | files::FileCommand::Trash { file }
            | files::FileCommand::SetStatus { file, .. } => file.clone(),
        };
        let renaming = matches!(command, files::FileCommand::Rename { .. });
        if renaming && (is_set_reference(&wanted) || parse_filter(&wanted).is_structured()) {
            return Ok(reply("I rename one file at a time. Name the file to rename."));
        }
        let targets = match self.resolve_files(request, &wanted).await? {
            Ok(resolved) => resolved.files,
            Err(answer) => return Ok(answer),
        };
        if targets.len() > SET_LIMIT {
            return Ok(reply(format!(
                "That is {} files, and I act on at most {SET_LIMIT} at a time. Narrow it by kind or folder.",
                targets.len()
            )));
        }
        Ok(match command {
            files::FileCommand::Rename { title, .. } => {
                rename_reply(targets.into_iter().next().expect("a file was found"), &title)
            }
            files::FileCommand::Trash { .. } => trash_reply(targets),
            files::FileCommand::SetStatus { status, .. } => status_reply(targets, status),
            files::FileCommand::Move { folder, .. } => {
                let folders = self.storage.list_folders(&request.workspace_id).await?;
                match choose_folder(&folder, &folders) {
                    FolderChoice::TopLevel => move_reply(targets, None),
                    FolderChoice::One(found) => move_reply(targets, Some(&found)),
                    FolderChoice::Many(names) => reply(format!(
                        "Several folders match “{folder}”: {}. Say which one by its full path.",
                        names.join(", ")
                    )),
                    FolderChoice::Missing(names) if names.is_empty() => {
                        reply("This workspace has no folders yet. Create one in Evidence, then ask me again.")
                    }
                    FolderChoice::Missing(names) => reply(format!(
                        "I can't find a folder named “{folder}”. The folders here are: {}.",
                        names.join(", ")
                    )),
                }
            }
        })
    }

    /// Changing the memory (V0.9.30): CAIRN finds the card by its title and proposes the change, which carries the revision it
    /// started from; nothing changes until the person confirms.
    pub(in crate::agent) async fn agent_manage_memory(&self, request: &AgentRequest, text: &str) -> Result<AgentReply> {
        let Some(command) = memory::memory_command(text, &text.to_ascii_lowercase()) else {
            return Ok(reply("I can change, add to, rename or delete a memory card, after you confirm. Try `update the memory card “Retry policy”: Retries wait twice as long`, `rename the memory card “Retry policy” to Backoff policy` or `delete the memory card “Old note”`."));
        };
        let name = match &command {
            memory::MemoryCommand::Replace { card, .. }
            | memory::MemoryCommand::Append { card, .. }
            | memory::MemoryCommand::Rename { card, .. }
            | memory::MemoryCommand::Delete { card } => card.clone(),
        };
        let wanted = name.to_lowercase();
        let mut candidates = self
            .search_memory_cards(request.workspace_id.clone(), name.clone())
            .await?;
        if candidates.is_empty() {
            candidates = self
                .list_memory_cards(request.workspace_id.clone())
                .await?
                .into_iter()
                .filter(|card| card.title.to_lowercase().contains(&wanted))
                .collect();
        }
        let chosen = match candidates.iter().position(|card| card.title.to_lowercase() == wanted) {
            Some(exact) => exact,
            None if candidates.len() == 1 => 0,
            None if candidates.is_empty() => {
                return Ok(reply(format!(
                    "No memory card matches “{name}”. Ask me to list the memory cards to see their titles."
                )))
            }
            None => {
                let mut several = reply(format!(
                    "Several memory cards match “{name}”. Say which one by its full title."
                ));
                several.memory_cards = candidates.into_iter().take(MEMORY_CARDS_SHOWN).collect();
                return Ok(several);
            }
        };
        let card = self.get_memory_card(candidates[chosen].id.clone()).await?;
        if card.card.workspace_id != request.workspace_id {
            return Ok(reply(format!("No memory card matches “{name}”.")));
        }
        Ok(match command {
            memory::MemoryCommand::Replace { text, .. } => {
                memory::update_reply(&card, None, Some(text), "Replace the text of")
            }
            memory::MemoryCommand::Append { text, .. } => memory::update_reply(
                &card,
                None,
                Some(memory::appended(&card.card.body_markdown, &text)),
                "Add a paragraph to",
            ),
            memory::MemoryCommand::Rename { title, .. } => memory::update_reply(&card, Some(&title), None, "Rename"),
            memory::MemoryCommand::Delete { .. } => memory::delete_reply(&card),
        })
    }

    /// Creating a note (V0.9.31): from a sentence the person wrote, or from the last answer with its sources, in the folder
    /// named if one is. CAIRN proposes it and the person confirms.
    pub(in crate::agent) async fn agent_create_note(
        &self,
        request: &AgentRequest,
        subject: &str,
    ) -> Result<AgentReply> {
        let lower = subject.to_ascii_lowercase();
        let asked = notes::note_request(subject, &lower).unwrap_or_else(|| notes::NoteRequest {
            title: None,
            text: subject.trim().to_owned(),
            folder: None,
        });
        let folder = match &asked.folder {
            None => None,
            Some(wanted) => {
                let folders = self.storage.list_folders(&request.workspace_id).await?;
                match choose_folder(wanted, &folders) {
                    FolderChoice::One(found) => Some((found.id, found.path)),
                    FolderChoice::TopLevel => None,
                    FolderChoice::Many(names) => {
                        return Ok(reply(format!(
                            "Several folders match “{wanted}”: {}. Say which one by its full path.",
                            names.join(", ")
                        )))
                    }
                    FolderChoice::Missing(names) if names.is_empty() => {
                        return Ok(reply("This workspace has no folders yet, so the note would go at the top level. Leave the folder out, or create it in Evidence first."))
                    }
                    FolderChoice::Missing(names) => {
                        return Ok(reply(format!(
                            "I can't find a folder named “{wanted}”. The folders here are: {}.",
                            names.join(", ")
                        )))
                    }
                }
            }
        };
        Ok(notes::note_reply(
            &asked,
            &request.exchanges,
            folder.as_ref().map(|(id, path)| (id.as_str(), path.as_str())),
        ))
    }
}
