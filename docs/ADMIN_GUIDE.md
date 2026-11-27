# Administrator guide: from an empty host to a working team

The path an administrator follows, in order, with **who does each step**. Details live in the guides it links; this page says what to do next and which account can do it.

## CAIRN (0.9)

What an administrator decides about CAIRN, the assistant, and where.

**The CAIRN administration page** (`/cairn/admin`) shows how CAIRN is set up: each workspace's model and mode, and the budgets and retention in force. System and CAIRN administrators see every section and app administrators read the same page. Owners and admins of an organization see only the sections that concern their organizations. On Overview, CAIRN and system administrators set the run budgets (steps, seconds, tokens, background files and the days a trace is kept) within fixed maximums; a change applies to the next run, is audited with its before and after, and *Restore default* returns a budget to the environment's value. On Models, the connections to the servers CAIRN's models run on (Ollama, OpenRouter or an OpenAI-compatible server) are added, changed, turned off, tested and deleted; their key is write-only and only ever sent to the address it was saved for, the address is held to the same allowed hosts as a workspace's provider, and a connection whose address leaves your network is turned on only after you confirm that what CAIRN sends there leaves it. Below them, the models: find what a connection's server offers, add the ones CAIRN may use with a purpose, test each for the tools it takes and its context size, and correct what the test found; a model in use cannot be deleted. The first start of this version also moved every workspace's providers into the catalog, one connection for each distinct server and key; workspaces still answer with their own provider until the next releases switch them over. A live evaluation (`npm run cairn:eval -- --catalog-model <id>`, with a system-scope token) attaches its report to a model; the table shows passed, failed or not evaluated with the date, and only a text model that takes tools natively and passed is offered Agent mode by default (`REPOMEMO_CAIRN_EVAL_PASSED_MODELS` stays as an override). A model can name a fallback that answers when it times out, answers with a server error or is rate limited; it falls back once, a local model never falls back to a cloud one, and the answer says that a fallback answered. Every connection and model says *Local* or *Cloud*, decided by its address: a connection that leaves your network is confirmed once (*Confirm cloud use*) before it is on, and until then none of its models can be chosen; withdrawing the confirmation turns it off. An organization's owners and admins (and CAIRN administrators) choose which catalog models the organization uses, its default and its Agent-mode default, on the Organizations section of the CAIRN administration page or in the organization's own Administration tab (or with `PUT /v1/organizations/{id}/cairn`), with a version on every save; CAIRN, Ask and the summaries of its workspaces then follow them: a workspace keeps its own model while the organization offers it, uses the organization's default when it does not (the answer says which one answered), and an organization that has not saved settings changes nothing. In Settings › AI a workspace's administrators then tick which of the organization's models the workspace offers and choose its default; a workspace sees only its organization's models, marked *From your organization*, and the choice travels with the workspace's archive. People then pick the model of each chat from the models the workspace offers (name, local or cloud, and whether it takes tools); the chat keeps it, and a model that stops being offered is replaced by the workspace's, with a note in the answer. A model can be limited to roles (*Everyone who may use AI*, *Members and up* or *Admins only*) by the organization or, within it, by the workspace; the person's picker shows only what their role may use and the server refuses the rest, after the workspace's AI policy. Owners and admins may always use every model. The organization, and a workspace within it, can also turn off any CAIRN job or tool (*What CAIRN may do*): a job that is off is greyed out in the + menu and answered with who turned it off, a tool that is off is not given to the model, and a workspace can turn off more but never turn on what its organization turned off. They also set the most changes one confirmation may make (200 by default) and the number above which the person types it to confirm; a workspace can only be stricter. A workspace's administrators can also close folders to CAIRN (Settings › AI): its tools, its chats across workspaces and the MCP endpoint never return what is in a closed folder or the folders inside it, and the file browser shows a lock; people and Ask outside CAIRN are unaffected. Before text goes to a cloud model, secrets (keys, tokens, passwords, connection strings) are replaced by a marker, or nothing is sent when the organization chooses that. The organization (or a workspace) can also have email addresses and phone numbers masked, which is off by default. They also set each person's daily CAIRN budgets of runs and tokens (a workspace can only lower them), and can exempt owners and admins. The organization can also cap the tokens its CAIRN runs use in a month: its owners and admins are told at 80% and 100%, and at the cap CAIRN stops or keeps to models on the organization's network. The organization or a workspace can also require strict citations: an AI answer about the workspace's content that cites nothing it read is not shown. They also limit routines (how many a person and a workspace may have, which schedules and whether event routines are on); routines over a new limit are paused with the reason, not deleted. The organization also sets how many days CAIRN's run histories, chats and personal memory are kept (the shorter of its setting and the server's applies, and the count is shown before saving). It can turn off the MCP endpoint and the inbox for outside agents, and lower how many proposals wait in an inbox; a call into such a workspace is refused with the reason. The Usage section shows runs, tokens, failures, fallbacks, steps and time for each organization, workspace and model over 7 or 30 days, as figures only (a group of fewer than five people shows nothing but its size), with a CSV export. The History section lists every change made to how CAIRN is administered, with who made it and the values before and after (keys never appear); the same events are in System › Audit. The per-workspace provider forms are gone and `PUT /v1/workspaces/{id}/ai-providers` answers `410`; providers that existed were moved into the catalog. A workspace's embedding and vision models are chosen in Settings › AI from the catalog models of that purpose the organization enabled; changing the embedding model asks first, because the stored vectors are removed and every passage is embedded again in the background. The guardrails, abilities, memory, usage and history sections are empty until later releases fill them.

