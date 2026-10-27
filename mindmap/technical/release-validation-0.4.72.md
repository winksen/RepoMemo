# V0.4.72 desktop validation record

Run on 2026-10-27 on Windows x64. The candidate was built from base commit `3ba2071` plus the uncommitted V0.4.72 version changes. The locally built files are under `target/desktop-validation-0.4.72/` and are **unsigned**. No artifact was published or pushed. A different RepoMemo process was already running from `target/release/repomemo-desktop.exe`, so the build used an isolated target directory and did not replace or close that app.

| Step | Date and platform | Result | Artifact identity or constraint | Evidence |
|---|---|---|---|---|
| D1 Windows installer | 2026-10-27, Windows x64 | **Built; install not run** | `RepoMemo_0.4.72_x64-setup.exe`, SHA-256 `2aca7af724d114c78582577b715683c494e6a2d2f8635d72f93916e6836a8d39`; unsigned | [Build](#windows-build-and-tests) |
| D2 A handout | 2026-10-27, Windows x64 | **Partial pass:** `--from` handout packaged and verified; build-with-preset variant not run | `repomemo-desktop.exe` SHA-256 `b82cfc41a25dbe5176553a55f96e71179687e7ea4c25a056b035ae364ebc9c78`; handout checksums all match | [Package](#handout-package) |
| D3 A bad file | 2026-10-27, Windows x64 | **Unit checks pass; live UI not run** | Configuration tests in desktop Rust suite | [Checks](#windows-build-and-tests) |
| D4 A private authority | 2026-10-27, Windows x64 | **Package checks pass; live relay not run** | PEM and key-refusal tests in `desktop:test`; no private-CA server | [Checks](#windows-build-and-tests) |
| D5 Everyday | 2026-10-27, Windows x64 | **Not run** | Existing desktop instance was left undisturbed; no fresh candidate launch | [Open paths](#open-desktop-paths) |
| D6 Notices and links | 2026-10-27, Windows x64 | **Not run** | No live candidate session or browser-to-app link run | [Open paths](#open-desktop-paths) |
| D7 Updates | 2026-10-27, Windows x64 | **Blocked** | No signing key or signed update feed | [Open paths](#open-desktop-paths) |
| D8 Removal | 2026-10-27, Windows x64 | **Not run** | No candidate install to remove; existing app left untouched | [Open paths](#open-desktop-paths) |
| D9 Linux | 2026-10-27, Windows x64 | **Unavailable on this host** | No Linux desktop artifact or Secret Service run | [Open paths](#open-desktop-paths) |
| D10 macOS | 2026-10-27, Windows x64 | **Unavailable on this host** | No macOS artifact, Gatekeeper or Keychain run | [Open paths](#open-desktop-paths) |

## Windows build and tests

`cargo test -p repomemo-desktop --locked` passed 38 tests. `desktop:check`, `desktop:test` (9 tests) and `desktop:parity:test` (6 tests) passed. `npx tauri build --bundles nsis` built the web client, Windows executable and installer in the isolated target directory. `Get-AuthenticodeSignature` returned `NotSigned` for both files. The build proves an artifact exists; it does not prove installation, SmartScreen behavior or a live session.

## Handout package

`node scripts/package-desktop.mjs --server http://127.0.0.1:8080 --name 'RepoMemo Validation' --from target/desktop-validation-0.4.72/ci-artifact --out target/desktop-validation-0.4.72/handout` produced a portable zip, copied installer, `repomemo-desktop.json`, and `SHA256SUMS`. All three sums matched. The zip holds `RepoMemo.exe`, the configuration and a README. The configuration is locked to the local test address; that server was not running during this package check. This handout is a validation artifact, not a team distribution.

## Open desktop paths

The installed Windows flows D1 and D3–D6/D8 still need a disposable user account or machine and a live candidate launch. D4 additionally needs a private-CA TLS server. D7 requires real update signing credentials and a feed. D9 and D10 need Linux and macOS machines. These statuses distinguish **built**, **package tested**, and **installed/tested**; no installation or signed distribution is claimed. The [clean-install sheet](clean-install.md#6-the-04-run-containers-and-the-desktop-app-v0466) remains the master checklist.
