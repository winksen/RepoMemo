# Web test debt at V0.4.68

**V0.4.69 resolution (2026-10-27):** the 22 assertions and the runner blocker below were repaired. The full local run completed with 88 files and 476 tests passing. The validation handler now inspects controls without the recursive `:invalid` selector. Vitest supplies jsdom's absent layout APIs. Tests use fresh response bodies, await asynchronous navigation and query notifications, and assert the current command, import and fieldset behavior. No test was skipped or removed. CI has not run on this unpushed commit.

The V0.4.67 changelog reported 22 failing web assertions. On 2026-10-27, all 88 test files were run separately at V0.4.68 with `node .test-tmp/run.mjs` (three isolated Vitest processes, a 25-second limit per file). This reproduced **22 failed assertions in 14 files**, plus a separate hang in `fieldValidation.test.tsx`. The ordinary `npm run test:web` run did not finish here: the sandbox's default temporary directory first caused `EPERM` on 88 files; with a writable temporary directory it reached assertions but stalled on the field validation loop. These are observations on Windows, not a claim about CI or other systems.

Each assertion below can be reproduced with `npm run test:web -- src/test/<file>` from the repository root. In this sandbox set `TEMP` and `TMP` to a writable directory first. The disposition is a triage hypothesis for V0.4.69, not permission to skip or delete a test. “Product” means inspect the implementation and fix it if the observed behavior is wrong; “assertion” means first verify the intended behavior, then update the test. The owner is the code area that should resolve it.

| # | File and assertion | Observed cause | Disposition | Owner |
|---|---|---|---|---|
| 1 | `citationChange.test.tsx`: repository citation author/date | Expected combined `Runbook · runbook.md` text is absent | Assertion: verify current citation markup | Web citations |
| 2 | `evidenceView.test.ts`: folder, top level and ledger page | Test reuses one `Response`; its body is consumed after the first request | Test fixture | Web API tests |
| 3 | `mailSettingsPieces.test.tsx`: editable settings | `ResizeObserver` is missing in jsdom; dropdown crashes before “Mail server” appears | Environment setup | Web UI tests |
| 4 | `mailSettingsPieces.test.tsx`: read-only settings | Same missing `ResizeObserver` prevents the form from rendering | Environment setup | Web UI tests |
| 5 | `narrowScreens.test.tsx`: breakpoint and stylesheet order | CSS loader finds no `@media (max-width: 900px)` in the path it reads | Assertion: reconcile split stylesheet | Web styles tests |
| 6 | `narrowScreens.test.tsx`: tablet overflow | The same empty CSS slice cannot contain the expected overflow rule | Assertion: reconcile split stylesheet | Web styles tests |
| 7 | `repositoriesData.test.tsx`: reload list | After `load()`, hook still exposes one repository rather than the two returned by the second mock | Product or query timing: investigate | Web repositories |
| 8 | `repositoriesData.test.tsx`: sync completion | Hook never calls `onChanged` after the mocked running-to-done transition | Product or query timing: investigate | Web repositories |
| 9 | `reviewQueue.test.tsx`: snooze | Test uses reference equality for the returned review summary; an equal object is a different instance | Assertion: verify value semantics | Web review tests |
| 10 | `session.test.ts`: sign out | `clearSharedSession()` makes no mocked logout fetch call | Product or changed logout contract: investigate | Web session |
| 11 | `systemData.test.tsx`: manual reload | Hook still exposes `{ users: 1 }` after the second mocked answer `{ users: 2 }` | Product or query timing: investigate | Web System |
| 12 | `systemData.test.tsx`: late answer after unmount | Toast mock records one call after the page unmounts | Product: inspect cache/error handling | Web System |
| 13 | `templateAssistant.test.tsx`: template jobs and questions | jsdom has no `scrollIntoView`; assistant crashes | Environment setup | Web UI tests |
| 14 | `templateAssistant.test.tsx`: no-template jobs and questions | Same missing `scrollIntoView` | Environment setup | Web UI tests |
| 15 | `templateAssistant.test.tsx`: default palette commands | Actual command list has one more entry than the expected four | Assertion: verify current commands | Web assistant tests |
| 16 | `templateAssistant.test.tsx`: template palette commands | Actual ordered list has one more entry than the expected four | Assertion: verify current commands | Web assistant tests |
| 17 | `templatePicker.test.tsx`: busy picker | The selected radio is not disabled when the form is busy | Product: verify busy interaction | Web templates |
| 18 | `unsavedChanges.test.tsx`: clean navigation | `navigate('/dashboard')` leaves the test router at `/` | Product or router setup: investigate | Web navigation |
| 19 | `workspaceImport.test.tsx`: archive check | Mocked `inspectSharedWorkspaceArchive` receives a payload that differs from the expected one | Assertion or request contract: inspect fields | Web import |
| 20 | `workspaceSections.test.tsx`: default palette sections | Actual list begins with `New note`, which the expected list omits | Assertion: verify new command | Web navigation tests |
| 21 | `workspaceSections.test.tsx`: template palette sections | Same additional `New note` command precedes the expected tabs | Assertion: verify new command | Web navigation tests |
| 22 | `workspaceSettingsPieces.test.tsx`: provider form | jsdom has no `scrollIntoView`; dropdown crashes before “API key” appears | Environment setup | Web UI tests |

**Additional runner blocker:** `fieldValidation.test.tsx` does not finish in an isolated process within 25 seconds. Its invalid form path recursively raises `RangeError: Maximum call stack size exceeded` through jsdom constraint validation and error reporting. Reproduce with `npm run test:web -- src/test/fieldValidation.test.tsx` and a process timeout. Web forms owns the investigation. It must be fixed before an all-at-once suite can be treated as a reliable release gate.

V0.4.69 resolves these findings, runs the full suite in CI and locally, and records any changed classification with evidence. No test has been skipped or removed in this triage release.
