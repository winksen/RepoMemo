# Guidelines for AI assistants working on RepoMemo

## Testing and running

- **Never start a server or run a build to test locally.** That means no `repomemo-server`, no Vite dev server, no Tauri app, no `cargo build`, no extra target folders and no smoke binaries. The maintainer runs and tests the application themselves.
- Hand over finished work with what changed and what to try, so it can be tested by hand.
- If a lighter check seems useful (`cargo check`, `cargo test`, `npm run typecheck`), ask first instead of assuming it is wanted.

## Where things are documented

- [mindmap/FUNCTIONAL_MINDMAP.md](mindmap/FUNCTIONAL_MINDMAP.md): what the product does.
- [mindmap/TECHNICAL_MINDMAP.md](mindmap/TECHNICAL_MINDMAP.md): how it works.
- [mindmap/ROADMAP.md](mindmap/ROADMAP.md): what has shipped and what comes next.
- [docs/DEVELOPMENT_COMMANDS.md](docs/DEVELOPMENT_COMMANDS.md): commands and every server setting.

Keep the mindmaps current when behavior, routes, settings or migrations change.

## Web client conventions

- Errors and success messages are toasts (`showToast` or `<Toast />`), never inline banners.
- Backgrounds and text use pure greys with no shared hue; follow [DESIGN.md](DESIGN.md).
