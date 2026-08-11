# RepoMemo

RepoMemo is a local-first technical memory workspace. It starts as a Tauri desktop app with a React UI, Rust core, SQLite metadata storage, and local filesystem blob storage.

## Current Capabilities

Phase 1A is implemented and running locally:

- Create and list local workspaces.
- Persist workspace metadata in SQLite.
- Show the local app data/storage root.
- Prove the React UI can call Rust through Tauri commands.
- Prepare the schema and blob folder for sources, artifacts, chunks, search, AI, and memory cards.
- Import supported files/folders into local blob storage.
- List imported artifacts and preview stored text content.

Not implemented yet:

- Chunking/indexing
- Search
- AI answers/summaries
- Memory-card editing beyond the initial create/save workflow

## Roadmap

Track implementation progress in [docs/IMPLEMENTATION_TRACKER.md](docs/IMPLEMENTATION_TRACKER.md).
The next-phase roadmap lives in [docs/ROADMAP.md](docs/ROADMAP.md).

UI work should follow the visual system in [docs/design/RepoMemo_UI_DESIGN_SYSTEM.md](docs/design/RepoMemo_UI_DESIGN_SYSTEM.md),
the per-screen specs in [docs/design/RepoMemo_PAGE_BLUEPRINTS.md](docs/design/RepoMemo_PAGE_BLUEPRINTS.md),
and the reference mockups in [docs/design/mockups/](docs/design/mockups/).

## Prerequisites

- Node.js 22+
- npm 10+
- Rust stable toolchain with Cargo
- Platform prerequisites for Tauri 2

Rust/Cargo must be available on `PATH` before the desktop app can compile.
For Windows setup details, see [docs/WINDOWS_SETUP.md](docs/WINDOWS_SETUP.md).

## Development

Install JavaScript dependencies:

```powershell
npm.cmd install
```

Run this after dependency changes too, such as adding the Tauri dialog plugin.

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
