# RepoMemo

RepoMemo is a local-first technical memory workspace. The first phase focuses on a Tauri desktop shell, a Rust core, SQLite-backed workspace metadata, and the project structure needed for ingestion and retrieval.

## Current Phase

Phase 1A is being built first:

- Rust workspace and core crate boundaries
- Tauri + React desktop app
- SQLite database setup and migrations
- Local app data folder
- Basic workspace create/list flow
- Basic settings surface

Track implementation progress in [docs/IMPLEMENTATION_TRACKER.md](docs/IMPLEMENTATION_TRACKER.md).

## Prerequisites

- Node.js 22+
- npm 10+
- Rust stable toolchain with Cargo
- Platform prerequisites for Tauri 2

Rust/Cargo must be available on `PATH` before the desktop app can compile.

## Development

Install JavaScript dependencies:

```powershell
npm.cmd install
```

Run the desktop app:

```powershell
npm.cmd run dev
```

Run only the Vite web shell:

```powershell
npm.cmd run web:dev
```

Build the frontend:

```powershell
npm.cmd run build
```
