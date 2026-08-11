# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

RepoMemo serves developers and technical teams working inside an active codebase. They use it while investigating implementation details, recalling why a decision was made, onboarding to unfamiliar systems, and recovering reliable context from code, documents, runbooks, incidents, and architecture records.

## Product Purpose

RepoMemo is a local-first technical memory workspace. It imports technical material, stores it durably, indexes its structure and contents, and makes that evidence searchable and retrievable. Optional AI providers can summarize or answer questions over the indexed material, but the product remains useful without AI.

Success means a user can move from an uncertain technical question to trustworthy source context quickly, understand where the answer came from, and preserve useful knowledge for later.

## Positioning

RepoMemo is structured local memory first and an AI interface second. Its source of truth is durable local storage plus explicit metadata, indexes, symbols, links, and citations—not a chat transcript or an opaque embedding store. Cloud AI is optional and must remain an explicit choice.

## Operating Context

The product is a Tauri desktop application with a React interface and Rust core. Users create workspaces, import local files or folders, inspect artifacts and code symbols, search indexed content, configure optional local or cloud AI providers, request summaries or cited answers, and eventually save durable memory cards.

The application is primarily used as a focused technical workbench on desktop displays. Interaction should support dense scanning, keyboard-friendly operation, long paths, code-oriented content, large result sets, and both light and dark environments.

## Capabilities and Constraints

- Preserve the RepoMemo name and all current workflows and functionality.
- Local storage, browsing, artifact inspection, and search must work without AI.
- SQLite metadata and local content-addressed file storage are the current source-of-truth mechanisms.
- AI providers are optional; sending content to a cloud provider requires explicit configuration.
- Answers and summaries must remain tied to inspectable source evidence and citations.
- The current implementation uses React, TypeScript, Tauri, and a Rust workspace.
- This redesign must not fabricate customers, benchmarks, deployment claims, pricing, or completed capabilities.
- Future collaboration, connectors, server mode, and enterprise controls remain roadmap items rather than present-tense product claims.

## Brand Commitments

The product name is RepoMemo. Its voice is calm, precise, technical, private by default, and evidence-oriented. It should communicate system state directly, avoid hype, and distinguish trustworthy local facts from optional AI interpretation.

## Evidence on Hand

- Product overview and development constraints: `README.md`
- Product position, domain model, and architecture: `docs/architecture/RepoMemo_ARCHITECTURE.md`
- Delivery sequence and acceptance criteria: `docs/ROADMAP.md`
- Implemented desktop interface and workflows: `apps/desktop/src/App.tsx`
- Existing visual implementation: `apps/desktop/src/styles.css`

There are no approved customer logos, testimonials, usage metrics, performance benchmarks, or marketing claims in the repository. Future work must not invent them.

## Product Principles

1. Evidence before inference: reveal sources, paths, line ranges, status, and provenance.
2. Local value without AI: core browsing, indexing, and retrieval remain useful offline.
3. Dense but legible: optimize for technical scanning without turning the workspace into visual noise.
4. State should be obvious: storage location, indexing progress, provider state, and errors must be visible and actionable.
5. Preserve user trust: never blur durable stored facts with generated interpretation.

## Accessibility & Inclusion

The interface should remain fully usable by keyboard, expose visible focus states, respect reduced-motion preferences, maintain readable contrast in light and dark themes, and accommodate long technical identifiers without hiding critical context.
