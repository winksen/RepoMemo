# Authentication and authorization

> **Branch of:** [TECHNICAL_MINDMAP.md §4](../TECHNICAL_MINDMAP.md#4-request-lifecycle-authentication-and-authorization) · **Related:** [api-reference.md](api-reference.md) (every route with its guard), [security review](../security/2026-10-10-security-review.md) · **Status:** V0.2.55 (§4 is what the 0.2.x line added)

This page is the model on one page: who a request is, what that person may do, and the rules a new handler must follow. The mindmap's §4 keeps the history and the details.

---

## 1. Who is calling

```mermaid
flowchart TD
  R[Request] --> H{Authorization header}
  H -->|none| P[public route only]
  H -->|"Bearer rmp_…"| T[personal access token<br/>SHA-256 lookup]
  H -->|Bearer JWT| J[HS256, issuer, expiry,<br/>session version]
  T --> S[AuthenticatedSubject]
  J --> S
  S --> X{extractor the handler takes}
  X -->|AuthenticatedSubject| A[session or token;<br/>read-only tokens only GET and HEAD]
  X -->|ReadingSubject| B[like A, and a read-only token<br/>may POST a read]
  X -->|SessionSubject| C[session only, never a token]
```

| Credential | Lifetime | Stored as | Ends when |
|---|---|---|---|
| Access token (JWT) | 60 min (`REPOMEMO_ACCESS_TOKEN_TTL_MINUTES`) | not stored | it expires, or the user's *session version* changes |
| Refresh token | 30 days (`REPOMEMO_REFRESH_TOKEN_TTL_DAYS`), rotated on every use | SHA-256 hash | reuse of a rotated one revokes the family; sign-out; password change |
| Web refresh cookie | same | the refresh token itself | `HttpOnly; SameSite=Strict; Path=/v1/auth`, `Secure` on HTTPS |
| Personal access token | 7–365 days | SHA-256 hash | revoked, expired, or a used password reset link |
| Invitation or reset link | 7 days / 24 hours, single use | SHA-256 hash | used, withdrawn, expired |
| File link (Open in Office) | 60 seconds, first client | signed JWT with a `jti` | expires or is used by another client |

A **session version** on the user is stamped into each access token and checked on every request, so changing a password, signing out everywhere or deleting an account ends every session at once.

Sign-in answers identically for an unknown email and a wrong password and spends the same time. Failed sign-ins lock an account per address (10) and everywhere (50) for 15 minutes; a device that holds one of the account's refresh tokens gets past a lockout other people caused.

---

## 2. What they may do

### Roles

| Level | Roles (high to low) | Comes from |
|---|---|---|
| Organization | owner, admin, member | `organization_memberships` |
| Workspace | owner, admin, member, viewer | `workspace_memberships`; an organization role propagates to every workspace of it, except that adding a person to one workspace does not make them a member of the organization (they appear as a *guest*) |
| Server | system administrator, app administrator | `users.is_system_admin`, `users.is_app_admin` |

System administrators act as **administrators in every organization and workspace** (and owners of what they own). App administrators use the System pages but have no wildcard access, and only *read* them and run jobs and maintenance, and list backups; settings, roles, sessions, lockouts, reset links, raw log files, detaching the environment, and making, restoring or deleting backups are for system administrators.

### What each workspace role can do

| | viewer | member | admin | owner |
|---|:-:|:-:|:-:|:-:|
| Read evidence, search, ask (if the AI policy allows), see memory, tasks, map and health | ✔ | ✔ | ✔ | ✔ |
| Upload, add notes, comment, create tasks and memory cards, move and delete files, change evidence states | | ✔ | ✔ | ✔ |
| Link repositories, act on health findings, manage AI providers and policy, members and invitations, see activity | | | ✔ | ✔ |
| See raw passages of a file | | | ✔ | ✔ |
| Rename or delete the workspace, hand it to another owner | | | | ✔ |

The server decides; `GET …/capabilities` only tells the web app what to hide.

### Rules that hold everywhere

1. **Nobody grants `owner`** through a member form; ownership moves only with *transfer ownership* (a session, by the current owner, to a current member).
2. **Nobody changes an owner's role** or removes an owner. Removing an organization member who owns one of its workspaces is refused (409).
3. **An admin cannot grant `admin`** or change or remove an owner or admin.
4. **Authors keep their content** only while they can still write: a viewer cannot edit their old comments or tasks.
5. **People are added by invitation**: a member form does not add someone who is not already in the organization or workspace (`invitation_required`, the same answer for known and unknown addresses).
6. **Limits protect the server, not people's rights**: per-person rate limits (heavy requests, AI, uploads), per-workspace allowances (storage, background AI), per-person workspace count and a free-disk floor. They answer 429, 413, 409 or 507.

---

## 3. Adding a handler

1. Pick the extractor: `AuthenticatedSubject` for anything a token may do, `ReadingSubject` for a POST that only reads, `SessionSubject` for anything that changes who can get in (passwords, tokens, roles, reset links, ownership, settings, detaching).
2. Call the guard **before any work**, with the workspace or organization of the *resource* (load it first when the route names a resource id): `require_workspace_read/write/admin/owner`, `require_organization_*`, `require_system_admin`, `require_full_system_admin`, `require_system_change`.
3. For anything that spends a limited thing, count it: `consume_heavy_quota` (cheap to ask, costly to answer), `authorize_ai_use` (calls a provider), `quotas::check_incoming` (stores content).
4. Return typed errors (`not_found!`, `invalid!`, `conflict!`); anything else is a generic 500.
5. Record who did it: `record_workspace_activity` for workspace changes, `audit` for System changes.
6. Run `node scripts/list-routes.mjs`: a route marked `check` has an extractor but no guard call, which is right only for a person's own data (profile, notifications, own tokens).
7. Add a test that a viewer, a stranger and an anonymous caller are refused.

---

## 4. What the 0.2.x line added

- **Verified addresses** (V0.2.13). `require_verified` stands before creating an organization or a workspace, importing one and joining one: an unverified account can sign in and read, not create or join. An invitation to that address counts as verification.
- **Deactivated and anonymized accounts** (V0.2.16, V0.2.17). A deactivated account cannot sign in or use a token, and its sessions, tokens and invitations end at once. Anonymizing also removes the address and name; what the person wrote stays, credited to "Former member".
- **Self-service reset** (V0.2.11). `POST /v1/auth/forgot-password` answers the same for every address; the link is one-time, valid for one hour, made from `REPOMEMO_PUBLIC_URL`, and ends every session when used.
- **Two ways to carry a session.** The web app asks with `X-RepoMemo-Client: web` and gets its refresh token in an `HttpOnly` cookie. Every other client (the console, scripts, and the desktop app, which says `desktop`) gets it in the response body and sends it back in the body; the desktop app keeps it in the operating system's credential store (V0.2.49).
- **Credentials that are not sessions.** A webhook secret signs a call for one source (GitHub, GitLab or `X-RepoMemo-Signature`) and can only queue a sync; a file link is a 60-second token for one file; a personal access token (`rmp_…`) has a read, write or `system` scope; repository and tracker tokens and deploy keys are sealed in `repo_credentials` and only ever sent to their own host.
- **The anonymous surface** is the public handlers of the [API reference](api-reference.md) (sign-in, registration, refresh, sign-out, reset and verification links, invitation previews, setup, the webhook receivers, shared file links, health). Since V0.2.52 each takes a small body (64 KiB; webhooks 1 MiB), failures count per client address, and none says whether an account or a source exists.
- **Mail is a power too.** Anything that sends mail (invitations, mentions, assignments, verification links) is limited per recipient (12 an hour) and per person who causes it (60 an hour), with a ceiling on the outbox (V0.2.52).
- **The desktop window** may call one server (a permission added at run time for that origin) and may show only the bundled app; it has no IPC for any other origin.
