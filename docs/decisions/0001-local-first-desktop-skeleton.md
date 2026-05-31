# 0001: Local-First Desktop Skeleton

Status: accepted

Date: 2026-05-31

## Context

RepoMemo should start as a local-first technical memory workspace. The architecture brief recommends a modular monorepo with a Tauri desktop shell, React/TypeScript UI, Rust backend/core crates, SQLite metadata storage, and local content-addressed blob storage.

## Decision

Phase 1A starts with one repository and local in-process boundaries:

- `apps/desktop` for the Tauri + React app
- `crates/domain` for shared product models
- `crates/storage` for SQLite, migrations, and local data folders
- `crates/api` for the command-facing core facade used by Tauri

The initial database is SQLite, stored under the Tauri app data directory as `repomemo.sqlite`. Raw blob folders are created now, while file ingestion begins in Phase 1B.

## Consequences

This keeps the MVP small while preserving future service boundaries. The app can create and list local workspaces now, and later phases can add import, chunking, FTS search, code symbols, AI providers, and memory cards without changing the top-level architecture.
