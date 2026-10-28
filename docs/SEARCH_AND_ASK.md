# Search, Ask and citations

How to find evidence in RepoMemo, how a cited answer is made, and what a citation does and does not prove. For the first steps, see the [user guide](USER_GUIDE.md#3a-five-workflows-to-learn-first); for every rule, the [functional mindmap](../mindmap/FUNCTIONAL_MINDMAP.md#7-retrieval-finding-evidence).

## 1. What works without AI

Search needs no AI provider and no network. These work on a server with every AI feature off:

- **Keyword search** over the indexed passages of a workspace. Every word must match, and words match as prefixes: `auth tok` finds "authentication token".
- **Filters:** file type, language, source, folder, evidence state and updated-between dates; **Code symbols** finds where a function, class or method is defined.
- **Saved searches**, private to you until a workspace owner shares one. Opening one runs it again under your current access; it never stores an old result set.
- **The assistant's** find and search actions, and the knowledge map and health checks.

What needs an AI provider: search **by meaning**, **Ask**, summaries, the AI overview, image descriptions, and the assistant's AI actions. An administrator sets providers up per workspace in *Settings › AI integration*; cloud providers need an explicit confirmation that excerpts leave your server.

## 2. Keyword and semantic search

| | Keyword | By meaning |
|---|---|---|
| Needs | indexed passages | an **embedding** provider for the workspace, and passages already embedded |
| Matches | the words you type, as prefixes | passages that say the same thing in other words |
| If it fails | | search falls back to keywords and says so |

When both are available, results are merged. Each result shows whether it matched **keywords**, **meaning** or **both**; a code definition says it matched a **symbol**. The query and filters stay in the page address, so you can share the search or open it again.

**Example.** A runbook says "restart the connection pool after a failover". Searching `failover` finds it by keyword. Searching `what to do when the database switches over` finds nothing by keyword, but finds the runbook by meaning if an embedding provider is set up. Without one, the page tells you it searched by keywords only.

Outdated and superseded evidence is left out by default; change the *evidence state* filter to include it. Verified evidence ranks a little higher.

The dashboard search covers every workspace you can read. Each workspace is searched under its own rules: where its AI policy does not let you use AI, it is searched by keywords only and the page says so.

## 3. Ask your evidence

**Ask** answers a question in plain language from your evidence. It always runs in this order:

1. **Retrieve.** RepoMemo finds the best matching passages with the search above. This step uses no generation.
2. **Stop if there is nothing.** If no passage matches, the answer is *"Indexed context is insufficient"* and **the AI is not called**. Nothing is invented.
3. **Generate.** Otherwise the question and *only those passages* go to the workspace's text provider, which is told to answer from them alone. The best passages may be re-ranked by AI first.
4. **Return** the answer, labelled as AI-generated, with numbered **citations** that match the `[1]`, `[2]` markers in the text, and any **warnings**, for example that only keyword retrieval could be used.

**Example.** *"Why did we choose SQLite?"* retrieves the decision record and two design notes. The answer says SQLite was chosen for single-node simplicity `[1]` and that Postgres is a later option `[2]`. `[1]` opens `docs/decisions/0004…` at the cited lines. If the records had never been uploaded, the answer would be *"Indexed context is insufficient"*.

## 4. What a citation proves

A citation names a **file and a line range** (for a repository file, also its last author and date). Clicking it opens that exact text.

| A citation shows | It does not show |
|---|---|
| The passage the answer was given | That the answer follows from it: generated text can summarize badly |
| The file, version and lines the passage came from | That the passage is true or current: check the evidence's status (verified, outdated…) |
| That the answer did not use anything outside your indexed evidence | That nothing relevant was missed: only the best-matching passages are sent |

So: **read the cited lines before you rely on an answer.** A number that has no matching passage is a reason to distrust the sentence.

Stored text (your files, notes and memory cards) and generated text (answers, overviews, summaries, image descriptions) are never blurred: generated text is labelled, and a memory card holds what a person chose to keep.

## 5. Missing and changed sources

- **Exports mark what is gone.** An Ask answer exported as Markdown resolves each citation to its current source identifier, file, line range and review state. A source that has been deleted or that you cannot read is written as an explicit **MISSING SOURCE**, not dropped.
- **Memory cards** keep their citations. If the evidence a card relies on changes, disappears or is retired, the card is flagged **Needs review**.
- **A file that changed** since it was cited is opened as it is now; for a repository file, *View at commit* shows it as it was at the commit the citation named, while that commit is still in the repository's copy.
- **A file you cannot read** is never cited to you.

## 6. Provider limits and failures

| Situation | What you see | What to do |
|---|---|---|
| No AI provider, or AI not allowed for your role | Ask and meaning search are unavailable or keyword-only, with a message | Use keyword search, or ask an admin; *Settings* sets who can use AI |
| Embedding provider down | Search falls back to keywords, with a warning | Wait, or an admin checks *Test connection* |
| Text provider down or slow | Ask reports the provider's failure; nothing is invented | Retry; an admin checks the provider |
| Too many AI requests | A rate-limit message | Wait; the server limit is `REPOMEMO_AI_REQUESTS_PER_HOUR` (120 per user by default, `0` is unlimited) |
| Images | They are searchable only through an AI description, made when they are indexed | An admin sets a vision provider, then re-indexes the image |
| A provider address outside the allowed hosts | It cannot be saved | An operator sets `REPOMEMO_AI_ALLOWED_HOSTS` |

Keys are stored encrypted on the server and never sent back to the browser. Setting up a provider and the consent a cloud provider needs: [user guide, § AI](USER_GUIDE.md#10-ai-optional-explicit-safe).
