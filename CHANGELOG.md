# Changelog

What changed in each release, newest first, with what to do when upgrading. Versions follow [semantic versioning](https://semver.org): `0.2.0` is the next minor release (see the [roadmap](mindmap/ROADMAP.md#-road-to-v020)), then `0.2.1` and so on for fixes. Releases before V0.1.63 are summarized in the roadmap's *Delivered* tables.

## V0.1.67 (2026-10-14)

Safe defaults and identity (bundle S2).

### Security
- **Registration is closed by default (M1).** `REPOMEMO_ALLOW_REGISTRATION` now defaults to `false`; people join with invitation links, and the first account of a new server can still register. `/health` no longer says whether registration is open (I2).
- **Adding someone no longer reveals whether they have an account (M4).** The member forms add only people who are already in the organization (or workspace); anyone else gets the same `409 invitation_required` answer, whether or not the address has an account, and the web app offers an invitation link. Nobody is added to an organization or workspace without accepting.
- **App administrators read, system administrators change (M8).** App administrators keep the System pages, jobs and maintenance. Settings, roles, other people's sessions, lockouts, reset links, raw log files and detaching the environment need a system administrator, and workspace activity records are hidden from app administrators in the logs.
- **A `system` scope for personal access tokens (N2 residual).** Only administrators can create one. Changes in the System area (sign people out, unlock, run maintenance, console `--yes`) need a signed-in session or a `system` token; read-and-write tokens no longer reach them. Migration 0024.
- **Transport checks (M9).** The server refuses to listen beyond loopback unless `REPOMEMO_PUBLIC_URL` is `https://…`, `REPOMEMO_TRUST_PROXY` is on, or `REPOMEMO_ALLOW_INSECURE_HTTP` says the network is trusted. An HTTPS public address turns on `Strict-Transport-Security`. `repomemo-console` warns when it talks plain `http://` to another machine.
- **Client addresses behind a proxy (M3).** `X-Forwarded-For` counts only when the TCP peer is loopback or listed in `REPOMEMO_TRUSTED_PROXIES`, and is read from the right, so a client cannot choose its own address.
- **Lockouts no longer lock owners out (L10).** A sign-in that carries a session this browser still holds gets past a lockout other people caused.
- **The account hears about reset links (L4):** a notification when an administrator creates one and another when it is used.
- **Weak passwords refused (L7):** the most common long passwords, passwords of fewer than five different characters, and passwords containing the email name.

### Changed
- The member forms say "is not in this organization yet. Create an invite link for them below." instead of asking whether an account exists.

### Upgrading
- A server that relied on open registration must set `REPOMEMO_ALLOW_REGISTRATION=true` (or turn it on in System › Settings). A value saved in System › Settings is kept as it was.
- A server bound beyond loopback over plain HTTP stops at startup until it has `REPOMEMO_PUBLIC_URL`, `REPOMEMO_TRUST_PROXY` or `REPOMEMO_ALLOW_INSECURE_HTTP=true`.
- Scripts that added existing accounts straight to a workspace must create an invitation instead.
- Scripts that ran System changes with a read-and-write token need a `system` token.

## V0.1.66 (2026-10-13)

Checkable releases: the first step of the road to V0.2.0.

### Added
- **Continuous integration** (`.github/workflows/ci.yml`) on every push to `main` and every pull request. Blocking: versions agree, Rust tests on Linux and Windows, the web type check, and security advisories for the server's dependencies (`cargo deny`, policy in `deny.toml`) and the web client's production dependencies (`npm audit`). Reported but not yet blocking: rustfmt and clippy, licenses and duplicate crates, and advisories that only reach the desktop shell.
- **One version source.** `npm run version:set -- <version>` (`scripts/set-version.mjs`) sets `Cargo.toml`, `Cargo.lock`, both `package.json` files, `package-lock.json` and `tauri.conf.json` together; `npm run version:check` (also in CI) fails when they disagree.
- **Performance timings.** Search, Ask, workspace metrics, the artifact query, the knowledge map and health checks record how long they took and how much they handled (`timing` target), with the workspace. Turn on **System › Settings › Logging › Performance** at *debug* to see them in System › Logs, filterable by workspace. New setting: `log_performance_level`.
- This changelog.

### Changed
- The version reported by System › Overview and the console is correct again: it had stayed at 0.1.60 since V0.1.60.
- `PRODUCT.md` describes the self-hosted team product instead of the desktop-only one. `docs/IMPLEMENTATION_TRACKER.md` is retired in favour of the roadmap and this changelog.

## V0.1.65 (2026-10-13)

The security second pass ([review §11](mindmap/security/2026-10-10-security-review.md#11-second-pass-2026-10-13)).

### Security
- **Stored XSS through image previews closed (N1, high).** Stored and served content types now come from the file name, never from the uploader (also I3); HTML, scripts and unknown types are served as plain text. Image previews open in a tab of their own only for raster images.
- Run-time settings changes need a signed-in session; personal access tokens can still run dry runs (N2).
- A workspace or organization can no longer lose its owner through the member list or an organization role change (N3).
- Someone downgraded to viewer can no longer edit or delete their earlier comments or tasks; saved searches are deleted by their author or an administrator (N4).
- Using a password reset link also revokes the account's personal access tokens (N5).
- `POST /v1/workspaces/{ws}/index` (synchronous, whole workspace) is for administrators (N6).

### Upgrading
- Scripts that changed settings with a personal access token must use a signed-in console (`repomemo-console` with a password) instead.

## V0.1.64 (2026-10-13)

Security ship-blockers for onboarding (bundle S1).

### Security
- Adding someone to one workspace, directly or by invitation, gives that workspace only; their organization shows on their dashboard as a guest entry (H1).
- Registering an address listed in `REPOMEMO_SYSTEM_ADMIN_EMAILS` grants nothing; only accounts that exist at startup are promoted (H4).
- What personal access tokens may reach is marked on the handlers instead of a path list (L2).
- Invitation and reset links carry their secret in the URL fragment (`/invite#…`, `/reset-password#…`) and the web app sends no referrer; older path links still open (L3).
- Only `VITE_` and `TAURI_ENV_` environment variables reach the web bundle (M7).

### Upgrading
- People added to a single workspace before V0.1.64 kept the organization membership they received then, and with it every workspace of the organization. Review organization members once and remove anyone who should only see one workspace.

## V0.1.63 (2026-10-13)

Onboarding without an account (migration 0023).

### Added
- **Invitation links** for one email address and role, single-use, valid 7 days, withdrawable, usable even when registration is closed.
- **Password reset links** issued in System › Users, one-time, valid 24 hours.
- **Personal access tokens** (`rmp_…`, read-only or read and write, 7–365 days) for scripts, CI and `repomemo-console` (`REPOMEMO_TOKEN`).
