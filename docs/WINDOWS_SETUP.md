# RepoMemo Windows Setup

RepoMemo uses a React frontend and a Rust/Tauri desktop backend. The frontend can typecheck and build with Node alone, but the desktop app requires Rust/Cargo.

## Current Required Tools

- Node.js 22+
- npm 10+
- Rust stable toolchain with Cargo
- Microsoft C++ build tools, including the MSVC linker `link.exe`
- WebView2 runtime, normally already present on current Windows installs

## Install Rust

Run one of these from PowerShell.

Recommended official installer:

```powershell
winget install --id Rustlang.Rustup -e
```

Why: installs `rustup`, Rust stable, and `cargo`, which Tauri needs to compile the desktop backend.

Alternative if `winget` is unavailable:

```powershell
Invoke-WebRequest https://win.rustup.rs/x86_64 -OutFile rustup-init.exe
.\rustup-init.exe
```

Why: downloads and runs the official Rust Windows installer directly.

After installation, close and reopen PowerShell so `cargo` is added to `PATH`.

## Verify Rust

```powershell
cargo --version
```

Why: confirms the terminal can find Cargo.

```powershell
cargo check
```

Why: verifies RepoMemo's Rust workspace, Tauri command bridge, SQLite migration setup, and crate dependencies.

## Install Microsoft C++ Build Tools

If `cargo check` fails with:

```text
error: linker `link.exe` not found
```

install Visual Studio Build Tools:

```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools -e
```

Why: installs the Visual Studio Build Tools installer, which provides the native compiler/linker toolchain Rust's MSVC target depends on.

When the installer opens, select:

- Desktop development with C++
- MSVC v143 C++ build tools
- Windows 10 or Windows 11 SDK

After installation, close and reopen PowerShell, then run:

```powershell
rustup default stable-msvc
cargo check
```

Why: ensures Rust uses the MSVC toolchain and verifies that `link.exe` is now available to native Rust builds.

## Run The Desktop App

```powershell
npm run dev
```

Why: starts the Tauri desktop app. This internally runs the Vite frontend and Rust backend together.

## Notes

If `npm run dev` prints Tauri messages about workspace inheritance for `tauri` or `tauri-build`, those are informational. The actual blocker is `program not found` for `cargo`.

If `cargo check` prints `link.exe not found`, Rust is installed correctly but the Microsoft C++ build tools are missing.
