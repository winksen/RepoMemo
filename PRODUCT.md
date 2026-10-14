# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

RepoMemo serves developers and technical teams working inside an active codebase. They use it while investigating implementation details, recalling why a decision was made, onboarding to unfamiliar systems, and recovering reliable context from code, documents, runbooks, incidents, and architecture records. The people who install and run a RepoMemo server (system and app administrators) are users too: they need its state, logs, settings and security to be plain and controllable.

## Product Purpose

RepoMemo is a self-hosted team memory workspace for technical knowledge. A team stores its evidence in shared workspaces (notes, uploaded files and folders, Office documents, PDFs, emails, images, and git repositories kept in step with their branch). RepoMemo indexes that evidence, makes it searchable by keyword and by meaning, and keeps what the team learns as memory cards linked to their evidence. Optional AI providers can answer questions, summarize and describe images, always citing the stored evidence, but the product remains useful without AI.

Success means a user can move from an uncertain technical question to trustworthy source context quickly, understand where the answer came from, and preserve useful knowledge for later, and that the team can trust what it preserved to still be true.

## Positioning

RepoMemo is structured memory first and an AI interface second. Its source of truth is durable storage plus explicit metadata, indexes, symbols, links, citations and review status, not a chat transcript or an opaque embedding store. It runs on the team's own server; cloud AI is optional and must remain an explicit choice made by an administrator.

## Operating Context

The primary product is a shared server (Rust, SQLite) with a React web client. Teams work in organizations and workspaces with roles; they add evidence, review it (lifecycle status), discuss it, track work on a task board, search and ask with citations, explore a knowledge map and health checks, and use a docked assistant. Administrators set the server up in the browser, manage it in the System area and from a command console, and invite people by link.

A Tauri desktop app with the same React code base remains as a fully local edition (no accounts or collaboration). It is frozen: it receives security fixes, not new features.

The application is used as a focused technical workbench on desktop displays. Interaction should support dense scanning, keyboard-friendly operation, long paths, code-oriented content, large result sets, live progress, and both light and dark environments.

## Capabilities and Constraints

- Preserve the RepoMemo name and all current workflows and functionality.
- Storage, browsing, indexing, keyword search, the knowledge map's pipeline and health checks must work without AI.
- SQLite metadata and content-addressed file storage, per environment folder, are the source of truth.
- AI providers are optional and configured per workspace; sending content to a cloud provider requires explicit configuration and acknowledgement.
- Answers and summaries must remain tied to inspectable source evidence and citations, and are labelled as AI-generated.
- The server is the authority on access: the client only hides what a role cannot do.
- The implementation uses Rust (Axum, sqlx), React, TypeScript, and Tauri for the local edition.
- Product copy must not fabricate customers, benchmarks, deployment claims, pricing, or completed capabilities.
- Remote repositories, mail delivery, connectors, single sign-on, scale-out and hosted deployment remain roadmap items rather than present-tense product claims (see `mindmap/ROADMAP.md`).

## Brand Commitments

The product name is RepoMemo. Its voice is calm, precise, technical, private by default, and evidence-oriented. It should communicate system state directly, avoid hype, and distinguish trustworthy stored facts from optional AI interpretation.

## Evidence on Hand

- Product overview and development constraints: `README.md`, `CLAUDE.md`
- What the product does: `mindmap/FUNCTIONAL_MINDMAP.md`
- How it works: `mindmap/TECHNICAL_MINDMAP.md`
- Delivery history and the plan to V0.2.0: `mindmap/ROADMAP.md`
- Security posture: `mindmap/security/2026-10-10-security-review.md`
- Implemented web interface: `apps/desktop/src/SharedWebApp.tsx` and `apps/desktop/src/components/`
- Visual rules and implementation: `DESIGN.md`, `apps/desktop/src/styles/`

There are no approved customer logos, testimonials, usage metrics, performance benchmarks, or marketing claims in the repository. Future work must not invent them.

## Product Principles

1. Evidence before inference: reveal sources, paths, line ranges, status, and provenance.
2. Value without AI: core browsing, indexing, and retrieval remain useful with AI switched off.
3. Dense but legible: optimize for technical scanning without turning the workspace into visual noise.
4. State should be obvious: indexing progress, provider state, background work, and errors must be visible and actionable.
5. Preserve user trust: never blur durable stored facts with generated interpretation, and say when stored knowledge may have gone stale.

## Accessibility & Inclusion

The interface should remain fully usable by keyboard, expose visible focus states, respect reduced-motion preferences, maintain readable contrast in light and dark themes, and accommodate long technical identifiers without hiding critical context.
