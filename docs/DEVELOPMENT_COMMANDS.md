# RepoMemo Development Commands

Run these from the repository root unless a command says otherwise.

## Install Dependencies

```powershell
npm.cmd install
```

Why: downloads the React, Vite, Tauri CLI, and UI icon dependencies declared in `package.json`.

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

```powershell
cargo check
```

Why: verifies Rust crates, Tauri command wiring, SQLx migrations, and compile-time Rust dependencies.

This requires Rust/Cargo to be installed and available on `PATH`.

## Run The Desktop App

```powershell
npm.cmd run dev
```

Why: launches the Tauri desktop app with the React dev server and Rust backend command bridge.

This requires Rust/Cargo and the platform dependencies required by Tauri.
