# Administrator guide: from an empty host to a working team

The path an administrator follows, in order, with **who does each step**. Details live in the guides it links; this page says what to do next and which account can do it.

## CAIRN (0.9)

What an administrator decides about CAIRN, the assistant, and where.

**The CAIRN administration page** (`/cairn/admin`) shows how CAIRN is set up: each workspace's model and mode, and the budgets and retention in force. System and CAIRN administrators see every section and app administrators read the same page. Owners and admins of an organization see only the sections that concern their organizations. On Overview, CAIRN and system administrators set the run budgets (steps, seconds, tokens, background files and the days a trace is kept) within fixed maximums; a change applies to the next run, is audited with its before and after, and *Restore default* returns a budget to the environment's value. On Models, the connections to the servers CAIRN's models run on (Ollama, OpenRouter or an OpenAI-compatible server) are added, changed, turned off, tested and deleted; their key is write-only and only ever sent to the address it was saved for, the address is held to the same allowed hosts as a workspace's provider, and a connection whose address leaves your network is turned on only after you confirm that what CAIRN sends there leaves it. Below them, the models: find what a connection's server offers, add the ones CAIRN may use with a purpose, test each for the tools it takes and its context size, and correct what the test found; a model in use cannot be deleted. The first start of this version also moved every workspace's providers into the catalog, one connection for each distinct server and key; workspaces still answer with their own provider until the next releases switch them over. A live evaluation (`npm run cairn:eval -- --catalog-model <id>`, with a system-scope token) attaches its report to a model; the table shows passed, failed or not evaluated with the date, and only a text model that takes tools natively and passed is offered Agent mode by default (`REPOMEMO_CAIRN_EVAL_PASSED_MODELS` stays as an override). A model can name a fallback that answers when it times out, answers with a server error or is rate limited; it falls back once, a local model never falls back to a cloud one, and the answer says that a fallback answered. Every connection and model says *Local* or *Cloud*, decided by its address: a connection that leaves your network is confirmed once (*Confirm cloud use*) before it is on, and until then none of its models can be chosen; withdrawing the confirmation turns it off. An organization's owners and admins (and CAIRN administrators) choose which catalog models the organization uses, its default and its Agent-mode default, on the Organizations section of the CAIRN administration page or in the organization's own Administration tab (or with `PUT /v1/organizations/{id}/cairn`), with a version on every save; CAIRN, Ask and the summaries of its workspaces then follow them: a workspace keeps its own model while the organization offers it, uses the organization's default when it does not (the answer says which one answered), and an organization that has not saved settings changes nothing. In Settings › AI a workspace's administrators then tick which of the organization's models the workspace offers and choose its default; a workspace sees only its organization's models, marked *From your organization*, and the choice travels with the workspace's archive. The guardrails, abilities, memory, usage and history sections are empty until later releases fill them.

| Decision | Where | What it does |
|---|---|---|
| **CAIRN mode** | *Settings › AI* for the workspace | *Jobs* (the router picks one job a message) or *Agent* (CAIRN calls tools in a loop). Agent mode needs a text provider with native tool calling; a workspace whose model is listed in `REPOMEMO_CAIRN_EVAL_PASSED_MODELS` starts in Agent mode unless you chose |
| **Who may use AI** | *Settings › AI*, *Who can use AI* | The role below which CAIRN answers without a model. It also decides who a routine, a chat across workspaces and an analysis can spend AI for |
| **Budgets** | server settings | `REPOMEMO_CAIRN_MAX_STEPS` (model calls in one request, 8), `REPOMEMO_CAIRN_MAX_RUN_SECONDS` (90), `REPOMEMO_CAIRN_BACKGROUND_MAX_FILES` (files one analysis reads, 200) and each person's `REPOMEMO_AI_REQUESTS_PER_HOUR`. Every model call counts against the hour |
| **Retention** | server settings | `REPOMEMO_CAIRN_TRACE_DAYS` (90): how long *What CAIRN did* is kept. Chats stay until their owner deletes them; personal memory until its owner deletes it or the account goes |
| **Weekly memory upkeep** | *Settings › AI* | Off by default. When on, once a week it adds to the review queue cards that may disagree, questions asked three times with no answer (it shows the question's text) and verified playbooks unused for 90 days. It changes nothing and uses no model |
| **Routines** | The chat list, *Routines* | At most 10 a workspace, run as their owner, read-only. A routine pauses when its owner loses write access, is deactivated, is no longer allowed AI, when AI is off, or after three failures in a row. An administrator can pause, resume or delete any |
| **MCP tokens** | *Profile › Access tokens* | An *MCP* token lets another AI agent read chosen workspaces and propose to its owner's inbox, and reaches nothing else. Revoke it there; the activity feed shows `mcp_read` and `mcp_proposed`. See [MCP](MCP.md) |
| **Team instructions and playbooks** | `CAIRN.md` at the workspace root, and the `Playbooks` folder | Notes writers edit; the workspace's review rules apply. A playbook is used only once verified, and grants no tool |

The open findings of the latest review are in [the CAIRN security review](../mindmap/security/2026-11-25-cairn-review.md). Reference: [API](../mindmap/technical/api-reference.md) · [settings](DEVELOPMENT_COMMANDS.md).

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
