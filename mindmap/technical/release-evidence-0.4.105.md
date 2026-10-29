# V0.4.105 final candidate evidence

Prepared 2026-10-29. Baseline: `e46f6822c4f10de83d038ebdf098f33ea5990f88`
(V0.4.104). The V0.4.105 commit contains the version update, this evidence,
and the missing lock entry for the unchanged migration 0044. It is a local
candidate, not a published artifact. Pre-existing roadmap proposals are retained.

## Candidate checks

| Check | Result | Scope |
|---|---|---|
| Version, generated types, styles, container wiring, accessibility rules | Pass | Local source checks |
| Migration lock | Pass after repair | 0044 was already shipped but missing from the lock; its SQL is unchanged |
| Templates, desktop parity, troubleshooting | Pass | Static checks and their test suites |
| Packaging, release notes, pins, desktop packaging, update feed | Pass | Tests generate fixtures; no application build or published artifact |
| TypeScript | Pass | `npm run typecheck` |
| Web tests | Pass | 92 files / 496 tests; initial sandbox EPERM refused temporary-file renames before execution |
| Rust workspace | Pass: 622 tests, 4 existing ignores | `cargo test --workspace --exclude repomemo-desktop --locked`, incremental compilation disabled |
| Rust format | Pass | `cargo fmt --all --check` |
| Production npm audit | Pass | `npm audit --omit=dev`: zero reported vulnerabilities; network-enabled retry |
| Documentation | Pass | Source and links |

The Rust sandbox attempt hit nine fixture-write permission errors; the full retry outside the sandbox passed. The four pre-existing ignored tests were not changed. Desktop Rust tests, clippy and Rust advisory checks were not rerun in this evidence-only change.

## Distribution decision

No distribution is newly certified ready by this candidate. The intended
validation scope remains a single-node server/web Compose deployment and the
Windows desktop client. Previous local artifacts and results are historical:
see the [0.4.73 matrix](release-evidence-0.4.73.md). They do not prove this candidate.

| Required evidence | Result and reason |
|---|---|
| Exact-commit GitHub tests and distribution workflows | Pending; commits must not be pushed |
| Images, image scan, SBOM, signatures and registry digests | Pending; no local builds or publication in this run |
| Clean install, upgrade, rollback and restore on candidate artifacts | Pending maintainer validation |
| Live user and administrator documentation walkthroughs | Pending maintainer validation |
| Windows installation, browser and desktop workflows | Pending maintainer validation |
| Linux/macOS installation and platform signing | Outside the validated scope; platform evidence/credentials unavailable |

On 2026-10-29 the maintainer explicitly authorized implementation through
V0.5.19 while recording live checks as pending. That permits implementation
to overlap validation; it does not waive the release-readiness checks or turn
them into passes. The identity checkpoint and closing record must retain the
missing live evidence. No push, tag, publication or release sign-off is authorized.
