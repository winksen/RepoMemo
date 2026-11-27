# V0.9.119 closing record: the CAIRN line

Prepared 2026-11-27. Baseline: V0.9.69 for the administration line (V0.9.70 to V0.9.118); every earlier row since V0.9.0 is listed below. This closes the 0.9.x CAIRN line locally. It is a local candidate, not a published artifact.
**None of the walkthroughs below has been performed**: the author started no server, no browser or desktop app, used no AI provider and ran no live evaluation, executed no test and built nothing (the project's rule is that the maintainer runs and tests the application). Everything in this line was checked by formatting, linting with warnings denied and type-checking only. They are written so the maintainer can do them and record the result; every result cell is pending, so V0.9.119 and every row below stay 🧪 (built, not validated) and not ✅.

## What the CAIRN administration line delivers

| Releases | Capability | Where it is described |
|---|---|---|
| V0.9.70 to V0.9.74 | The plan, the CAIRN administrator role, the administration page and its Administration tab, the run budgets as settings | [plan](../plans/2026-11-26-0.9-cairn-administration.md), [admin guide](../../docs/ADMIN_GUIDE.md) |
| V0.9.75 to V0.9.81 | The model catalog: connections with a write-only sealed key, models of a connection, the providers moved into it, live evaluation status, a fallback model, local and cloud | admin guide, [ADR 0007](../../docs/decisions/0007-cairn-models-are-a-system-catalog-chosen-per-organization.md) |
| V0.9.82 to V0.9.90 | Models by organization and workspace: versioned settings, workspaces that inherit and can only narrow, the model picker in the chat, models by role, the boundary across workspaces, embedding and vision from the catalog, the retired provider forms | admin guide |
| V0.9.91 to V0.9.103 | Guardrails: tool switches, change limits, closed folders, secret and personal-data masking, per-person and organization budgets, strict citations, routine limits, retention, MCP and inbox policy; the usage and history sections | admin guide |
| V0.9.104 to V0.9.108 | Abilities: storage, the administration page, workspace abilities, running one, playbook to ability | admin guide, user guide |
| V0.9.109, V0.9.110 | The organization's instructions and its knowledge workspace | admin guide |
| V0.9.111 to V0.9.116 | Personal memory policy with export and delete, the memory and routines sections, personal preferences and the organization's defaults, the greeting and starter questions | admin guide, user guide |
| V0.9.117 | The [security review](../security/2026-11-27-cairn-administration-review.md): 0 high, 2 medium and 2 low fixed, 2 low open | |
| V0.9.118 | The guides | admin guide, user guide |

Migrations added by the line: 0100 to 0109 (CAIRN administrators, the catalog, organization settings, workspace settings, the model of a chat, budget notices, abilities, workspaces' abilities turned off, the playbook an ability came from, personal preferences). All are locked and additive.

**Scope that was narrower than the plan:** the memory overview (V0.9.112) shows cards, those needing review, the last write and playbooks; it does not show open upkeep findings, unused playbooks or corrections waiting, and it does not turn memory upkeep on for several workspaces at once. **Trying an ability in a scratch chat** on the administration page (V0.9.105) was not built. The weakly verified items are named in the changelog of the release that built each: the cross-workspace model boundary (V0.9.88), the embedding change confirmation (V0.9.89), the closed folders (V0.9.93), the budget and retention queries (V0.9.96, V0.9.97, V0.9.100), the usage grouping (V0.9.102), and everything after V0.9.103, which has almost no test.

## Checks run for this record

| Check | Result | Scope |
|---|---|---|
| Version, migration lock (109 files), documentation links and counts (`npm run docs:check`) | Pass | Local source checks, at each release |
| TypeScript (`npm run typecheck`) | Pass | Compiles every web test |
| Clippy with warnings denied, whole workspace, all targets | Pass | Compiles every Rust test, runs none |
| Rust format | Pass | `cargo fmt --all` |
| Executing any Rust or web test | **Not run** | The maintainer's own run is owed |
| Any migration's SQL | **Not run** | It runs when the server first starts on a database |
| A live evaluation of a model (`npm run cairn:eval`) | **Not run** | Needs a provider and a token; the maintainer runs it |
| CI on a pushed commit | **Not run** | Nothing was pushed |

## The exit journey (on a test server, with a real provider)

A small organization **Alpha** with: a CAIRN administrator, an organization owner, an admin, a member and a viewer; a second organization **Alpha Labs** with its own admin; two workspaces in Alpha, **W1** and **W2**, and a third, **K**, to be its knowledge workspace. Two models: a local one that takes tools and has passed the live evaluation, and a cloud one. Record each step.

1. **Who reaches what.** The CAIRN administrator sees every section of the administration page; an app administrator reads and changes nothing; Alpha's admin sees only the sections for Alpha; a member is refused; a viewer is refused.
2. **The catalog.** Add a connection with a key (it is never shown again and not in the audit text), test it, list its models, add both models, run `npm run cairn:eval` for each model the guides name and attach the report, set a fallback, and see that a cloud connection cannot be on until it is confirmed.
3. **The organization chooses.** Alpha uses both models, with the local one as default and the cloud model limited to admins and owners. W1 offers both, W2 only the local one. In W1 the member sees a picker with the local model; the admin sees both; in W2 only the local one. A chat keeps its model.
4. **Guardrails, one at a time.** Turn off a tool and see it refused with who turned it off; set the changes per confirmation; close a folder in W1 and ask about its files; put a fake key in a file and ask a cloud model about it (masked or refused); mask personal data; set a per-person budget and use it up; set the organization's monthly budget low and see the notices; turn on strict citations; look at the retention preview before saving; turn MCP and the inbox off; limit routines; turn personal memory off.
5. **A workspace cannot loosen.** In W1 try to turn on what Alpha turned off, to raise a limit and to set a budget: each is refused with a reason.
6. **Abilities.** As Alpha's admin make an ability with a field and a tool; run it in W1 from the + menu and by its command (the fields are asked first); open the trace and the answer: "Ability: name, version N". Publish a second version while another person edits (the stale one is refused). As a viewer run it: no proposing tool. In a workspace on Jobs the ability shows as needing Agent mode. Turn an organization ability off in W2. Make a workspace ability from a verified playbook and check that each shows the other.
7. **Instructions and the knowledge workspace.** Write the organization's instructions; the trace of a run begins with their version. Choose K as the knowledge workspace: a person who reads K gets its current card and verified playbook in W2; a person who does not read K gets neither. A chat that used its card cannot be shared.
8. **People.** Set preferences in the profile (language, length, steps open, Agent or Jobs, a preferred model in W1) and see them applied; set Alpha's defaults and see the profile name them; export and delete personal memory, with the organization's policy off.
9. **Figures.** The Memory section shows the cards, those needing review and the playbooks of Alpha's workspaces and marks K; the routines table shows a routine and pauses and resumes it; Usage shows runs and tokens with no message and no person; History shows each change above with who and the values. **As Alpha Labs' admin, History shows none of Alpha's events**, and the reverse.
10. **Greeting.** Write a greeting and starter questions for Alpha and for W2; W2 shows its own, W1 Alpha's.
11. **The old provider forms.** A workspace that had its own provider before the catalog keeps working until it is moved; the old forms answer that they moved (410).

## Record the result

| Step | Result | Date | Notes (what stopped you, if anything) |
|---|---|---|---|
| 1. Who reaches what | pending | | |
| 2. The catalog | pending | | |
| 3. The organization chooses | pending | | |
| 4. Guardrails | pending | | |
| 5. A workspace cannot loosen | pending | | |
| 6. Abilities | pending | | |
| 7. Instructions and the knowledge workspace | pending | | |
| 8. People | pending | | |
| 9. Figures | pending | | |
| 10. Greeting | pending | | |
| 11. The old provider forms | pending | | |

## Live evaluation of each model the guides name

| Model | Report | Passed | Date |
|---|---|---|---|
| The local tool-calling model | `mindmap/technical/cairn-eval-<version>-<model>.md` | pending | |
| The cloud model | `mindmap/technical/cairn-eval-<version>-<model>.md` | pending | |

## Every row of the line, to validate

Walk each row once. A row that fails becomes a numbered fix row inserted before this one; do not edit a row to pass. When every row passes, the test run is recorded and the evaluations pass, change V0.9.119 and the rows below from 🧪 to ✅, or narrow what the guides claim.

| Row | Title | Track | Result | Date | Notes |
|---|---|---|---|---|---|
| V0.9.0 | CAIRN identity | AN. CAIRN agent | pending | | |
| V0.9.1 | Greetings and self-description | AN. CAIRN agent | pending | | |
| V0.9.2 | In-character replies | AN. CAIRN agent | pending | | |
| V0.9.3 | CAIRN voice | AN. CAIRN agent | pending | | |
| V0.9.4 | Evidence of an answer | AN. CAIRN agent | pending | | |
| V0.9.5 | Answers that find nothing | AN. CAIRN agent | pending | | |
| V0.9.6 | Workspace-aware replies | AN. CAIRN agent | pending | | |
| V0.9.7 | Suggested questions | AN. CAIRN agent | pending | | |
| V0.9.8 | Markers and evidence | AN. CAIRN agent | pending | | |
| V0.9.9 | What CAIRN read | AN. CAIRN agent | pending | | |
| V0.9.10 | Proposals and save as memory | AN. CAIRN agent | pending | | |
| V0.9.11 | Manage files | AN. CAIRN agent | pending | | |
| V0.9.12 | Uploads in the chat | AN. CAIRN agent | pending | | |
| V0.9.13 | Summarize chat | AN. CAIRN agent | pending | | |
| V0.9.14 | Chat summary as an indexed note | AN. CAIRN agent | pending | | |
| V0.9.15 | Borderless bar controls | AN. CAIRN agent | pending | | |
| V0.9.16 | Tabs stay in their safe zone | AN. CAIRN agent | pending | | |
| V0.9.17 | Workspace settings look | AN. CAIRN agent | pending | | |
| V0.9.18 | Template choice in Settings | AN. CAIRN agent | pending | | |
| V0.9.19 | Customization section | AN. CAIRN agent | pending | | |
| V0.9.20 | Customization refinements | AN. CAIRN agent | pending | | |
| V0.9.21 | Template choice polish | AN. CAIRN agent | pending | | |
| V0.9.22 | Template adjustments simplified | AN. CAIRN agent | pending | | |
| V0.9.23 | Customization pickers | AN. CAIRN agent | pending | | |
| V0.9.24 | No manual tab adjustments | AN. CAIRN agent | pending | | |
| V0.9.25 | File kinds and folders as filters | AN. CAIRN agent | pending | | |
| V0.9.26 | File sets and bulk file actions | AN. CAIRN agent | pending | | |
| V0.9.27 | Analyze files | AN. CAIRN agent | pending | | |
| V0.9.28 | Evidence state in bulk | AN. CAIRN agent | pending | | |
| V0.9.29 | Memory recall | AN. CAIRN agent | pending | | |
| V0.9.30 | Manage memory | AN. CAIRN agent | pending | | |
| V0.9.31 | Create notes | AN. CAIRN agent | pending | | |
| V0.9.32 | Job catalog | AN. CAIRN agent | pending | | |
| V0.9.33 | CAIRN harness plan | AO. CAIRN harness | pending | | |
| V0.9.34 | Conversation messages for providers | AO. CAIRN harness | pending | | |
| V0.9.35 | Tool calling and model traits | AO. CAIRN harness | pending | | |
| V0.9.36 | OpenAI-compatible provider | AO. CAIRN harness | pending | | |
| V0.9.37 | Tool registry | AO. CAIRN harness | pending | | |
| V0.9.38 | Evidence tools | AO. CAIRN harness | pending | | |
| V0.9.39 | Runs and their trace | AP. CAIRN loop | pending | | |
| V0.9.40 | Bounded tool loop | AP. CAIRN loop | pending | | |
| V0.9.41 | Citation ledger | AP. CAIRN loop | pending | | |
| V0.9.42 | Untrusted content | AP. CAIRN loop | pending | | |
| V0.9.43 | Live runs | AP. CAIRN loop | pending | | |
| V0.9.44 | Changesets | AP. CAIRN loop | pending | | |
| V0.9.45 | Trust signals in answers | AP. CAIRN loop | pending | | |
| V0.9.46 | Evaluation set | AP. CAIRN loop | pending | | |
| V0.9.47 | Agent mode by default | AP. CAIRN loop | pending | | |
| V0.9.48 | Conversation compaction | AQ. CAIRN memory | pending | | |
| V0.9.49 | Workspace instructions | AQ. CAIRN memory | pending | | |
| V0.9.50 | Team memory first | AQ. CAIRN memory | pending | | |
| V0.9.51 | Personal memory | AQ. CAIRN memory | pending | | |
| V0.9.52 | Search past chats | AQ. CAIRN memory | pending | | |
| V0.9.53 | Corrections and memory candidates | AQ. CAIRN memory | pending | | |
| V0.9.54 | Playbooks | AQ. CAIRN memory | pending | | |
| V0.9.55 | Playbooks from runs | AQ. CAIRN memory | pending | | |
| V0.9.56 | Memory upkeep | AQ. CAIRN memory | pending | | |
| V0.9.57 | Shared chats | AR. CAIRN teams | pending | | |
| V0.9.58 | Projects and tasks as tools | AR. CAIRN teams | pending | | |
| V0.9.59 | Task proposals | AR. CAIRN teams | pending | | |
| V0.9.60 | Chats across workspaces | AR. CAIRN teams | pending | | |
| V0.9.61 | Provider boundary across workspaces | AR. CAIRN teams | pending | | |
| V0.9.62 | Background runs over large sets | AS. CAIRN autonomy | pending | | |
| V0.9.63 | Routines | AS. CAIRN autonomy | pending | | |
| V0.9.64 | Event routines | AS. CAIRN autonomy | pending | | |
| V0.9.65 | Review assistant | AS. CAIRN autonomy | pending | | |
| V0.9.66 | MCP endpoint | AT. CAIRN for other agents | pending | | |
| V0.9.67 | CAIRN inbox | AT. CAIRN for other agents | pending | | |
| V0.9.68 | CAIRN security review | AU. CAIRN closing | pending | | |
| V0.9.69 | CAIRN guides | AU. CAIRN closing | pending | | |
| V0.9.70 | CAIRN administration plan | AV. CAIRN administration | pending | | |
| V0.9.71 | CAIRN administrators | AV. CAIRN administration | pending | | |
| V0.9.72 | CAIRN administration page | AV. CAIRN administration | pending | | |
| V0.9.73 | Administration tab in CAIRN's sidebar | AV. CAIRN administration | pending | | |
| V0.9.74 | Run budgets as settings | AV. CAIRN administration | pending | | |
| V0.9.75 | Model catalog storage | AW. Model catalog | pending | | |
| V0.9.76 | Connections | AW. Model catalog | pending | | |
| V0.9.77 | Models of a connection | AW. Model catalog | pending | | |
| V0.9.78 | Providers moved into the catalog | AW. Model catalog | pending | | |
| V0.9.79 | Evaluation status per model | AW. Model catalog | pending | | |
| V0.9.80 | Fallback model | AW. Model catalog | pending | | |
| V0.9.81 | Local and cloud models | AW. Model catalog | pending | | |
| V0.9.82 | Organization CAIRN settings | AX. Models by organization and workspace | pending | | |
| V0.9.83 | Organization models on screen | AX. Models by organization and workspace | pending | | |
| V0.9.84 | Workspaces inherit the organization | AX. Models by organization and workspace | pending | | |
| V0.9.85 | Workspace model list | AX. Models by organization and workspace | pending | | |
| V0.9.86 | Model picker in the chat | AX. Models by organization and workspace | pending | | |
| V0.9.87 | Models by role | AX. Models by organization and workspace | pending | | |
| V0.9.88 | Model boundary across workspaces | AX. Models by organization and workspace | pending | | |
| V0.9.89 | Embedding and vision from the catalog | AX. Models by organization and workspace | pending | | |
| V0.9.90 | Workspace provider forms retired | AX. Models by organization and workspace | pending | | |
| V0.9.91 | Tool switches | AY. Guardrails and limits | pending | | |
| V0.9.92 | Change limits | AY. Guardrails and limits | pending | | |
| V0.9.93 | Folders CAIRN does not read | AY. Guardrails and limits | pending | | |
| V0.9.94 | Secrets kept from cloud models | AY. Guardrails and limits | pending | | |
| V0.9.95 | Personal data masking | AY. Guardrails and limits | pending | | |
| V0.9.96 | Per-person CAIRN budgets | AY. Guardrails and limits | pending | | |
| V0.9.97 | Organization token budget | AY. Guardrails and limits | pending | | |
| V0.9.98 | Strict citations | AY. Guardrails and limits | pending | | |
| V0.9.99 | Routine limits | AY. Guardrails and limits | pending | | |
| V0.9.100 | Retention by organization | AY. Guardrails and limits | pending | | |
| V0.9.101 | MCP and inbox policy | AY. Guardrails and limits | pending | | |
| V0.9.102 | Usage and health | AY. Guardrails and limits | pending | | |
| V0.9.103 | Administration history | AY. Guardrails and limits | pending | | |
| V0.9.104 | Abilities | AZ. Abilities, memory and preferences | pending | | |
| V0.9.105 | Abilities on the administration page | AZ. Abilities, memory and preferences | pending | | |
| V0.9.106 | Workspace abilities | AZ. Abilities, memory and preferences | pending | | |
| V0.9.107 | Running an ability | AZ. Abilities, memory and preferences | pending | | |
| V0.9.108 | Playbook to ability | AZ. Abilities, memory and preferences | pending | | |
| V0.9.109 | Organization instructions | AZ. Abilities, memory and preferences | pending | | |
| V0.9.110 | Organization knowledge workspace | AZ. Abilities, memory and preferences | pending | | |
| V0.9.111 | Personal memory policy | AZ. Abilities, memory and preferences | pending | | |
| V0.9.112 | Memory overview | AZ. Abilities, memory and preferences | pending | | |
| V0.9.113 | Routines overview | AZ. Abilities, memory and preferences | pending | | |
| V0.9.114 | Personal CAIRN preferences | AZ. Abilities, memory and preferences | pending | | |
| V0.9.115 | Organization preference defaults | AZ. Abilities, memory and preferences | pending | | |
| V0.9.116 | Greeting and starter questions | AZ. Abilities, memory and preferences | pending | | |
| V0.9.117 | CAIRN administration security review | AU. CAIRN closing | pending | | |
| V0.9.118 | CAIRN administration guides | AU. CAIRN closing | pending | | |

## Open items carried into 0.10

- From the [administration review](../security/2026-11-27-cairn-administration-review.md): **CR-5** (the audit trail holds the full text of what administrators write) and **CR-6** (no second approval for instructions and abilities).
- From the [CAIRN review of V0.9.68](../security/2026-11-25-cairn-review.md): **CR-4**, **CR-5** and **CR-7** of that review.
- The scope that was narrower than the plan, above.

## Distribution decision

No distribution is newly certified ready by this candidate. The intended validation scope remains a single-node server and web deployment and the Windows desktop client. The 0.10 pilot starts from this candidate once the journey and the evaluations pass. The release tag for a future publication is `vX.Y.Z` ([release tags](../../docs/DEVELOPMENT_COMMANDS.md#release-tags)); nothing was tagged, built or published.
