# Workspace templates security review — 2026-10-22 (V0.2.192, the sixth pass, on the new surface only)

> **Scope:** what track N added (V0.2.163–V0.2.191): the `repomemo-templates` crate and its format, the catalogue and the operator's folder (`REPOMEMO_TEMPLATES_DIR`), the routes that read and change a workspace's template, the dashboard's template field, the download of a workspace's setup as a file, the console commands, System › Overview's Templates panel, the ranking and assistant behaviour a template can change, the checker (`scripts/check-templates.mjs`) and its CI step. Nothing else was re-read: the [fifth review](2026-10-21-security-review.md) stands for the rest of the server.
> **Method:** a static reading of the code at `V0.2.191` (commit `7d27c15`), asking of each place what a person, a workspace administrator or an operator can make it do that they could not before, and what a hostile template file can do. **Nothing was built, started or attacked**, as this repository's rules ask; the two fixes below come with tests that are written and compiled, not run.
> **Status:** closed on 2026-10-22 at `V0.2.192`. **0 high, 2 medium, 2 low, 4 info:** every medium and low finding is fixed, none is open; the four informational findings are accepted or tracked, and the first of them is now said in the guide for template authors.
> **Previous review:** [2026-10-21](2026-10-21-security-review.md) (the fifth pass), closed at V0.2.162.

