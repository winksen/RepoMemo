# Workspace templates: the format, the rules, how to add one

> **Branch of:** [TECHNICAL_MINDMAP.md](../TECHNICAL_MINDMAP.md) · **Roadmap:** [track N, V0.2.163–V0.2.193](../ROADMAP.md#track-n-workspace-templates-v02163v02193), decisions 24–28 · **Plan:** [the workspace templates plan](../plans/2026-10-22-workspace-templates.md) · **Decision record:** [0005](../../docs/decisions/0005-templates-are-data-over-a-closed-set-of-modules.md) · **Status:** designed at V0.2.163; the crate with the `format` and `meta` keys and `general` exists since V0.2.164, the built-in catalogue since V0.2.165, a workspace's choice and its resolver since V0.2.166, the routes since V0.2.167, and creating a workspace from a template since V0.2.168, the picker since V0.2.169, the first four templates since V0.2.170, the tab order since V0.2.171, the landing section and the palette's order since V0.2.172, the names since V0.2.173, the Overview's cards since V0.2.174, the evidence ledger's defaults since V0.2.175, the assistant's order and suggestions and the Retrieval chips since V0.2.176, the preview, the adjustments and Settings › Template since V0.2.177, the ranking factor since V0.2.178, the boosts since V0.2.179, and the preference for recent evidence since V0.2.180; each other block below is built by the release in its table row, and a block that no release has built yet is not read by anything.

Every workspace used to be organized the same way: twelve sections in one fixed order, one ranking rule (verified evidence gets 10 % more), one set of health checks, one set of assistant suggestions. A **workspace template** is a small piece of data that tells the backend, the engine and the web client *which modules matter most in this workspace and how they are shown*. It chooses and orders; it never adds code and never touches rights.

---

## 1. Terms

| Term | Meaning |
|---|---|
| **Module** | Something RepoMemo does and that is written in code: a workspace section, a health detector, an assistant capability, a pulse card, an artifact type, a ranking signal. The set of modules is **closed**; it grows only by a release |
| **Template** | A versioned JSON file that names modules and gives them an order, an emphasis or a weight. `general` is the template every workspace has today and reproduces today's behaviour exactly |
| **Effective template** | What a workspace actually uses: its template plus the few overrides an administrator saved. Resolved once on the server and cached; the web client, the engine and the assistant read the same answer |
| **Starter folders** | Folders a template creates when a workspace is *created* from it. Choosing a template later never moves, creates or deletes evidence |
| **Catalogue** | The templates this server knows: the built-in ones (JSON files in `crates/templates/definitions/`) and, from V0.2.189, the operator's folder |

## 2. What a template can say

The file is a JSON object. Every block is optional except `meta`; a missing block means "as today". A block exists in the format only when a release also reads it, so the format never has a key without a reader.

| Key | Says | Draws on (closed vocabulary) | Bound | Read by | Release |
|---|---|---|---|---|---|
| `format` | the format number the file is written for (`1`) | | must equal what this server reads | catalogue | V0.2.164 |
| `meta` | `id`, `version`, `name`, `summary`, `icon` | the 32 workspace icons | id `[a-z0-9-]`, 2–40 characters; name 40, summary 160 | catalogue, picker | V0.2.164 |
| `starter_folders` | folders made at creation | none | 12 folders, 2 levels, 40 characters a name | create workspace | V0.2.168 |
| `sections` | `order`, `more`, `landing` | the 12 `WorkspaceSection` ids | admin-only sections stay admin-only | web | V0.2.171, V0.2.172 |
| `labels` | the display name of a section or an artifact type | section ids, `ArtifactType` ids | 24 characters each | web | V0.2.173 |
| `overview` | which pulse cards, in what order | pulse card ids (existing metrics only) | 8 cards | web | V0.2.174 |
| `evidence` | default view, the types offered first in *Add* | view ids, `ArtifactType` | | web | V0.2.175 |
| `assistant` | capability `order`, `suggestions`, then `phrases` and `focus` | `AgentCapability` | 6 suggestions of 120 characters; 12 phrases; focus 160 characters | web, assistant | V0.2.176, V0.2.185 |
| `ranking` | `verified` factor, boosts by type, document kind or folder pattern, a `recency` preference | `ArtifactType`, document kinds, simple patterns | each factor 0.8–1.3, product at most 1.5; 16 rules | retrieval | V0.2.178–V0.2.180 |
| `health` | which detectors run and in what order, the stale-evidence age | `HealthDetector` | `stale_after_days` 7–3650 | health | V0.2.181, V0.2.182 |
| `review` | the order of the queue's three sources | review source ids | | review queue | V0.2.183 |
| `map` | the order of coverage kinds, which relations are drawn first | map layer ids | | knowledge map | V0.2.184 |
| `repositories` | include and exclude patterns offered when linking | none (`*` and `**` only) | 16 patterns | link form | V0.2.186 |

An unknown key, an unknown value of a closed vocabulary, a repeated entry or a value out of its bound makes the file invalid. There is no "ignore what you do not know": the checker (V0.2.188) and the server say the same thing.

### What a template can never do

- **Grant, widen or hide a right.** Roles, the AI policy, quotas, who may link what and what leaves the server are decided by code that does not read the template. A hidden section is still a valid address, and the server still checks the caller on every route (decision 26).
- **Run anything.** No scripts, no regular expressions, no URLs, no references to files. A template is data checked against closed vocabularies.
- **Lose evidence.** No template deletes, moves, re-indexes or re-labels stored data. Labels are display text.
- **Make a clearly better match lose.** Ranking factors are bounded, so the best match for the words typed still comes first.
- **Be required.** An existing workspace is `general` until an administrator chooses otherwise (decision 27). An unknown id or a newer version falls back to `general` with a reason, and no page fails.

## 3. How it fits together

```
definitions/*.json ──► repomemo-templates (parse, validate, catalogue)
operator folder ─────►        │
                              ▼
workspace settings  ──►  effective template (one resolver, cached per workspace
  { template: {id,            │             and refreshed by a settings change)
    version, overrides} }     │
        ┌─────────────┬───────┴────────┬──────────────┬───────────────┐
        ▼             ▼                ▼              ▼               ▼
   retrieval      health /         assistant     create / link     web client
   ranking        review / map     routing       defaults          (GET …/template)
```

- **One crate.** `crates/templates` (`repomemo-templates`) holds the types, the validation and the catalogue. `repomemo-retrieval`, `repomemo-api` and the server receive the already-resolved values they need (a ranking profile, a list of detectors); only the resolver knows about files.
- **No template id appears in code.** Components and engine code read fields (`ranking.boosts`, `sections.order`), never `if template == "operations"`. `npm run templates:check` (V0.2.188) fails on a template id found outside `definitions/`, the tests and the documents. This is the rule that makes adding a template a data change.
- **Where the choice is stored.** The workspace's `settings_json` already holds `ai_min_role` and `icon`, so the choice is one more key, `template` (`{ id, version, overrides }`, the domain type `WorkspaceTemplateRef`). No migration for assignment. `StorageEngine::workspace_template_ref` reads it (a value that is not shaped like a reference, with an id that is not a short slug, a version of 0 or adjustments that are not an object under 16 KiB, reads as no choice) and `set_workspace_template_ref` writes or clears it. A workspace archive already carries the whole settings object; import writes the reference back when it is well formed, and a server that lacks the template resolves it to `general` (§4). A later release that stores organization-defined templates adds a table; this track does not.
- **Cost.** The effective template is built once per workspace and kept in the derived-data cache, keyed by the workspace's settings, so a change refreshes it. The search path reads it from memory and adds one pass over the page of results it already has.
- **Ranking.** The verified rule (today a constant factor of 1.1) becomes the first rule of a ranking profile. In `general` the profile is that rule alone with the same factor, so the order of results is what V0.2.162 returned, and a test pins it. Other rules multiply a passage's score by the factor of each rule it matches, the product is capped at 1.5, and scores are rescaled to at most 1 as today.
- **Assistant.** A template reorders and suggests; it may add phrase rules to the keyword router that map onto existing capabilities, and one line of focus for the Ask prompt. It never touches the citation guard, the "insufficient context" answer or the AI policy.
- **Web.** `GET /v1/workspaces/{id}/template` answers the resolved sections, labels, cards and suggestions in the shape the client uses. Components read it through the query cache, like capabilities.

## 4. Resolution

`RepoMemoCore::effective_template(workspace_id)` (V0.2.166, in `crates/api/src/template.rs`) reads the saved choice and hands it to `Catalogue::resolve`, which follows these steps and never fails. The answer (`EffectiveTemplate`: the template in force, its origin and the choice as saved) is kept per workspace and reused while the saved choice is the same; deleting the workspace forgets it:

1. Read the workspace's `template` setting. Absent: `general`, no reason (the normal case for every workspace made before track N).
2. Look the id up in the catalogue. Not found: `general`, reason *template not available on this server*.
3. If the stored `version` is higher than the catalogue's, the server is older than the one that saved the choice: `general`, reason *saved by a newer server*. A lower stored version is fine: the catalogue's current file is used, because templates are data that improve.
4. Apply the stored overrides that are still valid against the template's vocabularies. An override that is not valid any more is dropped, not applied.

The answer carries where it came from (`workspace`, `default` or `fallback` with the reason) so Settings can say why a workspace is not on the template it was given.

## 5. The routes

| Route | For | Does |
|---|---|---|
| `GET /v1/templates` | anyone signed in | The catalogue as a picker lists it, `general` first: id, version, name, summary, icon, none of the blocks |
| `GET /v1/workspaces/{id}/template` | any member | The template in force, where it came from (`default`, `workspace` or `fallback` with the reason), the id the workspace chose and its saved adjustments |
| `PUT /v1/workspaces/{id}/template` | owners and administrators, with a session or an `admin` token | `{ "template_id": "…" }` chooses a template; `null` returns the workspace to the default. An id the catalogue lacks is `404` and writes nothing. A change is `workspace_template_changed` in the activity feed; choosing the template the workspace already has records nothing |

A read-only or read-and-write token may read a template and may not change it (decision 23). The routes decide nothing about rights: who may open the workspace, and what they may do in it, is settled by the same guards as before.

## 6. Creating a workspace from a template

`POST /v1/workspaces` takes an optional `template_id`. The server settles it before anything is made: an id the catalogue lacks is `400` and creates nothing, not even against the quota. The workspace, its saved choice (`template` in its settings), its starter folders and the "workspace created" activity are written in one transaction (`StorageEngine::create_shared_workspace_from`), so a folder that cannot be made leaves no workspace behind. With no `template_id` nothing is saved and no folder is made, as before; `general` named on purpose saves the choice and makes no folder.

The `starter_folders` block lists folders and, for each, the names of the folders one level below it. At most 12 folders in all; a name is 1 to 40 characters with no slash, backslash, control character or edge space; two folders beside each other cannot share a name, case aside. Starter folders exist only at creation: choosing or changing a template in an existing workspace never creates, moves or deletes anything.

## 7. In the web client

- `lib/sharedApi.ts`: `listSharedTemplates`, `getSharedWorkspaceTemplate`, `saveSharedWorkspaceTemplate`, and `createSharedWorkspace` with an optional template id.
- `web/workspace/data.ts`: `useTemplate(accessToken, workspaceId)` (cache key `workspaceKeys.template`) and `useTemplateCatalogue(accessToken)` (`serverKeys.templates`, kept five minutes: the catalogue changes only when the server restarts). A page and its sections that call the same hook share one request, like `useCapabilities`.
- `components/TemplatePicker.tsx`: a group of radio cards (icon, name, summary), used by the create-workspace form on the dashboard and by the first workspace of the setup wizard. It renders nothing while the catalogue holds `general` alone, and the form then sends no template id, so a server without other templates behaves as it did. General is preselected.

## 8. Sections: the tabs and the More menu

The `sections` block has two lists, `order` and `more`, both optional, drawing on the twelve workspace sections (`overview`, `evidence`, `documents`, `repositories`, `retrieval`, `map`, `health`, `review`, `memory`, `tasks`, `people`, `settings`; a test compares the list with the web client's `WORKSPACE_SECTIONS`). A section may be named once in all. `ResolvedSections::from_block` completes the arrangement: the tabs are `order` followed by every section named in neither list, in the usual order, and the rest are `more`. With no block it is the usual twelve and nothing under More, which is what `general` has. `GET /v1/workspaces/{id}/template` answers it as `sections: { primary, more }`.

The web client (`arrangeSections`, `WorkspaceTopbar`, `WorkspaceMoreMenu`) shows `primary` as tabs and `more` as a **More** menu at the end of the row. It never trusts the arrangement for rights: People and Settings are dropped for anyone who is not an owner or administrator, a section this client has no page for is ignored, a section the template does not name still gets a tab, and the section that is open is always a tab. Routes are untouched, so a section under More still opens by its address.

**The landing section** (V0.2.172) is `sections.landing`: the section a workspace opens on when its address names none. It is one of the twelve sections but not People or Settings (a member would be sent to a page they may not open), and may be one kept under More. The view answers it as `sections.landing` (`overview` when the template names none). In the client, `/workspaces/<id>` (no section; `workspaceHomePath`) is parsed as a workspace route with `landing: true`; `WorkspaceLanding` reads the template and replaces the address with `/workspaces/<id>/<landing>` (so Back leaves the workspace), falling back to Overview when the template cannot be read or names a section this client lacks. The dashboard, the workspace list, the side rail, the System area and the command palette open a workspace by that address; an address that names a section is not redirected. The palette lists the sections in the order of `arrangeSections` (tabs, then More).

## 9. Labels: the names a template gives

The `labels` block renames sections (`labels.sections`, keyed by the twelve section ids) and kinds of evidence (`labels.artifact_types`, keyed by the twelve `ArtifactType` ids: `file`, `markdown_doc`, `code_file`, `image`, `issue`, `pr`, `decision`, `incident`, `runbook`, `api_spec`, `note`, `repository`; a test in the api crate fails to compile until the list in `repomemo-templates` has every one). A name is 1 to 24 characters, one line, with no edge space. It is display text: ids, addresses, rights, the API and the documentation keep the product's own names. `GET …/template` answers `labels` with only the names the template changes.

The client reads them in four places: the tab (and its tooltip, which keeps the usual name), the page title and screen-reader announcement (`pageTitle` through `PageAnnouncer`), the command palette (listed and matched under the new name), and the type tags of the evidence ledger and the Retrieval page (`artifactTypeLabel`, `artifactKindLabel`). A section's own page heading and the cross-workspace search keep the usual words.

## 10. Overview: the pulse cards

`overview.cards` lists the cards of the Overview's *Workspace pulse*, in order: one to eight of `index_coverage`, `indexed_storage`, `fresh_evidence`, `knowledge_density`, `code_symbols`, `sources`, `memory_cards`, `team_access`, `active_tasks`, `at_risk` and `team_discussion`, each once. Every card is a figure the page already loads (the workspace metrics and overview), so choosing cards computes nothing and asks the server for nothing; `web/workspace/pulseCards.tsx` draws them and a test compares its list with the crate's. A card not listed is not shown; the charts below the cards stay. Without a block the nine that have always been shown appear in their order (`DEFAULT_CARDS`), which is what `general` has. `GET …/template` answers the resolved list as `overview.cards`; the client ignores an id it cannot draw and falls back to the usual nine if none is left.

The plan's first proposal named figures the metrics do not carry (repositories in sync, evidence reviewed in 90 days, needing review, duplicates, documents by kind); the four templates use the closest figures that exist (sources, memory cards, index coverage, fresh evidence), and a figure the metrics gain later becomes a card with the release that adds it.

## 11. Evidence: how the ledger opens

`evidence.default_view` is `grid` or `list`; `evidence.types_first` lists, without repeats, the file types of the ledger's *File types* filter (`markdown`, `note`, `text`, `code`, `word`, `excel`, `powerpoint`, `pdf`, `onenote`, `email`, `image`, `repository`: the ledger's own file categories, which a test compares with `FILE_CATEGORIES` in the web client) to put before the others. Without a block the ledger opens as a grid with the usual order, which is what `general` has. `GET …/template` answers both as `evidence`.

In the client the template's view is a default, not a rule: the ledger applies it until the person picks a view themselves, and that choice stays for the visit (the page store). `orderedFileCategories` puts the preferred types first in the template's order and the rest after them in the usual order; a type this client does not list is ignored. The plan called this block's second key "the types offered first in *Add*"; what the ledger offers by type is its filter, so that is where the order applies.

## 12. Assistant and Retrieval: suggestions

`assistant.order` lists, without repeats, the assistant's jobs to show first (`find_files`, `find_symbol`, `search_content`, `summarize_file`, `ask_question`, `workspace_overview`: the `AgentCapability` ids, which a test in the api crate matches on so the list cannot fall behind); the assistant's buttons and the command palette's assistant commands put them first in that order and keep the rest in the server's order. `assistant.suggestions` is up to six questions of up to 120 characters on one line, none repeated (case aside), shown as buttons above the message box while it is empty and no chat has begun; choosing one fills the box and sends nothing, and the message is the person's to send. A template never reaches the citation guard, the "insufficient context" answer or the AI policy: suggestions are text for a box, nothing more.

`retrieval.type_chips` is up to six kinds of evidence (the artifact type ids) the Retrieval page shows as chips above the search, only for kinds the workspace has files of when the facets are known. A chip sets the search's file-type filter when chosen and chosen again clears it; nothing is applied before. This is a block of its own because the plan's table had no key for it; it is read by one page and bounded like the others. `GET …/template` answers both as `assistant` and `retrieval`. The next release of the assistant block (V0.2.185) adds the phrases for the keyword router and one line of focus for the Ask prompt.

## 13. Choosing, previewing and adjusting

`GET /v1/templates/{id}` answers a template as `TemplateDetail`: its resolved sections, names, cards, ledger defaults, assistant and chips, without any workspace's choice, so a person can see what choosing it would do. `PUT /v1/workspaces/{id}/template` takes `{ "template_id", "overrides"? }`. The adjustments (`Overrides`) have three keys that use the rules of the blocks they adjust: `landing` (a section but not People or Settings), `more` (exactly the sections kept under More; the others are tabs) and `suggestions` (replacing the template's questions). Left out they stay when the same template is chosen again, `{}` clears them, choosing another template or returning to the default drops them, and a bad one is `400` naming its key under `overrides`. They are saved in the workspace's `template` setting; the resolver lays them over the template (`TemplateDefinition::with_overrides`) before the view is made, ignores them when the choice fell back to `general`, and drops a stale key on its own instead of failing the page.

