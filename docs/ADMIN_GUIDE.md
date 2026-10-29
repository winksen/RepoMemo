# Administrator guide: from an empty host to a working team

The path an administrator follows, in order, with **who does each step**. Details live in the guides it links; this page says what to do next and which account can do it.

## 1. The accounts involved

| Who | What they are | What they can do |
|---|---|---|
| **Host operator** | A person with shell access to the machine (not a RepoMemo account) | Install, edit `.env` or the settings file, read logs, run `docker compose` and backup scripts |
| **System administrator** | A RepoMemo account made in the first-run setup | Everything in **System**: users, settings, logs, jobs, backups, audit trail, console. Does not automatically see every workspace's content |
| **App administrator** | A RepoMemo account a system administrator promotes | The same pages as the system administrator, with no automatic access to every workspace |
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

The [team-access guide](TEAM_ACCESS.md) separates existing guests, transfer and audit from the identity and access features being added in 0.5. Follow its current implementation status before changing sign-in policy.
