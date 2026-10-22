# Writing a workspace template

A workspace template is a small JSON file. It does not add anything to RepoMemo: it chooses, orders, names and weighs modules RepoMemo already has, so the same product fits a software team, an operations team or a library of policies. This page is for the person who writes one. The design is in [mindmap/technical/workspace-templates.md](../mindmap/technical/workspace-templates.md), and the decision behind it is [0005](decisions/0005-templates-are-data-over-a-closed-set-of-modules.md).

## What a template can and cannot do

- It can arrange the tabs, rename a section or a kind of evidence, choose the Overview's cards, open the ledger as a list, suggest questions to the assistant and teach it a few phrases, weigh search results, choose which health checks run, order the review queue, put some kinds of file first on the map, and offer include and exclude rules when a repository is linked.
- It can never grant, widen or hide a right, run anything, or lose evidence. There are no scripts, no regular expressions and no references to other files. Every value comes from a closed list that the server knows, and a value it does not know makes the file invalid: there is no "ignore what you do not know".
- Leaving a block out means "as it is without templates". The `general` template is exactly that, and it is defined in code: no file can replace it.

## Write one

1. Copy [`crates/templates/definitions/_example.json`](../crates/templates/definitions/_example.json) to `crates/templates/definitions/<id>.json`. The example uses every block and is not registered, because its name starts with `_`. The file is named after its id.
2. Change `meta` first: the `id` (2 to 40 characters of `a-z`, `0-9` and `-`), `version` (start at `1`), `name` (40 characters), `summary` (160 characters) and one of the workspace `icon`s.
3. Keep the blocks you need and delete the others. Each is optional; a block that is present must be complete in the ways its table below says.
4. Run the checker:

   ```bash
   npm run templates:check
   ```

   It reads every file of the folder and says, for each problem, the file, the line and the column, the place in the template, and what is wrong:

   ```text
   crates/templates/definitions/support.json:9:34  sections.order[1]  `evidnce` is not a workspace section
   ```

5. Try it in a workspace. A file in this folder is built into the server (`build.rs` registers it; nothing in Rust is edited), so a new template reaches a server with the next release.
6. When you change a template that was released, raise its `meta.version`. The checker compares the file with the last commit (or `--base <ref>`) and refuses a changed file whose version is the same, and a raised version for a file that did not change. A workspace that chose the template keeps working when the file improves; one saved by a newer server than this one falls back to General and says so.

## The blocks

| Block | What it says | Closed lists and limits |
|---|---|---|
| `format` | the number of the format the file is written for | `1` |
| `meta` | `id`, `version`, `name`, `summary`, `icon` | the id rules above; 32 workspace icons |
| `starter_folders` | folders made when a workspace is created from the template | 12 folders in all, two levels; 40 characters a name |
| `sections` | `order` of the tabs, `more` for the *More* menu, `landing` | the 12 workspace sections, each named once; `people` and `settings` stay for administrators and cannot be the landing |
| `labels` | display names for `sections` and `artifact_types` | the section and artifact-type ids; 24 characters each |
| `overview` | `cards` of the Overview in order; `headline` for the dashboard card | 1 to 8 of the 11 pulse cards; a headline is `memory_cards`, `active_tasks`, `code_symbols` or `sources` |
| `evidence` | `default_view` (`grid` or `list`), `types_first` | the 12 file types of the ledger |
| `assistant` | `order` of its jobs, up to 6 `suggestions`, up to 12 `phrases`, a `focus` line | its 6 jobs; 120 characters a suggestion, 60 a phrase, 160 the focus |
| `retrieval` | `type_chips` offered on the Retrieval page | up to 6 kinds of evidence |
| `ranking` | `verified` factor, `boosts` by kind, document kind or folder pattern, a `recency` preference | each factor 0.8 to 1.3; 16 boosts; recency up to 1.2 |
| `health` | which `detectors` run and in what order, `stale_after_days` | 8 checks; the days are required exactly when `stale_evidence` is listed (7 to 3650) |
| `review` | the order of the queue's three kinds of item | `evidence`, `memory_card`, `health_finding` |
| `map` | `coverage_first` file types, `relations_first` links | the file types; `similar` and `cites` |
| `repositories` | `include` and `exclude` patterns the link form starts with | 16 patterns in each list |

A folder pattern (a ranking `path`, a repository `include` or `exclude`) is names, `*` inside a name and `**` for any number of folders, such as `docs/adr/**`, `README*` or `**/__snapshots__/**`. It is never a regular expression.

The exact lists and numbers are in the Rust source of [`crates/templates`](../crates/templates/src), which is where the checker reads them from. If the checker and this page disagree, the source wins; please correct the page.

## Who sees a template

Every account that can sign in to the server can read every template it offers, the custom ones included: the picker needs them, and the preview in Settings › Template shows each block. A workspace's administrators can choose a template and adjust the section it opens on, what is under *More* and the suggested questions, but they cannot edit its file, so what is in the file is the operator's text. Two of its texts travel further than the page: the `focus` line is added to the system prompt of Ask and goes to the AI provider the workspace uses, and a phrase or a suggested question is typed into the assistant like any message. **A template holds no secrets, no names of customers or people and no addresses**, only the kind of material a workspace is for. Files copied from somewhere else deserve the same reading you would give a script: the checker proves a file is within bounds, not that its words are yours.

## What the checker refuses

- **A bad value.** An unknown key (at the key), a word that is not in its closed list, a number outside its bounds, a text that is empty, too long, padded or has a control character, an entry listed twice, a list that is too long.
- **A file that does not match its id**, an id used twice, and `general`.
- **A changed file with the same version, or a new version with no change** (above).
- **A template id written in code.** Nothing outside the definitions, the tests and the documents may name `operations` or any other template: a component asks the workspace's template for what it arranges, and `general` reproduces the behaviour from before templates. If you need to know which template a workspace has, you are writing a feature that belongs in a block.

The server validates every built-in file when it builds its catalogue, and its answer is the final one; the checker makes the same checks before a release, on a machine with only Node, and says where in the file each problem is.

## Checklist

- [ ] The file is named `<id>.json` and `npm run templates:check` passes.
- [ ] `meta.version` is raised if the template was already released.
- [ ] Nothing in the template needs a right, a script or a file that is not here.
- [ ] It was tried in a workspace: the tabs, the Overview, the ledger, the assistant's questions, a search, Health, Review and the link form.
- [ ] The changelog mentions it.
