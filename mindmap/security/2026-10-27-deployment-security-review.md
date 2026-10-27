# Deployment security review — 2026-10-27 (V0.4.65, the eighth pass, on what 0.4.x added)

> **Scope:** what V0.4.1–V0.4.64 added around the server and the client rather than inside them: the cross-origin cookie (V0.4.17), the web image and its `config.json` (V0.4.15–V0.4.18), `_FILE` secrets (V0.4.1), the container files and scripts (`deploy/`, V0.4.23–V0.4.41), the release pipeline and the images (V0.4.37–V0.4.42, V0.4.35), and the desktop app's new surface (V0.4.44–V0.4.61): the transport, the preset and the configuration file, the installers, notices, links, the certificate relay, updates, signing and the save dialog.
> **Method:** a static reading of the code and the workflows at `V0.4.64`, asking of each place what a person on the network, a website the person visits, another process on the same computer, the holder of a tag, an operator or a dependency can make it do that they should not. Nothing was run against a live system. The checks that already guard the surface (`container:check`, `pins:test`, `desktop:check`, `cargo deny`, the image scan) were read as part of it.
> **Status:** closed on 2026-10-27 at `V0.4.65`. **0 high, 1 medium, 2 low, 5 info:** the medium and both lows are fixed, none is open; the informational findings are accepted and written down.
> **Previous review:** [2026-10-24](2026-10-24-0.3-security-review.md) (the seventh pass, on 0.3.x), closed at V0.3.26.

