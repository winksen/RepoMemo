# V0.4.71 container validation record

Run on 2026-10-27 on Windows with Docker Desktop's Linux engine. This is a **local candidate**, built from the V0.4.70 base commit `3eceafdfb5535c1bfc46575318685ad1ffae2401` plus the uncommitted V0.4.71 changes. The image IDs below identify these local builds; they are not registry digests or signed release artifacts. No tag was created and nothing was pushed.

| Step | Date and platform | Candidate identity | Result | Evidence |
|---|---|---|---|---|
| C1 Fetch | 2026-10-27, Windows Git Bash | Compose archive SHA-256 `d6bc8876261f38d92170743e8bcafb92b0cc21cd4afc502d626c6a8941097990` | **Pass, local package** | [C1](#c1-local-package) |
| C2 Initialise and start | 2026-10-27, Docker Desktop Linux | Server image ID `sha256:2a5a3fe2bd6b639c994e1e9c811baff6cda9dec396b0051c6b19422c01a87520`; web image ID `sha256:9b31f353b93ef830ebf1eea09e332d88ab8857afb16a7b60678389f9a100bd42` | **Pass, local build, no edge** | [C2](#c2-local-stack) |
| C3 First-run setup and smoke | 2026-10-27, Docker Desktop Linux | Same local images | **Pass, 25/25 checks** | [C3](#c3-first-run-smoke) |
| C4 Verify images | 2026-10-27, Docker Desktop Linux | Registry digest unavailable | **Blocked:** no pushed tag, registry images or attestations | [Unrun steps](#unrun-release-steps) |
| C5 Backups and volumes | 2026-10-27, Docker Desktop Linux | Same local images and four copied volumes | **Pass, restored second stack** | [C5](#c5-restore) |
| C6 Upgrade and fall back | 2026-10-27, Docker Desktop Linux | Previous and current registry digests unavailable | **Blocked:** the upgrade script pulls released images | [Unrun steps](#unrun-release-steps) |
| C7 Previews and monitoring | 2026-10-27, Docker Desktop Linux | Converter and monitoring images not built | **Not run** | [Unrun steps](#unrun-release-steps) |
| C8 Direct install | 2026-10-27, Windows host | Archive IDs unavailable | **Not run:** Linux and Windows release archives absent | [Unrun steps](#unrun-release-steps) |
| C9 Release page | 2026-10-27, local checkout | Tag unavailable | **Blocked:** no tag or published release page | [Unrun steps](#unrun-release-steps) |

## C1 local package

`deploy/package-compose.sh 0.4.71` made the 16-file archive. `deploy/quickstart.sh --version 0.4.71 --base-url file:///C:/…` reported `checksum ok`, unpacked the bundle and set `REPOMEMO_VERSION=0.4.71`. A one-byte modified archive was rejected with `nothing was unpacked`; the destination directory did not exist afterward. The local `file://` mirror exercised the same checksum and unpack path, without asserting that a remote release exists.

## C2 local stack

`deploy/init.sh --no-edge` generated secrets for an isolated `repomemo-validation-0471` Compose project. The locally built server and web containers both reached `healthy`. `GET http://127.0.0.1:8080/health` returned `status: ok`, `setup_required: true`, `version: 0.4.71`, image tag `0.4.71`, and base commit `3eceafdf…`. TLS and registry pull were outside this local route.

## C3 first-run smoke

The first run exposed a missing one-time setup code in the logs of a pinned environment. V0.4.71 now prints that code on startup. The smoke script also now reads the current `workspace.id` response shape. After rebuilding the server, the script created and completed the administrator setup, then reported **25 of 25 checks passed**: sign-in, security responses, workspace creation, upload, indexing, search, backup, export and import. The test account and content existed only in the validation volumes.

## C5 restore

`deploy/backup-volumes.sh --all --project repomemo-validation-0471` copied the backup, data, database and file volumes while the stack was stopped. `deploy/restore-volumes.sh` filled four empty volumes under `repomemo-validation-0471-restored`. The second stack became healthy, accepted the original account's sign-in, and listed the original `Smoke Workspace` with its `smoke-note.md` artifact. Both validation stacks were stopped after this check; their isolated volumes remain for review.

## Unrun release steps

C4 and C9 require a published tag, images, attestations and release page, which the requested no-push workflow does not create. C6 needs released previous and current images because `deploy/upgrade.sh` pulls them. C7 needs a converter and monitoring run; C8 needs release archives and a Linux machine in addition to Windows. These remain open and must not be interpreted as passed installation paths. The [clean-install sheet](clean-install.md#6-the-04-run-containers-and-the-desktop-app-v0466) remains the master checklist.
