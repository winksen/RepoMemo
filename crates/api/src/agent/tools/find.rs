//! The Find jobs: files by name, symbols, passages and memory cards, and the helpers that resolve which files a request means.

use crate::agent::*;

impl RepoMemoCore {
    pub(in crate::agent) async fn agent_find_files(&self, workspace_id: &str, subject: &str) -> Result<AgentReply> {
        if subject.is_empty() {
            return Ok(reply(
                "Which file are you looking for? Give a name, part of a path, or an extension like `.pdf`.",
            ));
        }
        // A kind of file or a folder ("the Word documents", "pdfs in Finance") is a filter, not a piece of a name (V0.9.25).
        let filter = parse_filter(subject);
        if filter.is_structured() {
            return self.agent_find_by_filter(workspace_id, &filter).await;
        }
        let matches = self.match_files(workspace_id, subject).await?;
        if matches.is_empty() {
            // Nothing is named like that, but the words may appear inside files.
            let passages = self.search_passages(workspace_id, subject).await?;
            let closest = if passages.is_empty() {
                self.closest_files(workspace_id, subject, CLOSEST_SHOWN)
                    .await
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            let mut result = reply(if passages.is_empty() && !closest.is_empty() {
                format!(
                    "No file names, paths or indexed content match “{subject}”. The closest files are: {}.",
                    closest
                        .iter()
                        .map(|file| file.path.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            } else if passages.is_empty() {
                format!("No file names, paths or indexed content match “{subject}”.")
            } else {
                format!(
                    "No file is named like “{subject}”, but {} indexed {} mention it.",
                    passages.len(),
                    plural(passages.len(), "passage", "passages")
                )
            });
            result.matches = passages;
            return Ok(result);
        }
        let total = matches.len();
        let mut result = reply(format!(
            "Found {total} {} whose name or path matches “{subject}”.{}",
            plural(total, "file", "files"),
            if total > FILE_RESULT_LIMIT {
                format!(" Showing the first {FILE_RESULT_LIMIT}.")
            } else {
                String::new()
            }
        ));
        let found = matches.into_iter().map(|file| file.artifact).collect::<Vec<_>>();
        with_file_set(&mut result, format!("files named like “{subject}”"), found);
        Ok(result)
    }

    /// The files a filter means: of the kind named (by extension), in the folder named and with the words in the name. A folder
    /// that cannot be told is answered with what to say instead.
    pub(in crate::agent) async fn filter_files(
        &self,
        workspace_id: &str,
        filter: &FileFilter,
    ) -> Result<std::result::Result<Vec<ArtifactSummary>, String>> {
        let in_folder = match &filter.folder {
            None => None,
            Some(wanted) => match self.folder_file_ids(workspace_id, wanted).await? {
                Ok(ids) => Some(ids),
                Err(problem) => return Ok(Err(problem)),
            },
        };
        let mut files = self
            .cairn_artifacts(workspace_id)
            .await?
            .into_iter()
            .filter(|file| filter.keeps_kind(file) && filter.keeps_name(file))
            .filter(|file| in_folder.as_ref().is_none_or(|ids| ids.contains(&file.id)))
            .collect::<Vec<_>>();
        files.sort_by_key(|file| file.path.to_lowercase());
        Ok(Ok(files))
    }

    /// The ids of the files in the folder a request names, with its subfolders; or what to say when the folder cannot be told.
    pub(in crate::agent) async fn folder_file_ids(
        &self,
        workspace_id: &str,
        wanted: &str,
    ) -> Result<std::result::Result<std::collections::HashSet<String>, String>> {
        let folders = self.storage.list_folders(workspace_id).await?;
        Ok(match choose_folder(wanted, &folders) {
            FolderChoice::One(found) => {
                let (_, ids) = self.storage.folder_subtree(&found.id).await?;
                Ok(ids.into_iter().collect())
            }
            FolderChoice::Many(names) => Err(format!(
                "Several folders match “{wanted}”: {}. Say which one by its full path.",
                names.join(", ")
            )),
            FolderChoice::Missing(names) if names.is_empty() => Err("This workspace has no folders yet.".to_owned()),
            FolderChoice::Missing(names) => Err(format!(
                "I can't find a folder named “{wanted}”. The folders here are: {}.",
                names.join(", ")
            )),
            FolderChoice::TopLevel => Err("Name a folder to look in; I cannot list only the top level.".to_owned()),
        })
    }

    /// "The Word documents", "pdfs in the Finance folder": every file the filter means, said in words.
    pub(in crate::agent) async fn agent_find_by_filter(
        &self,
        workspace_id: &str,
        filter: &FileFilter,
    ) -> Result<AgentReply> {
        let files = match self.filter_files(workspace_id, filter).await? {
            Ok(files) => files,
            Err(problem) => return Ok(reply(problem)),
        };
        if files.is_empty() {
            return Ok(reply(format!("There are no {} in this workspace.", filter.describe())));
        }
        let total = files.len();
        let mut result = reply(format!(
            "Found {total} {}.{}",
            filter.describe(),
            if total > FILE_RESULT_LIMIT {
                format!(" Showing the first {FILE_RESULT_LIMIT}.")
            } else {
                String::new()
            }
        ));
        with_file_set(&mut result, filter.describe(), files);
        Ok(result)
    }

    /// The workspace's files with these ids. Ids from a client name nothing in other workspaces and nothing that is gone.
    pub(in crate::agent) async fn files_by_ids(
        &self,
        workspace_id: &str,
        ids: &[String],
    ) -> Result<Vec<ArtifactSummary>> {
        let wanted = ids.iter().map(String::as_str).collect::<std::collections::HashSet<_>>();
        Ok(self
            .cairn_artifacts(workspace_id)
            .await?
            .into_iter()
            .filter(|file| wanted.contains(file.id.as_str()))
            .collect())
    }

    pub(in crate::agent) async fn agent_find_symbol(&self, workspace_id: &str, subject: &str) -> Result<AgentReply> {
        let name = subject.trim_matches(|c: char| c == '`' || c == '(' || c == ')' || c.is_whitespace());
        if name.is_empty() {
            return Ok(reply(
                "Which function, class or method are you looking for? Give its name.",
            ));
        }
        let found = self.search_symbols(workspace_id.to_owned(), name.to_owned()).await?;
        if found.is_empty() {
            return Ok(reply(format!(
                "No indexed code defines a symbol named like “{name}”. Only TypeScript, JavaScript, Python, Rust, Go, Java, C# and PHP files are read for symbols, and files still being indexed are not searchable yet."
            )));
        }
        let mut lines = vec![format!(
            "Found {} {} named like “{name}”:",
            found.len(),
            plural(found.len(), "definition", "definitions")
        )];
        let mut artifact_ids = Vec::new();
        for hit in &found {
            let at = hit.symbol.start_line.map(|line| format!(":{line}")).unwrap_or_default();
            lines.push(format!(
                "- `{}` ({:?}) in {}{at}",
                hit.symbol.name, hit.symbol.kind, hit.path
            ));
            if !artifact_ids.contains(&hit.symbol.artifact_id) {
                artifact_ids.push(hit.symbol.artifact_id.clone());
            }
        }
        let mut result = reply(lines.join("\n"));
        for artifact_id in artifact_ids.into_iter().take(FILE_CHOICE_LIMIT) {
            if let Ok(summary) = self.storage.get_artifact_summary(&artifact_id).await {
                result.files.push(summary);
            }
        }
        Ok(result)
    }

    pub(in crate::agent) async fn agent_search(&self, workspace_id: &str, subject: &str) -> Result<AgentReply> {
        if subject.is_empty() {
            return Ok(reply("What should I search for? Give a few words or a phrase."));
        }
        let passages = self.search_passages(workspace_id, subject).await?;
        let mut result = reply(if passages.is_empty() {
            format!(
                "No indexed content mentions “{subject}”. Files that are still being indexed are not searchable yet."
            )
        } else {
            format!(
                "{} indexed {} “{subject}”.",
                passages.len(),
                plural(passages.len(), "passage mentions", "passages mention")
            )
        });
        result.matches = passages;
        Ok(result)
    }

    pub(in crate::agent) async fn search_passages(
        &self,
        workspace_id: &str,
        query: &str,
    ) -> Result<Vec<repomemo_domain::SearchResult>> {
        let mut hits = self
            .search_workspace(SearchRequest {
                workspace_id: workspace_id.to_owned(),
                query: query.to_owned(),
                artifact_types: Vec::new(),
                languages: Vec::new(),
                source_ids: Vec::new(),
                limit: Some(SEARCH_RESULT_LIMIT),
                ..Default::default()
            })
            .await?;
        self.drop_closed_hits(workspace_id, &mut hits).await;
        Ok(hits)
    }

    /// Files whose title or path contains every word of `subject`, best first:
    /// exact names, then names starting with or containing the phrase.
    /// The files whose names are closest to what was asked for, best first (V0.9.140): a misspelled name still finds its file. A file
    /// counts when its title or file name is close to the words, or holds one of them.
    pub(in crate::agent) async fn closest_files(
        &self,
        workspace_id: &str,
        wanted: &str,
        limit: usize,
    ) -> Result<Vec<ArtifactSummary>> {
        let wanted = wanted.trim().to_lowercase();
        if wanted.is_empty() {
            return Ok(Vec::new());
        }
        let mut scored = self
            .cairn_artifacts(workspace_id)
            .await?
            .into_iter()
            .filter_map(|file| {
                let score = name_closeness(&wanted, &file);
                (score >= CLOSE_ENOUGH).then_some((score, file))
            })
            .collect::<Vec<_>>();
        scored.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.1.path.cmp(&b.1.path))
        });
        Ok(scored.into_iter().take(limit).map(|(_, file)| file).collect())
    }

