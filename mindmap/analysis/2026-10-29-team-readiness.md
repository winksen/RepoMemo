# RepoMemo: functional readiness for teams and the public 1.0

> **Review date:** 2026-10-29 · **Baseline:** V0.4.104, commit `e46f682` · **Method:** repository and workflow inspection, plus primary-source competitor research. This is a product assessment, not a live usability test, security certification or measured capacity claim. No application, build or test suite was started for this review.
> **Delivery plan:** [Roadmap: proposed path to 1.0](../ROADMAP.md#proposed-path-to-10-team-readiness) · [0.5](../RELEASES_0.5.md) · [0.6](../RELEASES_0.6.md) · [0.7](../RELEASES_0.7.md) · [0.8](../RELEASES_0.8.md) · [0.9](../RELEASES_0.9.md).

## Verdict and product promise

RepoMemo has a substantial foundation for a **self-hosted technical team knowledge and coordination product**. It can collect code and business evidence, retrieve it, preserve cited conclusions, identify stale knowledge, assign follow-up work and recover a workspace. Those capabilities address a coherent problem: teams repeatedly lose the context behind decisions and cannot tell whether an answer is still trustworthy.

It is not yet a complete project management suite. The task model is an action board with four fixed states, an assignee, priority, due date, checklist and one artifact link. Projects, milestones, dependencies, task discussions and project reporting are missing from the inspected model. Calling it a replacement for a mature project tracker would set expectations it cannot currently meet.

**Recommended 1.0 promise:** a team can bring its technical knowledge into a controlled workspace, find and verify an answer, turn that answer into owned project work, keep the decision and delivery evidence together, and leave with its data. Small teams can manage straightforward projects inside RepoMemo; organizations with an established tracker can keep execution there and use RepoMemo for evidence and decisions. This recommendation extends the current [product purpose](../../PRODUCT.md); it does not silently redefine it or claim that the proposed functionality exists.

The biggest release risk is the distance between implemented code and a proven adoption journey. The [distribution evidence matrix](../technical/release-evidence-0.4.73.md) has partial local passes, but no released 0.4 artifacts, incomplete desktop installation evidence, and pending exact-commit CI and signing evidence. Complete V0.4.105 before declaring a supported distribution ready.

## Fit by audience

Sizes below are planning personas, not validated server limits. Readiness is qualitative: **pilot fit** means useful enough to evaluate with a team, not recommended for an unattended production rollout.

| Audience | Useful today | Main adoption obstacles | Recommended fit and release path |
|---|---|---|---|
| Small technical teams, roughly 3–15 people | Shared evidence, git sync, notes, search without AI, memory cards and simple actions | Setup still requires an operator; first value is not guided through the whole journey; no central personal review/action queue; editing and deletion need stronger recovery | Strongest initial pilot audience. 0.5 access basics and 0.6 daily usability first; 0.7 lets them run lightweight projects |
| Startups, roughly 10–50 people | Decision history, onboarding context, code/issue evidence, optional local AI | Work is already in Linear/GitHub and chat; importing and maintaining the same information twice creates friction; project ownership and milestones are missing | Strong potential as the knowledge layer beside a tracker. Prioritize 0.7 traceability and 0.8 connector reliability over portfolio features |
| Companies with several departments or client teams | Self-hosting, organization/workspace roles, audit export, backups and controlled AI configuration | Broad organization inheritance, no shipped SSO, manual access reviews, no source-permission synchronization, incomplete operating evidence | Conditional fit for a bounded departmental deployment after 0.5 and pilot proof. Do not claim enterprise-wide readiness or source ACL equivalence |
| Agencies and external collaborators | Direct workspace membership already limits a guest to invited workspaces | Guest expiry, access explanations, offboarding handover and client/project separation need a complete workflow | Good bounded use case after 0.5; each client stays in a deliberately restricted workspace |
| Open-source and other technical communities | Public repository evidence, decisions, runbooks, portability and self-hosting | Anonymous readers cannot browse a curated knowledge site; contributor onboarding and public-project support policies are incomplete | Maintainer-only pilot today. 0.8 adds explicitly published snapshots; 0.9 makes public contribution and support practical |
| Nontechnical, regulated or large portfolio organizations | Document previews and task basics help | Advanced planning, procurement/security requirements, source permissions, support commitments and scale are not established | Do not target as the default 1.0 audience. Validate specific requirements before promising compliance, high availability or a full suite |

## What already exists: avoid rebuilding it

| Capability | Evidence inspected | Assessment |
|---|---|---|
| Evidence and repository intake | [Functional reference](../FUNCTIONAL_MINDMAP.md#5-evidence-artifacts), [repository workflow](../FUNCTIONAL_MINDMAP.md#5b-repositories) | Notes, files, Office/PDF/email previews, remote git, incremental sync and provenance are substantial strengths |
| Retrieval and durable conclusions | [Memory storage](../../crates/storage/src/memory.rs), [memory editor](../../apps/desktop/src/web/MemoryCardDetail.tsx), [search guide](../../docs/SEARCH_AND_ASK.md) | Citations can already be edited, cards confirmed and exported, and stale cards reviewed. Extend with revision history and review accountability |
| Access and external collaborators | [Organization storage](../../crates/storage/src/organizations.rs), [role model](../FUNCTIONAL_MINDMAP.md#1-who-uses-it-roles-and-permissions) | Workspace guests, membership-origin tracking, revocation and ownership transfer exist. Add restricted-workspace policy and access lifecycle, not another guest or transfer implementation |
| Collaboration | [Task schema](../../crates/storage/migrations/0006_collaboration.sql), [checklist schema](../../crates/storage/migrations/0009_workflow_depth.sql), [task UI](../../apps/desktop/src/web/workspace/TasksSection.tsx) | Assignees, priorities, due dates, checklists and evidence discussions exist. Task-specific discussions and project structure are different missing capabilities |
| Notifications and administration | [Collaboration guide](../../docs/COLLABORATION.md), [security administration](../../docs/SECURITY_ADMIN.md) | Mail, mentions, preferences, digests and audit export exist. Improve routing and coverage; do not plan them as new foundations |
| Data exit and operations | [Archive model](../../crates/domain/src/archive.rs), [runbook](../../docs/RUNBOOK.md), [performance record](../technical/performance.md) | Export/import, backups, restore and operational tooling exist. Every new entity needs round-trip support; larger current workloads still need recorded measurements |

Documentation has drift in places: the main memory section says citations cannot be changed, while the editor and storage support it; the main cross-workspace-search section describes fewer filters than the later known-gaps section. The roadmap's top-level continuation status also lags its release rows. The new plan includes reconciliation and treats implementation as the stronger evidence. These discrepancies are not missing product features.

## Team needs and competitor lessons

Research accessed 2026-10-29. Product documentation establishes available patterns, not customer satisfaction or proof that RepoMemo users need every feature. Vendor-sponsored research is directional. There were no customer interviews, product analytics or observed pilot sessions available for this assessment.

| Evidence | What it supports | Implication for RepoMemo |
|---|---|---|
| Atlassian's [2025 developer-experience study](https://www.atlassian.com/teams/software-development/state-of-developer-experience-2025) describes a survey of 3,500 developers and managers and continuing organizational friction alongside AI adoption | Coding assistance alone does not settle the coordination problem | Test time to find a reliable answer and complete a handoff; do not use AI feature count as the success measure |
| [Linear's conceptual model](https://linear.app/docs/conceptual-model) distinguishes issues, projects, initiatives, cycles and views | Execution requires ownership and structure at more than one level | Add a bounded project/task/milestone model, with links to decisions; defer initiatives and sprint machinery until demand is demonstrated |
| [OpenProject work packages](https://www.openproject.org/docs/user-guide/work-packages/) and [relations](https://www.openproject.org/docs/user-guide/work-packages/work-package-relations-hierarchies/) support typed work, views, import and dependency relationships | A board is only one view of project work | Make a task addressable and discussable, add dependencies, list views and migration; a complex scheduling engine is unnecessary for the proposed 1.0 |
| [Outline](https://www.getoutline.com/) presents collaborative editing, search, chat integration and public sharing; its [roles documentation](https://docs.getoutline.com/s/guide/doc/users-roles-cwCxXP8R3V) explicitly limits guests by edition | Knowledge must be easy to contribute, reach and share; edition differences matter | Improve conflict recovery and drafts first, then chat links and curated publication. Do not imply every competitor feature is available in a free self-hosted edition |
| [Guru's verification workflow](https://help.getguru.com/docs/verifying-and-unverifying-cards) makes verification responsibility explicit | A trust badge needs an accountable reviewer | Add owners, review dates, reminders and revision-specific approval; keep human review meaningful and optional AI separate |
| [Notion's connector overview](https://www.notion.com/help/notion-ai-connectors) covers knowledge, chat and project sources with cited retrieval; [connection management](https://www.notion.com/help/add-and-manage-connections-with-the-api) exposes permissions and administrator controls | Integration is also an access and administration problem | Show what will be copied, to whom, freshness, exclusions and revocation behavior. A connector count alone is not a readiness measure |

The opportunity is the complete chain **source → cited decision → accountable action → review after change**. RepoMemo already has much of that chain. Strengthening its connections is a more defensible direction than reproducing every competitor's database, chat, agile planning or AI-agent feature.

## Gaps ranked by functional impact

**P0** blocks the proposed public promise or safe adoption. **P1** makes that promise useful in daily work. **P2** is a demand-dependent expansion. These are product priorities, not vulnerability severities.

| ID | Priority / gap | User consequence and minimum response | Planned delivery |
|---|---|---|---|
| G01 | P0 · Distribution proof | A new operator cannot rely on an unpublished or unrehearsed install path. Verify released images, upgrade, restore and supported clients against the same candidate | V0.4.105; 0.9.7–0.9.12; 1.0.0 |
| G02 | P0 · Membership boundaries and identity | Joining an organization currently populates workspace memberships. Add explicit restricted workspaces, clear effective access, OIDC, bounded session lifetime and local-account MFA | 0.5.1–0.5.7, 0.5.11–0.5.13, 0.5.16–0.5.18 |
| G03 | P0 · Access lifecycle | Existing deactivation/transfer mechanisms need one handover and review path; guests need expiry and integrations need constrained credentials | 0.5.14–0.5.15 |
| G04 | P1 · Activation and personal work | Dashboard counts and many sections do not answer “what should I do first/today?”. Connect invite → evidence → search → verified memory → assigned action without requiring AI | 0.6.1–0.6.2 |
| G05 | P0 · Safe collaborative authoring | Live refresh and unsaved-change warnings are not edit conflict control or durable revision history. Preserve drafts, detect stale writes and recover accidental deletion | 0.6.3–0.6.5, 0.6.11; 0.7.6 |
| G06 | P0 · Knowledge accountability | A changed source can flag a card, but useful governance also needs an owner, review date, explicit approval and an explanation of the affected evidence | 0.6.6–0.6.10 |
| G07 | P1 · Intake and organization | Image storage currently requires vision AI; scanned evidence, moving folders and organization-maintained templates need clearer support | 0.6.12–0.6.15; local OCR is conditional |
| G08 | P1 · Search quality and recovery | Existing filters/citations help, but unanswered questions and wrong answers need a feedback loop and a known evaluation set | 0.6.16; 0.8.14; 0.9.1 |
| G09 | P1 · Project execution | Teams cannot group tasks into projects or milestones, discuss tasks directly, express dependencies or communicate a grounded project update | 0.7.1–0.7.17 |
| G10 | P0 · Integration trust | Current connectors copy evidence into workspace access. Source permissions, deletion, failures and stale copies must have explicit semantics before more business sources are added | 0.8.0–0.8.3; all new adapters depend on these |
| G11 | P1 · Adoption in existing tools | Manual uploads and duplicate tracking deter teams. Provide a first tracker/chat path and migration formats, with one-way ownership rules; add second adapters only with pilot demand | 0.7.18; 0.8.4–0.8.12; second adapters are conditional |
| G12 | P1 · Community access | A public repository is not a public knowledge site. Publish only reviewed snapshots, preview the public audience and revoke them without exposing private citations | 0.8.15–0.8.17 |
| G13 | P0 · Accessibility and understandable state | Automated checks exist; live keyboard, assistive-technology, zoom and mobile tasks remain unproven. Preserve drafts and context through failure and navigation | 0.6.17; 0.9.4–0.9.5 |
| G14 | P0 · Public-project readiness | Root contribution/security/license documents were not found in the inspected tracked file inventory; MIT is declared in Cargo metadata. Public users also need support and upgrade expectations | 0.9.2–0.9.3, 0.9.10 |
| G15 | P0 · Evidence of value and capacity | Existing targets and historical measurements do not prove the new workflows, concurrent use or customer outcomes | 0.9.0–0.9.1, 0.9.4, 0.9.6–0.9.12 |

## UX direction and acceptance journeys

Use the existing restrained workbench design. This review used the Impeccable planning/Operate guidance for task clarity and interaction states; it did not run the formal visual critique workflow or produce a visual quality score. Source evidence includes [navigation constants](../../apps/desktop/src/web/constants.ts), [dashboard](../../apps/desktop/src/web/Dashboard.tsx), [task board](../../apps/desktop/src/web/workspace/TasksSection.tsx) and [design rules](../../DESIGN.md).

1. **A new teammate:** invite opens the intended workspace; explain their role; offer a short, dismissible checklist with real starter evidence; guide them to a source and a memory card. A server without any AI provider must complete this journey.
2. **A returning teammate:** open My work to see assigned tasks, due reviews and mentions; retain workspace, filters and scroll position when inspecting evidence; offer the next action in context. Existing notifications remain the delivery system.
3. **A reviewer:** see what changed since verification, inspect the cited revision, amend or supersede the conclusion, record who approved which revision and when it is due again. Avoid implying that clicking “confirmed” proves an AI answer correct.
4. **A project lead:** create a project brief with a goal and owner, attach decisions and evidence, plan milestones, resolve blockers and post an update. Progress must name its basis, not be inferred from an unexplained percentage.
5. **An administrator:** preview effective access before inviting, restricting or connecting a source; identify who owns orphaned work; revoke access and verify it across search, exports, previews, events and tokens.
6. **A community reader:** navigate published knowledge without an account, follow only public-safe references and see when it was reviewed. Publishing a card must never automatically publish its private source material.

For each journey, cover first use, empty content, partial permission, loading, failure/retry, reconnect, long names, large lists and keyboard use. A narrow screen should support reading, commenting and action triage; a native mobile application and full offline editing are not 1.0 requirements. UI localization infrastructure and locale-correct dates matter; supported translated languages should follow actual pilot demand and reviewer availability.

## Integration contract and scope boundaries

**Workspace access is not source ACL mirroring.** For 1.0, import only a source partition whose owner deliberately approves sharing its entire selected contents with the destination workspace. Preflight must explain this, list the audience and refuse sources that require unsupported per-person restrictions. A later source change that makes that assertion invalid must quarantine affected evidence from search, Ask, previews, exports and new publication until an administrator resolves it. Refresh failure must be distinguished from confirmed deletion; retained private bytes are not permission to keep serving them. Full per-document/per-person ACL synchronization is a separate project.

Tracker imports are **read-only mirrors**, with stable remote IDs and links. RepoMemo tasks remain locally owned and may reference mirrored issues; changing one does not silently change the other. Chat delivery defaults to authenticated links without sensitive titles or excerpts; the channel is not assumed to have the workspace's membership. Webhook delivery has bounded retries, event IDs and an explicit destination approval. Imports need dry runs, resumability, exclusions and a useful final report.

Public sharing uses a reviewed snapshot and separate public search scope. Changes to private evidence do not automatically republish; withdrawal or a source permission change invalidates affected publication and caches. Existing workspace export remains for private team portability, not a publication mechanism.

## Sequence, scope control and proof

The roadmap retains the existing OIDC version reservations, then extends 0.5 with access lifecycle. The historical SSO plan stays unchanged as a record; the new tables refine unsafe email-only linking assumptions, reuse existing audit export, and treat 0.5.10 as an identity checkpoint rather than the end of the expanded line.

| Line | Outcome | Why in this order |
|---|---|---|
| 0.5 · Team access | Join, restrict, review and end access predictably | More integrations and public readers are premature before the audience is explicit |
| 0.6 · Trusted daily knowledge | Reach first value, preserve edits, own review and recover mistakes | Establishes the core reason to adopt RepoMemo |
| 0.7 · Evidence-backed projects | Plan and discuss lightweight project work without losing context | Adds the missing functional depth for teams using RepoMemo as their coordinator |
| 0.8 · Connected teams and communities | Bring existing work in and publish deliberate knowledge out | Builds on permissions, revision history and project links |
| 0.9 · Proven public candidate | Observe pilots, close adoption failures, prove operations and prepare support | Makes 1.0 a verified product promise rather than another feature milestone |

Each row is an intended `Update V0.N.P` commit with a small vertical outcome and acceptance criterion. These are ordered scopes, not delivery dates or effort estimates. A schema/API/UI change may form one vertical commit; if it becomes independently reviewable pieces, split and renumber unshipped rows. Do not make hundred-item implementation batches. At every minor checkpoint, use findings to reduce or refine later scope.

**Conditional before 1.0:** local OCR, Linear in addition to Jira, and Teams in addition to Slack. Each needs a pilot owner and representative fixtures before its row starts. If demand or safe access cannot be established, record it as deferred and remove the corresponding promise. Jira is the default first new tracker; pilot evidence may replace it with Linear before implementation. Markdown and CSV migration provide a lower-cost entry route meanwhile. The required integration contract cannot be deferred merely to ship an adapter.

**After 1.0 unless validated demand changes the product brief:** SAML/SCIM and multiple identity providers, fine-grained source ACL mirroring, live Drive/SharePoint/Confluence ingestion, full bidirectional tracker sync, real-time collaborative cursors, configurable databases, portfolio planning, Gantt auto-scheduling, time/budget/resource accounting, a broad automation builder, write-capable AI agents, MCP/IDE extensions, native mobile/offline editing, a hosted paid service, and multi-node storage. No enterprise or compliance claims follow from shipping OIDC. SQLite should be replaced only after measured workload or deployment requirements justify it.

## Proposed pilot and 1.0 gates

Recruit at least three teams with different workflows (small technical team, startup with an existing tracker, company/agency with restricted workspaces) and one community maintainer group. Run for at least two working weeks after onboarding. These participants are proposed, not already recruited. Record environment, dataset size, version, observed task success, support needed and decisions to fix or defer.

| Gate | Proposed evidence / threshold |
|---|---|
| First value | On a provisioned server, at least 80% of pilot participants complete the invite → add evidence → find source → save memory → assign action journey within 15 minutes without facilitator intervention; record failures, not just averages |
| Retrieval usefulness | A versioned set of at least 30 real pilot questions, including absent and conflicting evidence; at least 80% find an acceptable source in the first five results. Human review checks answer support and abstention; unauthorized-source leakage is always a release blocker |
| Continued usefulness | Each pilot team completes a knowledge review and an evidence-linked handoff in week two, and names a recurring use case it intends to keep. These are product-learning gates, not manufactured retention statistics |
| Access and recovery | Verified join/role-change/expiry/leaver cases, concurrent-edit recovery, deleted-item restore, and export/import round trips for every added entity; no unresolved unauthorized-access or data-loss defect |
| Usability | Core journeys completed with keyboard, a screen reader, 200% zoom, narrow viewport and both themes; record device/browser combinations and remediate blocking findings |
| Operations | Published support envelope with hardware, source counts, active users, query/job workload, measured p95, disk growth, restore time and recovery point. Preserve existing latency targets where applicable; a target is not a measurement |
| Public release | Exact-candidate checks, upgrade from supported prior data, released-artifact install and restore, security review with no open high/medium findings, an honest supported-platform list, license and contribution/security/support documents |

The maintainer signs off on this evidence before 1.0.0. If a mandatory gate fails, cut another focused pre-1.0 fix and repeat the affected journey. A missing optional distribution can be labelled unsupported; a broken access boundary or unreliable recovery cannot be relabelled into readiness.
