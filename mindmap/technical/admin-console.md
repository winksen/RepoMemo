# Admin console (System › Console)

> **Branch of:** [TECHNICAL_MINDMAP.md](../TECHNICAL_MINDMAP.md) · **Functional view:** [FUNCTIONAL_MINDMAP.md §1b](../FUNCTIONAL_MINDMAP.md#1b-system-area-system-administrators) · **Status:** draft, first slice implemented (see [ROADMAP.md](../ROADMAP.md))

Some technical administrators would rather **type** than click. They know what they want ("show failed jobs", "unlock this account", "turn HTTP logs to debug") and want to reach the server in one line, script it, and get an answer they can copy. The System pages cover the same ground, but they are built for browsing.

The console gives them a **command line to the server** in two places that look and behave the same: the **`repomemo-console`** terminal program (`npm run console`), which opens with the RepoMemo logo, and a prompt under **System › Console** in the web app. Both send the same commands to the same endpoint, and answers show as tables and facts (or raw JSON).

---

## Goals and non-goals

| Goals | Non-goals (for now) |
|---|---|
| One text command per action, with an answer in under a second | A shell on the host. The console never runs operating-system commands, SQL or arbitrary code |
| Same rights as the System pages: system and app administrators only, with the same per-action limits | A new permission model. The console adds no rights of its own |
| Every change is explicit (`--yes`) and lands in the audit trail, marked as coming from the console | Bulk or scripted multi-step operations (pipes, variables, loops) |
| A terminal client with the same logo, commands and prompt behaviour as the web page | A shell on the host or an SSH entry point: the terminal client talks HTTP like the browser |
| Built only on existing handlers, so behavior and validation never drift from the pages | Streaming output (`logs --follow`). Planned on the SSE stream |

---

## Who uses it, and for what

| Need today | Clicks today | Console |
|---|---|---|
| Is the server healthy, is anything queued? | System › Overview, scroll | `status` |
| Which jobs failed? | System › Jobs, choose a status | `jobs list --status failed` |
| Stop a stuck sync | Find it in the table, Stop | `jobs cancel <job-id> --yes` |
| Turn HTTP logs to debug for a while | Settings › Logging, choose, Save | `settings set log_http_level debug --yes` |
| What did the security log say in the last minutes? | Logs, choose the kind and level | `logs --category security --level warn` |
| Someone is locked out | Users, find them, menu, Unlock | `users unlock alice@example.com --yes` |
| Free disk space now | Jobs & maintenance, Run | `maintenance run --yes` |

---

## Design

```mermaid
flowchart LR
  A["repomemo-console in a terminal<br/>System › Console in the web app<br/>or curl"] -->|"POST /v1/system/console<br/>{command}"| B["Tokenize<br/>(quotes, --flags, --yes)"]
  B --> C{"Command<br/>catalog"}
  C -- unknown or bad usage --> E["400 with the usage line"]
  C -- read --> R["Existing System handler<br/>(overview, jobs, logs…)"]
  C -- change without --yes --> P["Dry run: says what would happen<br/>nothing changes"]
  C -- change with --yes --> W["Existing handler<br/>(same rights, same audit)"]
  W --> AU["+ audit 'console_command'<br/>with the exact line"]
  R --> O["{command, summary, dry_run, view, data}"]
  P --> O
  W --> O
```

### Server ([system/console.rs](../../apps/server/src/system/console.rs))

- **`POST /v1/system/console`** with `{"command": "jobs list --status failed"}`. Requires a system or app administrator, like every System route.
- **`GET /v1/system/console`**: the welcome every console opens with: logo, tagline, tips, version, who is signed in and their role, and the catalog (name, usage, summary, whether it changes something) for help, Tab completion and usage hints. The logo and opening lines are constants in `repomemo_domain::console`, so the terminal client can show the logo before signing in.
- **Parsing.** Words, `"double"` or `'single'` quoted values, `--flag value` or `--flag=value`, and the bare `--yes` (or `-y`). Each command declares its flags, so a typo (`--stauts`) is refused with the usage line instead of being ignored. Lines are capped at 2,000 characters.
- **Execution.** Each command calls the **existing handler** (for example `system::jobs`, `system::update_settings`, the workspace `cancel_job`) with the caller's identity. Access checks, validation messages and audit records are therefore the same as on the pages. The console has no storage access of its own, except to resolve an email to a user.
- **Changes need `--yes`.** Without it the command is a **dry run**: it checks what it can (the user exists, the setting value is valid, the job is running) and answers "Would …". With it, the handler runs, and a `console_command` audit event records the exact line, so the audit trail shows the action and that it came from the console.
- **Answer.** `{command, summary, dry_run, view, data}`. `data` is the handler's JSON. `view` is a hint for display: `table` (with the columns worth showing), `facts`, or `json`. A script can ignore the hint and read `data`.
- **Errors** use the normal error envelope: 400 for usage, 403 for rights, 404 for an unknown user or job, 409 for a conflict (maintenance already running).

### Terminal client ([apps/console](../../apps/console/src/main.rs))

- **`repomemo-console`**, started with `npm run console` (or `cargo run -p repomemo-console`). It prints the logo, asks for an email and a **hidden password**, greets the administrator and opens a `repomemo ›` prompt.
- The prompt ([prompt.rs](../../apps/console/src/prompt.rs), `rustyline`) works like the web one: **Up/Down** history (kept in `~/.repomemo/console-history.txt`), **Tab** completes command names, a grey **usage hint** follows what you type (Right arrow accepts the rest of a name), `clear` or **Ctrl+L**, and `exit` or **Ctrl+D** to leave.
- Answers are drawn by [render.rs](../../apps/console/src/render.rs): the summary, a **DRY RUN** tag in yellow, aligned facts and tables (long cells cut with `…`), refusals in red. Colours are off when the output is not a terminal or `NO_COLOR` is set.
- **Sessions** stay in memory only. The access token is renewed silently with the refresh token, like the web app; if the session ends (for example after "sign out everywhere") the console asks to sign in again.
- **Scripts.** `repomemo-console jobs list --status failed --json` runs one command and exits with `0` (done), `1` (refused) or `2` (sign-in or connection problem). `REPOMEMO_TOKEN` skips the sign-in; `--server`/`REPOMEMO_SERVER_URL` and `--email`/`REPOMEMO_EMAIL` pick the server and account.
- Requests carry `X-RepoMemo-Client: console`, so the terminal shows under System › Overview › clients.

### Web client ([SystemConsole.tsx](../../apps/desktop/src/components/SystemConsole.tsx))

- A new **Console** tab in the System area (`/system/console`), opening with the same logo, greeting and tips as the terminal.
- A transcript (command, then its answer as a table, facts or JSON) above a prompt. **Up/Down** walks the history, **Tab** completes command names, **Ctrl+L** or `clear` empties the transcript, and `--json` on any command shows the raw answer.
- The history is kept per browser (local storage, last 100 lines), as a convenience only.
- A failed command is part of the console's output, so its message is shown in the transcript, marked as an error. Problems that are not about the command (session expired, server unreachable) are toasts, like everywhere else.

---

## First slice: commands

| Command | Changes? | Backed by |
|---|---|---|
| `help [command]` | no | catalog |
| `status` | no | `GET /v1/system/overview` |
| `jobs list [--status s] [--limit n]` | no | `GET /v1/system/jobs` |
| `jobs cancel <job-id>` | **yes** | `POST /v1/jobs/{id}/cancel` |
| `settings list` | no | `GET /v1/system/settings` |
| `settings set <key> <value>` | **yes** | `PUT /v1/system/settings` |
| `settings reset <key>` | **yes** | `DELETE /v1/system/settings/{key}` |
| `logs [--level] [--category] [--grep] [--day] [--limit]` | no | `GET /v1/system/logs` |
| `users list` | no | `GET /v1/system/users` |
| `users unlock <email>` | **yes** | `POST /v1/system/users/{u}/unlock` |
| `users signout <email>` | **yes** | `POST /v1/system/users/{u}/sessions/end` |
| `maintenance status` | no | `GET /v1/system/maintenance` |
| `maintenance run` | **yes** | `POST /v1/system/maintenance/run` |

**Deliberately left out:** detaching the environment and granting the system administrator role. Both are rare, hard to undo, and the pages' confirmation dialogs explain what follows. They stay on the pages.

---

## Risks and decisions

| Topic | Decision |
|---|---|
| **A console feels like a shell** | It is a fixed command catalog over existing handlers. Nothing is evaluated, nothing reaches the operating system or the database directly |
| **Fat-finger changes** | Changes need `--yes`. Without it, the dry run says what would happen |
| **Traceability** | Changes are recorded twice: the handler's usual audit event, and `console_command` with the exact line typed |
| **Drift from the pages** | No business logic in the console: it only parses, calls the handler and picks a display |
| **App administrators** | Same limits as on the pages (for example, cancelling a job still needs write access to its workspace) |
| **Error-toast convention** | Command errors are console output and stay in the transcript. Transport errors are toasts |

## Open questions

1. **Live logs.** `logs --follow` over a system-wide SSE stream (the per-workspace stream exists; a system one does not).
2. **Workspace commands.** `workspace reindex <name>`, `repo sync <name>`, `embeddings rebuild`: useful, but they reach into workspaces. Should they follow workspace roles (app administrators refused) or system roles?
3. **Personal tokens for scripts.** Unattended scripts need `REPOMEMO_TOKEN`, a short-lived access token. A personal, revocable API token for administrators would make scripting practical.
4. **Should reads be audited?** Today only changes are. Every command, read or change, is logged at debug level (server category) with the user and command name.
5. **Remembered sign-in in the terminal.** The terminal client signs in every time. Keeping the refresh token in the operating system's credential store would avoid that, at the cost of a long-lived secret on the machine.

---

## How to try it

1. With the server running, run `npm run console` in a terminal, sign in as a system administrator, and try the steps below there; then open **System › Console** in the web app and try them again.
2. Type `help`, then `status`, then `jobs list --status failed`.
3. Try a change without and then with confirmation: `settings set log_http_level debug`, then the same line with `--yes`. Check **System › Audit trail**: both `system_settings_changed` and `console_command` appear.
4. Restore it: `settings reset log_http_level --yes`.
5. From a terminal, with an access token copied from the browser's storage (`repomemo.shared.access-token`):

```bash
curl -s -X POST http://127.0.0.1:3020/v1/system/console -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" -d "{\"command\":\"jobs list --status running\"}"
```
