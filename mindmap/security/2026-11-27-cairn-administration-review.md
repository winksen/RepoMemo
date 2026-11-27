# Security review: the CAIRN administration line (V0.9.117)

> **Date:** 2026-11-27 · **Code:** V0.9.116 reviewed, fixes in V0.9.117 · **Line:** [CAIRN administration plan](../plans/2026-11-26-0.9-cairn-administration.md) · **Release row:** [V0.9.117](../RELEASES_0.9.md) · **Before it:** [the CAIRN review of V0.9.68](2026-11-25-cairn-review.md)

## Scope and method

The surfaces added in V0.9.70 to V0.9.116: the CAIRN administrator role and the administration page (V0.9.71 to V0.9.74); the model catalog, its connections, its sealed keys and the move of the workspaces' providers into it (V0.9.75 to V0.9.90); the organization and workspace settings and what a workspace may loosen (V0.9.82 to V0.9.103); abilities, workspace abilities and playbooks made abilities (V0.9.104 to V0.9.108); the organization's instructions and knowledge workspace (V0.9.109, V0.9.110); personal memory policy, preferences and their defaults (V0.9.111, V0.9.114, V0.9.115); the memory, routines, usage and history sections (V0.9.102, V0.9.103, V0.9.112, V0.9.113); the greeting and starter questions (V0.9.116).

For each, the code was read looking for: a route that a person who should not reach it can reach; a setting that a lower tier can loosen; text of one organization, workspace or person that reaches another; a way to obtain a tool, a right or a model that the person does not have; a secret that is written somewhere it should not be; and what the audit trail holds and who reads it. **This is a review by reading the code and reasoning about it. Nothing was run, no scanner was used, no penetration test was done, and no two-person walkthrough was made. The tests named below compile and were not executed.**

## Findings

| # | Severity | Finding | Status |
|---|---|---|---|
| CR-1 | **Medium** | The History section (V0.9.103) showed an organization's owners and admins the audit events whose **text contained the name** of one of their organizations. A short or common name (`Acme`, `Lab`) is contained in the text of other organizations' events, and those events hold the full text of the organization's instructions (V0.9.109), its welcome, preference defaults and guardrails, before and after. An administrator of one organization could read another's | **Fixed:** the events now lead with `[org:<id>]` (organization settings, abilities, routines) and carry the workspace id (workspace settings, workspace abilities), and the filter matches those ids exactly, the organization's and its workspaces'. **Residual:** events written before this release name an organization by its name only, so owners and admins no longer see them; CAIRN and system administrators still do |
| CR-2 | **Medium** | A reply that used a card of the organization's **knowledge workspace** (V0.9.110) stores that card's title and excerpt in the chat. **Sharing the chat** with its workspace (V0.9.57) would show them to people who cannot read the knowledge workspace, which is the check V0.9.110 makes before offering the card | **Fixed:** `share_agent_conversation` refuses a chat whose replies hold a card of another workspace, with the reason. **Residual:** a chat that was already shared before this fix stays shared until its owner stops sharing it; the chat's replies are also in the person's own export and in their run history, which are theirs |
| CR-3 | Low | The knowledge workspace was read from the organization's settings and checked as a workspace the person can read, but not as **still a workspace of that organization**. One moved out of it or deleted would still be offered | **Fixed:** `knowledge_workspace_for` checks the owner again on each run |
| CR-4 | Low | An organization or a workspace could keep any number of **abilities** | **Fixed:** at most 100 each; a further one says so (conflict) |
| CR-5 | Low, **open** | The audit trail holds the **whole text** of what an administrator writes: instructions, greeting, abilities' names, before and after. Secret masking (V0.9.94) protects what goes to a cloud model, not the trail, so a secret pasted into the instructions would stay in System › Audit and History for as long as events are kept | **Open.** Proposed fix: keep the lengths and a short difference in the event, not the text; or apply the masking to the event text |
| CR-6 | Low, **open** | The organization's instructions, its abilities and its knowledge workspace change what CAIRN does **in every workspace of the organization**, by one administrator, with no second person. Every change is versioned and in History, and none can add a tool or a right (below), but a compromised owner or admin account can steer CAIRN's wording and what it offers | **Accepted as the design of the tier**; **open** as a hardening: proposed fix, a second administrator's approval for instructions and abilities, as the review rules do for files |
| CR-7 | Info | A workspace's guardrails accept the retention fields (V0.9.100), which only an organization's settings read. They do nothing in a workspace | None: no loosening is possible, the fields are not read |
| CR-8 | Info | Reviewed with no finding, **by reading**: see the next section | none |
| CR-9 | Info | **Not reviewed:** the injection corpus of V0.9.42 was not extended for abilities, instructions or the knowledge workspace, and was not run (there is no model here); sealed keys were read for how they are written, shown and sent, not attacked; the live evaluation of the models and the walkthrough with two organizations are V0.9.119 | The line's evidence row is where these are tried |