| Decision | Where | What it does |
|---|---|---|
| **CAIRN mode** | *Settings › AI* for the workspace | *Jobs* (the router picks one job a message) or *Agent* (CAIRN calls tools in a loop). Any text provider that takes tools (native, JSON, or one nobody set) answers in Agent mode unless the organization, the workspace or the person chose *Jobs*; a model set to *none* answers with jobs |
| **Who may use AI** | *Settings › AI*, *Who can use AI* | The role below which CAIRN answers without a model. It also decides who a routine, a chat across workspaces and an analysis can spend AI for |
| **Budgets** | server settings | `REPOMEMO_CAIRN_MAX_STEPS` (model calls in one request, 8), `REPOMEMO_CAIRN_MAX_RUN_SECONDS` (90), `REPOMEMO_CAIRN_BACKGROUND_MAX_FILES` (files one analysis reads, 200) and each person's `REPOMEMO_AI_REQUESTS_PER_HOUR`. Every model call counts against the hour |
| **Retention** | server settings | `REPOMEMO_CAIRN_TRACE_DAYS` (90): how long *What CAIRN did* is kept. Chats stay until their owner deletes them; personal memory until its owner deletes it or the account goes |
| **Weekly memory upkeep** | *Settings › AI* | Off by default. When on, once a week it adds to the review queue cards that may disagree, questions asked three times with no answer (it shows the question's text) and verified playbooks unused for 90 days. It changes nothing and uses no model |
| **Routines** | The chat list, *Routines* | At most 10 a workspace, run as their owner, read-only. A routine pauses when its owner loses write access, is deactivated, is no longer allowed AI, when AI is off, or after three failures in a row. An administrator can pause, resume or delete any |
| **MCP tokens** | *Profile › Access tokens* | An *MCP* token lets another AI agent read chosen workspaces and propose to its owner's inbox, and reaches nothing else. Revoke it there; the activity feed shows `mcp_read` and `mcp_proposed`. See [MCP](MCP.md) |
| **Team instructions and playbooks** | `CAIRN.md` at the workspace root, and the `Playbooks` folder | Notes writers edit; the workspace's review rules apply. A playbook is used only once verified, and grants no tool |

The open findings of the latest reviews are in [the CAIRN security review](../mindmap/security/2026-11-25-cairn-review.md) and [the CAIRN administration review](../mindmap/security/2026-11-27-cairn-administration-review.md). Reference: [API](../mindmap/technical/api-reference.md) · [settings](DEVELOPMENT_COMMANDS.md).

## CAIRN administration (0.9.70 to 0.9.119)

Who decides what about CAIRN, in four tiers. A lower tier can only be **stricter** than the one above it; where it is not, the page says why and the server refuses.

| Tier | Who | What they decide |
|---|---|---|
| **The server** | CAIRN administrators and system administrators (app administrators read) | The **catalog**: the connections to model servers (Ollama, OpenRouter, any OpenAI-compatible address; the key is write-only and sealed, and a connection that leaves your network must be confirmed) and the models on them (what each is for, whether it takes tools, its context, its live evaluation, its fallback). The run budgets. |
| **The organization** | Its owners and admins (and the server's administrators) | Which catalog models it uses and its default, Agent mode, which roles may use which model, the **guardrails**, its instructions, its knowledge workspace, its preference defaults, its greeting and starter questions, and its abilities |
| **The workspace** | Its administrators | Which of the organization's models it offers and its default, stricter guardrails, the folders closed to CAIRN, its embedding and vision model, its own abilities and which of the organization's it keeps on, its greeting and starter questions |
| **The person** | Anyone who may use AI | The model of a chat (from the list offered), their preferences, their personal memory |

**The administration page** (`/cairn/admin/{section}`; the rail's *Administration* tab): *Overview*, *Models* (connections and models), *Organizations* (an organization's settings), *Abilities*, *Memory* (cards, playbooks and routines of the workspaces, in figures), *Usage* (runs, tokens, failures and fallbacks over 7 or 30 days, in figures, never a message or a person; a group of fewer than five people shows only its size) and *History* (every change, with who and the values before and after; the same events are in System › Audit). Owners and admins of an organization see only their own organizations, and the History matches events by the organization's and its workspaces' ids.

**Guardrails, in the rough order a request meets them.**

1. *Who may use AI* in the workspace (Settings › AI) decides whether CAIRN uses a model for that role.
2. *Which model.* The organization's models, narrowed by the workspace, narrowed by the role limits; the person's choice or preference is used only if it is still offered. A workspace with no model chosen by its organization keeps its own provider until it moves to the catalog.
3. *Per-person daily budgets* (runs and tokens), then the organization's *monthly token budget* (administrators are told at 80 and 100 percent; at the cap CAIRN stops, or keeps to models on your network).
4. *Tools and jobs turned off* by the organization or the workspace: the two lists add up, and a workspace cannot turn one on.
5. *Closed folders*: a workspace closes folders (and their subfolders) to CAIRN.
6. *Before a cloud model sees anything*: secrets are masked or the request is refused, and, if set, email addresses and phone numbers are masked.
7. *Strict citations*: an answer about the workspace's content that cites nothing it read is not shown, and says what was read.
8. *Changes*: at most so many changes per confirmation, and the number typed above a threshold.

Also set here: *routine limits* (routines a person and a workspace may have, the schedules allowed, event routines), *MCP and inbox* (whether outside agents are served, may propose, and how many proposals wait), *personal memory* (on or off for the organization's workspaces, and a cap on entries per person: a workspace can only turn it off) and *retention* (days to keep run history, chats and personal memory in the organization's workspaces; the shorter of this and the server's applies, and the settings page shows what a change would delete before you save it).

**Abilities.** A procedure taught to CAIRN: a name, a `/command`, instructions, the tools it may use (only registered ones), the fields it asks first and an output template. Organizations and workspaces keep up to 100 each; every publish is a new version and older ones are kept; a workspace's command cannot repeat its organization's. An ability can narrow a run but never widen it: it keeps only those of its tools the run already has, so it gives no tool and no right. It needs Agent mode, and the trace and the answer end with "Ability: name, version N". A verified playbook becomes an ability in one step (*Workspace settings › Playbooks*), and the two show each other.

**Instructions and the knowledge workspace.** The organization's instructions (4,000 characters) are read into every Agent-mode run in its workspaces, after the rules and before the workspace's `CAIRN.md`, and the run's trace begins with their version. The knowledge workspace's current memory cards and verified playbooks are offered in the others, only to people who can read it; a chat that used its cards cannot be shared.

**Preferences and welcome.** People set their own language, length, steps and Agent or Jobs in their profile; the organization's defaults apply where they chose nothing. The greeting and up to six starter questions replace the standard ones.

**Open items of the latest review** are in [the CAIRN administration review](../mindmap/security/2026-11-27-cairn-administration-review.md): the audit text keeps what administrators write in full, and one administrator can change instructions and abilities for a whole organization. No setting was added to `DEVELOPMENT_COMMANDS.md` and no console command: the run budgets are on the page, and the evaluation is still `npm run cairn:eval`.

## AI usage and limits (0.8)

The calls the server makes on its own for a workspace (embedding batches and image descriptions) are counted **in the database**, so a limit stops them the same way before and after a restart. **API:** `GET /v1/workspaces/{id}/ai-usage` shows the limit and where it comes from (the workspace's own, the server's default or none), today's calls, the calls a limit refused, what is left, the last days by kind, how many files, passages and stored vectors the workspace holds, and which providers are on for text, embedding and vision. `PUT /v1/workspaces/{id}/ai-limit` with `{"daily_calls": 200}` sets the workspace's own daily limit, `0` removes the limit, and `null` returns to the server's default. At the limit, embedding and image description stop (and say why on the job) and resume the next UTC day. **Keyword search never calls an AI provider**, so it keeps working when AI is off, not configured, over its limit or failing; only semantic matching, Ask and summaries need a provider. No screen yet.

## 1. The accounts involved

| Who | What they are | What they can do |
|---|---|---|
| **Host operator** | A person with shell access to the machine (not a RepoMemo account) | Install, edit `.env` or the settings file, read logs, run `docker compose` and backup scripts |
| **System administrator** | A RepoMemo account made in the first-run setup | Everything in **System**: users, settings, logs, jobs, backups, audit trail, console. Does not automatically see every workspace's content |
| **App administrator** | A RepoMemo account a system administrator promotes | The same pages as the system administrator, with no automatic access to every workspace |
| **CAIRN administrator** | A RepoMemo account a system administrator promotes (System › Users, or `users cairn-admin <email> on --yes` in the console); system administrators are CAIRN administrators by right | Administers CAIRN's models, limits and settings as the CAIRN administration rows ship, and nothing else: not the System pages, and no access to workspaces |
| **Organization owner / admin** | A member of an organization with that role | Create workspaces; add members and change roles (admins cannot touch other admins or the owner); the owner also renames the organization |
| **Workspace owner / admin** | A member of a workspace with that role | Invite people, set AI and who may use it, link repositories, choose the template, act on health findings; only the owner renames or deletes the workspace |
| **Member / viewer** | Ordinary users | Members read and write evidence, tasks and cards; viewers only read |

Roles in full: [user guide, § 2](USER_GUIDE.md#2-who-does-what).

## 2. The setup path

| # | Step | Who | Guide |
|---|---|---|---|
| 1 | Choose containers or a direct install, and size the host | host operator | [Deploying RepoMemo](DEPLOYMENT.md#1-choose-containers-or-a-direct-install) |
| 2 | Install and start the server; keep a copy of `REPOMEMO_SECRET_KEY` | host operator | [Quick start](QUICK_START.md), or [Front doors](FRONT_DOORS.md) for a direct install |
| 3 | Finish the first-run setup with the code from the log; this creates the system administrator | whoever has the log, becoming the system administrator | [Quick start, § First sign-in](QUICK_START.md#first-sign-in) |
| 4 | Read *System › Overview*: the version, free disk, last backup | system administrator | [Production guide](../mindmap/technical/operations.md) |
| 5 | Decide registration and sign-in rules (registration is closed by default, so people join by invitation) | system administrator | *System › Settings* |
| 6 | Set up mail, so invitations and resets are emailed; without it, links are shown to copy | system administrator | *System › Settings › Mail* (test button and outbox) |
| 7 | **Make the first backup** and copy it off the machine | system administrator or host operator | [Deploying RepoMemo, § 4](DEPLOYMENT.md#4-backups) |
| 8 | Create the organization (the setup can do it) and a workspace, with a template | any signed-in user creates an organization and owns it; organization owner or admin creates workspaces | [User guide, workflow 1](USER_GUIDE.md#workflow-1-create-a-workspace) |
| 9 | Invite people with a role | organization or workspace owner or admin | below |
| 10 | Optionally set an AI provider per workspace | workspace owner or admin | [Search, Ask and citations](SEARCH_AND_ASK.md#1-what-works-without-ai) |
| 11 | Hand out the desktop app | host operator or system administrator | below |

## 3. Invitations

An admin enters an **email address and a role**; RepoMemo emails an **invitation link** or, without mail, shows the link to copy. The link works **once, only for that email address, for seven days**. The person creates an account (even if registration is closed) or signs in, and lands on the dashboard with the role given.

- **Organization membership** gives access to all of its workspaces. A **workspace guest** invitation shows only that workspace.
- An admin can grant only `member` or `viewer`. Nobody assigns `owner` in the interface, so an organization or workspace never loses its last owner.
- Pending invitations can be withdrawn. A person who leaves is **deactivated** in *System › Users*, where you can also sign them out everywhere; anonymize a deactivated account for good if required, and what they wrote is credited to "Former member".

## 4. The desktop app

The desktop app is the web client in its own window, showing your server; it keeps only the server's address and the sign-in. To give a team an app that opens on your server with no address to type, package it: [Desktop app administrator guide](DESKTOP_ADMIN.md) (the handout, the configuration file, the optional certificate of a private authority). Where it keeps files on each system: [Desktop files](DESKTOP_FILES.md). The app and the server must be of the same release line; a window of another line asks to be updated or reloaded.

## 5. Which guide for the next problem

| Need | Guide |
|---|---|
| Upgrade, roll back | [Deploying RepoMemo, § 5](DEPLOYMENT.md#5-upgrading), [Direct upgrade](DIRECT_UPGRADE.md) |
| Run, monitor, back up, restore | [Production guide](../mindmap/technical/operations.md) |
| A start or first-run error | [Troubleshooting](TROUBLESHOOTING.md) |
| Every server setting | [Development commands](DEVELOPMENT_COMMANDS.md) |
| Commands for the server | [Admin console](../mindmap/technical/admin-console.md) |

## Team-access rollout

Provider sign-in, group mapping, restricted workspaces, guest end dates, offboarding, limited tokens and local two-step sign-in arrived in 0.5. Follow the [team-access operating guide](TEAM_ACCESS_OPERATIONS.md) (setup order, joiners and leavers, recovery drills, limits) and use the [team-access reference](TEAM_ACCESS.md) for what each feature does. None of it has been rehearsed against a real provider yet: do the rehearsal on a test server before requiring provider sign-in.
