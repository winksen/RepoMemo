# RepoMemo security review — 2026-10-16 (V0.2.52, the fourth pass)

> **Scope:** the surfaces the 0.2.x line opened since the [review of 2026-10-14](2026-10-14-security-review.md): **mail** (SMTP delivery, invitations, self-service reset, verification, notifications, the digest), **webhooks** (repositories and connectors), **remote clones** (`https` and `ssh`), **connectors** (GitHub and GitLab issues and pull or merge requests), **export and import** of workspaces, the **audit export** and the **desktop client** (the server connection, the credential store, the move of a local workspace). It uses [§8 of the previous review](2026-10-14-security-review.md#8-security-requirements-for-the-03-surfaces) as its checklist.
> **Method:** a static review of the code at `V0.2.51`, read for these surfaces with the checklist beside it. The code of each requirement was found and read; where a requirement was not met, it was fixed in this release, with a test. **Nothing was run, built or attacked**; the tests written for the fixes were compiled (`cargo check --tests`) and not executed. `cargo deny` and `npm audit` were not run here and remain the check of record in CI.
> **Status:** **closed.** 0 high, 3 medium, 4 low and 5 informational findings; the 3 medium and the 4 low are fixed in `V0.2.52`. No high or medium finding is open.
> **Not covered:** the surfaces of earlier reviews (sessions, roles, repositories on disk, AI providers, file previews), which have their own passes and were not read again; the retired local edition beyond its Markdown (it gets security fixes only and is removed in a 0.3.x release); dependencies (CI); a running system.

| ID | Severity | Finding | Status |
|---|---|---|---|
| S4-1 | **Medium** | Mail had no limits. Any address could be sent as much as someone could cause: invitations to the same address, mentions and assignments, and a verification link to any address by changing one's own email. The server was a free mail relay with the inviter's words in it, and the outbox had no ceiling | ✅ Fixed |
| S4-2 | **Medium** | The public endpoints (sign-in, registration, refresh, reset, invitations, setup, and the two webhook receivers) read a request body up to the upload limit, 10 MB by default and up to 1 GiB when an operator raises it, before looking at who sent it | ✅ Fixed |
| S4-3 | **Medium** | The import trusted the sizes an archive declares. A few hundred bytes of archive could unpack to 1 GiB in memory per file, and the total of the declared sizes could wrap around to a small number, which also defeated the storage allowance check | ✅ Fixed |
| S4-4 | Low | A signed webhook call could be replayed for as long as its secret lived: nothing remembered which deliveries had been received, so a captured call kept a source syncing and used up its per-minute allowance | ✅ Fixed |
| S4-5 | Low | A connector read a tracker's whole answer into memory before comparing it with its 20 MiB cap, so a tracker that never stops sending could fill memory | ✅ Fixed |
| S4-6 | Low | The desktop app's HTTP client followed redirects, and the shell's permission only names the first address, so a server that redirects could have the client follow it to another host | ✅ Fixed |
| S4-7 | Low | `ssh` for a remote clone relied on `-F /dev/null` and the deploy key's options to avoid a proxy command; `ProxyCommand=none` was not stated, as the checklist asks | ✅ Fixed |
| S4-8 | Info | The self-service reset allows a few mails per address per hour. Someone who knows an address can use that budget up; the administrator's reset link still works | Accepted |
| S4-9 | Info | git resolves host names itself, so the allowed-hosts list is advisory for names that resolve to private addresses; an egress rule is what enforces it (already in the production guide) | Documented |
| S4-10 | Info | The desktop app's session sits in the operating system's credential store, which any process of the same user can read. It is protection against files, backups and other users, not against malware running as the user | Documented |
| S4-11 | Info | A generic webhook sender that sends no `X-RepoMemo-Delivery` can still be replayed within the per-source rate limit (60 a minute). A call only queues a sync, which a running sync absorbs | Accepted |
| S4-12 | Info | The retired local edition is still in the app until a 0.3.x release. It talks to nothing, renders Markdown safely since V0.2.51 and gets security fixes only | Tracked |

| Severity | Count | IDs |
|---|---:|---|
| Critical | 0 | — |
| High | 0 | — |
| Medium | 3 | S4-1, S4-2, S4-3 |
| Low | 4 | S4-4–S4-7 |
| Info | 5 | S4-8–S4-12 |

---

## 1. The checklist, surface by surface

The requirements are those of §8 of the previous review. A tick is a requirement found in the code and read; "fixed" is one this review found missing.

| Track | Requirement | Result |
|---|---|---|
| **B. Mail** | No header injection: addresses, names and subjects checked; no control characters; non-ASCII encoded | ✅ `crates/mail/src/compose.rs` refuses control characters in every header value, accepts only plain ASCII addresses with one `@`, encodes names and subjects (RFC 2047) and the body (base64), so a crafted name cannot add a header or a recipient |
| | SMTP host through the outbound rules; TLS required with certificate checks, no plaintext fallback | ✅ as shipped in V0.2.9; not read again beyond the policy type |
| | Links built from `REPOMEMO_PUBLIC_URL`, never from `Host` | ✅ `MailService::link` is the only maker; a reset or invitation link is not sent when the address is unknown |
| | **Rate limits per sender and per recipient** | **Fixed (S4-1)**: 12 messages an hour to one address, 60 an hour caused by one person, none queued while 1,000 wait |
| | Forgot password: same answer and time for every address, a one-time link, newer replaces older, an email change ends pending links | ✅ the answer is built before and apart from the lookup, which runs in a spawned task; one answer for known, unknown and malformed addresses |
| **D. Remote clones** | git version, bare partial clone, `protocol.allow=never` then one protocol, `transfer.fsckObjects`, no submodules, no redirects | ✅ `crates/git/src/remote.rs` (`remote_command`): cleared environment, `GIT_ALLOW_PROTOCOL`, `protocol.allow=never`, `http.followRedirects=false`, `transfer.fsckObjects`, `submodule.recurse=false`, `core.fsmonitor=false`, empty `credential.helper` and `core.askPass`, a refused git older than the patched releases |
| | URLs: no leading `-`, `ext::`, `file://`, local paths, no embedded credentials | ✅ `remote_policy.rs` (`transport_marker`, the leading `-`, host labels, `user` characters) with tests for each |
| | Credentials through `GIT_ASKPASS`, never in the URL or arguments; environment scrubbed | ✅ the program itself as askpass, secrets only in that one process's environment; `Debug` hides them |
| | `ssh`: batch mode, strict host key, pinned `known_hosts`, `ProxyCommand=none` | ✅ all of it, with **`ProxyCommand=none` added (S4-7)** |
| | Limits: size and time caps | ✅ `TransferLimits`, measured while the transfer runs (V0.2.22) |
| **D. Webhooks** | HMAC-SHA256 or the token in constant time; same `401` for every failure; failures and calls limited | ✅ `same_secret` and `ring`'s verify; a stand-in secret is checked when the source does not exist, so time does not tell |
| | A body size limit before the body is read | **Fixed (S4-2)**: the route takes 1 MiB; before, the global upload limit applied first |
| | **A delivery id against replay** | **Fixed (S4-4)**: a delivery id received in the last hour is answered `202 already received` without counting against the source |
| | The payload only triggers a sync | ✅ only the branch name is read from it |
| **E. Connectors** | Tokens encrypted like provider keys, never returned | ✅ the same store as the repository credentials |
| | Self-hosted URLs through the outbound rules; redirects not followed; a next page on another host never followed | ✅ `reqwest` with `Policy::none()`; the token stays with its host |
| | A response cap | **Fixed (S4-5)**: read piece by piece up to 20 MiB |
| | Issue and review bodies are untrusted | ✅ rendered by `SafeMarkdown` (no images, links named); the assistant's capability router unchanged |
| **G. Portability** | Import: no path traversal, no symlinks | ✅ entry names are only ever used as keys: blobs are stored under the hash of their bytes (checked to equal the name), the other entries are skipped; only regular file entries are read |
| | Import: size and ratio limits against bombs | **Fixed (S4-3)**: the expansion is limited to 100 to 1 (at least 64 MiB), each entry is read up to what the manifest declares, and the declared total no longer wraps |
| | Import: fresh ids, references re-validated, quotas applied | ✅ V0.2.41 (`WorkspaceArchiveData::check`, the allowance and disk floor in `room_problems`) |
| | Export: owners and administrators only, audited | ✅ `require_workspace_admin`, a `workspace_exported` activity line, and the audit export (full system administrators) is itself audited before its first byte |
| | Audit export: no spreadsheet formulas | ✅ cells that start with `=`, `+`, `-`, `@`, a tab or a return get an apostrophe |
| **I. Desktop** | The session in the operating system's credential store | ✅ the `keyring` crate; nothing of the refresh token in a file or the page's storage (a test asserts it) |
| | The window stays on the app's own pages; no IPC for other origins | ✅ `on_navigation(is_app_url)`, no capability with a remote URL, the HTTP permission added at run time for one origin and one window |
| | No signing key in the bundle | ✅ no updater, no signing configuration; `envPrefix` still excludes `TAURI_` |
| | Redirects | **Fixed (S4-6)**: `maxRedirections: 0` |
| | Moving a local workspace | ✅ the shell touches only files directly inside its `exports` folder (`resolve_in`, canonicalized); the server's import is the one that checks the archive |
| | Retiring the local edition removes its unsafe Markdown (E10) | ✅ `SafeMarkdown` since V0.2.51; the edition itself goes in a 0.3.x release |

---

## 2. Medium

### S4-1. Mail had no limits

**Where:** `apps/server/src/mail.rs` (`MailService::send_to`), the invitation path (`mail_invitation`), notifications (`mail_notification`), `verification.rs` (`change_email`).

**What happened:** the checklist asks for per-sender and per-recipient limits "so invitations and mentions cannot be used to flood a mailbox". The only limits in the mail code were the test message cap and the per-address budget of the self-service reset. Everything else queued without a count:
- a workspace administrator (any verified account can still create an organization and so be one: E3 of the previous review) could invite one address as often as they liked: each invitation is a mail with their own wording (the workspace's name) sent from the server's sender address;
- a member could mention or assign a colleague over and over, each a mail;
- any account could change its own email address to a victim's as often as it liked (the current password is needed, nothing else), each time sending the victim a verification link;
- the outbox had no ceiling.

**Fix:** `send_to` counts per recipient (12 messages an hour, whatever the kind except the admin's own test message) and refuses when 1,000 messages are waiting; invitations and notifications are also counted per person who caused them (60 an hour). The invitation is still created and its link returned, with `mail: "rate_limited"` and a warning in the page; a notification over the limit is dropped and logged. Test: `an_address_cannot_be_flooded_with_invitations_through_the_server` sends 14 invitations to one address (12 queued, 2 not, 12 in the outbox) and shows another address is unaffected.

### S4-2. The public endpoints read a body as large as the upload limit

**Where:** `apps/server/src/lib.rs`, the global `DefaultBodyLimit::max(config.max_upload_bytes)`.

**What happened:** `REPOMEMO_MAX_UPLOAD_MB` (default 10, up to 1,024) is one limit for the whole API. The sign-in, registration, refresh, reset, invitation and setup endpoints, and the two webhook receivers, are public and take small JSON documents, but their extractors buffered up to that limit before anything could refuse the caller. The webhook handler checked its own 1 MiB only after `Bytes` had read the body. With the limit raised, a few hundred anonymous requests held gigabytes.

**Fix:** a route-level `DefaultBodyLimit` of 64 KiB on those twelve endpoints and 1 MiB on the webhook receivers. Test: `anonymous_endpoints_take_small_bodies_whatever_the_upload_limit_is`.

### S4-3. The import believed the sizes an archive declares

**Where:** `apps/server/src/workspace_import.rs` (`read_archive`), `crates/domain/src/archive_import.rs` (`blob_bytes`).

**What happened:** `ArchiveReader::next_entry` reads an entry into memory up to a maximum chosen by the caller, and the importer allowed 256 MiB for the data and 1 GiB for each stored file whatever the manifest said. A gzip stream of zeros is a thousand times smaller than what it unpacks to, so a 1 MB upload could make the server hold 1 GiB (and 256 MiB of JSON that serde then parses into more). Separately, `blob_bytes()` summed the declared sizes with `.sum()`, which wraps in a release build: two declared files of 2^63 bytes added up to 0, so the storage allowance and disk checks saw nothing. The caller has to be an owner or administrator of an organization, which any verified account can become (E3 of the previous review).

**Fix:** the sum saturates; the importer refuses, before unpacking anything, an archive whose declared content is more than 100 times its own size (and over 64 MiB, which any archive may reach); and each entry is read only up to what the manifest declares for it. Tests: `the_total_of_declared_sizes_saturates_instead_of_wrapping` and `an_archive_that_claims_far_more_than_it_is_made_of_is_refused_before_it_is_unpacked`.

---

## 3. Low

- **S4-4. Webhook replay.** `webhooks.rs` now remembers the delivery ids it has answered for an hour (`X-GitHub-Delivery`, `X-Gitlab-Event-UUID`, `X-RepoMemo-Delivery`) and answers a repeat `202` with `"already received"` before the per-source count, so replaying a captured call neither syncs again nor uses up the real sender's allowance. GitHub's manual redelivery keeps its id, so within the hour it is answered the same way. Test: in `a_repository_webhook_…` (`tests/evidence.rs`).
- **S4-5. Connector answers.** `read_capped` in `crates/api/src/connectors.rs` refuses a declared length over the cap and stops reading at the cap.
- **S4-6. Desktop redirects.** The HTTP plugin checks its permission against the address it is given, not against where a redirect leads. The client now passes `maxRedirections: 0`, and the server API has no redirects to follow.
- **S4-7. `ProxyCommand=none`** is part of the `ssh` command line of a remote clone.

---

## 4. Information

- **S4-8.** Accepted: the reset budget is per address and per hour (`FORGOT_PER_ADDRESS_PER_HOUR`), the same for every address so it tells nothing. A system administrator can always issue a link.
- **S4-9.** The production guide says the allowed-hosts list is advisory for names that resolve to private addresses.
- **S4-10.** The guide for the desktop app should say what the credential store is and is not (see the changelog of V0.2.49).
- **S4-11.** A call from a sender with no delivery id carries nothing to remember; the per-source limit and the single running sync bound what a replay can do. A sender of generic calls is asked to send `X-RepoMemo-Delivery`.
- **S4-12.** Until it is removed, the local edition receives security fixes only.

---

## 5. Checked and found sound

- **Webhook secrets** are compared with `ring` in constant time; an unknown source, a source with no webhook and a wrong signature all answer the same `401` after the same work; failures count against the client address, not successes.
- **Mail composition** cannot carry an extra header or recipient (see the checklist).
- **Self-service reset** never reveals whether an address has an account, in the answer or in the time taken, and sends only when mail is set up and the public address is known.
- **Git for remote clones** runs with an empty environment and a single allowed protocol; credentials never appear in an address or argument; hosts starting with `-`, `ext::`, `file:` and local paths are refused before any network call.
- **Connector tokens** are encrypted, never returned and never sent to another host, because redirects and next pages on other hosts are not followed.
- **The archive reader** only reads regular file entries, and the importer never turns an entry name into a path.
- **The desktop shell's file commands** resolve every path to a file directly inside the exports folder before reading, copying or deleting it.

---

## 6. For the next pass

- Run `cargo deny check` and `npm audit` (CI does) after the new crates (`tauri-plugin-http`, `keyring`) of V0.2.49.
- Try the fixes with a running server and the desktop app: a flood of invitations, a 200 KiB body to sign-in, a bomb archive, a replayed webhook delivery.
- Review the 0.3.x removal of the local edition, which takes `App.tsx`, 26 Tauri commands and `RepoMemoCore::boot` out of the desktop shell.
