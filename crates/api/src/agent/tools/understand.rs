//! The Understand jobs: summaries, answers with citations, the workspace overview and the analysis of several files.

use crate::agent::*;

impl RepoMemoCore {
    pub(in crate::agent) async fn agent_summarize(&self, request: &AgentRequest, subject: &str) -> Result<AgentReply> {
        let provider_id = request.provider_id.clone().unwrap_or_default();
        let target = match request.artifact_id.as_deref() {
            Some(artifact_id) => {
                let detail = self.storage.get_artifact(artifact_id).await?;
                if detail.summary.workspace_id != request.workspace_id {
                    repomemo_domain::bail_invalid!("That file belongs to a different workspace.");
                }
                if self
                    .storage
                    .closed_artifact_ids(&request.workspace_id)
                    .await?
                    .contains(&detail.summary.id)
                {
                    repomemo_domain::bail_invalid!(
                        "That file is in a folder a workspace administrator closed to CAIRN."
                    );
                }
                detail.summary
            }
            None => {
                if subject.is_empty() {
                    return Ok(reply(
                        "Which file should I summarize? Give its name or part of its path.",
                    ));
                }
                let mut matches = self.match_files(&request.workspace_id, subject).await?;
                let clear_winner = matches.len() == 1 || (matches.len() > 1 && matches[0].exact && !matches[1].exact);
                if matches.is_empty() {
                    return Ok(reply(format!(
                        "No file name or path matches “{subject}”. Use Find files to look it up first."
                    )));
                }
                if !clear_winner {
                    let mut result = reply(format!("Several files match “{subject}”. Pick the one to summarize."));
                    result.files = matches
                        .into_iter()
                        .take(FILE_CHOICE_LIMIT)
                        .map(|file| file.artifact)
                        .collect();
                    return Ok(result);
                }
                matches.swap_remove(0).artifact
            }
        };
        let summary = self.summarize_artifact(target.id.clone(), provider_id).await?;
        let mut result = reply(summary.summary_markdown);
        result.generated = true;
        result.files = vec![target];
        result.citations = summary.citations;
        result.warnings = summary.warnings;
        Ok(result)
    }

    pub(in crate::agent) async fn agent_ask(
        &self,
        request: &AgentRequest,
        question: &str,
        history: &[AgentHistoryTurn],
    ) -> Result<AgentReply> {
        if question.is_empty() {
            return Ok(reply("What would you like to know?"));
        }
        let answer = self
            .ask_workspace_in_conversation(
                AskRequest {
                    workspace_id: request.workspace_id.clone(),
                    question: question.to_owned(),
                    provider_id: request.provider_id.clone(),
                    limit: Some(8),
                },
                history,
            )
            .await?;
        let mut result = reply(answer.answer_markdown);
        // With no matching context the core answers without calling the provider.
        result.generated = !answer.retrieved_context.is_empty();
        if result.generated {
            result.context_turns = history.len() as i64;
        }
        // Only the passages the answer cites are its evidence (V0.9.4).
        let (text, citations) = evidence_for(&result.reply_markdown, answer.citations);
        result.reply_markdown = text;
        result.citations = citations;
        // A fixed refusal or an insufficient answer says what to try next, in CAIRN's own voice (V0.9.5).
        if result.citations.is_empty() && says_insufficient(&result.reply_markdown) {
            if answer.retrieved_context.is_empty() {
                result.reply_markdown = "I couldn't find anything in the indexed files that answers this.".to_owned();
            }
            result.reply_markdown.push_str(&next_steps(question));
        }
        result.warnings = answer.warnings;
        Ok(result)
    }

    pub(in crate::agent) async fn agent_overview(&self, request: &AgentRequest) -> Result<AgentReply> {
        let summary = self
            .summarize_workspace(
                request.workspace_id.clone(),
                request.provider_id.clone().unwrap_or_default(),
            )
            .await?;
        let mut result = reply(summary.summary_markdown);
        result.generated = true;
        result.citations = summary.citations;
        result.warnings = summary.warnings;
        Ok(result)
    }

    /// Summarizing the whole chat (V0.9.13): the conversation as stored, not the few turns that fit CAIRN's memory.
    pub(in crate::agent) async fn agent_summarize_chat(&self, request: &AgentRequest) -> Result<AgentReply> {
        if request.exchanges.is_empty() {
            return Ok(reply(
                "There is nothing to summarize yet: this chat has no earlier messages. Ask me something first.",
            ));
        }
        let provider_id = request.provider_id.clone().unwrap_or_default();
        let settings = self.storage.get_provider_settings(&provider_id).await?;
        if !settings.enabled || settings.workspace_id.as_deref() != Some(request.workspace_id.as_str()) {
            repomemo_domain::bail_invalid!("The selected provider is not enabled for this workspace.");
        }
        let provider = provider_from_settings(settings)?;
        let (conversation, cut) = chat_summary::transcript(&request.exchanges);
        let summary = provider
            .generate(GenerateRequest {
                system: Some(format!("{} {}", cairn_persona(), chat_summary::CHAT_SUMMARY_RULES)),
                prompt: "Summarize this conversation.".to_owned(),
                context: conversation,
                options: json!({ "temperature": 0.2 }),
            })
            .await?;
        let mut result = reply(summary.trim().to_owned());
        result.generated = true;
        result.context_turns = request.exchanges.len() as i64;
        result.citations = chat_summary::sources(&request.exchanges);
        // Keeping it as a note is offered, never done (V0.9.14).
        result.proposals = vec![chat_summary::save_proposal(
            &request.exchanges,
            &result.reply_markdown,
            &result.citations,
        )];
        if cut {
            result
                .warnings
                .push("This chat is long, so the summary is based on its most recent messages.".to_owned());
        }
        Ok(result)
    }