No high finding. **Open: CR-5 and CR-6.**

## Reviewed with no finding

- **Who reaches the catalog.** Every route that adds, changes, tests, deletes or evaluates a connection or a model requires a CAIRN administrator (`require_cairn_admin`); reading them requires a CAIRN reader. A key is written once, sealed, and the connection shows only whether one is set; the audit text of a connection says "with a key", never the key (`cairn_connections.rs`).
- **Organization and workspace settings.** Changing an organization's settings requires CAIRN and system administrators or that organization's owners and admins (`require_access`); a workspace's require its administrators (`require_workspace_admin`). A save names the version it read, so a stale one is refused.
- **What a workspace may loosen.** `workspace_cairn.rs` refuses, with a reason, a workspace that turns on what its organization turned off or sets a looser number: changes per confirmation, the number typed, secrets to the cloud, personal-data masking, strict citations, routine limits, MCP and inbox, personal memory, per-person budgets. The tools that are turned off are the two lists added together, so a workspace cannot turn one on. The organization's monthly budget and its cap on personal memory entries cannot be set by a workspace at all.
- **Models across workspaces and roles.** A model is offered only if the organization enabled it, the workspace offers it and the role's limit lists it; the person's preferred model is used only while it is still in that list; an embedding or vision change asks before it removes vectors.
- **Abilities cannot give a tool or a right.** The tools an ability names are checked against the registry when it is saved, and at run time `loop_with` keeps only those of them that the run already has: a viewer's run has no proposing tool, and a tool turned off stays off. Its instructions go in the prompt under the same rule as `CAIRN.md`, and every change it proposes still waits for the person. An ability runs only in the workspace that offers it, only when it is on, and not in a chat across workspaces.
- **Playbook to ability.** Only a verified playbook of the workspace can be used, the link is set by the server alone (`serde(skip)` on the draft), and the copy does not follow later edits.
- **The knowledge workspace's read check.** `run_message` gives a run the knowledge workspace only when `effective_workspace_role` finds a role for the person in it and a personal token reaches it; only current cards (not those needing review) and verified playbooks are offered.
- **Personal data.** Personal memory and preferences are the person's own in storage and in prompts, are exported and deleted by them whatever an organization sets, are not in workspace archives and are deleted when the account is deleted or anonymized; the policy turns off reading and proposing, never the person's own access.
- **Aggregates.** Usage groups show counts only, and a group of fewer than five people shows its size and nothing else; the memory and routines sections show counts, dates and names of routines and owners, never a prompt, an answer, a card or a chat title.
- **Pause and resume of routines** needs the same rights as changing the organization's CAIRN settings.

## Regression coverage

| Finding | Test |
|---|---|
| CR-1 to CR-4 | No test was added: each is a filter or a check on a server path that needs stored events, two organizations or a stored chat. The read of the code is what was done |
| Earlier rows | The unit tests of each release stay: the command and the fields of an ability, the abilities' versions and stale publish, the team memory block, the style note, the starter questions |

## Rows left for later

CR-5 and CR-6 change what an audit event holds and add an approval step. They are listed here rather than as new release rows; the maintainer should decide whether to put them before V0.9.119 or into 0.10.
