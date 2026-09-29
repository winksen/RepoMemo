<p align="center">
  <img src="apps/desktop/public/RM-logofull.svg" alt="RepoMemo" width="260" />
</p>

<p align="center">
  <strong>A team memory workspace for technical knowledge.</strong><br />
  Store your evidence, index it, search it, and ask questions that are answered with citations.
</p>

---

## What is RepoMemo?

Engineering knowledge is scattered across code, Markdown docs, Word files, screenshots, runbooks and people's heads. RepoMemo gives a team one place to keep that **evidence**. It makes the evidence **searchable** and turns what the team learns into **durable, cited memory**.

It is **evidence-first**. Storing, browsing, indexing and searching all work without AI. AI is an optional layer, and it is only allowed to answer from your indexed evidence, with a citation to the exact file and lines it used.

## Features

| | |
|---|---|
| 🗂️ **Evidence workspaces** | Organizations and workspaces with owner, admin, member and viewer roles |
| 📥 **Capture** | Paste notes, or upload code, Markdown, Word documents and images of up to 10 MiB |
| 🔎 **Index & search** | Heading-aware chunking, code symbols for TS/JS/Python/Rust, ranked full-text search with filters, and saved searches |
| 🤖 **Ask with citations** | Optional AI through local **Ollama** or cloud **OpenRouter**. Answers and workspace overviews are grounded in retrieved evidence |
| 🧠 **Memory cards** | Keep conclusions as durable, evidence-linked cards, with Markdown export |
| ✅ **Review & collaborate** | Evidence lifecycle (verified, outdated, superseded), comments with @mentions, a task board with checklists, notifications, an activity feed and metrics |
| 🔒 **Private by default** | Self-hosted server, and nothing is sent to an AI provider until an admin enables one explicitly |

## How it works

```
 Browser (React SPA)  ──JWT──►  RepoMemo API (Rust · Axum)  ──►  SQLite + content-addressed file store
                                        │
                                        └──► optional AI provider (Ollama / OpenRouter)
```

RepoMemo can be run two ways:

- **Shared mode (primary).** A self-hosted API server and a web client for teams.
- **Desktop mode.** A local-only Tauri app for Windows that uses the same React UI and Rust core.

## Quick start (shared mode)

**Prerequisites:** Node.js 22+, npm 10+, and the stable Rust toolchain. For Windows specifics, see [docs/WINDOWS_SETUP.md](docs/WINDOWS_SETUP.md).

```bash
npm install
```

Start the API on `127.0.0.1:3020`. It needs a JWT secret of at least 32 characters.

```bash
REPOMEMO_JWT_SECRET="replace-with-a-random-secret-of-32+-chars" cargo run -p repomemo-server
```

In PowerShell, set the secret with `$env:REPOMEMO_JWT_SECRET = '...'` before running `cargo run -p repomemo-server`.

Start the web client on `127.0.0.1:3021` from a second terminal:

```bash
npm run web:dev
```

Open <http://127.0.0.1:3021>, create an account, create an organization and a workspace, and add some evidence.

**Desktop mode:**

```bash
npm run dev
```

More commands, including tests, builds and the worker, are in [docs/DEVELOPMENT_COMMANDS.md](docs/DEVELOPMENT_COMMANDS.md).

## Documentation

The documentation starts in the **[`mindmap/`](mindmap/)** folder:

| Document | For | What it covers |
|---|---|---|
| 🧭 **[Functional mindmap](mindmap/FUNCTIONAL_MINDMAP.md)** | Everyone | What RepoMemo does: roles, features, workflows, product principles, known gaps |
| 🛠️ **[Technical mindmap](mindmap/TECHNICAL_MINDMAP.md)** | Engineers | Architecture, the full API reference, pipelines, data model, risks |
| 🗺️ **[Roadmap](mindmap/ROADMAP.md)** | Everyone | What has shipped, what is next, and the longer-term direction |

Other references:

- [Architecture decisions (ADRs)](docs/decisions/)
- [Development commands](docs/DEVELOPMENT_COMMANDS.md)
- [Implementation tracker](docs/IMPLEMENTATION_TRACKER.md)

## Repository layout

```
apps/
  server/      Shared-mode HTTP API (Axum)
  desktop/     React web client + Tauri desktop shell
  worker/      Background worker (placeholder)
crates/
  api/         RepoMemoCore: import, index, search, ask, memory
  storage/     SQLite (sqlx) + blob store + migrations
  ingestion/   File detection and Word text extraction
  indexer/     Chunking and tree-sitter symbol extraction
  retrieval/   Full-text and hybrid retrieval
  ai/          Ollama and OpenRouter providers
  domain/      Shared data types
mindmap/       Functional and technical documentation, roadmap
docs/          Setup guides, ADRs, historical phase specs
```

## Status

RepoMemo is in **early development** (`v0.1.x`). APIs and storage formats may change between versions. See the [roadmap](mindmap/ROADMAP.md) for the current focus.

## License

MIT, as declared in the workspace [`Cargo.toml`](Cargo.toml).
