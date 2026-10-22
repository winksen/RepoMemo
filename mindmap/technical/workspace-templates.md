# Workspace templates: the format, the rules, how to add one

> **Branch of:** [TECHNICAL_MINDMAP.md](../TECHNICAL_MINDMAP.md) · **Roadmap:** [track N, V0.2.163–V0.2.193](../ROADMAP.md#track-n-workspace-templates-v02163v02193), decisions 24–28 · **Plan:** [the workspace templates plan](../plans/2026-10-22-workspace-templates.md) · **Decision record:** [0005](../../docs/decisions/0005-templates-are-data-over-a-closed-set-of-modules.md) · **Status:** designed at V0.2.163; the crate with the `format` and `meta` keys and `general` exists since V0.2.164, and the built-in catalogue since V0.2.165; each other block below is built by the release in its table row, and a block that no release has built yet is not read by anything.

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
- **Where the choice is stored.** The workspace's `settings_json` already holds `ai_min_role` and `icon`, so the choice is one more key, `template` (`{ id, version, overrides }`). No migration for assignment. A later release that stores organization-defined templates adds a table; this track does not.
- **Cost.** The effective template is built once per workspace and kept in the derived-data cache, keyed by the workspace's settings, so a change refreshes it. The search path reads it from memory and adds one pass over the page of results it already has.
- **Ranking.** The verified rule (today a constant factor of 1.1) becomes the first rule of a ranking profile. In `general` the profile is that rule alone with the same factor, so the order of results is what V0.2.162 returned, and a test pins it. Other rules multiply a passage's score by the factor of each rule it matches, the product is capped at 1.5, and scores are rescaled to at most 1 as today.
- **Assistant.** A template reorders and suggests; it may add phrase rules to the keyword router that map onto existing capabilities, and one line of focus for the Ask prompt. It never touches the citation guard, the "insufficient context" answer or the AI policy.
- **Web.** `GET /v1/workspaces/{id}/template` answers the resolved sections, labels, cards and suggestions in the shape the client uses. Components read it through the query cache, like capabilities.

## 4. Resolution

`effective_template(workspace_id)` follows these steps and never fails:

1. Read the workspace's `template` setting. Absent: `general`, no reason (the normal case for every workspace made before track N).
2. Look the id up in the catalogue. Not found: `general`, reason *template not available on this server*.
3. If the stored `version` is higher than the catalogue's, the server is older than the one that saved the choice: `general`, reason *saved by a newer server*. A lower stored version is fine: the catalogue's current file is used, because templates are data that improve.
4. Apply the stored overrides that are still valid against the template's vocabularies. An override that is not valid any more is dropped, not applied.

The answer carries where it came from (`workspace`, `default` or `fallback` with the reason) so Settings can say why a workspace is not on the template it was given.

## 5. Adding a template

Until V0.2.188 the only way is a release: add a JSON file named after the template's id (`operations.json` for `"id": "operations"`) to `crates/templates/definitions/`, and `build.rs` registers it (every `*.json` whose name does not start with `_`, in file-name order). No Rust is edited. `Catalogue::load` leaves out and names a file that does not validate, is not named after its id, repeats an id or tries to replace `general`; the test over the folder fails while any such file exists. The catalogue lists `general` first and the others by name. From V0.2.188 the guide `docs/TEMPLATES.md` and `npm run templates:check` cover the same steps, and from V0.2.189 an operator can drop a file in `REPOMEMO_TEMPLATES_DIR` with no release.

## 6. Decisions 24–28

| # | Decision | As agreed (V0.2.163) |
|---|---|---|
| 24 | Where the work goes, and the freeze | Track N is numbered V0.2.163–V0.2.193, so the tag waits for it; the freeze is lifted for the track and V0.2.193 restores it |
| 25 | What a template is made of | JSON checked against closed vocabularies, built into the binary from `crates/templates/definitions/`, plus an optional operator folder of the same format; no new dependency |
| 26 | Templates and rights | A template never reads or changes a right, the AI policy, a quota or what leaves the server; choosing one needs a workspace administrator with a session or the `admin` token scope; every change is in the activity feed |
| 27 | Existing workspaces | They stay on `general`; a new workspace asks at creation with `general` preselected |
| 28 | What ships first | `general` plus Software engineering, Operations and incidents, Product and decisions and Knowledge base, as data. Organization-defined templates are V0.4.0 direction |

## 7. Not in the track

Organization-defined templates, new sections or modules defined by a template, per-user layouts, different chunking or indexing per template, installing a template from a URL, AI-written templates and moving existing evidence into starter folders are listed with their reasons in the plan's [section 7](../plans/2026-10-22-workspace-templates.md#7-not-in-this-track).