    /// What a person is told when no file matches a name: the closest files by name, and what to try next (V0.9.140). A workspace
    /// with no files says so.
    pub(in crate::agent) async fn no_match_message(&self, workspace_id: &str, wanted: &str, next: &str) -> String {
        let closest = self
            .closest_files(workspace_id, wanted, CLOSEST_SHOWN)
            .await
            .unwrap_or_default();
        if closest.is_empty()
            && self
                .cairn_artifacts(workspace_id)
                .await
                .is_ok_and(|files| files.is_empty())
        {
            return "This workspace has no files yet.".to_owned();
        }
        let mut text = format!("No file name or path matches “{wanted}”.");
        if !closest.is_empty() {
            text.push_str(&format!(
                " The closest files are: {}.",
                closest
                    .iter()
                    .map(|file| file.path.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        text.push(' ');
        text.push_str(next);
        text
    }

    /// The files of a workspace that CAIRN may read: those outside the folders a workspace administrator closed to it (V0.9.93).
    pub(in crate::agent) async fn cairn_artifacts(&self, workspace_id: &str) -> Result<Vec<ArtifactSummary>> {
        let closed = self.storage.closed_artifact_ids(workspace_id).await?;
        let mut files = self.storage.list_artifacts(workspace_id).await?;
        if !closed.is_empty() {
            files.retain(|file| !closed.contains(&file.id));
        }
        Ok(files)
    }

    /// Drops the passages of closed folders from what a search found.
    pub(in crate::agent) async fn drop_closed_hits(
        &self,
        workspace_id: &str,
        hits: &mut Vec<repomemo_domain::SearchResult>,
    ) {
        if let Ok(closed) = self.storage.closed_artifact_ids(workspace_id).await {
            if !closed.is_empty() {
                hits.retain(|hit| !closed.contains(&hit.artifact_id));
            }
        }
    }

    pub(in crate::agent) async fn match_files(&self, workspace_id: &str, subject: &str) -> Result<Vec<FileMatch>> {
        let phrase = subject.to_lowercase();
        let terms = phrase
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|term| !term.is_empty())
            .collect::<Vec<_>>();
        if terms.is_empty() {
            return Ok(Vec::new());
        }
        let mut scored = self
            .cairn_artifacts(workspace_id)
            .await?
            .into_iter()
            .filter_map(|artifact| {
                let title = artifact.title.to_lowercase();
                let path = artifact.path.to_lowercase();
                let matches_term = |term: &&str| {
                    title.contains(term)
                        || path.contains(term)
                        || extensions_for(term).iter().any(|ext| path.ends_with(ext))
                };
                if !terms.iter().all(matches_term) {
                    return None;
                }
                let stem = title.rsplit_once('.').map_or(title.as_str(), |(stem, _)| stem);
                let rank = if title == phrase || stem == phrase {
                    0
                } else if title.starts_with(&phrase) {
                    1
                } else if title.contains(&phrase) {
                    2
                } else if terms.iter().all(|term| title.contains(term)) {
                    3
                } else {
                    4
                };
                Some((rank, artifact))
            })
            .collect::<Vec<_>>();
        scored.sort_by(|(rank_a, a), (rank_b, b)| {
            rank_a
                .cmp(rank_b)
                .then_with(|| a.path.len().cmp(&b.path.len()))
                .then_with(|| a.title.cmp(&b.title))
        });
        Ok(scored
            .into_iter()
            .map(|(rank, artifact)| FileMatch {
                exact: rank == 0,
                artifact,
            })
            .collect())
    }

    /// Recalling memory cards (V0.9.29): the newest when no words are given, else those the words find. With none found and an
    /// AI provider, a request that was not explicitly for memory is answered from the files instead, and says so.
    pub(in crate::agent) async fn agent_recall_memory(
        &self,
        request: &AgentRequest,
        subject: &str,
        history: &[AgentHistoryTurn],
    ) -> Result<AgentReply> {
        let lower = subject.to_ascii_lowercase();
        let query = memory::recall_request(subject, &lower).unwrap_or_else(|| clean_subject(subject));
        let found = if query.is_empty() {
            self.list_memory_cards(request.workspace_id.clone()).await?
        } else {
            self.search_memory_cards(request.workspace_id.clone(), query.clone())
                .await?
        };
        if found.is_empty() {
            if !query.is_empty() && request.capability.is_none() && request.provider_id.is_some() {
                let mut answered = self.agent_ask(request, subject.trim(), history).await?;
                answered.reply_markdown = format!(
                    "There is no memory card about “{query}”, so here is what the files say.\n\n{}",
                    answered.reply_markdown
                );
                return Ok(answered);
            }
            return Ok(reply(if query.is_empty() {
                "This workspace has no memory cards yet. After an answer, say `save that as a memory` to make one."
                    .to_owned()
            } else {
                format!("There is no memory card about “{query}”.")
            }));
        }
        let total = found.len();
        let mut result = reply(if query.is_empty() {
            format!(
                "The newest of the workspace's {total} memory {}:",
                plural(total, "card", "cards")
            )
        } else {
            format!(
                "I found {total} memory {} about “{query}”:",
                plural(total, "card", "cards")
            )
        });
        result.memory_cards = found.into_iter().take(MEMORY_CARDS_SHOWN).collect();
        Ok(result)
    }

    /// The files a request means (V0.9.26): those in view (selected, or just listed), a kind or folder of files, or one file by its
    /// name. When it cannot tell, the answer is what to say to CAIRN.
    pub(in crate::agent) async fn resolve_files(
        &self,
        request: &AgentRequest,
        wanted: &str,
    ) -> Result<std::result::Result<ResolvedFiles, AgentReply>> {
        let filter = parse_filter(wanted);
        if is_set_reference(wanted) {
            let ids = if request.artifact_ids.is_empty() {
                request
                    .exchanges
                    .last()
                    .map(|last| last.file_ids.clone())
                    .unwrap_or_default()
            } else {
                request.artifact_ids.clone()
            };
            let files = self.files_by_ids(&request.workspace_id, &ids).await?;
            if files.is_empty() {
                return Ok(Err(reply("I am not sure which files you mean. Select files in a list above, or say which: for example `move the Word documents to Contracts`.")));
            }
            return Ok(Ok(ResolvedFiles {
                files,
                description: "the selected files".to_owned(),
            }));
        }
        if filter.is_structured() {
            return Ok(match self.filter_files(&request.workspace_id, &filter).await? {
                Ok(files) if files.is_empty() => {
                    Err(reply(format!("There are no {} in this workspace.", filter.describe())))
                }
                Ok(files) => Ok(ResolvedFiles {
                    files,
                    description: filter.describe(),
                }),
                Err(problem) => Err(reply(problem)),
            });
        }
        let mut matches = self.match_files(&request.workspace_id, wanted).await?;
        if matches.is_empty() {
            return Ok(Err(reply(
                self.no_match_message(&request.workspace_id, wanted, "Use Find files to look it up first.")
                    .await,
            )));
        }
        let clear = matches.len() == 1 || (matches[0].exact && !matches[1].exact);
        if !clear {
            let mut several = reply(format!(
                "Several files match “{wanted}”. Say which one with more of its name or its path, and I will propose the change."
            ));
            several.files = matches
                .into_iter()
                .take(FILE_CHOICE_LIMIT)
                .map(|file| file.artifact)
                .collect();
            return Ok(Err(several));
        }
        let file = matches.swap_remove(0).artifact;
        Ok(Ok(ResolvedFiles {
            description: file.title.clone(),
            files: vec![file],
        }))
    }
}

/// The files a request means, and how to say them ("Word documents", "the selected files").
pub(in crate::agent) struct ResolvedFiles {
    pub(in crate::agent) files: Vec<ArtifactSummary>,
    pub(in crate::agent) description: String,
}

/// How many closest files a miss names.
pub(in crate::agent) const CLOSEST_SHOWN: usize = 5;
/// How close a name has to be, from 0 to 1, to be offered.
const CLOSE_ENOUGH: f64 = 0.5;

/// How close the words asked for are to a file's name, from 0 to 1: the best of the title and the file name, with or without its
/// extension, by edit distance; a name that holds the words, or that the words hold, counts as at least 0.8.
fn name_closeness(wanted: &str, file: &ArtifactSummary) -> f64 {
    let name = file.path.rsplit('/').next().unwrap_or(&file.path).to_lowercase();
    let title = file.title.to_lowercase();
    let stem = |text: &str| {
        text.rsplit_once('.')
            .map_or(text.to_owned(), |(stem, _)| stem.to_owned())
    };
    let wanted_stem = stem(wanted);
    let mut best = 0.0_f64;
    for candidate in [name.clone(), stem(&name), title.clone(), stem(&title)] {
        for asked in [wanted, wanted_stem.as_str()] {
            if asked.is_empty() || candidate.is_empty() {
                continue;
            }
            best = best.max(similarity(asked, &candidate));
            if asked.chars().count() >= 3 && (candidate.contains(asked) || asked.contains(candidate.as_str())) {
                best = best.max(0.8);
            }
        }
    }
    best
}

/// `1 - distance / length` of the longer text, by edit distance.
fn similarity(a: &str, b: &str) -> f64 {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let longest = a.len().max(b.len());
    if longest == 0 || a.len().abs_diff(b.len()) * 2 > longest {
        return 0.0;
    }
    let mut previous = (0..=b.len()).collect::<Vec<_>>();
    for (row, left) in a.iter().enumerate() {
        let mut current = vec![row + 1];
        for (column, right) in b.iter().enumerate() {
            let substitution = previous[column] + usize::from(left != right);
            current.push(substitution.min(previous[column + 1] + 1).min(current[column] + 1));
        }
        previous = current;
    }
    1.0 - previous[b.len()] as f64 / longest as f64
}

pub(in crate::agent) struct FileMatch {
    pub(in crate::agent) exact: bool,
    pub(in crate::agent) artifact: ArtifactSummary,
}

/// File extensions meant by a kind word such as "pdfs" or "markdown".
fn extensions_for(term: &str) -> &'static [&'static str] {
    match term.trim_end_matches('s') {
        "markdown" | "md" => &[".md", ".mdx"],
        "word" => &[".doc", ".docx"],
        "excel" | "spreadsheet" => &[".xls", ".xlsx", ".csv"],
        "powerpoint" | "slide" | "presentation" => &[".ppt", ".pptx"],
        "pdf" => &[".pdf"],
        "image" | "picture" | "screenshot" => &[".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg", ".bmp"],
        "email" | "mail" => &[".msg", ".eml"],
        _ => &[],
    }
}