In the client, Settings › Template (`TemplateSettings`) shows the current template, previews another with `previewLines`, asks before changing it, and saves only what differs from the template's own values (`adjustmentsFrom`) so later improvements to a template still reach every workspace for what was not adjusted. A change is `workspace_template_changed` in the feed ("Changed the workspace template to…", "Adjusted the workspace template.", "Reset the workspace template's adjustments.") and the open page picks it up through the live event. What a template would boost in search is added to the preview by the ranking releases (V0.2.178–V0.2.180).

## 14. Ranking: how search weighs what it finds

Search finds passages by words and by meaning, fuses the two lists by rank, and then weighs the result with the workspace's **ranking profile**: a list of rules, each a factor for the passages it matches. The product of the factors that match a passage multiplies its score, the scores are rescaled to at most 1 and the list is sorted again. `repomemo-retrieval` owns the profile (`RankingProfile`, `apply_profile`); `hybrid_search` takes it from the caller, and the core builds it from the workspace's effective template (`RepoMemoCore::ranking_profile`) for search, Ask, the assistant and the search across workspaces. If the template cannot be read the usual profile is used: a ranking must never be the reason a search fails.

V0.2.178 gives the profile its first rule, *lifecycle*: passages of files in a state (`verified`) by a factor. `ranking.verified` is that factor, from 0.8 to 1.3 (`FACTOR_MIN`, `FACTOR_MAX`), 1.1 when left out. A factor can lower as well as raise, but the bounds keep it a nudge: even at 1.3 a passage that scores 0.6 cannot pass one that scores 1.0. General's profile is the one rule that has always been there, and a test pins it against the figures V0.2.162 produced for the same lists. `GET …/template` and the preview answer `ranking.verified`; Operations and incidents and the Knowledge base use 1.15, the other two 1.1. **Boosts** (V0.2.179) are further rules of the same profile. `ranking.boosts` lists up to 16, each with exactly one selector and a factor from 0.8 to 1.3: an `artifact_type` (one of the twelve kinds of evidence), a `document_kind` (one of the ledger's twelve file types: a document format by its extension first, then the evidence's own type, then plain text, as the ledger classifies a file) or a `path` pattern. A pattern is written with `/` between folders, `*` inside a name and `**` as a whole folder name for any number of folders; it may start anywhere in the path (`docs/adr/**` finds `payments/docs/adr/0003.md`, `README*` the README of any folder) and ignores case. At most 120 characters and 12 folders; nothing that looks like a regular expression (`( ) [ ] { } | ? ^ $` and a backslash) is accepted, and the matcher is a plain walk over the folders, so a pattern cannot cost more than its length. All the factors that match one passage multiply (a verified runbook gets verified x runbook), and the product is held between 1/1.5 and 1.5 (`MAX_PRODUCT`), so sixteen boosts that all match cannot bury a clearly better match: the cap is tested with sixteen rules at 1.3. **A preference for recent evidence** (V0.2.180) is the last rule of the profile. `ranking.recency` has a `half_life_days` (7 to 3650, whole days) and a `max_factor` (above 1, at most 1.2): a file changed just now is favoured by `max_factor`, and the part above 1 halves every half-life, so with 90 days and 1.2 a file a month old gets 1.16 and one a year old 1.01. It is off when the block is left out (General and three of the four templates). The files' times are read for the results of one search only (`artifact_updated_times`, one query over their ids), and a file whose time cannot be read is left as it is. The factor joins the product of the others, so the cap of 1.5 holds for it too. Operations and incidents uses 90 days and 1.2, because a runbook that was touched this quarter is more likely to be right.

