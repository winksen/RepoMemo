# Guidelines for AI assistants working on RepoMemo

## Testing and running

- **Never start a server or run a build to test locally.** That means no `repomemo-server`, no Vite dev server, no Tauri app, no `cargo build`, no extra target folders and no smoke binaries. The maintainer runs and tests the application themselves.
- Hand over finished work with what changed and what to try, so it can be tested by hand.
- If a lighter check seems useful (`cargo check`, `cargo test`, `npm run typecheck`), ask first instead of assuming it is wanted.

## Where things are documented

- [mindmap/FUNCTIONAL_MINDMAP.md](mindmap/FUNCTIONAL_MINDMAP.md): what the product does.
- [mindmap/TECHNICAL_MINDMAP.md](mindmap/TECHNICAL_MINDMAP.md): how it works.
- [mindmap/ROADMAP.md](mindmap/ROADMAP.md): what has shipped and what comes next.
- `mindmap/RELEASES_<minor>.md` ([0.1](mindmap/RELEASES_0.1.md), [0.2](mindmap/RELEASES_0.2.md), [0.3](mindmap/RELEASES_0.3.md)): one row per release of a minor line, with its track and type. A release updates its row; a new minor line gets a new file.
- [docs/USER_GUIDE.md](docs/USER_GUIDE.md): a light, human-oriented tour of what the product offers, for users and admins. Update it when a user-visible feature ships.
- [docs/DEVELOPMENT_COMMANDS.md](docs/DEVELOPMENT_COMMANDS.md): commands and every server setting.
- [docs/TEMPLATES.md](docs/TEMPLATES.md): how to write a workspace template (`npm run templates:check` checks it).

Keep the mindmaps current when behavior, routes, settings or migrations change.

## Web client conventions

- Errors and success messages are toasts (`showToast` or `<Toast />`), never inline banners.
- Backgrounds and text use pure greys with no shared hue; follow [DESIGN.md](DESIGN.md).
