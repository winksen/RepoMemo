# Workspace templates: V0.2.163 → V0.2.193

> **Status:** planned on 2026-10-22; decisions 24–28 taken as proposed and the track started at V0.2.163. **Line:** after V0.2.162, before `V0.3.0`.
> **For:** the model or person implementing it, one release at a time, in order.
> **Related:** [Roadmap, track N](../ROADMAP.md#track-n-workspace-templates-v02163v02193) · [Release index](../RELEASES_0.2.md) · [Functional mindmap §4](../FUNCTIONAL_MINDMAP.md) · [Technical mindmap](../TECHNICAL_MINDMAP.md) · [DESIGN.md](../../DESIGN.md)

Every workspace is organized the same way today: twelve sections in one fixed order, one ranking rule (verified evidence gets 10 % more), one set of health checks, one set of assistant suggestions. A team that keeps runbooks and incidents does not need the same front door as a team that keeps code, and a team that keeps contracts and policies needs a third.

A **workspace template** is a small piece of data that tells the backend, the engine and the web client *which modules matter most in this workspace and how they are shown*. It chooses and orders; it never adds code and never touches rights.

---

## 1. What a template is

| Term | Meaning |
|---|---|
| **Module** | Something RepoMemo already does and that is written in code: a workspace section, a health detector, an assistant capability, a pulse card, an artifact type, a ranking signal. The set of modules is **closed** and grows only by a release |
| **Template** | A versioned JSON file naming modules and giving them an order, an emphasis or a weight. `general` is the template every workspace has today, and it reproduces today's behaviour exactly |
| **Effective template** | What a workspace actually uses: its template, plus the few overrides an administrator saved. Resolved once on the server and cached; the web client, the engine and the assistant all read the same answer |
| **Starter folders** | Folders a template creates when a workspace is *created* from it. Choosing a template later never moves, creates or deletes evidence |

### What a template can say (the blocks)

Each block is optional. A missing block means "as today". Blocks arrive one per release (§4): the format never has a key without a reader.

| Block | Says | Closed vocabulary it draws on | Bound | Read by | Release |
|---|---|---|---|---|---|
| `meta` | `id`, `version`, `name`, `summary`, `icon` | the 32 workspace icons | id `[a-z0-9-]`, 40 characters | catalogue, picker | V0.2.164 |
| `starter_folders` | folders made at creation | none | 12 folders, 2 levels | create workspace | V0.2.168 |
| `sections` | order, `primary` or `more`, the landing section | `WorkspaceSection` (12) | admin-only sections stay admin-only | web | V0.2.171, V0.2.172 |
| `labels` | the name of a section or an artifact type | section and `ArtifactType` ids | 24 characters each | web | V0.2.173 |
| `overview` | which pulse cards, in what order | pulse card ids (existing metrics only) | 8 cards | web | V0.2.174 |
| `evidence` | default view, types offered first in *Add* | `ArtifactType`, view ids | | web | V0.2.175 |
| `assistant` | capability order, suggested questions | `AgentCapability` | 6 suggestions, 120 characters | web, assistant | V0.2.176, V0.2.185 |
| `ranking` | boosts by type, document kind, folder pattern or lifecycle; a recency preference | `ArtifactType`, document kinds, lifecycle states | each factor 0.8–1.3, product at most 1.5; 16 rules | retrieval | V0.2.178–V0.2.180 |
| `health` | which detectors run, their order, the stale-evidence age | `HealthDetector` | | health | V0.2.181, V0.2.182 |
| `review` | the order of the queue's three sources | review source ids | | review queue | V0.2.183 |
| `map` | coverage order, which relations are emphasised | map layer ids | | knowledge map | V0.2.184 |
| `repositories` | include and exclude rules offered when linking | none (simple `*` and `**` patterns, **no regular expressions**) | 16 patterns | link form | V0.2.186 |

### What a template can never do

- **Grant, widen or hide a right.** Roles, the AI policy, quotas, who may link what and what leaves the server are decided by code that does not read the template. A hidden section is still a valid address, and the server still checks the caller on every route.
- **Run anything.** No scripts, no regular expressions, no URLs, no references to files. A template is data checked against the closed vocabularies above; an unknown key or value fails validation.
- **Lose evidence.** No template deletes, moves, re-indexes or re-labels stored data. Labels are display text.
- **Make a clearly better match lose.** Ranking factors are bounded (§2), so the best match for the words typed still comes first.
- **Be required.** An existing workspace is `general` until an administrator chooses otherwise. An unknown id or a newer format falls back to `general` with a notice, and no page fails.

---

## 2. How it fits together

```
definitions/*.json ──► repomemo-templates (parse, validate, catalogue)
operator folder ─────►        │
                              ▼
workspace settings  ──►  effective template (one resolver, cached per workspace
  { template: {id,            │             and invalidated by a settings change)
    version, overrides} }     │
        ┌─────────────┬───────┴────────┬──────────────┬───────────────┐
        ▼             ▼                ▼              ▼               ▼
   retrieval      health /         assistant     create / link     web client
   ranking        review / map     routing       defaults          (GET …/template)
```

- **One crate, no dependency on the others' internals.** `crates/templates` (`repomemo-templates`) holds the types, validation and catalogue. `repomemo-retrieval`, `repomemo-api` and the server receive the already-resolved values they need (a `RankingProfile`, a list of detectors); only the resolver knows about files.
- **No template id appears in code.** Components and engine code read fields (`ranking.boosts`, `sections.primary`), never `if template == "operations"`. `npm run templates:check` (V0.2.188) fails on a template id found outside `definitions/`, the tests and the docs. This is the rule that makes adding a template a data change.
- **Where it is stored.** The workspace's `settings_json` already holds `ai_min_role` and `icon`, so the choice is one more key (`template`). No migration for assignment. A later release that stores organization-defined templates adds a table; this track does not.
- **Cost.** The effective template is built once per workspace and kept in `DerivedCache` (keyed by the settings, so a change refreshes it). The search path reads it from memory and adds one pass over the page of results it already has.
- **Ranking.** `favour_verified` becomes the first rule of a `RankingProfile`. In `general` the profile is the verified rule alone with the same 1.1 factor, so the order of results is byte-for-byte what V0.2.162 returned; a test pins it. Other rules multiply the score of a passage by the factor of each rule it matches, then the scores are rescaled to at most 1 as `apply_verified_boost` does today. The product of factors is capped at 1.5.
- **Assistant.** A template reorders and suggests; it can add a few phrase rules to the keyword router (`route_by_rules`) that map onto the existing capabilities, and one line of focus for the Ask prompt ("This workspace holds runbooks and incident reports"). It does not touch the citation guard, the "insufficient context" answer or the AI policy.
- **Web.** `GET /v1/workspaces/{id}/template` answers the resolved sections, labels, cards and suggestions in the shape the client uses. Components read it through the query cache (`useTemplate`), like capabilities.

---

## 3. The first templates

The catalogue ships `general` plus four. They are data: each release of block X fills the block for all of them, so every consumer release can be tried by hand with a real template.

| | **General** (`general`) | **Software engineering** (`software-engineering`) | **Operations and incidents** (`operations`) | **Product and decisions** (`product-decisions`) | **Knowledge base** (`knowledge-base`) |
|---|---|---|---|---|---|
| **For** | Today's behaviour, unchanged | Code, repositories, design notes | Runbooks, incidents, on-call knowledge | Specs, decisions, research, meeting notes | Policies, contracts, reports, business documents |
| **Section order** | Today's twelve | Overview, Repositories, Retrieval, Evidence, Map, Health, Review, Memory, Tasks; *More*: Documents | Overview, Evidence, Review, Health, Retrieval, Tasks, Memory; *More*: Map, Repositories, Documents | Overview, Memory, Evidence, Retrieval, Tasks, Review, Documents; *More*: Health, Map, Repositories | Overview, Documents, Retrieval, Evidence, Review, Health, Memory, Tasks; *More*: Map, Repositories |
| **Lands on** | Overview | Overview | Review | Memory | Documents |
| **Labels** | none | none | Evidence → *Runbooks and incidents*, Tasks → *Actions*, Memory → *Lessons* | Memory → *Decisions and memory* | Evidence → *Library* |
| **Pulse cards** | all | symbols, repositories in sync, index coverage, tasks at risk, fresh evidence | evidence reviewed in 90 days, open actions, tasks at risk, fresh evidence | memory cards, needing review, open tasks, fresh evidence | documents by kind, index coverage, needing review, duplicates |
| **Boosts** | verified ×1.1 | verified ×1.1; code, API specs, decisions ×1.1; `docs/adr/**`, `README*` ×1.15 | verified ×1.15; runbooks, incidents ×1.2; recency preferred | verified ×1.1; decisions, notes, Markdown ×1.15 | verified ×1.15; business documents ×1.15 |
| **Health first** | all, today's order | removed symbols, outdated references, stale memory, index failures | stale evidence (90 days), outdated references, stale memory | outdated references, older version active, stale memory | older version active, duplicates, stale evidence (180 days), outdated references |
| **Assistant first** | today's | find symbol, ask, search | ask, search, find files | ask, find files, summarize | find files, ask, summarize |
| **Starter folders** | none | *Architecture*, *Decisions*, *Runbooks* | *Runbooks*, *Incidents*, *Postmortems*, *On-call* | *Decisions*, *Specs*, *Research*, *Meeting notes* | *Policies*, *Contracts*, *Reports*, *Archive* |
| **Link defaults** | none | exclude `node_modules/**`, `target/**`, `dist/**`, `*.lock` | include `**/*.md`, `runbooks/**` | include `**/*.md`, `docs/**` | include `**/*.md`, `docs/**` |

Figures are the first proposal; the maintainer tunes them by trying the workspace. They change in data, not in code.

---

## 4. Decisions to agree before V0.2.163

| # | Decision | Proposal |
|---|---|---|
| 24 | **Where the work goes, and the freeze** | Lift the feature freeze for track N the way decisions 15 and 20 did, and cut the releases as V0.2.163–V0.2.193 so the tag waits for them; V0.2.193 repeats the release checks and restores the freeze. *Alternative:* tag `v0.3.0` first and number the track V0.3.1–V0.3.31 (a new `RELEASES_0.3.md`). The content does not change, only the numbers and the place in the history |
| 25 | **What a template is made of** | JSON files validated against closed vocabularies, built into the binary from `crates/templates/definitions/`, plus an optional operator folder (`REPOMEMO_TEMPLATES_DIR`) of the same format. JSON because it needs no new dependency; the authoring guide and the checker give it the comments a file lacks |
| 26 | **Templates and rights** | A template never reads or changes a right. Choosing one needs a workspace administrator with a session or the `admin` token scope (decision 23); every change is in the activity feed |
| 27 | **Existing workspaces** | They stay `general` and nothing changes until an administrator chooses. A new workspace asks for a template at creation, with `general` preselected; the setup wizard offers it for the first workspace |
| 28 | **What ships first** | `general` plus the four of §3, as data. Organization-defined templates ("save this workspace as a template" for the whole organization) are V0.4.0 direction, because they need their own table, review and sharing rules |

---

## 5. How to work through this track

- **One release per row, in order.** A later row relies on the earlier ones.
- **Each consumer release does three things:** adds its block to the format (type, limits, validation, a test for each limit), reads it in one place, and fills the block for the four templates. `general` must come out unchanged: each release adds or extends a test that compares `general` with the previous behaviour.
- **No template id in code.** Read fields only (§2).
- **Errors and successes are toasts** (`showToast`, `<Toast>`), never inline banners. Backgrounds and text use neutral tokens (CLAUDE.md, DESIGN.md).
- **Checks before each commit:** `npm run typecheck`, `npm run styles:check`, `npm run a11y:check`, `npm run types:check`, `npm run version:check`, `npm run docs:check`. `cargo check` and the other light checks only when the maintainer says so. **Do not** start a server, build, run Vitest or `cargo test` (CLAUDE.md): write the tests and say in the commit when one was written but not run.
- **Every release:** `npm run version:set -- 0.2.N`; a `CHANGELOG.md` entry; its row in [RELEASES_0.2.md](../RELEASES_0.2.md) set to ✅ with the date and what was built; its box in the [roadmap](../ROADMAP.md) ticked; [FUNCTIONAL_MINDMAP.md](../FUNCTIONAL_MINDMAP.md) (§4 and the section the block changes), [TECHNICAL_MINDMAP.md](../TECHNICAL_MINDMAP.md), `technical/workspace-templates.md` (written in V0.2.163) and, for a route or a setting, [technical/api-reference.md](../technical/api-reference.md) and [docs/DEVELOPMENT_COMMANDS.md](../../docs/DEVELOPMENT_COMMANDS.md). A change that needs care for existing servers goes in [UPGRADING.md](../../UPGRADING.md).
- **Commit** with exactly this shape, never pushed:

```
Update V0.2.N

- What changed, in one line
- A second line when needed
- Updated changelog, roadmap, mindmaps and release index
```

---

## 6. The releases

**Type:** 🐞 fix · ✨ feature · 🔧 enhancement · 📘 update (docs, process)

### N1. The model (V0.2.163–V0.2.168)

Nothing a person can see changes until V0.2.169 except the creation request.

#### V0.2.163 · 📘 · The design, the decisions and the format
- **Change:** `mindmap/technical/workspace-templates.md` (the format, the closed vocabularies, resolution, limits, how to add a template, the rules of §1 and §2) and an ADR in `docs/decisions/` (*Templates are data over a closed set of modules*). Decisions 24–28 recorded as agreed or changed.
- **Done when:** the page matches this plan; `npm run docs:check` passes.
- **Commit:** `- Workspace templates designed: a data format over a closed set of modules, with rules that keep rights and stored evidence out of reach`

#### V0.2.164 · ✨ · The template crate
- **Change:** `crates/templates` (`repomemo-templates`): `TemplateDefinition` with the `meta` block, `Limits`, `validate` (unknown keys refused, id, length and icon checks), and `general()`. Added to the workspace; nothing depends on it yet.
- **Done when:** unit tests cover every limit and refusal; `general` validates.
- **Commit:** `- A template crate with the format's identity block, its limits and validation; the General template defined`

#### V0.2.165 · ✨ · The built-in catalogue
- **Change:** `crates/templates/definitions/*.json` read by a `build.rs` that writes the `include_str!` list, so a new file is registered without editing Rust. `Catalogue::{get, list}` in a stable order, `general` first. A test fails if a definition does not validate or two share an id.
- **Done when:** adding a valid file to the folder makes it appear in the catalogue with no other edit (a test uses a temporary copy).
- **Commit:** `- Built-in templates are the JSON files of one folder; a new file is registered without code`

#### V0.2.166 · ✨ · A workspace's template
- **Change:** the workspace setting `template` (`{ id, version, overrides }`) with `StorageEngine::workspace_template_ref` and `set_workspace_template_ref`; `RepoMemoCore::effective_template(workspace_id)` resolves it (absent, unknown or newer than this server → `general`, with the reason) and caches the result in `DerivedCache`. Workspace export writes the reference; import restores it, falling back to `general` for an id this server lacks (`archive.rs`, `archive_import.rs` which today copies `ai_min_role` only).
- **Done when:** a workspace with no setting resolves to `general`; a setting naming a missing template resolves to `general` with a reason; an exported and imported workspace keeps its template.
- **Commit:** `- A workspace remembers its template; one resolver gives every part the same answer and falls back to General when it cannot`

#### V0.2.167 · ✨ · The template routes
- **Change:** `GET /v1/templates` (id, name, summary, icon, version: no blocks), `GET /v1/workspaces/{id}/template` (the effective template and where it came from) for any member, `PUT /v1/workspaces/{id}/template` for owners and administrators with a session or the `admin` scope. A change writes `workspace_template_changed` to the activity feed. Types go in `crates/domain` so `generate-types` carries them to the web client.
- **Done when:** a viewer reads but cannot change; a read-and-write token gets `403` on `PUT`; an unknown id is `404` and writes nothing.
- **Commit:** `- Routes to list templates and to read and change a workspace's template, for administrators, recorded in the activity feed`

#### V0.2.168 · ✨ · Create a workspace from a template
- **Change:** `CreateWorkspaceRequest.template_id` (optional, `general` when absent) and the `starter_folders` block: the folders are created in the same transaction as the workspace. A refused id creates nothing.
- **Done when:** a workspace created with a template that lists folders has exactly those; `general` and no id create none.
- **Commit:** `- A workspace can be created from a template, with the template's starter folders`

### N2. The web client (V0.2.169–V0.2.177)

#### V0.2.169 · 🔧 · Template types and the picker
- **Change:** `useTemplate` and `useTemplateCatalogue` on the query cache (`web/workspace/data.ts`); a template picker (cards with name, summary and icon) in the create-workspace dialog (`web/Dashboard.tsx`) and in the first workspace of the setup wizard (`web/setup/OrganizationStep.tsx`); `createSharedWorkspace` sends `template_id`.
- **Done when:** the dialog creates a workspace with the chosen template; with only `general` in the catalogue it shows no picker.
- **Commit:** `- The create-workspace dialog and the setup wizard offer the available templates`

#### V0.2.170 · ✨ · The four first templates
- **Change:** `definitions/software-engineering.json`, `operations.json`, `product-decisions.json` and `knowledge-base.json` with `meta` and `starter_folders` from §3. No other block yet.
- **Done when:** the picker lists five templates; creating each makes its folders; a workspace of each opens as it did before.
- **Commit:** `- Four templates ship: Software engineering, Operations and incidents, Product and decisions, Knowledge base`

#### V0.2.171 · 🔧 · Tab order and the More menu
- **Change:** the `sections` block (`order`, `more`). `WorkspaceTopbar` (`web/Organization.tsx`) renders the primary sections in the template's order and the rest under a **More** menu; the active section is always visible. `ADMIN_WORKSPACE_SECTIONS` still hides People and Settings from non-administrators. Routes are untouched.
- **Done when:** in a Knowledge base workspace Documents is second and Map is in More; opening `/workspaces/:id/map` directly works.
- **Commit:** `- Workspace tabs follow the template's order, with the lesser sections under More`

#### V0.2.172 · 🔧 · The landing section and the palette
- **Change:** `sections.landing`: opening a workspace with no section goes to it. `paletteEntries.ts` lists sections in the template's order and puts More sections after the primary ones. Admin-only sections keep their rule.
- **Done when:** an Operations workspace opens on Review; the palette's order matches the tabs.
- **Commit:** `- A workspace opens on its template's landing section, and the palette lists sections in the template's order`

#### V0.2.173 · 🔧 · Names from the template
- **Change:** the `labels` block (sections and artifact types). `SECTION_LABELS`, the tab labels, page titles (`pageTitle.ts`), the route announcer and the evidence type chips read the effective label; the English name stays in `aria-label`s where a screen reader needs the product's own word.
- **Done when:** an Operations workspace shows *Actions* for Tasks in the tab, the title and the palette; `general` shows today's words.
- **Commit:** `- Sections and evidence types can be named by the template`

#### V0.2.174 · 🔧 · Overview cards
- **Change:** the `overview` block. `OverviewSection.tsx` shows the listed pulse cards in order, using the metrics it already loads; the charts stay below. No new data is computed.
- **Done when:** a Software engineering workspace shows symbols and repositories in sync first; `general` shows today's cards.
- **Commit:** `- The Overview shows the cards its template asks for, in its order`

#### V0.2.175 · 🔧 · Evidence defaults
- **Change:** the `evidence` block: the default view of the ledger and the types offered first in *Add*. A person's own choice (kept between visits by the page store) wins over the default.
- **Done when:** a Knowledge base workspace opens its ledger as documents; a person who switched views keeps theirs.
- **Commit:** `- The evidence ledger opens in the template's view and offers its types first`

#### V0.2.176 · 🔧 · Assistant suggestions and Retrieval defaults
- **Change:** the `assistant` block's order and suggestions: `AssistantPanel.tsx` shows the suggestions as buttons that fill the box, and the palette's assistant entries follow the order. Retrieval opens with the template's suggested saved filters (type chips), not applied until chosen.
- **Done when:** an Operations workspace suggests *What is the runbook for…*; clicking fills the box and sends nothing.
- **Commit:** `- The assistant suggests questions that fit the workspace's template`

#### V0.2.177 · ✨ · Settings › Template
- **Change:** a **Template** panel in `SettingsSection.tsx`: the current template and its version, a preview of what another would change (order of sections, labels, what would be boosted), the change with a confirmation that says nothing stored is moved, and the overrides an administrator may save (landing section, sections moved to More, suggestions) with **Reset to template**. Overrides go to `PUT …/template`.
- **Done when:** a change is visible on the next navigation and in the activity feed; Reset returns to the template's own values.
- **Commit:** `- Administrators choose and adjust the workspace's template in Settings, with a preview and a reset`

### N3. The engine (V0.2.178–V0.2.187)

#### V0.2.178 · 🔧 · Ranking as data
- **Change:** `RankingProfile` in `repomemo-retrieval` with one rule kind, *lifecycle* (`verified`, factor), replacing `favour_verified`'s constant. The `ranking` block gets its `verified` factor; `hybrid_search` takes the profile from the caller. Search, Ask, the assistant and search across workspaces pass their workspace's profile.
- **Done when:** with `general`, a test that runs the V0.2.162 search fixtures gets the same order and scores; a template with a different verified factor changes them.
- **Commit:** `- Search ranking is described by a profile that comes from the workspace's template; General ranks exactly as before`

#### V0.2.179 · 🔧 · Boosts by type, kind and folder
- **Change:** rule kinds `artifact_type`, `document_kind` and `path` (simple `*` and `**`, matched in Rust, never a regular expression). Factors 0.8–1.3, product capped at 1.5, at most 16 rules. Passage results already carry the file's type and path; the lookup is by artifact id from the page, as `workspace_lifecycle_states` is today.
- **Done when:** a runbook outranks an equally good match from a note in an Operations workspace; a far better note match still wins; the cap is tested with 16 rules that all match.
- **Commit:** `- Evidence of the types, kinds and folders a template cares about ranks a little higher, never above a clearly better match`

#### V0.2.180 · 🔧 · A preference for recent evidence
- **Change:** a `recency` rule (half-life in days, maximum factor 1.2) using the file's updated time. Off in `general`.
- **Done when:** with a 90-day half-life a month-old runbook edges out a year-old one of equal relevance; with the rule off the order is unchanged.
- **Commit:** `- A template can prefer recent evidence, within the same bounds as other boosts`

#### V0.2.181 · 🔧 · Health checks chosen and ordered
- **Change:** the `health` block: `detectors` (which run and in what order) read by `RepoMemoCore::workspace_health` (`crates/api/src/health.rs`); the page and the Review queue show findings in that order. A detector left out is not run; its recorded outcomes stay.
- **Done when:** a Product and decisions workspace lists outdated references first and runs no symbol check; `general` runs every detector as before.
- **Commit:** `- The health checks that run, and their order, follow the workspace's template`

#### V0.2.182 · ✨ · A check for stale evidence
- **Change:** a new `HealthDetector::StaleEvidence`: a live file not updated for the template's `stale_after_days` (and not marked verified within that time). Off in `general`; each finding offers the existing actions (mark outdated, verify, open).
- **Done when:** an Operations workspace flags a runbook untouched for 91 days and not one at 89; `general` reports none; the finding is a normal one in Health and in Review.
- **Commit:** `- A new health check finds evidence nobody has touched for as long as the template allows`

#### V0.2.183 · 🔧 · The review queue's order
- **Change:** the `review` block: the order of the queue's three sources (files needing review, memory cards that changed, open health findings). `ReviewQueuePanel.tsx` and its endpoint keep their rights.
- **Done when:** Operations lists health findings first; `general` keeps today's order.
- **Commit:** `- The review queue lists what the template cares about first`

#### V0.2.184 · 🔧 · The map's emphasis
- **Change:** the `map` block: the order of the coverage kinds and which relations (similar files, memory citations, repository files) are drawn first in `KnowledgeMapPanel` and `crates/api/src/knowledge_map.rs`. Nothing is dropped from the data; the choice is presentation.
- **Done when:** a Software engineering map shows repositories and symbols first; `general` is unchanged.
- **Commit:** `- The knowledge map puts the template's kinds of evidence first`

#### V0.2.185 · 🔧 · The assistant's phrases and focus
- **Change:** the `assistant` block's `phrases` (at most 12 rules, each a phrase and an existing capability) added to `route_by_rules` after the built-in rules, and `focus` (one line, 160 characters) added to the Ask prompt as context. The citation guard, the "insufficient context" answer and the AI policy are untouched.
- **Done when:** "runbook for payments" routes to *find files* in an Operations workspace and to *ask* in `general`; the Ask prompt of `general` is byte-identical to before.
- **Commit:** `- The assistant recognises a template's phrases and knows what the workspace is for, without loosening any rule`

#### V0.2.186 · 🔧 · Defaults when linking a repository
- **Change:** the `repositories` block: include and exclude patterns the link form offers (prefilled, editable, never applied behind the person's back), and returned by `GET …/template` for the form.
- **Done when:** the link form of a Software engineering workspace offers the excludes; the saved link records what the person kept.
- **Commit:** `- The repository link form offers include and exclude rules from the template`

#### V0.2.187 · 🔧 · The template on the dashboard
- **Change:** each workspace card on the dashboard and in the organization menu shows its template's name, and one figure the template names as headline (one of the pulse cards). `GET /v1/dashboard` carries the template id for the page's workspaces only.
- **Done when:** the dashboard makes no extra call per workspace; `general` shows no badge.
- **Commit:** `- Workspace cards name their template and show its headline figure`

### N4. Adding templates later (V0.2.188–V0.2.191)

#### V0.2.188 · 📘 · A checker and an authoring guide
- **Change:** `scripts/check-templates.mjs` and `npm run templates:check`: validates every file of `definitions/` against the same limits (a JSON schema generated by a test from the Rust types, so there is one source), refuses a template id found in code outside `definitions/`, the tests and the docs, and refuses a changed `version` without a change to the file. `docs/TEMPLATES.md`: copy `definitions/_example.json` (not registered), fill the blocks, run the checker, try it in a workspace. A CI step runs it.
- **Done when:** a misspelled section id, a factor of 1.6 and an unknown key each fail with the file and the line; a hard-coded id in a component fails.
- **Commit:** `- A checker and a guide for writing a template; a template id in code, a bad value or an unknown key is refused`

#### V0.2.189 · ✨ · Templates from a folder
- **Change:** `REPOMEMO_TEMPLATES_DIR`: the server reads `*.json` from it at start, with the same validation and limits (64 KiB a file, 64 files, nesting depth 8, no symlinks followed, the id may not shadow a built-in). A bad file is skipped and named in System › Overview and the server check; it never stops the server. Custom templates carry a *Custom* tag in the picker.
- **Done when:** a valid file appears in the picker after a restart; a file with an unknown section is reported and ignored; a file named like a built-in is refused.
- **Commit:** `- Operators can add templates in a folder without a release; invalid files are named and ignored`

#### V0.2.190 · 🔧 · Console and System view
- **Change:** `repomemo-console templates list|show|validate <file>`; a *Templates* list in System › Overview (built-in and custom, with each custom file's state).
- **Done when:** `validate` on a file prints the same errors as the server would; the list shows the problem files.
- **Commit:** `- Templates can be listed and checked from the console, and System shows which are loaded`

#### V0.2.191 · ✨ · Export a workspace's setup as a template
- **Change:** **Download as template file** in Settings › Template: the effective template with the workspace's overrides, as JSON ready for the operator folder (`id` and `name` asked for). Nothing of the workspace's content is in it.
- **Done when:** the file passes `templates:check`; loading it on another server gives the same sections, labels and boosts.
- **Commit:** `- A workspace's adjusted setup can be downloaded as a template file`

### N5. Closing the track (V0.2.192–V0.2.193)

#### V0.2.192 · 📘 · Security pass on the new surface
- **Change:** a short review (`security/` page) of: who may read and change a template, the operator folder (path handling, size, depth, symlinks, shadowing), ranking bounds and their effect on the cap tests, the activity records, and what the `GET` answers to a viewer. Findings fixed in the same release or tracked.
- **Done when:** no high or medium finding is open.
- **Commit:** `- Security review of workspace templates with its fixes`

#### V0.2.193 · 📘 · Release checks and the freeze restored
- **Change:** typecheck, styles, accessibility, types, versions, documentation, container and migrations checks repeated; `templates:check` in CI; `cargo deny` and `npm audit` clean; mindmaps, `technical/workspace-templates.md`, the API reference, `UPGRADING.md` and the changelog match the code; decision 24's freeze restored.
- **Done when:** every check passes; the functional mindmap's gaps table lists what the templates do not do.
- **Commit:** `- Release checks repeated after workspace templates; the freeze restored`

---

## 7. Not in this track

| Item | Why not |
|---|---|
| **Organization-defined templates** ("save as template" for everyone in an organization) | Needs a table, who may publish, and review; a V0.4.0 direction (decision 28). V0.2.191 gives the file route in the meantime |
| **New sections or modules defined by a template** | A module is code. A template orders and weights what exists |
| **Per-user layouts** | A template is the workspace's choice. People already keep filters and views for themselves |
| **Different chunking or indexing per template** | Changes what is stored and forces re-indexing, which a template must never do |
| **Installing a template from a URL** | A new outside connection; it would bring its own security rules (decision 11's pattern) |
| **AI-written templates** | Possible later on top of the closed vocabularies; the checker is what makes it safe |
| **Moving existing evidence into a template's starter folders** | Templates never move stored data. A "suggest a move" action could be a later enhancement of the review queue |
| **Desktop (local edition)** | Retired in V0.2.51 |
