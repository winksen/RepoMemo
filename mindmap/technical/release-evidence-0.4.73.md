# V0.4.73 release evidence matrix

Prepared 2026-10-27 for the local 0.4.x continuation. The versions in this matrix are **commits, not published releases**: the user requested local commits and no push. A local pass below does not stand in for a GitHub Actions run on the committed SHA.

## Checks and install evidence

| Area | Evidence | Local result | Exact-commit CI result |
|---|---|---|---|
| Web tests | [V0.4.69 triage and release](../RELEASES_0.4.md), repeated for V0.4.70 | 88 files, 476 tests passed | No run on these unpushed commits |
| Rust workspace gate | [V0.4.70 changelog](../../CHANGELOG.md#v0470-2026-10-27), [`release-tests.yml`](../../.github/workflows/release-tests.yml) | `cargo test --workspace --exclude repomemo-desktop --locked` passed locally with incremental compilation disabled | No run on commit `3eceafd` |
| Container wiring | [`check-container.mjs`](../../scripts/check-container.mjs), V0.4.71 [install record](release-validation-0.4.71.md) | Version, docs, pins and container wiring passed; C1–C3 and C5 passed on a local build | Distribution workflow unrun |
| Desktop tests and package | V0.4.72 [D1–D10 record](release-validation-0.4.72.md) | 38 Rust tests, 9 package tests, 6 parity tests passed; unsigned Windows installer and handout built | Windows, Linux and macOS workflow runs unavailable |
| Image scan | [`ci.yml` image-scan job](../../.github/workflows/ci.yml), [scan policy](operations.md#in-containers-since-v0245) | The four-image Trivy job is configured; no scan report was produced by this local validation | No `image-scan-reports` artifact for these unpushed commits |
| Security review | [Deployment security review](../security/2026-10-27-deployment-security-review.md) | Static review at V0.4.65 recorded 0 open high/medium findings; it did not run a live system | No new review on 0.4.73 |

## Distribution status

| Path | Built | Tested | Released | Open evidence |
|---|---|---|---|---|
| Compose package | Local 0.4.71 archive | Checksum and tamper refusal passed | No | A release download and tag still need verification |
| Server and web containers | Local 0.4.71 images | Healthy startup, 25/25 smoke checks and second-stack restore passed | No | Registry digests, signed attestations, scan report, released upgrade and TLS edge remain open |
| Converter and edge images | No current local validation build | No | No | Preview and monitoring profiles, image scan and published digest remain open |
| Windows desktop | Local unsigned 0.4.72 executable and installer | Rust/package tests and handout checks passed; installation and live flows untested | No | Installation, SmartScreen, private-CA relay, signed feed and removal need evidence |
| Linux and macOS desktop | No current artifact | No | No | Each platform needs its own build and install run; Apple signing/notarization needs credentials |
| Direct server archives | No current artifact | No | No | Linux and Windows direct install and upgrade need artifacts and hosts |

## Risks, controls and sign-off

| Item | Status | Evidence needed to close |
|---|---|---|
| X7 tag ruleset and release environment approval | **Unverified operator control** from the [deployment review](../security/2026-10-27-deployment-security-review.md) | Maintainer records the active `v*` tag ruleset and required release-environment reviewer |
| Published image and archive provenance | **Blocked by no push/tag** | Run [Release images](../../.github/workflows/release-images.yml) and archive workflows on the same protected tag; record digests, attestations, SBOM and checksums |
| Image vulnerability scan | **Unrun for this candidate** | Green [`image-scan` job](../../.github/workflows/ci.yml) and its four reports on the exact commit |
| Clean install and upgrade | **Partial local pass** | Complete the open C4/C6–C9 and D1–D10 entries in the [clean-install sheet](clean-install.md#6-the-04-run-containers-and-the-desktop-app-v0466) |
| Update and platform signing | **Credentials absent or unverified** | Sign and test an update feed, Windows certificate and Apple notarization with real keys |
| Maintainer release sign-off | **Not given** | Review exact-commit CI, install records, residual risks and distribution artifacts before a tag or 0.5.x |

**Decision at V0.4.73:** the local code and selected container paths have useful evidence; no path is marked released. The matrix is a review checklist, not a sign-off. V0.4.105 repeats affected tests and installation paths on its final candidate.