## 15. The first templates

`general` is defined in code. Four more are JSON files in `crates/templates/definitions/`: `software-engineering`, `operations`, `product-decisions` and `knowledge-base`, each with its `meta` (icon `code`, `heartbeat`, `compass` and `book`) and `starter_folders` as in the plan's table. The catalogue lists `general` first and the others by name, so the picker shows General, Knowledge base, Operations and incidents, Product and decisions, Software engineering. Every later block is filled in these four files by the release that reads it; nothing in code names one of them. Since V0.2.171 each has its `sections`: Software engineering leads with Overview, Repositories and Retrieval and keeps Documents under More; Operations and incidents leads with Overview, Evidence and Review and keeps Map, Repositories and Documents under More; Product and decisions leads with Overview, Memory and Evidence and keeps Health, Map and Repositories under More; Knowledge base leads with Overview, Documents and Retrieval and keeps Map and Repositories under More.

## 16. Adding a template

Until V0.2.188 the only way is a release: add a JSON file named after the template's id (`operations.json` for `"id": "operations"`) to `crates/templates/definitions/`, and `build.rs` registers it (every `*.json` whose name does not start with `_`, in file-name order). No Rust is edited. `Catalogue::load` leaves out and names a file that does not validate, is not named after its id, repeats an id or tries to replace `general`; the test over the folder fails while any such file exists. The catalogue lists `general` first and the others by name. From V0.2.188 the guide `docs/TEMPLATES.md` and `npm run templates:check` cover the same steps, and from V0.2.189 an operator can drop a file in `REPOMEMO_TEMPLATES_DIR` with no release.

## 17. Decisions 24–28

| # | Decision | As agreed (V0.2.163) |
|---|---|---|
| 24 | Where the work goes, and the freeze | Track N is numbered V0.2.163–V0.2.193, so the tag waits for it; the freeze is lifted for the track and V0.2.193 restores it |
| 25 | What a template is made of | JSON checked against closed vocabularies, built into the binary from `crates/templates/definitions/`, plus an optional operator folder of the same format; no new dependency |
| 26 | Templates and rights | A template never reads or changes a right, the AI policy, a quota or what leaves the server; choosing one needs a workspace administrator with a session or the `admin` token scope; every change is in the activity feed |
| 27 | Existing workspaces | They stay on `general`; a new workspace asks at creation with `general` preselected |
| 28 | What ships first | `general` plus Software engineering, Operations and incidents, Product and decisions and Knowledge base, as data. Organization-defined templates are V0.4.0 direction |

## 18. Not in the track

Organization-defined templates, new sections or modules defined by a template, per-user layouts, different chunking or indexing per template, installing a template from a URL, AI-written templates and moving existing evidence into starter folders are listed with their reasons in the plan's [section 7](../plans/2026-10-22-workspace-templates.md#7-not-in-this-track).