| ID | Severity | Finding | Status | Release |
|---|---|---|---|---|
| X1 | Medium | The Windows desktop workflow gave the update **signing private key** (and its password) to the whole job as environment variables, so every step saw it: `npm ci`, whose dependencies' install scripts run other people's code, and the Rust build scripts, as well as the step that signs. A compromised dependency could have read the key from the environment and signed an update for every installed app. | Fixed: the key is given to the one build step that signs; `npm ci` and the rest no longer see it. `container:check` refuses it at job level. The Rust build scripts of the locked dependencies still run inside that step, because the bundler signs at the end of the build: see X4 | V0.4.65 |
| X2 | Low | The web image put `REPOMEMO_PRODUCT_NAME` into `/config.json` unchecked (the server checks its own copy, the image did not). A name with a quote breaks the page's configuration; one with `{placeholders}` is read by Caddy at **request time**, so `{env.…}` in a name would have been answered to anyone who asks for `/config.json`. Only the operator sets it, so no outsider could use it, but a copied setting or a templated one could leak an environment variable. | Fixed: the entrypoint refuses a quote, backslash, brace, dollar sign, backtick, control character, second line or more than 60 characters, naming the rule; CI starts the image with three bad names and requires the refusal | V0.4.65 |
| X3 | Low | A `NAME_FILE` secret was read whole with no limit, so a path that named something huge or endless (a wrong mount, a device) would have been read into memory before the server could refuse to start. | Fixed: at most 64 KiB is read and a larger file is refused with its name; a test covers both sides of the limit | V0.4.65 |
| X4 | Info | The update signing key is still in the environment of the build step, and so visible to the build scripts of the Rust dependencies while the bundler runs. | Accepted: the bundler needs it at the end of the same command; the dependencies are the locked ones (`--locked`, `cargo deny`, Dependabot's grouped weekly proposals); the key and the build exist only on a runner that is thrown away. Revisit if the bundler can sign a finished file with the version in its signature | — |
| X5 | Info | Under `SameSite=Lax` or `None`, a request that carries the refresh cookie and **no** `Origin` header is let through (the console and scripts send none). | Accepted: a browser always sends `Origin` on a cross-site `POST`; the route needs the `X-RepoMemo-Client: web` header, which a cross-site page cannot send without a CORS preflight that the server answers only for listed origins; `Origin: null` is refused | — |
| X6 | Info | The configuration file's `locked`, the compiled-in address and the certificate bundle are policy, not a barrier against the person at the keyboard: a user can edit a file beside a program they own, or a portable copy, and the relay's secret is in the page's memory. | Accepted and written down in the administrator's guide (§ 2 says which handout cannot be un-pointed and which can); the server's own accounts and sessions are the boundary | — |
| X7 | Info | A tag publishes images, archives and (with the keys) a signed update, and the workflow that runs is the one in the tagged commit. Whoever can push a `v*` tag can therefore change the workflow that publishes. | Accepted, with a recommendation for the maintainer: protect the `v*` tags (a tag ruleset limited to maintainers) and require the release environment's approval; GitHub, not the repository, enforces it | — |
| X8 | Info | `quickstart.sh` is often run as `curl … \| sh`, and its checksum covers the archive it downloads, not itself. | Accepted: the guide gives the careful way (download, check against `SHA256SUMS`, read, run) first-class; the script refuses an archive that does not match and unpacks nothing | — |

| Severity | Count | IDs |
|---|---:|---|
| Critical | 0 | — |
| High | 0 | — |
| Medium | 1 | X1 |
| Low | 2 | X2, X3 |
| Info | 5 | X4, X5, X6, X7, X8 |

---

## 1. The cross-origin cookie (V0.4.17)

| Concern | What was read | Result |
|---|---|---|
| Who gets the cookie | `web_session` touches only requests with `X-RepoMemo-Client: web`; the cookie is `HttpOnly`, `Secure` (the `__Secure-` name) over https, path `/v1/auth`, `SameSite` from the setting | Sound |
| `SameSite=None` without https | `config.rs` refuses it unless the public address is `https://` | Sound |
| A site the person visits | Under `Lax` and `None` an `Origin` that is not listed is refused; `null` is not listed; CORS lists origins, never `*`, and credentials only for them | Sound; see X5 for a request with no `Origin` |
| Logout and refresh forged from another site | They need the custom header, hence a preflight, hence a listed origin | Sound |
| The cookie's reach | Sign-in, refresh, sign-out and sign-out-everywhere only; the refresh token is replaced in the body by the cookie and blanked in the answer | Sound |

## 2. The web image and `config.json` (V0.4.15–V0.4.18)

The image serves static files as user `10001` on a read-only root with no capability; the content security policy is the build's, plus the API's origin when one is given, which the entrypoint restricts to letters, digits and `. : / _ - [ ]` (nothing that could add a directive). `/config.json` is `no-store` and carries the API address and the product name. The upstream is checked to be a host and a port. **X2** was the product name. The forwarded client address is believed only from private ranges by default (`REPOMEMO_WEB_TRUSTED_PROXIES`), which the guide tells an operator to narrow when the edge is elsewhere. Nothing else found.

## 3. `_FILE` secrets (V0.4.1)

A variable and its `_FILE` together are refused, an empty file is refused, the contents are trimmed, no error prints a value (`--check-config` says "set, N characters" or "from the file …"), and the entrypoints never print the environment. **X3** was the missing limit. Docker's `inspect` shows plain variables, which is why the guide offers `_FILE` for every secret.

## 4. The release pipeline and the images (V0.4.35–V0.4.42, V0.4.56–V0.4.60)

| Concern | What was read | Result |
|---|---|---|
| Triggers | Tags and a manual run; never a pull request or a push to a branch; a manual run pushes only if started on a tag | Sound |
| Permissions | Read by default; write only in the jobs that push, name or attach, and only on a tag; `id-token`/`attestations` write in the join job alone | Sound |
| Untrusted input | Branch, tag and actor names reach shell steps through environment variables, never through `${{ }}` in a `run`; the matrix values are the file's own | Sound |
| Actions and images | Every action is a 40-character commit and every image a digest (`pins.mjs`, tested, run by CI); Dependabot proposes the next, grouped | Sound |
| Provenance | A signed build attestation and a signed SBOM per joined image digest, verifiable with `gh attestation verify`; the tag must equal the version before anything is built | Sound |
| Secrets in a workflow | The Windows certificate, the Apple credentials and the update key are read by the steps that use them; the certificate file is removed at once; nothing is echoed. **X1** was the update key's reach; **X4** what is left | Fixed / accepted |
| Who can publish | See X7 | Accepted |
| The quick start | See X8 | Accepted |

## 5. The desktop app

| Surface | What was read | Result |
|---|---|---|
| The window | It may show only the bundled client (`is_app_url`); its content security policy allows only itself; the HTTP plugin may call only the chosen server, or only the relay, never both | Sound |
| The shell's commands | They choose and remember a server, keep the session in the credential store, open `http(s)` pages, save a file the person chose in a dialog, show a notification and a badge, take a link and an update. None reads or writes a path the page names; the save dialog is the person's | Sound |
| The configuration file | A closed set of fields, unknown ones refused, every value checked by the typed-address rules, at most 16 KB; a bad file is shown and obeyed in nothing, and the app connects to nothing. The certificate path is read only as a PEM of at most 256 KB, and a failure names the path, not the contents | Sound; see X6 |
| The certificate relay | Loopback only, a random secret per run in a header a foreign page cannot send, one origin, no redirect, only the bundle's roots, a 403 without the secret, a 400 for another target | Sound; the secret is compared with `==` on the loopback (nothing to time), accepted |
| `repomemo://` links | Only the host `open`, only `path` and `server`, only the client's own paths (no `//`, `..`, encoded slash, control character, 2048 characters), only the connected server; a refusal is a sentence | Sound |
| Notices | The page's text is cleaned to one line, 100/300 characters, before it is shown | Sound |
| Updates | The administrator's feed or the person's choice; the signature is checked against the feed's key and must be for the version announced; no "dangerous" option is on; a plain-http feed is allowed on the person's own network only, and the signature still decides. `zip` 4.6.1's three advisories are ignored with the maintainer's approval and a revisit date | Sound |
| The installer | Per person, no administrator rights, nothing outside the person's folders; the uninstaller runs the installed program with one flag that opens no window and deletes the person's own session; the scheme is registered for the person | Sound |
| Signing | A valid, timestamped signature is required of every file when a certificate exists; an unsigned build says so | Sound |

## 6. What was fixed, and how it is kept fixed

- **X1:** `container:check` fails if the update signing key is set at the job level of the Windows workflow.
- **X2:** the web image's entrypoint check, and the CI step that starts it with bad names; the refusal is a row of the troubleshooting page, which `troubleshooting:check` requires.
- **X3:** a Rust test of both sides of the limit; the refusal is a row of the troubleshooting page.

## 7. Recommended, not enforced by the repository

- Protect the `v*` tags and the release environment (X7).
- Keep the update signing key and the Windows and Apple credentials as repository secrets only, with a different password for each, and rotate them on a schedule.
- Pin images by digest where you deploy (`VERIFYING_IMAGES.md`) and run the image scan on your own registry's copy.
