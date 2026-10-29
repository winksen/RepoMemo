# RepoMemo releases: 0.9 → 1.0 (proposed)

> **Theme:** Pilot proof and public release candidate · **Baseline:** V0.4.104 · **Related:** [Roadmap](ROADMAP.md#proposed-path-to-10-team-readiness) · [Functional readiness report](analysis/2026-10-29-team-readiness.md)

**Status:** proposed on 2026-10-29; no row below is shipped or a release commitment. One intended commit per row, named `Update V0.N.P`, with a short body describing the change. Split oversized rows before implementation and renumber only unshipped rows. Update the changelog, user/API guides, mindmaps, archive compatibility and focused checks with the implementation, not in this planning change. No product version is bumped by this plan.

**Entry:** Required 0.5–0.8 outcomes are implemented; conditional deferrals and supported integrations/platforms are explicit. This line freezes features and closes observed failures.

**Exit:** Named pilots and exact-candidate release evidence satisfy the non-waivable access, data integrity, accessibility and recoverability gates; the maintainer can approve a concrete 1.0 artifact set.

**Scope control:** local OCR (0.6.13), a second tracker (0.8.7) and a second chat adapter (0.8.10) require a pilot owner before implementation. Jira/Slack are the default first choices; pilot evidence may select Linear/Teams instead before work starts. Record a deferral and narrow the supported feature list when a conditional item is not justified. Access boundaries, trustworthy evidence, recovery and publication isolation are mandatory. Full details and pilot thresholds are in the [readiness report](analysis/2026-10-29-team-readiness.md).

**Type:** ✨ Feature · 🔧 Enhancement · 🐞 Fix · 🔒 Security · 📘 Update (docs, process, evidence, release).
**Status:** ✅ shipped (with date) · 🔄 in progress · 🔜 planned. Every row starts planned; a written plan is not an acceptance-test pass.

## Every release

| Release | Status | Track | Type | Area | What changes | Done when |
|---|---|---|---|---|---|---|
| **V0.9.0** | 🔜 | AM. Public readiness | 📘 | Pilot protocol | Freeze the public promise and recruit three different team pilots plus one community maintainer group | Participants, workloads, consent, two-week observation plan and the report thresholds are recorded; unavailable participants are marked pending rather than simulated |
| **V0.9.1** | 🔜 | AM. Public readiness | ✨ | Product evidence | Add administrator-controlled local aggregate activation, search-feedback and review metrics | No query body or per-person productivity ranking is collected by default; retention and opt-out are explicit and counts can be reconciled with pilot tasks |
| **V0.9.2** | 🔜 | AM. Public readiness | 📘 | Public repository | Add root license, contribution, security-reporting and community conduct documents with issue templates | License text matches declared licensing, contribution steps are reproducible and a real private reporting contact is supplied by the maintainer |
| **V0.9.3** | 🔜 | AM. Public readiness | 📘 | Adoption guides | Publish task-based guides and realistic sample data for the four pilot audiences | A new reader can find setup, first value, migration, guest access and troubleshooting from the README; no invented customer or performance claims appear |
| **V0.9.4** | 🔜 | AM. Public readiness | 📘 | Observed usability | Record pilots performing activation, retrieval, review, project handoff and admin offboarding | Success, time, assistance and failures are recorded against the proposed thresholds; every blocking finding has an owner and a required fix before the candidate |
| **V0.9.5** | 🔜 | AM. Public readiness | 🔧 | Inclusive use | Close observed keyboard, screen-reader, zoom and narrow-screen blockers in the core journeys | The affected journeys are rerun with recorded devices/themes; fixes are split into additional numbered commits if they are independent |
| **V0.9.6** | 🔜 | AM. Public readiness | 📘 | Supported capacity | Measure concurrent search, indexing, projects and sync on representative pilot data | The support envelope records hardware, dataset, concurrency, p95, disk and saturation behavior; unmeasured targets are labelled and overload is recoverable |
| **V0.9.7** | 🔜 | AM. Public readiness | 🔧 | Upgrade and recovery | Exercise supported-version upgrades, backup/restore and complete archive round trips | The exact candidate preserves identity, citations and new entities; recovery point/time and supported rollback procedure are demonstrated on a clean host |
| **V0.9.8** | 🔜 | AM. Public readiness | 🔒 | Release assurance | Review access, integrations, public sharing, AI boundaries and dependency/image findings | No open high/medium finding, unauthorized disclosure or data-loss defect remains; negative integration and publication cases have regression evidence |
| **V0.9.9** | 🔜 | AM. Public readiness | 📘 | Distribution rehearsal | Validate downloadable candidate images/installers and signatures for each claimed platform | A clean machine installs, upgrades and removes the real artifacts; digests, SBOM, provenance, signing and update behavior are recorded where claimed |
| **V0.9.10** | 🔜 | AM. Public readiness | 📘 | Support contract | Publish supported versions/platforms, upgrade policy, backup expectations and support/incident routes | A new operator knows what is supported and how to recover or report a problem; API/archive compatibility commitments and deprecation policy are explicit |
| **V0.9.11** | 🔜 | AM. Public readiness | 📘 | Pilot acceptance | Review week-two usage and the full readiness matrix, then close observed product blockers | All required report gates pass with linked evidence; unresolved failures produce focused pre-1.0 fix rows rather than an assumed waiver |
| **V0.9.12** | 🔜 | AM. Public readiness | 📘 | Release candidate | Freeze the final candidate and repeat affected checks on its exact commit | Documentation, release notes, migrations, CI and manual records name the same candidate; a subsequent code change invalidates affected sign-offs |

## Verification and handoff

Before opening the next line, record the exit journey, changed-data upgrade and export/import checks, role-denial cases and supported-client behavior. Tests must demonstrate the user outcome, including failure and recovery. Follow the repository's execution rules; unrun checks remain explicitly unrun and the maintainer supplies live evidence.

Any new storage field, enum or entity must include its archive compatibility and migration handling in its own implementation; the line's portability row verifies the complete journey, rather than postponing data exit until the end.

When a row ships, update this index and the matching roadmap row together; put the actual behavior and limitations in the changelog. The version range is an ordered plan, not a calendar forecast.
