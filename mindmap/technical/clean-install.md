# Clean-machine install: the run before the tag

> **Branch of:** [TECHNICAL_MINDMAP.md](../TECHNICAL_MINDMAP.md) · **Roadmap:** [R6 (V0.2.0)](../ROADMAP.md#v020-team-ready), [R12 (V0.2.56)](../ROADMAP.md) · **Guide followed:** [operations.md](operations.md) · **Status:** V0.3.4. The install was made by the maintainer, who reported it good on 2026-10-24 (V0.3.0 was tagged on that report); §4 records that, and §3 says why each box is still open and what closes it. Runs need a second machine and a running server, which this repository's rules keep from the automated work, so the per-step results, the browser pass and the performance figures are filled by the maintainer in the sheets of this page and of [performance.md](performance.md).

`V0.3.0` is tagged when a person who has nothing of RepoMemo on their machine installs it **following only the production guide**, with mail and a remote repository, and meets **no workaround**. A workaround is anything done that the guide does not say: a command guessed, a setting found in the code, a retry after a message that should have said why. Each one is written down and becomes a fix of the guide or the server (the way V0.2.1 came from the first run), and the run is made again.

---

## 1. What you need

- A **clean machine** (a virtual machine is fine): Linux with systemd, or Windows Server. Nothing of RepoMemo, no Rust or Node toolchain unless the guide's build route is the one being tested.
- A **name** that reaches it, and a certificate (the guide's Caddy route obtains one; a container run does it for you).
- A **mail account** the server may send from, over TLS or STARTTLS, and an address you can read.
- A **git repository** on a host the machine can reach: a public one is enough, a private one with a read-only token tests more.
- A **second computer** with the desktop app, for the last part.
- This repository at the release to be tagged, for `scripts/smoke-test.mjs` (Node 22 on any machine that can reach the server).

## 2. The run

Do the steps in order. After each, write one line in the table of §4: what happened, and any workaround.

| # | Step | The guide says | You should see |
|---|---|---|---|
| 1 | **Get the server** by the route you are testing: the build route (`cargo build --release -p repomemo-server` and `npm ci && npm run build` for the web app) or `docker compose up -d --build` | [§1](operations.md#1-what-runs-where), [In containers](operations.md#in-containers-since-v0245) | a binary and a `dist/`, or two healthy containers |
| 2 | **Set what must be right**: `REPOMEMO_JWT_SECRET`, `REPOMEMO_SECRET_KEY`, `REPOMEMO_PUBLIC_URL`, `REPOMEMO_TRUST_PROXY`, `REPOMEMO_GIT_ALLOWED_HOSTS` for the host of your repository | [§2](operations.md#2-the-settings-that-must-be-right) | the server starts and names no setting it refuses |
| 3 | **Reverse proxy and service**: nginx or Caddy, then systemd, NSSM or the compose file | [§3](operations.md#3-the-reverse-proxy), [§4](operations.md#4-running-it-as-a-service) | `https://your-name/health` answers `ok`; the service restarts after a reboot |
| 4 | **First-run setup** in the browser: the setup code from the server's console, the first administrator, the checks, *Finish* | [§2](operations.md#2-the-settings-that-must-be-right) | the setup checks list nothing red; you land on the dashboard |
| 5 | **Smoke test**: `node scripts/smoke-test.mjs https://your-name` | [Is it up?](operations.md#is-it-up) | every line `ok` (the 200 KiB sign-in and the webhook lines included) |
| 6 | **Mail**: System › Settings › Mail (server, port, TLS or STARTTLS, sender), *Send test mail* to yourself; then `REPOMEMO_SMOKE_MAIL=you@example.com REPOMEMO_SMOKE_EMAIL=… REPOMEMO_SMOKE_PASSWORD=… node scripts/smoke-test.mjs …` | [§2](operations.md#2-the-settings-that-must-be-right) | the mail arrives and the outbox shows it *sent* |
| 7 | **Mail in use**: invite a second address (the invitation arrives, its link works), use *Forgot your password?* on that account (the link works once and ends its sessions), register on an open server if you open registration (the confirmation arrives) | [§2](operations.md#2-the-settings-that-must-be-right) | each mail arrives; 13 invitations to one address in an hour mail only 12 |
| 8 | **A remote repository**: create a workspace, Settings › Repositories, paste the address (and the token), *Check access*, *Link and sync*. For a private one by `ssh`, the deploy key and the host key from `ssh-keyscan` | [repository sources](repository-sources.md#how-to-try-it-locally) | the repository syncs; its files are searchable; `REPOMEMO_SMOKE_REMOTE_REPO=… REPOMEMO_SMOKE_WORKSPACE=…` in the smoke test agrees |
| 9 | **A push webhook**: create it on the link (the secret is shown once), add it on the host with the shown address, push a commit | [repository sources](repository-sources.md) | the sync starts within seconds; the same delivery sent again is answered `already received` |
| 10 | **A tracker** (optional): Settings › Issues and pull requests for the same repository | [connectors](connectors.md) | issues appear as evidence |
| 11 | **Backups**: make one (sealed with `REPOMEMO_BACKUP_KEY` if you test that), copy it off the machine, restore it into a new environment, open it | [§6](operations.md#6-backups-and-getting-back) | the restored copy answers a search |
| 12 | **An upgrade**: install the release before this one, add some data, put this one in place | [§9](operations.md#9-upgrading), [UPGRADING.md](../../UPGRADING.md) | the data is there; System › Overview shows the new version |
| 13 | **The desktop app** on the other computer: enter the address, sign in, restart (no password asked), open a file, *Change server* | [DEVELOPMENT_COMMANDS.md](../../docs/DEVELOPMENT_COMMANDS.md#run-the-desktop-app) | the same pages as the browser; the session survives a restart; the entry *RepoMemo* is in the credential manager |
| 14 | **Spot checks of the fourth security pass**: a body of 200 KiB to sign-in (`413`), an archive that claims 300 MiB of a few hundred bytes (refused as a compression bomb), a replayed webhook delivery | [security review](../security/2026-10-16-security-review.md#6-for-the-next-pass) | each is refused as written |

## 3. Before the tag

The releases of the 0.2.x line were written and compiled without running the server or the app, so the tag also waits for what only a run can show. Tick each when done and record the result in the roadmap's checklist for that release.

- [ ] **CI is green** on the release commit, including the `upgrade` job, `cargo test --workspace --exclude repomemo-desktop`, `cargo clippy … -D warnings`, `cargo fmt --all --check`, `cargo deny`, `npm audit`, `npm run test:web` and the web checks (`typecheck`, `a11y:check`, `styles:check`, `types:check`, `docs:check`, `migrations:check`, `container:check`). Many tests written since V0.2.46 have been compiled and never executed.
- [ ] **`cargo test -p repomemo-desktop`** passes on the maintainer's machine (CI leaves the crate out).
- [ ] **The performance runs** of [performance.md](performance.md) are made, recorded, and the targets hold (V0.2.54).
- [ ] **A browser pass**: axe or Lighthouse on the signed-in pages and a keyboard-only pass (V0.2.47); the pages at 375 px and 768 px (V0.2.48).
- [ ] **The polished pages** (track K, V0.2.57–V0.2.129) are looked at in both themes at 375 px, 768 px and desktop width: a dialog from each area (focus lands on its first field, Enter submits, Escape closes, focus returns to the button), a form with a missing required field, a long list (24 rows and the pager), a deletion (focus moves on), a dropdown opened inside a dialog, and the web steps 4, 6 to 8 and 11 of §2 made once on them. Nothing in track K was seen on screen while it was written.
- [ ] **The desktop app** is tried against a server: connect, sign in, upload, open a PDF, a link in an answer, *Change server*, (V0.2.49; from V0.3.5 the window has no mode but the server's).
- [ ] **The security fixes** of V0.2.52 are tried against a running server (the spot checks above).
- [ ] **The two images** of the container route are built and started once (V0.2.45).
- [ ] **Tracks L and M** (V0.2.130–V0.2.162) are tried: the container comes up with the read-only root and the converter (and again with `COMPOSE_PROFILES=previews`, a Word file opening as a PDF); a 500 MiB upload leaves the server's memory flat and two uploads at once with `REPOMEMO_UPLOAD_INFLIGHT_MB=1` make the second wait (`503`); a repository sync with nothing new says *Nothing changed*; the evidence section of a workspace with more than 500 files pages by folder, by file type and by name, and a picker (memory card, task, replacement evidence) finds an old file by typing its name; `GET /v1/system/metrics` shows the in-flight and shed counters; and the `performance_at_scale` run is made twice (read tuning off and on) and recorded in [performance.md](performance.md).
- [x] The open items in the checklists of the roadmap's releases are read once more, and each is either done, accepted or moved (V0.3.4: the table below).

### Where each box stands (V0.3.4)

The maintainer reported on 2026-10-24 that the tests of the clean install came out good, and did not give the result of each step. Nothing below is ticked on that report alone, because a tick should say what was seen. The reason each box is open, and what closes it:

| Box | Why it is open | What closes it |
|---|---|---|
| CI is green | CI runs on the host, which this work cannot see. At V0.3.3 here: `cargo fmt --all --check`, `cargo clippy -D warnings` (tests included), `typecheck`, `version:check`, `docs:check` and `a11y:check` pass; the tests, `cargo deny` and `npm audit` were not run | Green run on the release commit; tick it with the run's address |
| `cargo test -p repomemo-desktop` | The crate is left out of CI and of this work; V0.3.6 changes it | The maintainer's run after V0.3.6 |
| Performance runs | They need release builds, which this repository keeps from the automated work | [performance.md](performance.md) §3: the record sheet, with the commits to compare |
| Browser pass; polished pages | They need a running app in a browser; the static `a11y:check` passes (126 components, 94 colour pairs, the dialogs' skip link and the 24 px buttons) but is not a pass | The record sheet below |
| Desktop app tried | It needs a second computer. Step 13 was part of the reported run; its result was not written down | Write the step's result in §4, then tick |
| Security fixes of V0.2.52; two images; tracks L and M | They need a running server and Docker | Steps 14 and 1 of §2, and the track L and M line above, written in §4. V0.3.23 puts `docker compose up` in CI |

### Browser pass: record sheet

One row per page, one mark per cell: `ok`, or the finding (a finding that needs a release takes the next number of the 0.3 line). Tool: axe or Lighthouse for the first column, device mode for the widths, the keyboard alone for the last.

| Page | axe or Lighthouse: serious findings | 375 px light | 375 px dark | 768 px light | 768 px dark | Keyboard only |
|---|---|---|---|---|---|---|
| Sign-in and setup | | | | | | |
| Dashboard and search across workspaces | | | | | | |
| Overview, Evidence, Retrieval | | | | | | |
| An artifact page and the notes editor | | | | | | |
| Memory, Tasks, Review | | | | | | |
| Repositories and connectors | | | | | | |
| Assistant and the command palette | | | | | | |
| Settings, People, System | | | | | | |

### The open items of the roadmap's checklists

Read once more at V0.3.4. Each open box of a 0.2.x release is one of three things:

| Kind | Which boxes | Where it goes |
|---|---|---|
| **Moved to a 0.3.x release** | assistant answers do not name the last author (→ V0.3.16); `CODEOWNERS` owners not matched (→ V0.3.15); memory-card links do not offer *View at commit* (→ V0.3.14); approvals, issue links and mentions between items (→ V0.3.21); webhook deliveries (→ V0.3.22); lookups that read earlier turns (→ V0.3.11); filters and memory cards across workspaces (→ V0.3.12); palette recents and *New note* (→ V0.3.13); Go, Java, C# and PHP symbols (→ V0.3.17); review count and snooze (→ V0.3.10); restoring a version (→ V0.3.8); mentions in tasks and cards (→ V0.3.9); versions and saved searches in archives (→ V0.3.19); import as a job and the whole archive in the browser (→ V0.3.20); backup passphrase rotation (→ V0.3.24); `docker compose` in CI and an `arm64` image (→ V0.3.23); the skip link in dialogs, toolbar roles and 24 px targets (→ V0.3.25); the desktop app's second mode (done in V0.3.5–V0.3.7) | The release named |
| **The maintainer's to try** | a real mail server and mailbox; a clone and calls from a real host, GitHub and GitLab; a very large real repository; the release-build run of `map_and_health_at_scale`; the desktop app and the narrow screens on a real display; the first `docker compose up`; a second clean install | The clean install (§2, §4) and the record sheets above and in [performance.md](performance.md) |
| **Accepted as stated** | a copy of a backup older than the number kept; the plain snapshot in `.partial` for a moment; a child process without a sandbox; no signed or scheduled audit export; *Change server* restarting the app; separate-line-ending checkouts and sqlx's migration checksums; the review's own "not run" notes; prose claims that need a person | The box's own text; they stay as written |

## 4. Result

| Date | Machine | Route | Who | Workarounds found | Fixed in | Verdict |
|---|---|---|---|---|---|---|
| 2026-10-24 | the maintainer's | not recorded | the maintainer | none reported | | **Reported good** for the release `V0.3.0`; the result of each step of §2 was not written down, so none of §3 below is ticked on it |
| *next run* | | | | | | *fill in the result of each step of §2, then tick what it shows* |

When the run passes with no workaround, record it here and in the roadmap's R6 and R12 lines, tick §3, set the version to `0.3.0` (`npm run version:set -- 0.3.0`), write the changelog's `V0.3.0` entry from the releases V0.2.1 to V0.2.129 (the [release index](../RELEASES_0.2.md) is the list; §5 has the notes for the polish track), and tag `v0.3.0`.

## 5. Notes for the V0.3.0 changelog: the frontend polish (V0.2.57–V0.2.129)

A draft for the entry; the [plan](../plans/2026-10-17-frontend-polish.md) and the [release index](../RELEASES_0.2.md) have the detail. No product feature was added; the look is the one that shipped.

**What a person sees**
- One look for every form: fields, dropdowns and buttons are the same height (36 px, 40 px on touch screens), checkboxes and switches are the right shape, and neutral greys have no tint.
- Dropdowns open above dialogs and stay inside the window; long lists show 24 rows a page with the same pager; a repository's file list no longer stops at 300 files.
- Every dialog focuses its first field, submits on Enter, closes on Escape, returns focus to its opener and has one of three widths; a deletion moves focus to the next item.
- A required or malformed field is marked before anything is sent, with a one-line message; Ctrl+Enter (Cmd+Enter on macOS) sends comments, notes, questions and memory statements.
- Leaving a form with unsaved changes asks first, and every change ends in a success toast or a visible change.
- The searchable picker, search field, copy button, tabs, badges, loading and empty states look and behave the same on every page.

**What a developer gets**
- A set of building blocks in `components/ui` described in [DESIGN.md](../../DESIGN.md), tests for each of them, and hooks for mutations, forms, unsaved changes, paging and focus.
- The large components split into folders (`web/system`, `web/repositories`, `web/setup`, `web/map`) and server data loaded through the query cache.
- `npm run styles:check` refuses a new bare element style, raw colour, raw layer or raw control height; 111 older cases are listed in `scripts/styles-baseline.json` and can only shrink.

**Not done:** none of this was seen on screen or run as tests while it was written; §3 lists the browser pass and CI.

## 6. Notes for the V0.3.0 changelog: security and backend (V0.2.130–V0.2.162)

A draft for the entry; the [fifth review](../security/2026-10-21-security-review.md), [UPGRADING.md](../../UPGRADING.md) and the [release index](../RELEASES_0.2.md) have the detail. No product feature was added.

**What an operator gets**
- `cargo deny` and `npm audit` (development packages included) are clean; the TLS stack, the spreadsheet reader and the web toolchain were updated.
- The container's server has a read-only root and no capabilities; LibreOffice previews are opt-in, need a fixed release and, in the container, run in a container of their own with no network (`COMPOSE_PROFILES=previews`).
- Uploads stream to disk, JSON bodies stop at 1 MiB, answers are compressed for a client that asks, and a full server refuses at once (`503` or `429` with `Retry-After`); the counts are in `/v1/system/metrics`.
- New settings: `REPOMEMO_PREVIEW_ROOT`, `REPOMEMO_UPLOAD_INFLIGHT_MB`, `REPOMEMO_SQLITE_CACHE_MB`, `REPOMEMO_SQLITE_MMAP_MB`, `REPOMEMO_MAX_IN_FLIGHT`, `REPOMEMO_MAX_IN_FLIGHT_PER_ACCOUNT`; migrations 0037 to 0041.

**What a person sees**
- The evidence section is cut by the server, 24 files a page, by folder, file type and name, the pickers and the command palette search the server, and the workspace page holds only the 500 newest files for its indexing indicator; a repository sync with nothing new says *Nothing changed*.
- Links that leave the site say so; Office files that can carry macros are not opened in their application; a job is stopped by whoever started it or an administrator.

**Not done:** none of this was run as tests, built or seen while it was written; §3 lists what to try, and the performance runs (before and after, [performance.md](performance.md)) are the maintainer's.