| ID | Severity | Finding | Status | Release |
|---|---|---|---|---|
| T1 | Medium | A ranking boost names a folder pattern (`path`), and the matcher that applies it took time that grew with every `**` in the pattern. A pattern of eleven `**` against a path of eighty folders is millions of steps for each result, so one template file could stall every search of the workspaces that use it | Fixed in V0.2.192: the matcher's work is the pattern's folders times the path's, whatever the pattern says | V0.2.192 |
| T2 | Medium | The matcher of a repository's own include and exclude patterns (older code) has the same shape with `*` and `**`. Since V0.2.186 a template offers such patterns in the link form, and the person keeps them without reading each one; a pattern like `*a*a*a*a*a*a*a*a*b` stalls the sync thread on a file with a long name | Fixed in V0.2.192: a pattern gives up after 100,000 steps on one path and matches nothing | V0.2.192 |
| T3 | Low | The messages about a template file quote it (an unknown key, a word outside a list, the file's name) as it is. A line break in a key forges a line in the log (as E12 did for the sign-in address), a terminal escape reaches the administrator's terminal through `repomemo-console templates validate`, and a megabyte-long word reaches System › Overview | Fixed in V0.2.192: control and reordering characters are written out (`\n`, `\u{1b}`) and a text is cut at 300 characters, in every message and in the names of the files left out | V0.2.192 |
| T4 | Low | The checker prints the same quotations into the CI log, where a line that starts with `::` is a command to GitHub Actions | Fixed in V0.2.192: the checker writes control characters out and cuts long texts before it prints | V0.2.192 |
| T5 | Info | Every signed-in account can read every template, the operator's custom ones included (`GET /v1/templates` and `/{id}`), and a template's `focus` line is added to the Ask prompt that goes to the workspace's AI provider. A workspace administrator cannot edit a template (they can only choose one and adjust the section it opens on, what is under More and the suggested questions), so the text is the operator's | Accepted, and said in `docs/TEMPLATES.md` and the setting's description: a template holds no secrets, names or addresses | V0.2.192 |
| T6 | Info | The operator's folder is trusted as much as its owner. Someone who can write it could swap a file for a link or a named pipe between the check and the read, or fill the folder with a million entries | Accepted: the reader opens only what a check found to be a plain file, reads at most 64 KiB, and anyone who can write that folder can already change the server's configuration | — |
| T7 | Info | The template resolver keeps the answers of 256 workspaces and drops them all when it is full, so the dashboard of an account that lists more than 256 workspaces settles them again on every load (a copy of a small template each) | Tracked, no change: microseconds a workspace, and only for accounts that list that many | — |
| T8 | Info | The suggested questions a workspace administrator saves are shown to every member as buttons, and a click sends the text as the clicker's own question | Accepted: it is the same text the administrator could write in a message, it is shown as text and not as markup, and the assistant keeps its own rules | — |

| Severity | Count | IDs |
|---|---:|---|
| Critical | 0 | — |
| High | 0 | — |
| Medium | 2 | T1, T2 |
| Low | 2 | T3, T4 |
| Info | 4 | T5, T6, T7, T8 |

---

## 1. Who may read and change a template

| Route | Guard | What it gives | Checked |
|---|---|---|---|
| `GET /v1/templates`, `GET /v1/templates/{id}` | any signed-in account | the catalogue and a template's resolved blocks, built-in and custom | The same for everyone by design: the picker needs it. An unknown id is a `404`; nothing about workspaces is in the answer. T5 |
| `GET /v1/workspaces/{ws}/template` | `require_workspace_read` | the template in force, where it came from, the saved adjustments | A member sees what a member sees all day: the arrangement of their own workspace. A `fallback` reason names the saved id, which passed the id rules when it was saved |
| `PUT /v1/workspaces/{ws}/template` | `ManagingSubject` and `require_workspace_admin` | chooses a template and saves adjustments | Session or `admin`-scope token, as for the other workspace settings. A read-only token is refused; the adjustments are parsed by `Overrides::parse` (known keys, the 12 sections, 6 questions of 120 characters) and capped in size. A change is in the activity feed once |
| `GET /v1/workspaces/{ws}/template/file` | `require_workspace_admin` | the download of V0.2.191 | The file holds only what `GET …/template` already tells a member, so it widens nothing; it is limited to administrators because it sits in Settings. The id and the name are checked by the rules of every template file before anything is sent, and the file name in the header is that checked id |
| `POST /v1/workspaces` with `template_id` | the existing creation rules | the workspace starts with the template's folders | An unknown id is refused before anything is made; the folder names passed the rules (no slash, 40 characters) |
| `GET /v1/dashboard` | the signed-in account | `template` for the workspaces the page lists | The template comes from the settings listed with the account's own workspaces, so another account's workspace is never named |
| `GET /v1/system/overview`, `templates …` in the console | system or app administrator | the templates, the folder, the files left out | Administrators only, like every System route. The folder's path is shown to them, as the data folder is |

**A template never touches a right.** The catalogue's code has no access to roles, tokens, policies or quotas, and the web client only decides what to show: `people` and `settings` are dropped for anyone who is not an administrator, a section under *More* or missing from the tabs still opens by its address, and every route checks the caller as before (decision 26). Checked by reading: no handler reads a template to decide who may do something.

## 2. The operator's folder

| Concern | What happens |
|---|---|
| The path | Set by the operator in the environment, read once; nothing in a template builds a path. Files are listed with `read_dir` and looked up by the names that listing gave; a template's id is a–z, 0–9 and `-` and is only ever compared with the file's name, never joined to a folder |
| Which entries | `*.json` files of the folder itself, not those starting with `_`, in name order, at most 64; a folder inside is not entered |
| Links and special files | `symlink_metadata` decides: a link is a problem and is not followed, anything that is not a plain file is skipped (so a named pipe is never opened). T6 for the swap between the check and the read |
| Size | 64 KiB, checked on the metadata and again while reading (`take` stops one byte past the limit), so a file that grew is still refused. UTF-8 text only |
| Depth and shape | Nesting of 8 is checked on the text before the JSON reader runs; unknown keys, words outside a list and numbers outside their bounds make the file invalid |
| Shadowing | A file cannot take the id of `general` or of a built-in template, must be named after its id and cannot repeat an id; the catalogue is rebuilt from the built-in templates and the valid files, so a refused file leaves no trace in it |
| Failing | Every problem is a `SourceProblem` returned and shown (System › Overview, the server check, the log); nothing panics or returns an error to the start-up, so a bad file never stops the server. T3 for what the messages quote |
| What a valid file can do | Only what a built-in template can: the closed lists, the bounds of §3, texts without control characters. It cannot grant, run or fetch anything. T1 and T2 for the two patterns, T5 for who reads it |

## 3. Ranking bounds

| What | Bound | Where |
|---|---|---|
| A factor (`verified` or a boost) | 0.8 to 1.3, finite | `factor_problem`, checked for every file |
| The preference for recent evidence | a half-life of 7 to 3650 days, a maximum factor above 1 and at most 1.2; `recency_factor` writes `max(age, 0)` so a future-dated file and an age that is not a number give the factor of a file changed just now | `RankingBlock`, `recency_factor` |
| Boosts | at most 16; each names exactly one of a kind of evidence, a kind of document or a folder pattern | `RankingBlock` |
| The product of what matches one passage | held between 1/1.5 and 1.5 whatever the number of rules | `RankingProfile::multiplier` (`MAX_PRODUCT`) |
| The list | scores are rescaled so none passes 1, the order of passages the profile does not touch is kept, and the sort uses a total order, so a score that is not a number cannot panic it | `apply_profile` |
| A folder pattern | at most 120 characters and 12 folders, no regular expression; the work of matching it is now bounded (T1) | `pattern_problem`, `path_matches` |

A template can therefore move a result up or down by at most half again, can never push a score past 1, and cannot change which passages are searched, only how they are ordered; General is pinned to the figures of V0.2.162 by a test. The caps and the tests that hold them are unchanged by this review.

## 4. Activity records

A change of template or of its adjustments is `workspace_template_changed` with a summary built by the server from the template's name (a validated text) and a fixed sentence; the suggested questions an administrator saves are not copied into it. Choosing the same template again records nothing, so the feed cannot be filled by repeating the call. The feed is for owners and administrators (E6). The download of a file and a read of a template record nothing: they show no more than a member sees.

## 5. What `GET` answers a viewer

A viewer receives the arrangement of their own workspace and nothing about any other: no path of the server, no operator folder, no list of files left out (that is System), no id of another workspace. The `custom` flag says only that a template came from the operator. The adjustments are the administrator's saved JSON, capped in size, and are rendered as text.

## 6. The console, the download and the checker

- **`repomemo-console templates validate <file>`** reads a file of the machine it runs on, with the folder's limits (a link is not followed, 64 KiB, UTF-8), prints cleaned messages (T3) and needs no sign-in because it sends nothing anywhere. Sent to the server it is a `400`: the server never opens a path a console user typed.
- **`templates list` and `templates show`** run on the server for administrators; the id typed is looked up in the catalogue and echoed in the not-found message as JSON text.
- **The download** builds its text from the template in force, re-reads it with the parser before sending it, and refuses an id that exists on the server, so a file cannot silently shadow anything when it is put back.
- **The checker** reads files of the repository and runs `git show <base>:<path>` with the arguments given as a list (no shell). In CI the base is the pull request's or the push's SHA, supplied by GitHub; its output is cleaned (T4). It does not run any code from the files it reads.

## 7. Looked at and found fine

- No SQL is built from a template: the saved choice is a JSON setting written with bound parameters, and an archive's template reference goes through `WorkspaceTemplateRef::from_setting` (a slug, a version of 1 or more, adjustments that are an object of bounded size) before it is kept.
- Every text a template puts on a page (names, summaries, labels, questions, phrases, the Custom tag, System's panel) is rendered by React as text, never as markup.
- The assistant's phrases only add a route after the router's own rules and map onto the six jobs it already has; every job runs with the caller's rights. The focus line is advice under the unchanged citation rule and AI policy (T5).
- The dashboard resolves templates from settings already loaded, so it adds no query for each workspace and no way to read another account's settings.
- Ids in URLs (`/v1/templates/{id}`) are compared for equality with the catalogue; nothing is joined to a path or a query.

## 8. Fixes in V0.2.192

| Finding | Change | Test (written, not run) |
|---|---|---|
| T1 | `path_matches` is a table over the pattern's folders and the path's folders, with a leading `**` for "may start anywhere" and doubled `**` collapsed | `a_pattern_of_many_double_stars_cannot_stall_a_search` in `repomemo-retrieval` |
| T2 | `glob_matches_bytes` counts its steps and stops at `GLOB_STEPS_MAX` (100,000), answering "no match" | `a_pattern_of_many_stars_gives_up_instead_of_stalling_a_sync` in `repomemo-ingestion` |
| T3 | `clean_text` in `TemplateIssue::new`, `TemplateIssue::from_json` and the names of `SourceProblem`s | `a_message_that_quotes_the_file_carries_no_control_character_and_is_not_long` in `repomemo-templates` |
| T4 | `shown()` in the checker's output | the case "writes out what a file makes a message quote" in `scripts/check-templates.test.mjs` |
| T5 | A paragraph in `docs/TEMPLATES.md` and a sentence in the setting's description | — |
