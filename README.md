<p align="center">
  <img src="apps/desktop/public/RM-logofull.svg" alt="RepoMemo" width="260" />
</p>

<p align="center">
  <strong>A team memory workspace for technical knowledge.</strong><br />
  Store your evidence, index it, search it, and ask questions that are answered with citations.
</p>

---

## What is RepoMemo?

Engineering knowledge is scattered across code, Markdown docs, Office files, screenshots, runbooks and people's heads. RepoMemo gives a team one place to keep that **evidence**, makes it **searchable**, and turns what the team learns into **durable, cited memory**.

It is **evidence-first**. Storing, browsing, indexing and searching work without AI. AI is an optional layer, and it answers only from your indexed evidence, with a citation to the file and lines it used.

**Current version:** `0.5.2`. Releases before `1.0` may change APIs and storage formats; read the *Upgrading* part of the [changelog](CHANGELOG.md) before you upgrade.

## What it does

- **Workspaces** inside organizations, with owner, admin, member and viewer roles.
- **Evidence** from notes, uploaded files and folders, Office documents, PDFs, emails, images and git repositories.
- **Search** by keyword and by meaning, with filters and saved searches. **Ask** for cited answers when an AI provider is set up.
- **Memory cards** that keep conclusions linked to their evidence.
- **Review and collaboration**: evidence status, comments, tasks, notifications, an activity feed.
- **Private by default**: a self-hosted server, and nothing is sent to an AI provider until an administrator turns one on.

```
 Browser or desktop window ──►  RepoMemo server (Rust · Axum)  ──►  SQLite + file store
                                          │
                                          └──► optional AI provider (Ollama / OpenRouter)
```

## Choose how to run it

| You want to… | Use | Start here |
|---|---|---|
| Run a server for a team | **Containers** (preferred) | [Quick start](docs/QUICK_START.md) |
| Run a server without containers | **Direct install** (Linux service or Windows) | [Deploying RepoMemo](docs/DEPLOYMENT.md) |
| Use a server from a desktop window | **Desktop app**, which asks for your server's address | [Desktop files](docs/DESKTOP_FILES.md) |
| Change RepoMemo itself | **Development checkout** | [Development quick start](#development-quick-start) |

A browser is enough to use a server. The desktop app is the same client in its own window.

## Find the right guide

| I am… | I want to… | Read |
|---|---|---|
| A user | Add evidence, search, ask and keep memory | [User guide](docs/USER_GUIDE.md) |
| An administrator | Install, back up and upgrade a server | [Deploying RepoMemo](docs/DEPLOYMENT.md) |
| Stuck | Fix a start or first-run error | [Troubleshooting](docs/TROUBLESHOOTING.md) |
| A contributor | Know what the product does and how it works | [Functional mindmap](mindmap/FUNCTIONAL_MINDMAP.md), [Technical mindmap](mindmap/TECHNICAL_MINDMAP.md) |
| Planning | See what shipped and what is next | [Roadmap](mindmap/ROADMAP.md) |

Every guide, by task and role: [docs/README.md](docs/README.md).

## Development quick start

**Prerequisites:** Node.js 22.12+, npm 10+, and the stable Rust toolchain (1.88 or later). On Windows, see [docs/WINDOWS_SETUP.md](docs/WINDOWS_SETUP.md).

```bash
npm install
REPOMEMO_JWT_SECRET="replace-with-a-random-secret-of-32+-chars" cargo run -p repomemo-server
```

In PowerShell, set `$env:REPOMEMO_JWT_SECRET = '...'` first. The server listens on `127.0.0.1:3020`. In a second terminal, start the web client:

```bash
npm run web:dev
```

Open <http://127.0.0.1:3021>, create an account, then an organization and a workspace. `npm run dev` starts the desktop app instead. Every command, test and setting is in [docs/DEVELOPMENT_COMMANDS.md](docs/DEVELOPMENT_COMMANDS.md).

## Repository layout

```
apps/
  server/      Shared-mode HTTP API (Axum)
  console/     Command console for administrators
  desktop/     React web client + Tauri desktop shell
crates/        Core, storage, ingestion, indexer, retrieval, AI providers, domain types
deploy/        Container and direct-install scripts
mindmap/       Functional and technical documentation, roadmap
docs/          Setup guides, ADRs, historical specifications
```

## License

MIT, as declared in the workspace [`Cargo.toml`](Cargo.toml).