    /// Analyzing several files together (V0.9.27): notes on each file's indexed text for what was asked, then one analysis over
    /// the notes. In Jobs mode at most `analyze::MAX_FILES` are read; a file with no indexed text is named as not read.
    pub(in crate::agent) async fn agent_analyze_files(&self, request: &AgentRequest, text: &str) -> Result<AgentReply> {
        self.analyze_with(request, text, None).await
    }

    /// The same over a large set (V0.9.62), run as the loop's tool: the files are read in batches of eight, each batch is noted and
    /// summarized on its own with fresh context, and one last step writes the analysis over the batch summaries. Before each call
    /// the person's AI allowance is checked, the person's Stop is looked for, and progress is told; if the allowance runs out or
    /// the person stops, what was read so far is analyzed and the files not read are listed.
    pub(in crate::agent) async fn analyze_with(
        &self,
        request: &AgentRequest,
        text: &str,
        gate: Option<&analyze::AnalyzeGate>,
    ) -> Result<AgentReply> {
        let lower = text.to_ascii_lowercase();
        let asked = analyze::analyze_request(text, &lower).unwrap_or_else(|| analyze::split_request(text));
        let wanted = if !asked.files.is_empty() {
            asked.files.clone()
        } else if !request.artifact_ids.is_empty() {
            "selected".to_owned()
        } else {
            return Ok(reply("Which files should I analyze? Name a kind or a folder, such as `the Word documents` or `the pdfs in Finance`, or select files in a list first."));
        };
        let resolved = match self.resolve_files(request, &wanted).await? {
            Ok(resolved) => resolved,
            Err(answer) => return Ok(answer),
        };
        let provider_id = request.provider_id.clone().unwrap_or_default();
        let settings = self.storage.get_provider_settings(&provider_id).await?;
        if !settings.enabled || settings.workspace_id.as_deref() != Some(request.workspace_id.as_str()) {
            repomemo_domain::bail_invalid!("The selected provider is not enabled for this workspace.");
        }
        let provider = provider_from_settings(settings)?;
        let total = resolved.files.len();
        let cap = gate.map_or(analyze::MAX_FILES, |gate| gate.max_files.max(analyze::MAX_FILES));
        let focus = asked.focus.clone();
        let question = if focus.is_empty() {
            String::new()
        } else {
            format!(" Question: {focus}")
        };
        let mut notes: Vec<(ArtifactSummary, String, repomemo_domain::Citation)> = Vec::new();
        let mut batch_summaries: Vec<String> = Vec::new();
        let mut batch_start = 0;
        let mut skipped: Vec<String> = Vec::new();
        let mut stopped: Option<&str> = None;
        let mut not_read: Vec<String> = Vec::new();
        let mut read = 0;
        let files = resolved.files.iter().take(cap).collect::<Vec<_>>();
        'files: for (index, file) in files.iter().enumerate() {
            if let Some(gate) = gate {
                if gate.stopped() {
                    stopped = Some("you asked me to stop");
                    not_read = files[index..].iter().map(|file| file.title.clone()).collect();
                    break 'files;
                }
                if gate.allow().is_err() {
                    stopped = Some("your AI allowance is used up");
                    not_read = files[index..].iter().map(|file| file.title.clone()).collect();
                    break 'files;
                }
            }
            let detail = self.storage.get_artifact(&file.id).await?;
            let sections = detail
                .chunks
                .iter()
                .map(
                    |chunk| match chunk.heading_path.as_deref().filter(|heading| !heading.is_empty()) {
                        Some(heading) => format!("{heading}\n{}", chunk.text),
                        None => chunk.text.clone(),
                    },
                )
                .collect::<Vec<_>>();
            let Some(first) = detail.chunks.first() else {
                skipped.push(file.title.clone());
                continue;
            };
            let written = provider
                .generate(GenerateRequest {
                    system: Some(format!("{} {}", cairn_persona(), analyze::NOTES_RULES)),
                    prompt: format!("Take notes on the file '{}'.{question}", file.title),
                    context: analyze::sample(&sections, analyze::PER_FILE_CHARS),
                    options: json!({ "temperature": 0.1 }),
                })
                .await?;
            let cite = repomemo_domain::Citation {
                artifact_id: file.id.clone(),
                chunk_id: Some(first.id.clone()),
                title: file.title.clone(),
                path: file.path.clone(),
                start_line: first.start_line,
                end_line: first.end_line,
                confidence: None,
                commit: None,
                last_author: None,
                last_changed_at: None,
            };
            notes.push(((*file).clone(), written.trim().to_owned(), cite));
            read += 1;
            if let Some(gate) = gate {
                gate.told(format!("{read} of {} files read", files.len()));
            }
            // A batch is done: its notes are summarized on their own, so the last step reads a few summaries and not every note.
            let in_batch = notes.len() - batch_start;
            if gate.is_some() && files.len() > analyze::MAX_FILES && in_batch >= analyze::MAX_FILES {
                let summary = self.summarize_notes(&provider, &notes[batch_start..], &focus).await?;
                batch_summaries.push(summary);
                batch_start = notes.len();
            }
        }
        if notes.is_empty() {
            let mut none = reply(format!(
                "I could not read any of those files{}: {}. A file is read once it is indexed and has text; try again in a moment.",
                stopped.map(|why| format!(" ({why})")).unwrap_or_default(),
                skipped.iter().chain(not_read.iter()).cloned().collect::<Vec<_>>().join(", ")
            ));
            with_file_set(&mut none, resolved.description, resolved.files);
            return Ok(none);
        }
        if gate.is_some() && batch_start < notes.len() && !batch_summaries.is_empty() {
            let summary = self.summarize_notes(&provider, &notes[batch_start..], &focus).await?;
            batch_summaries.push(summary);
        }
        let context = if batch_summaries.is_empty() {
            notes
                .iter()
                .map(|(file, text, _)| format!("## {} ({})\n{text}", file.title, file.path))
                .collect::<Vec<_>>()
                .join("\n\n")
        } else {
            batch_summaries
                .iter()
                .enumerate()
                .map(|(index, summary)| format!("## Group {} of the files\n{summary}", index + 1))
                .collect::<Vec<_>>()
                .join("\n\n")
        };
        let prompt = if focus.is_empty() {
            "Analyze these files together.".to_owned()
        } else {
            format!("Analyze these files for: {focus}")
        };
        let analysis = provider
            .generate(GenerateRequest {
                system: Some(format!("{} {}", cairn_persona(), analyze::ANALYSIS_RULES)),
                prompt,
                context,
                options: json!({ "temperature": 0.2 }),
            })
            .await?;
        let mut result = reply(analysis.trim().to_owned());
        result.generated = true;
        result.citations = notes.iter().map(|(_, _, cite)| cite.clone()).collect();
        let title = if focus.is_empty() {
            format!("Analysis: {}", resolved.description)
        } else {
            format!("Analysis: {} for {focus}", resolved.description)
        };
        result.proposals = vec![chat_summary::note_proposal(
            shorten(&title, 90),
            &result.reply_markdown,
            &result.citations,
            "analysis",
        )];
        if total > cap {
            result.warnings.push(format!(
                "I read the first {cap} of {total} files, which is the most one request reads. Narrow the request, by a folder or a name, to cover the rest."
            ));
        }
        if let Some(why) = stopped {
            result.warnings.push(format!(
                "I stopped after {read} of {} files because {why}. Not read: {}.",
                files.len(),
                not_read.iter().take(20).cloned().collect::<Vec<_>>().join(", ")
                    + &if not_read.len() > 20 {
                        format!(" and {} more", not_read.len() - 20)
                    } else {
                        String::new()
                    }
            ));
        }
        if !skipped.is_empty() {
            result.warnings.push(format!(
                "Not read, because they are not indexed yet or have no text: {}.",
                skipped.join(", ")
            ));
        }
        with_file_set(&mut result, resolved.description, resolved.files);
        Ok(result)
    }

    /// A batch's notes written up as one short summary, so the last step reads the batches and not every file's notes.
    async fn summarize_notes(
        &self,
        provider: &repomemo_ai::ConfiguredProvider,
        batch: &[(ArtifactSummary, String, repomemo_domain::Citation)],
        focus: &str,
    ) -> Result<String> {
        let context = batch
            .iter()
            .map(|(file, text, _)| format!("## {} ({})\n{text}", file.title, file.path))
            .collect::<Vec<_>>()
            .join("\n\n");
        let prompt = if focus.is_empty() {
            "Summarize what these notes say, keeping the file names.".to_owned()
        } else {
            format!("Summarize what these notes say about: {focus}. Keep the file names.")
        };
        Ok(provider
            .generate(GenerateRequest {
                system: Some(format!("{} {}", cairn_persona(), analyze::ANALYSIS_RULES)),
                prompt,
                context,
                options: json!({ "temperature": 0.2 }),
            })
            .await?
            .trim()
            .to_owned())
    }
}
