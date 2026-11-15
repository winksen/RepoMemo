# V0.6.13 test checkpoint

Prepared 2026-11-17. Baseline: V0.4.105 (`84ec290`), the last release with a recorded full run
([evidence](release-evidence-0.4.105.md): 622 Rust tests with 4 existing ignores, 496 web tests in 92 files). This record
covers V0.4.105 to V0.6.13 (HEAD at the time of writing, before the V0.6.13 commit).

**Nothing below has been executed.** The project's rule is that the maintainer runs and tests the application, so the
author ran only static checks. Every result cell is pending. A check that is not run stays marked *not run*; no test was
skipped, edited or removed to make a result pass, and a failure the maintainer finds becomes its own numbered fix row.

## What was added since V0.4.105

Counted by test attribute in the source (`#[test]` and `#[tokio::test]`, and `it(`/`test(` in web and script tests):

| | V0.4.105 | Now | Added |
|---|---|---|---|
| Rust test functions | 665 | 703 | 38 |
| Web and script test cases | 533 | 553 | 20 |
| Web test files | 92 | 101 | 9 new files |

These are source counts, not run counts: the 622 recorded Rust tests exclude the desktop crate and the 4 ignores, so the
two figures are not directly comparable. The maintainer's run reports the number that matters.

### Rust tests added, with their result

| Area | Test | Result |
|---|---|---|
| Identity (0.5) | `identity_tokens_require_signature_issuer_audience_nonce_and_freshness`, `identity_urls_refuse_insecure_or_ambiguous_addresses`, `identity_discovery_rejects_mixup_and_cross_origin_endpoints` (server) | pending |
| Identity (0.5) | `group_sync_previews_keeps_direct_grants_and_never_touches_owners`, `provider_logout_finds_accounts_and_refuses_a_repeated_token`, `a_finished_handoff_needs_the_confirmation_code_and_three_wrong_codes_destroy_it`, `identity_linking_never_reassigns_email_or_subject`, `identity_handoffs_refuse_replay_wrong_proof_and_expiry`, `identity_secret_is_sealed_and_configuration_survives_reopen`, `the_identity_scope_exports_only_provider_and_sign_in_events` (storage) | pending |
| Two-step sign-in (0.5) | `totp_matches_the_rfc_6238_sha1_vectors`, `base32_round_trips_and_codes_are_accepted_once_within_a_step_either_side`, `mfa_secrets_are_sealed_steps_and_recovery_codes_work_once` | pending |
| Access (0.5) | `identity_completion_issues_the_existing_session_once`, `identity_settings_require_a_system_session_and_never_return_secrets`, `enforced_provider_sign_in_leaves_only_break_glass_password_access`, `provider_sessions_refresh_only_until_their_absolute_deadline`, `restricted_workspaces_never_enroll_organization_members_and_policy_changes_move_nobody`, `a_restricted_workspace_is_invisible_to_people_who_were_not_added_and_access_has_origins`, `guest_end_dates_are_set_by_administrators_and_validated`, `offboarding_hands_over_work_checks_everything_first_and_verifies_revocation`, `a_token_limited_to_workspaces_cannot_reach_others_and_ends_with_its_owners_access`, `a_password_alone_never_opens_a_session_of_an_account_with_two_step_sign_in`, `the_access_matrix_holds_for_every_role_guest_token_and_restricted_workspace` (server), `a_guest_loses_access_at_the_end_date_and_regains_it_when_renewed`, `access_added_after_0_5_leaves_every_existing_grant_as_it_was` (storage) | pending |
| Knowledge (0.6) | `the_knowledge_fixture_has_stale_and_missing_evidence_a_guest_and_no_ai_or_mail`, `the_checklist_follows_real_progress_and_hiding_it_is_personal`, `my_work_lists_what_waits_and_loses_a_workspace_the_person_left`, `a_card_keeps_attributed_revisions_that_compare_and_restore_without_rewriting_history`, `a_stale_card_save_is_refused_and_the_losing_editor_can_retry_from_the_newer_revision`, `reviewers_and_due_dates_separate_overdue_from_changed_evidence_and_surface_departures`, `a_due_review_is_reminded_once_per_window_and_skips_snoozed_viewers_and_departed_reviewers`, `a_reminder_names_the_item_and_the_day_it_was_due`, `an_approval_is_tied_to_a_revision_and_independent_review_forbids_self_approval`, `a_card_that_needs_review_explains_retired_and_missing_evidence_to_everyone_who_can_read_it` | pending |
| Knowledge (0.6.10–0.6.12) | `a_historical_citation_says_when_its_source_changed_or_is_gone_and_shows_the_kept_text`, `each_save_credits_its_own_writer_even_when_two_saves_overlap`, and the extended `a_local_workspace_archived_by_the_desktop_app_imports_with_its_memory_and_citations` | pending |

### Web test files added

`drafts.test.ts`, `identitySettings.test.tsx`, `offboardDialog.test.tsx`, `onboardingChecklist.test.tsx`,
`providerSignIn.test.tsx`, `tokenWorkspaces.test.tsx`, `twoStepPanel.test.tsx`, `workspaceAccessPolicy.test.tsx`,
`workspaceGuests.test.tsx`: result pending. `answerExport.test.ts` was extended in V0.6.10.
V0.6.20 added `memoryHistoryPanel`, `memoryApprovalPanel`, `reviewOwnershipPanel`, `memoryChangesPanel` and `myWorkPanel` (31 cases): result pending.

## Static checks run for this record

| Check | Result | Scope |
|---|---|---|
| Version, migration lock (59 files), generated types, documentation links | Pass | Local source checks |
| TypeScript | Pass | `npm run typecheck` |
| Clippy with warnings denied, storage, api and server crates, all targets | Pass | Compiles every test, runs none |
| Rust format | Pass | `cargo fmt --all` |
| Executing any Rust or web test | **Not run** | Owed by the maintainer |
| CI on a pushed commit | **Not run** | Nothing was pushed; CI has not seen 0.6 |

## What the maintainer runs and records

```bash
cargo test --workspace --exclude repomemo-desktop --locked
npm run test:web
npm run docs:check
```

Record the totals below. For each failing test, add a numbered fix row to the roadmap's 0.6 line and leave the test as
it is; do not skip or delete it. After pushing, record the CI run for the pushed commit as well.

| Run | Date | Commit | Result | Failures (fix row) |
|---|---|---|---|---|
| Rust workspace | pending | | pending | |
| Web tests | pending | | pending | |
| CI on the pushed commit | pending | | pending | |
