# Clean-machine install: the run before the tag

> **Branch of:** [TECHNICAL_MINDMAP.md](../TECHNICAL_MINDMAP.md) · **Roadmap:** [R6 (V0.2.0)](../ROADMAP.md#v020-team-ready), [R12 (V0.2.56)](../ROADMAP.md) · **Guide followed:** [operations.md](operations.md) · **Status:** V0.2.56. The run itself is the maintainer's (it needs a second machine and a running server, which this repository's rules keep from the automated work), so the result table at the end is empty until it is made.

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
- [ ] **The desktop app** is tried against a server: connect, sign in, upload, open a PDF, a link in an answer, *Change server*, and *Move a workspace to a server* (V0.2.49, V0.2.50).
- [ ] **The security fixes** of V0.2.52 are tried against a running server (the spot checks above).
- [ ] **The two images** of the container route are built and started once (V0.2.45).
- [ ] The open items in the checklists of the roadmap's releases are read once more, and each is either done, accepted or moved.

## 4. Result

| Date | Machine | Route | Who | Workarounds found | Fixed in | Verdict |
|---|---|---|---|---|---|---|
| *not yet run* | | | | | | |

When the run passes with no workaround, record it here and in the roadmap's R6 and R12 lines, tick §3, set the version to `0.3.0` (`npm run version:set -- 0.3.0`), write the changelog's `V0.3.0` entry from the releases V0.2.1 to V0.2.56 (the [release index](../RELEASES_0.2.md) is the list), and tag `v0.3.0`.
