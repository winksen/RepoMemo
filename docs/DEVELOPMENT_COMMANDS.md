# RepoMemo Development Commands

Run these from the repository root unless a command says otherwise.

## Install Dependencies

```powershell
npm.cmd install
```

Why: downloads the React, Vite, Tauri CLI, and UI icon dependencies declared in `package.json`.
Run this again whenever `package.json` changes, such as after adding a Tauri plugin.

## Typecheck The Frontend

```powershell
npm.cmd run typecheck
```

Why: verifies the TypeScript React shell and Tauri command client types.

## Build The Frontend

```powershell
npm.cmd run build
```

Why: verifies the production Vite bundle that Tauri will load in packaged builds.

## Run The Web Preview Shell

```powershell
npm.cmd run web:dev
```

Why: starts the React UI in browser-preview mode. This does not launch the native Tauri shell, but it lets you inspect the interface quickly.

## Verify The Rust Workspace

If this is a fresh checkout, create the local Tauri icon first:

```powershell
npm run icons
```

Why: Tauri's Windows resource build requires `apps/desktop/src-tauri/icons/icon.ico`.

```powershell
cargo check
```

Why: verifies Rust crates, Tauri command wiring, SQLx migrations, and compile-time Rust dependencies.

This requires Rust/Cargo to be installed and available on `PATH`.
See `docs/WINDOWS_SETUP.md` if PowerShell reports `cargo` as not recognized.

## Run Rust Tests

```powershell
cargo test
```

Why: runs unit tests for ingestion rules, content hashing, and future Rust core behavior.

## Run The Desktop App

```powershell
npm.cmd run dev
```

Why: launches the Tauri desktop app with the React dev server and Rust backend command bridge.

This requires Rust/Cargo and the platform dependencies required by Tauri.

## Run The Shared API

```powershell
$env:REPOMEMO_JWT_SECRET = 'replace-this-with-a-random-development-secret-of-at-least-32-characters'
cargo run -p repomemo-server
```

The API binds to `127.0.0.1:8787` by default. It stores server-owned
development data in `.repomemo-server/` and exposes JWT-protected workspace
routes. Import `docs/api/RepoMemo_Shared_API_v2.postman_collection.json` into
Postman and run its numbered folders in order to exercise authentication,
workspace setup, evidence, indexing, retrieval, team memory, and workspace
membership management. For the membership folder, register the teammate first,
then set its email in the `memberEmail` collection variable.

Run the React web client in a second terminal:

```powershell
npm.cmd run web:dev
```

Open `http://127.0.0.1:5173`. The web client reads `VITE_REPOMEMO_API_URL`,
which defaults to `http://127.0.0.1:8787`; copy `apps/desktop/.env.example` to
`apps/desktop/.env.local` to override it.

Run the background worker foundation separately:

```powershell
cargo run -p repomemo-worker
```
