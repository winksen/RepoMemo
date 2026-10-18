# Changelog

What changed in each release, newest first, with what to do when upgrading. Versions follow [semantic versioning](https://semver.org): `0.2.0` is the next minor release (see the [roadmap](mindmap/ROADMAP.md#-road-to-v020)); the 0.2.x releases after it lead to `0.3.0` ([Road to V0.3.0](mindmap/ROADMAP.md#-road-to-v030), proposed). Releases before V0.1.63 are summarized in the roadmap's *Delivered* tables.

## V0.2.83 (2026-10-18)

One heading for every panel.

### Changed
- **`PanelHeading`** is in `components/ui/panel-heading.tsx`: `<PanelHeading icon title description actions level id />`. The icon is a Tabler component drawn at the heading's size (18 px in an `h2`, 16 px in an `h3`), `description` is the muted note at the end (a count, a state), `actions` hold buttons or a status element, and `id` lets a section name itself by it with `aria-labelledby`.
- **It replaces all 59 hand-built `shared-panel-heading` blocks** in 24 files (System, Profile, repositories, connectors, the knowledge map, workspace pages and others). The rules moved to `14-building-blocks.css` as `rm-panel-heading`, and the page-specific margin and note rules were renamed with it; `grep shared-panel-heading` finds nothing. The two status pills in a heading (a repository's status, an evidence lifecycle status) now take their own size and colour instead of the heading's note style.

### Upgrading
- Nothing to do.

## V0.2.82 (2026-10-18)

One segmented control for switches and tab strips.

### Changed
- **`SegmentedControl`** is in `components/ui/segmented-control.tsx`, built on Radix Toggle Group (`@radix-ui/react-toggle-group` is a new dependency). Options have a label, an optional icon and count, or only an icon with a `name`; one is always chosen; the arrow keys, Home and End move between them. `fill` shares the width, `followFocus` makes the arrow keys choose as they move.
- **It replaces five hand-made switches:** sign in or create account (`shared-auth-switch`), the activity type filter of People (`shared-activity-filter`), the grid and list switch of the evidence view and the graph and table switch of the knowledge map (`shared-artifact-view-switch`, which were `IconButton`s) and the sheet tabs of a workbook preview (`shared-sheet-tabs`, with `followFocus`). Their styles are gone; the sheets are a labelled group of radio buttons now, with the sheet shown in a region of its name.
- `rm-desktop-switch` was a single button (*Connect to a server*) with no style of its own, not a switch; it lost the class.
- The workbook sheets test in `accessibility.test.tsx` was rewritten for the new roles (written, not run; CI runs it).

### Upgrading
- Nothing to do.

## V0.2.81 (2026-10-18)

One badge for tags, roles and chips.

### Changed
- **`Badge`** is in `components/ui/badge.tsx`: `<Badge tone icon>`. A neutral surface with a border and grey text; a tone (`success`, `warning`, `danger`, `info`) adds a 6 px status dot and nothing else, so there is no tinted fill with text of the same hue, as DESIGN.md asks. It forwards its ref, so a `Tooltip` can wrap it.
- **It replaces six classes:** `rm-system-tag` and `rm-system-role` (token scopes, the client of an access log line, backup trigger and encryption, user roles, deactivated users), `shared-review-badge` (*Needs review* on a memory card, now with a warning dot and a tooltip), `rm-console-tag` (*dry run*, which had a tinted warning fill), `workspace-context-chip` (the retired local edition's workspace chip) and `rm-stack-chip` (a repository's technologies). Their rules are gone; an expired API token now has a danger dot.
- The role badges keep their hints as tooltips instead of `title` attributes.

### Upgrading
- Nothing to do.

## V0.2.80 (2026-10-18)

One empty state for lists with nothing in them.

### Changed
- **`EmptyState`** is in `components/ui/empty-state.tsx`: `<EmptyState icon title description action />` is a muted block centred in its panel, with an illustration-sized icon, a title, one sentence and at most one action.
- **First use:** the task board when no task matches the filters, the memory cards (one wording when a search finds nothing, another when the workspace has no card yet) and the notifications page. The memory cards had a bare line of text and now say what to do next. The older `shared-empty-state` block stays on the pages that still use it until their move in V0.2.102 to V0.2.118.

### Upgrading
- Nothing to do.

## V0.2.79 (2026-10-18)

One loading state with one wording rule.

### Changed
- **`LoadingState`** is in `components/ui/loading-state.tsx`: `<LoadingState label="Loading jobs" />` shows the `Spinner` and one muted line, announced as a status. The rule for the label is "Loading <thing>"; the component adds the ellipsis.
- **First use:** the System pages. The overview, usage, users, settings, logs, backups, jobs and audit pages each had their own sentence ("Loading the system overview…", "Loading usage…" and so on, some with a spinner, some without) and now share the component and its look.

### Upgrading
- Nothing to do.

## V0.2.78 (2026-10-18)

No more browser confirmations.

### Changed
- **`useConfirm` replaces the last four `window.confirm` calls** of the web client: handing an organization to a member (Organization), handing a workspace to a member (People), deleting a workspace (Settings, with the danger button) and deleting a task (Tasks, with the danger button). Handing over names the member in the title and says in the description that the previous owner stays on as an administrator and cannot take it back.
- `grep window.confirm` finds nothing in the web client.

### Upgrading
- Nothing to do.

## V0.2.77 (2026-10-18)

Confirmations in the app's own dialog.

### Changed
- **`ConfirmDialog` and `useConfirm`** are in `components/ui/confirm-dialog.tsx`. `const confirm = useConfirm(); if (!(await confirm({ title, description, action, danger }))) return;` opens the app's dialog with Cancel on the left and the decisive action on the right (the `danger` button when `danger` is set) and resolves `true` or `false`. Escape and a click outside answer `false`. `ConfirmProvider` hosts the one dialog and is mounted in `SharedWebApp`; a second question while one is open answers the first with `false`.
- **First use:** deleting evidence (it names the file and says that linked memory citations are removed), deleting an evidence comment and deleting a memory card. They were the browser's `window.confirm`.

### Upgrading
- Nothing to do.

## V0.2.76 (2026-10-18)

A danger button for destructive confirmations.

### Changed
- **`variant="danger"` on `Button`.** A solid `--color-danger` fill with white text and a darker hover, defined once in `14-building-blocks.css`. It replaces the one-off `.rm-dialog-footer .rm-button-danger` rule, which only worked by className inside a dialog footer.
- **Used by** the confirmations that delete or remove for good: deleting a folder, a file or several files, removing a repository, removing a connector and deleting an assistant chat. The `rm-button-danger` class that five row buttons (*Remove*, *Turn off*, bulk delete) carried without effect is gone; they stay secondary buttons.

### Upgrading
- Nothing to do.

## V0.2.75 (2026-10-18)

Icon-only buttons name themselves in a tooltip.

### Changed
- **`Tooltip`** is in `components/ui/tooltip.tsx`, built on Radix Tooltip (`@radix-ui/react-tooltip` is a new dependency): a small label in inverted greys above the control, after 400 ms of hover and at once on keyboard focus. It sits in the popover layer, so it shows above dialogs.
- **`IconButton` shows its `label` in a tooltip** instead of a `title` attribute, which keyboard users never saw. The theme toggle of the account controls is an `IconButton` now.
- **The other `title` hints on controls** became tooltips: *New folder* in the evidence browser (including why it is disabled at the folder depth limit) and *Similar content* in the knowledge map when AI for search is not set up.

### Upgrading
- Nothing to do.

## V0.2.74 (2026-10-18)

A shared button for a single icon.

### Changed
- **`IconButton`** is in `components/ui/icon-button.tsx`: a square button holding one icon with a required `label` (its `aria-label`, and for now its `title`). It is 32 px (`size="sm"`) or the control height of 36 px (`md`, the default), has an `outline` or a `plain` look (bare until hovered) and takes `pressed` for a toggle that is on and `busy` for work in progress.
- **Where it is used:** the *more actions* trigger of every `ActionMenu` (it was `rm-action-trigger`, 28 px, now 32 px), the System pages' *Refresh* button (an icon now, not an icon with a word), the remove button of an organization member, and the grid and list switch of the evidence view and the graph and table switch of the knowledge map (they were 34 px squares).
- Removed: `rm-action-trigger` and the 34 px rules of the view switch and of the organization member actions. The latter also applied to the *Make owner* text button, which now has the normal button width.

### Upgrading
- Nothing to do.

## V0.2.73 (2026-10-18)

Buttons can show that they are working.

### Changed
- **`busy` on `Button`.** `<Button busy={isSubmitting}>` puts a spinner in front of the label, hides the button's own icon, disables the button and sets `aria-busy`. The label does not change, so the button keeps its width. A new `Spinner` (`components/ui/spinner.tsx`) is the turning ring, also for inline use.
- **First use:** the sign-in and account creation button, and Profile's *Save details*, *Change password* and *Sign out everywhere* buttons, which each had their own spinner swap.

### Upgrading
- Nothing to do.

## V0.2.72 (2026-10-18)

A button only submits a form when it says so.

### Changed
- **`Button` is a plain button by default.** It renders `type="button"` unless the caller gives a type, so a button inside a form no longer submits it by accident (an HTML button submits by default). `asChild` buttons keep whatever the child element is.
- **Every form was checked** with a syntax-tree pass over the web client: every `Button` and native button in the web client already carried an explicit `type`, and every form has exactly one `type="submit"` button, either inside it or tied to it with `form="…"` (the repository edit dialog). The one form without a submit button, *Who can use AI* in the workspace settings (a single dropdown that saves when it changes), is a `div` now: it never submitted anything.
- No submit button lives outside a form without a `form` attribute.

### Upgrading
- Nothing to do.

## V0.2.71 (2026-10-18)

A shared switch for settings that turn on and off.

### Changed
- **`Switch`** is in `components/ui/switch.tsx`: `<Switch checked onCheckedChange label>`, built on Radix Switch (`@radix-ui/react-switch` is a new dependency). A 36 by 20 px track with a round thumb, the accent colour when on, with a keyboard focus ring. It takes a `label` beside it (clicking the label toggles it), an `aria-label`, or a `<label htmlFor>` tied to its `id`.
- **First use:** the boolean settings of System › Settings, which were a bare checkbox next to "On" or "Off". They keep their saving: a change is still saved with the page's save button. The other hand-made toggles (`shared-ai-provider-toggle`, in mail settings, the setup wizard and the log follow switch) move with their pages in V0.2.103 to V0.2.116.

### Upgrading
- Nothing to do.

## V0.2.70 (2026-10-18)

A shared checkbox with a label and a description.

### Changed
- **`Checkbox`** is in `components/ui/checkbox.tsx`: `<Checkbox checked onCheckedChange label description>`, built on Radix Checkbox (`@radix-ui/react-checkbox` is a new dependency). A 16 px box that is the accent colour when checked, its label (clicking it toggles the box) and an optional muted description under it. It is a button with the checkbox role, so the keyboard and screen readers behave as for a native one.
- **First use:** the evidence type filter (*All types* and one row per file type). Its hand-drawn check mark and the seven rules that styled the bare inputs of that menu are removed; the rows look the same.

### Upgrading
- Nothing to do.

## V0.2.69 (2026-10-18)

A shared field: a label, a control and a short message.

### Changed
- **`Field` and `FieldRow`** are in `components/ui/field.tsx`. `<Field label hint required invalid message htmlFor>` renders a label tied to its control (the id is generated when none is given), an optional muted hint and, while `invalid`, a one-line message under the control. The control gets `aria-invalid` and `aria-describedby`; only its border turns red, with no banner and no fill. `FieldRow` puts fields side by side and stacks them below about 220 px. The styles are in a new `14-building-blocks.css`, which holds every building block of this track.
- **First use:** Profile › Personal details (display name and email), whose labels are now tied to their fields. The look is unchanged. Required fields are still the browser's own `required`; marking and blocking them with a message comes in V0.2.122.
- `DESIGN.md` has a *Building blocks* section that lists each shared piece.
- The accessibility check counts a control inside a `Field` as labelled.

### Upgrading
- Nothing to do.

## V0.2.68 (2026-10-17)

Icons come in three sizes.

### Changed
- **One constant for icon sizes.** `ICON_SIZE = { sm: 14, md: 16, lg: 18 }` is in `components/ui/icon.ts`. The 503 icons from 12 to 18 px, in 53 files, read it instead of a number: icons of 12, 13 and 15 px became 14 px (the small ones in rows, badges and compact actions) and 17 px ones became 16 px. They used to come in seven sizes, 12 to 18 px. Nothing else in these files changed.
- **Illustrations keep their size**: avatars, the empty states (25 and 26 px), the setup steps and their status icons (20 and 22 px), file and folder tiles, the page loader, the assistant's introduction and the retired local edition's larger icons.

### Upgrading
- Nothing to do.

## V0.2.67 (2026-10-17)

Checkboxes and radio buttons are the same size everywhere.

### Changed
- **One rule for checkboxes, radio buttons and file pickers.** A base rule in `01…` sets a checkbox or radio button to 16 px with the accent colour and leaves a file picker its natural width. Six places had patched the size of their own (the evidence type filter, the task checklist, the AI provider and System toggles, the local edition's provider toggle and filter options, and the duplicate-file radio buttons); the size and colour parts are removed and only their vertical alignment stays.
- The evidence type filter keeps its drawn check mark until the shared `Checkbox` (V0.2.70) replaces it.

### Upgrading
- Nothing to do.

## V0.2.66 (2026-10-17)

Every form uses the same field.

### Changed
- **Page forms no longer resize the shared field.** The sign-in, setup and create-workspace fields were 42 px high, the member and memory fields 38 px with a 1 px border or 10 px of padding, each with its own background and corner. Those rules are gone: the fields of these forms and of the note and search forms are the shared field (36 px, a 2 px border, an 8 px corner), and the stylesheet keeps only their layout (full width, the create-workspace field's width, the minimum height of text areas). The comment form never overrode it.
- `.shared-memory-form select` is deleted; no form has a raw select any more.

### Upgrading
- Nothing to do.

## V0.2.65 (2026-10-17)

Fields are styled by their shared classes only.

### Changed
- **The competing global field rules are out of the web client.** Three more stylesheets (`09…`, `10…`, `11…`) restyled every bare `input`, `textarea` and `select` — a 6, 8 or 10 px corner, a 1 or 2 px border, a white, grey or surface background — and the dark theme repeated each. They now apply only inside the retired local edition (the same `:where(.rm-local-edition)` scope as V0.2.64, so that edition's look and cascade are kept); in the web client a field is styled by `.rm-input`, `.rm-textarea` or `.rm-dropdown-trigger`, plus one base reset in `01…` (font and colour inherited, the focus ring).
- The evidence picker of the memory card page, a raw `select` until V0.2.86 replaces it, takes the shared field class.
- Checkboxes, radio buttons and file pickers are no longer stretched like text fields; V0.2.67 gives them their size and accent colour explicitly.

### Upgrading
- Nothing to do.

## V0.2.64 (2026-10-17)

The retired local edition's styles stay in the local edition.

### Fixed
- **The web client's fields inherited the local edition's rules.** The local app styled bare `input`, `textarea` and `select` elements (a borderless field with a 6 px corner, then a 3 px one with a 1 px border) and some classes (`.paste-language`, `.workspace-create-controls`, `.search-input-wrap`, `.filter-options`, `.artifact-search`) for the whole page, so every field in the web client had to override them. These 36 selectors are now under `:where(.rm-local-edition)`, a class the local app's root carries. `:where` adds no specificity, so the local edition keeps the cascade it had; the web client matches none of them.

### Upgrading
- Nothing to do.

## V0.2.63 (2026-10-17)

Colours come from tokens.

### Changed
- **No raw colours in the page rules.** Every hex, `rgb()` and `rgba()` colour outside a `:root` block in the stylesheets now reads a token: whites are `--color-surface` (or `--color-on-accent` for text on a solid fill), the light greys are the neutral tokens, status colours are the status tokens, focus rings and hover shadows are `color-mix` of the accent or of black. Fields, panels and chips in the dark theme are no longer white where an older rule hard-coded it.
- **One danger red.** The destructive button in dialog footers used its own `#c9372c`; it uses `--color-danger`, with `--color-on-danger` for its text (white on the light theme, dark on the dark theme's light red). Destructive menu items use `--color-danger` too.
- **New tokens** for what had no token: `--color-toast-error`, `-success`, `-warning`, `-info` (solid toast fills), `--file-markdown` and the other file-type icon colours, and the knowledge map's `--viz-*` colours, which moved to `:root`.

### Removed
- **164 token declarations that a later stylesheet overrides.** The cascade's last `:root` blocks decide every token (the blocks in `01…`, `09…` and `10…`, with the green accent no page ever showed, were dead). They are deleted; the effective value of every token in both themes was compared before and after and is identical.

### Upgrading
- Nothing to do.

## V0.2.62 (2026-10-17)

One accent: the workspace palette is retired.

### Changed
- **The `--workspace-*` colours are gone.** Seven colours, a dot-grid colour and a font, with a dark-theme copy, were defined for the "workspaces v3" pages and read about sixty times. They now read the shared tokens (`--color-accent`, `-accent-hover`, `-accent-soft`, `-accent-text`, `--color-border`, `-border-strong`, `--color-surface-2`, `-surface-3`, `--font-ui`). `grep -- --workspace- styles` finds nothing.
- Those pages are the retired local edition's (`App.tsx`): the web client does not use them, so it looks as before, and the local edition's teal turns into the shared accent.

### Upgrading
- Nothing to do.

## V0.2.61 (2026-10-17)

Backgrounds, borders and text are true greys.

### Changed
- **The light theme's neutrals have no hue.** The tokens that win the cascade (`--color-bg`, `-canvas`, `-surface*`, `-code-surface`, `-text*`, `-border*`, in `11-blueprint-primitives.css`) were blue-grey (hue 220); they are now greys of the same lightness (the muted-subtle text is one point darker, 42 % instead of 43 %, to keep its contrast on hovered rows). The dark theme already was grey. The accent and the status colours are untouched, and the contrast check still passes.
- The older, overridden copies of these tokens in `01…`, `09…` and `10…` (green-grey) are greys too, so no neutral token has a hue anywhere.

### Fixed
- **The plan of track K was corrected.** Its audit read the first stylesheet's tokens and took the accent for green and the neutrals for green-tinted; the tokens that win the cascade are the last ones, where the accent is blue, as DESIGN.md says. Decision 16 now keeps that blue, and V0.2.63 removes the overridden token blocks.

### Upgrading
- Nothing to do.

## V0.2.60 (2026-10-17)

Text fields, dropdowns and buttons are the same height.

### Changed
- **One size for controls.** The tokens `--control-height` (36 px), `--control-height-touch` (40 px), `--control-radius` (8 px), `--control-border` (2 px) and `--control-padding-x` (12 px) are in `01-tokens-and-shared-web.css`. Text fields, text areas, dropdowns, dropdown rows and buttons use them, so they are 36 px high instead of 34 px, which is what DESIGN.md says (36–40 px); on narrow screens they use the touch height of 40 px. Colours, borders and corners look as before.
- Fields of the sign-in, setup, member and memory forms keep their own sizes until V0.2.66.

### Upgrading
- Nothing to do.

## V0.2.59 (2026-10-17)

Dropdowns no longer stretch across toolbars and filter rows.

### Changed
- **A dropdown is as wide as its content, 160 px at least.** It used to fill whatever it was placed in, so a dropdown in the System logs toolbar, the Jobs filter or the Tasks and Retrieval filter rows stretched across the row. The Retrieval and Tasks filters stay at the start of their cell.
- **Forms still stretch it.** The forms whose dropdown is a field (AI providers and policy, API tokens, invitations, the move-to-server dialog, evidence lifecycle, profile notifications, members, task creation and cards, mail and System settings, repository and connector forms) say so in one rule, until the shared `Field` (V0.2.69) does.

### Upgrading
- Nothing to do.

## V0.2.58 (2026-10-17)

Long dropdown lists stay inside the window.

### Fixed
- **A long dropdown list ran off the screen.** The list had no maximum height and clipped what did not fit, so a list opened near the bottom of the window (the level in System › Settings, the language filter in Retrieval) could not be reached. A list is now at most 320 px tall or the room the window leaves, whichever is less, and scrolls; arrows at the top and the bottom scroll it while the pointer rests on them.

### Upgrading
- Nothing to do.

## V0.2.57 (2026-10-17)

Dropdown lists and menus open above dialogs. The first release of the frontend polish track (K1, [plan](mindmap/plans/2026-10-17-frontend-polish.md)): it changes no look and no behaviour besides the stacking.

### Fixed
- **A dropdown inside a dialog opened behind it.** A dropdown list was at layer 50 and a dialog at 901, so the list of a dropdown in the API token, connector, move-to-server, System and profile dialogs was hidden. Dropdown lists, action menus, mention lists and the map tooltip are now above dialogs.

### Changed
- **One layer scale.** `--layer-raised` (1), `--layer-sticky` (10), `--layer-float` (20), `--layer-sheet` (800) and `--layer-sheet-panel` (801) for the assistant on narrow screens, `--layer-overlay` (900), `--layer-dialog` (901), `--layer-popover` (950) and `--layer-toast` (1000) are tokens in `01-tokens-and-shared-web.css`. Every `z-index` of the stylesheets uses one, except a single `0` behind the pipeline connectors.

### Upgrading
- Nothing to do.

## V0.2.56 (2026-10-16)

The clean-machine install is prepared. `V0.3.0` is not tagged.

### Added
- **A runbook for the run before the tag** ([mindmap/technical/clean-install.md](mindmap/technical/clean-install.md)): fourteen steps from getting the server to the desktop app, each with the guide section it follows and what you should see, a list of what has to be done before the tag, and a table for the result. A workaround is anything the guide does not say; each becomes a fix and the run is made again.
- **A smoke test that covers the new surfaces.** `scripts/smoke-test.mjs` also checks that sign-in refuses a 200 KiB body and that an unverifiable webhook answers `401`, and with an administrator reads the mail settings, sends a test mail to the address in `REPOMEMO_SMOKE_MAIL` and waits for it to leave the outbox, and checks that a remote repository can be linked (`REPOMEMO_SMOKE_REMOTE_REPO`, nothing is stored).

### Not done
- **The install itself.** It needs a second machine, a mail account and a running server. Until it passes with no workaround, and until the tests written since V0.2.46 have been run, the performance runs made and the browser passes done, `V0.3.0` should not be tagged. The page lists each of them.

### Upgrading
- Nothing to do.

## V0.2.55 (2026-10-16)

The documentation matches the code, and a check keeps it so.

### Added
- **A documentation check in CI** (`npm run docs:check`). It fails when a route of the server has no row in the API reference, when a setting the code reads is not described in the development commands, when a mindmap or the release index describes another version than the product's, when a release is done in the index and missing from this changelog (or the other way round), or when a link in the documents points at a file or a heading that is not there.
- **Upgrade notes from 0.2.x** in `UPGRADING.md`: what stays off until you turn it on (mail, remote repositories and trackers, sealed backups, containers), what changed in behaviour, and a build note about line endings.

### Changed
- **The API reference** gained the nine routes it was missing (mail settings and outbox, verification, notification preferences), the route count is right (203 in the main router) and it lists the body limits.
- **Pages that were out of date** now say what shipped: the repository sources page (remote links, credentials, webhooks), the connectors, production, console and authentication pages, which has a new section for what the 0.2.x line added. The technical mindmap's counts of migrations and tests are current.
- `scripts/list-routes.mjs` finds every route again (it had lost the ones with a body limit of their own) and reads Windows checkouts correctly.

### Upgrading
- Nothing to do.

## V0.2.54 (2026-10-16)

Performance runs for repositories and trackers.

### Added
- **A performance run for a repository of 100,000 files.** `repo_sync_at_scale` builds a repository that size, syncs it, syncs it again with nothing changed and after one changed file, and then asks the workspace what it is asked (search, the map, health). A remote repository is read by the same engine as a local one, so this measures everything after the clone.
- **A performance run for a tracker with 10,000 issues.** `ten_thousand_issues_sync_and_stay_searchable` serves that many issues the way GitHub pages them, syncs them through a connector pass after pass, times their indexing and asks the same questions.
- **[A page for performance](mindmap/technical/performance.md):** the targets, the four commands (these two and the earlier two at 50,000 passages), what each measures, and a table for the figures.

### Not done
- **The runs were not made.** They need release builds, which the automated work does not make, so no result is claimed and "the targets still hold" is open until the maintainer runs the four commands and records the figures in the page's table.

### Upgrading
- Nothing to do. The runs are tests marked as ignored: they never run in CI or with `cargo test`.

## V0.2.53 (2026-10-16)

Upgrading from 0.2.0 is tested.

### Added
- **An upgrade test in CI.** A server environment made by 0.2.0 (its database schema, the rows it wrote and the files it stored) is opened by the current code, which migrates it in place, and everything in it is read back: the people, the workspace, its folder, files and their text, the memory card with the passage and the file it cites, the verified file with its history, the task with its checklist, the comment and the system setting. Text search still finds the passage, a new workspace can be written, and opening the folder again changes nothing.
- **Released migrations are locked.** `crates/storage/migrations.lock` holds the checksum of every database migration that has shipped, and `npm run migrations:check` (in CI) fails if one is edited, removed, renumbered or left out of the lock. The server refuses an environment whose recorded checksum differs from its file, so a change belongs in a new migration.

### Upgrading
- Nothing to do. If you build the server yourself, build from a checkout with LF line endings, as CI does: a build with other line ends computes other checksums and refuses an environment another build made. The test was compiled, not run here; its data was run through all 36 migrations in SQLite.

## V0.2.52 (2026-10-16)

Feature freeze, and the fourth security pass.

### Security
- **The fourth security pass** is in [mindmap/security/2026-10-16-security-review.md](mindmap/security/2026-10-16-security-review.md). It checked mail, webhooks, remote clones, connectors, export and import, and the desktop client against the requirements set before they were built. It found no high-severity problem; three medium and four low ones are fixed in this release, and five informational notes are recorded. No high or medium finding is open.
- **Mail cannot be used to flood a mailbox.** One address is sent at most 12 messages an hour (invitations, mentions, assignments, verification links), one person causes at most 60 an hour, and nothing is queued while 1,000 messages are waiting. An invitation over the limit is still created, with its link to pass on by hand.
- **Anonymous requests are small.** Sign-in, registration, refresh, password reset, invitations, first-run setup and the two webhook receivers now take 64 KiB (webhooks 1 MiB) instead of the upload limit, which an operator can raise to 1 GiB.
- **Imports cannot be used as a bomb.** An archive that claims to unpack to more than 100 times its own size (and over 64 MiB) is refused before anything is unpacked, each file is read only up to what the archive says it holds, and the total of the sizes it declares can no longer wrap around to a small number.
- **A webhook delivery is acted on once.** A delivery id (`X-GitHub-Delivery`, `X-Gitlab-Event-UUID`, or `X-RepoMemo-Delivery` for your own sender) received in the last hour is answered `202` with `already received`, so a captured call cannot be replayed to keep a repository syncing.
- Smaller: a tracker's answer is read only up to its 20 MiB cap, the desktop app follows no redirects away from its server, and `ssh` clones set `ProxyCommand=none`.

### Changed
- The feature freeze starts: until `V0.3.0`, releases carry only the release candidate's checks and their fixes.

### Upgrading
- Nothing to do. If a script posts large bodies to sign-in or the webhooks, it now gets `413`. Generic webhook senders should send a unique `X-RepoMemo-Delivery` header per call. The fixes were compiled and their tests written, not executed or attacked; run `cargo test` and try a flood of invitations before relying on them.

## V0.2.51 (2026-10-16)

The local desktop workspace is retired.

### Changed
- **The local workspace is labelled as retired.** In the desktop app it now sits under a standing notice: it stays available for now, receives security fixes only and will be removed in a 0.3.x release. The notice offers *Move a workspace to a server* and *Connect to a server*, and the first screen says the same next to *Use the local workspace*. Nothing is removed in this release.
- **The docs describe the desktop app as a window onto a server.** `PRODUCT.md`, the README and the mindmaps no longer present the local edition as a current mode of the product.
- **Safer Markdown in the retired edition.** AI answers, summaries and memory cards in the local workspace are shown with the same Markdown rules as the web client: no images are loaded, and links name their host and open in your system browser.

### Upgrading
- Nothing to do. Move any local workspace you still need to a server with *Move a workspace to a server* (V0.2.50) before the removal release.

## V0.2.50 (2026-10-16)

A workspace from the local desktop edition can be moved to a server.

### Added
- **Move a workspace to a server.** In the local workspace of the desktop app (bottom-left), choose a workspace and *Prepare the archive*. It is written as a workspace archive, the same file a server exports, so the server's import reads it as it reads any other. Then *Go to the server to import it* (connect and sign in), *Save a copy…* or *Discard the archive*.
- **The import is checked first.** On the server side of the app the dashboard's *Import a workspace* opens with the prepared archive already checked: it shows the files, folders, memory cards with their citations, tasks and comments it holds, and writes nothing until you confirm, under the name you give. The prepared file is deleted afterwards. Your local workspace is not changed or removed.
- The archive is complete or not there: it is written under another name and renamed when whole, and a workspace with a file whose stored content is missing is refused with the first file named.

### Not moved
- AI provider settings and keys, saved searches, assistant chats, the activity feed, earlier versions of a file and repository links, as in every archive. The server indexes the files again.

### Upgrading
- Nothing to do on servers: the import is the one from V0.2.41. Importing needs an organization owner or administrator. Prepared archives older than seven days are removed from the app's data folder when a new one is made.

## V0.2.49 (2026-10-16)

The desktop app opens a RepoMemo server.

### Added
- **The desktop app is a client of a server.** On first start it asks for the server's address and shows the same pages as the browser once you sign in. `https://` is assumed; plain `http://` is accepted only for a server on your own network (this computer, a private address, `.local` or a single-word name). The app keeps the address, and only a server that answered like RepoMemo is kept.
- **Your session is kept in the system credential store** (Windows Credential Manager, macOS Keychain, the Secret Service on Linux) and nowhere else, so restarting the app does not ask for the password again. Signing out ends the session on the server and deletes the entry.
- **The app can only go where it should.** Its window can show only the bundled app, so a link or a script cannot take it to another site, and it may call only the server you chose. Links to the outside open in your system browser.
- **Change server** (in the left rail, and on the sign-in page) forgets the server and the session and restarts the app.
- **The local workspace is still there**, behind *Use the local workspace* on the first screen, until it is retired. Moving it to a server comes next.

### Changed
- In the desktop app, *Open in Word* and *Open in PDF viewer* save the file instead of opening a browser tab. The desktop app is listed as a `desktop` client under System › Overview.

### Upgrading
- Servers need nothing: they already return the refresh token in the response to clients that are not the web app.
- Developers: `cargo check -p repomemo-desktop` pulls `tauri-plugin-http` and `keyring`; `npm install` adds `@tauri-apps/plugin-http`. The app was not started in this release: try it against `cargo run -p repomemo-server`.

## V0.2.48 (2026-10-16)

The web client works on a tablet and a phone.

### Fixed
- **The page could not scroll on a narrow screen.** The page was clipped to the window at every width, which suits the wide layout (each pane scrolls on its own) but left the stacked tablet layout unreachable below the first screen. Below 900 px the page now scrolls, and only sideways overflow is cut.

### Added
- **A layout for narrow screens** (below 900 px, and a phone layout below 640 px, checked down to 375 px by reading the styles): grids fall to one column, long names and paths wrap, tables, the knowledge map and the contribution calendar scroll inside their own box, and controls are at least 40 px high on a phone.
- **The side rail folds** behind a *Workspaces and organizations* button on narrow screens, so each page starts at its top.
- **The assistant becomes a sheet** on narrow screens. It starts closed, a floating *Assistant* button opens it over the bottom of the page, and *Esc*, its close button or a tap outside closes it. The conversation is kept while it is closed. The wide-screen dock, its width and its open or shrunk state are unchanged.

### Changed
- On a phone the header keeps its buttons as icons (Profile and Sign out keep their names for screen readers), notifications sit above the floating assistant button, and dialogs and the command palette use the window.

### Upgrading
- Nothing to do. The layout was written from the styles, not seen on a screen: look at a few pages at 375 px and 768 px (browser device mode) and report what overflows.

## V0.2.47 (2026-10-16)

The web client can be used by keyboard and screen reader.

### Added
- **A frame that can be navigated.** Every signed-in page starts with a *Skip to the page* link, then the header, a labelled side rail and one main area. The browser tab names the page (*Atlas · Evidence*, *System · Audit trail*).
- **Moving to another page is announced.** A screen reader hears the page's name and keyboard focus goes to the start of the page, so it is not left on a control that is gone. Changing a tab inside a workspace keeps focus on the tab, and the first page after signing in or reloading keeps the browser's starting point.
- **A check for the rules, in CI.** `npm run a11y:check` reads every component for images without `alt`, fields and buttons without a name, click handlers on elements that are not controls, `aria-label` where it is not read, table headers without `scope` and positive `tabIndex`, and measures the contrast of the colour pairs used for text in both themes (4.5:1) and of the focus ring (3:1). It reads files only.

### Changed
- **Every field has a label and every button a name.** The note title and text, memory title and text, memory and evidence search, retrieval question, saved-search name and checklist step fields had only a placeholder.
- **Charts say what they show.** The activity timeline lists its days and figures, bars and the indexing coverage are images with their values in the name, and legends are groups.
- **The sign-in / create account switch is a pair of toggle buttons**, not tabs that did not behave like tabs. The workbook sheet tabs now follow the tabs pattern: arrow keys, Home and End, one tab stop, a labelled panel, and `scope`d table headers.
- **Notifications wait for the keyboard too.** A notification stays while a control inside it has focus, and *Esc* dismisses it.
- **Less motion on request.** With the system setting for reduced motion, transitions and animations stop (the working spinner turns slowly), and in high-contrast modes the focus ring is the system's.
- **Readable greys and status colours.** In the light theme the muted and subtle text greys and the success and warning text colours were darkened, and in the dark theme the subtle grey was lightened and the primary button's hover darkened, so each reaches 4.5:1 on every surface it is used on.

### Upgrading
- Nothing to do. This release was checked by reading files; it was not run in a browser or with a screen reader, so an axe or Lighthouse run and a pass with NVDA or VoiceOver are still worth doing.

## V0.2.46 (2026-10-16)

The audit trail can be downloaded.

### Added
- **Export the audit trail** (System › Audit trail, full system administrators). Choose JSON lines or CSV and a range of days, or none for the whole trail, and download every event of it, oldest first: its id, time, who did it (id, address and name), the action and the detail. It is for an auditor, or for keeping the trail longer than the server does.
- **The download is itself audited.** An *audit exported* event, with the range and format, is recorded before the first byte is sent, so even a download that is cut short is on record. The file stops at the moment the export began, so it does not contain its own record; the next export does.
- **`GET /v1/system/audit/export?format=jsonl|csv&from=&to=`.** `from` and `to` are days (UTC, both included) or times such as `2026-10-15T09:30:00Z` (`to` excluded). The file is streamed in pages, so a long trail does not sit in memory, and `X-Total-Count` gives the number of events. A bad format or date, or a range that ends before it starts, is refused.
- **Spreadsheet-safe CSV.** A cell that starts with `=`, `+`, `-`, `@`, a tab or a return (event details can hold text people typed) is prefixed with an apostrophe so a spreadsheet shows it instead of running it. Cells with commas, quotes or line breaks are quoted.

### Upgrading
- Nothing to do. The trail is still trimmed by `REPOMEMO_SYSTEM_AUDIT_RETENTION_DAYS`; export before it ages out if you need it longer. The production guide shows a `curl` line for a scheduled export with a `system`-scope token.

## V0.2.45 (2026-10-16)

RepoMemo can be run with `docker compose up`.

### Added
- **A `Dockerfile` with two images.** `server` is `repomemo-server` on Debian slim with `git` and CA certificates, running unprivileged with its data folder in a volume and a health check on `/health/ready`. `proxy` is Caddy with the web app built into it. LibreOffice for layout-accurate Office previews is left out to keep the image small; `--build-arg WITH_LIBREOFFICE=true` adds it.
- **`docker-compose.yml`, `docker/Caddyfile` and `.env.example`.** Copy `.env.example` to `.env`, set `REPOMEMO_HOST` and the two required secrets, and run `docker compose up -d --build`. The server is reachable only through the proxy, which gets the certificate (Let's Encrypt for a real name, its own authority for `localhost`), serves the web app with its content security policy, sends `/v1` and `/health` to the server without buffering the live event streams, and is the only address the server trusts for client addresses. Backups run every 24 hours inside the volume.
- **The web app can be built for the page's own host.** An empty `VITE_REPOMEMO_API_URL` now means "the same host" instead of falling back to `127.0.0.1:3020`, so one image works under any name. Unset still means `http://127.0.0.1:3020`.
- **`npm run container:check`** (also run by CI) compares the container files with each other and with the variables the server reads. It starts nothing and needs no Docker.
- The production guide has a new section, *In containers*: the volumes, backups with a second copy on a mount, repositories on disk, upgrading and using your own proxy.

### Upgrading
- Nothing to do for a server that is not run in containers. To move to containers, back up, restore the backup into the new environment (production guide §6) or mount the old data folder as the volume, and keep the same `REPOMEMO_JWT_SECRET` and `REPOMEMO_SECRET_KEY`.
- The images were not built or started when this release was cut; the first `docker compose up` is yours to try.

## V0.2.44 (2026-10-16)

A document that cannot be read no longer costs the server anything.

### Fixed
- **Documents are read in a process of their own.** Word, Excel, PowerPoint, PDF, OneNote and email files are now read by a child process (the server started with `--document-reader`). A file that takes too long is **killed** at `REPOMEMO_DOCUMENT_TIMEOUT_SECONDS` instead of being left running, and one that needs too much memory stops at the new `REPOMEMO_DOCUMENT_MEMORY_MB` (default 1024; `0` for no limit). A parser that crashes or overflows its stack ends the child only: the file is reported as possibly damaged and the server carries on.
- The child starts with an empty environment, so even a parser bug has no key or token to read, and it is given the file name only, never a folder of the server.
- Previews report the same reasons ("took longer than 60 seconds", "needed more than 1024 MB of memory") as indexing does.

### Added
- `REPOMEMO_DOCUMENT_READER` (`process`, the default, or `thread` for the earlier behaviour as a stopgap), `REPOMEMO_DOCUMENT_MEMORY_MB`, a *Document readings* line in System › Overview and the counter `repomemo_document_readings_killed_total`.

### Security
- Closes the accepted review finding **R2-4** (an overrunning reading was abandoned, not killed, and had no memory limit). A stack overflow or out-of-memory abort in a document parser can no longer stop the whole server.

### Upgrading
- Nothing to do. Reading a file now starts a short-lived process, which takes a few milliseconds. If readings fail with "could not be started", the server could not start its own program file: check where it runs from, or set `REPOMEMO_DOCUMENT_READER=thread` until it is fixed.

## V0.2.43 (2026-10-16)

Each backup can be copied somewhere else, and the copy is checked.

### Added
- **`REPOMEMO_BACKUP_COPY_DIR`.** Set it to a folder on another disk or a mounted network share and the server copies every backup there once it is made. The copy is written as `<name>.partial` and renamed only when it is whole.
- **The copy is verified.** Every file is hashed as it is read, written and read back from the destination; the hashes are saved as `SHA256SUMS` (`sha256sum -c SHA256SUMS` repeats the check), and the number of files and their total size must equal the backup's manifest. A copy that does not add up is removed and reported.
- **Unchanged files are not sent again.** A file whose hash matches the previous copy is linked to it, so the second location grows like the first. It keeps the same number of backups (`REPOMEMO_BACKUP_KEEP`). A sealed backup is copied sealed.
- **Failures are seen and retried.** If the share is down, the backup is kept, the job shows as failed with the reason, and the copy is tried again on every maintenance pass. System › Backups shows the second location, when each backup was checked there (or *waiting*) and a *Copy to the second location now* button (`POST /v1/system/backups/copy`). The new metric `repomemo_backup_copy_last_timestamp_seconds` can be alerted on, and the setup checks have a *Second copy of backups* line.
- Backup manifests now record `files` and `bytes`.

### Upgrading
- Nothing to do. Without `REPOMEMO_BACKUP_COPY_DIR` nothing changes. Mount the share before the server starts and give the account that runs the server write access; the folder must not be inside the backups folder. Backups made before this version have no totals in their manifest and are checked file by file only.

## V0.2.42 (2026-10-16)

Backups can be encrypted.

### Added
- **`REPOMEMO_BACKUP_KEY`** (16 or more characters). When it is set, every file of every new backup is sealed with AES-256-GCM before it is written: the database, the key file and each stored file (`*.enc`). Only `manifest.json` stays readable, and it holds nothing that can open the backup. Off by default.
- **A wrong key is refused before anything is written.** Restoring a sealed backup needs the same `REPOMEMO_BACKUP_KEY`. The server checks it against the manifest first, and says so plainly when it does not match or is not set; a file that was changed, moved or cut fails to open instead of restoring wrong data.
- **Backups stay small.** Unchanged files are still shared between backups made with the same key (a new backup keeps the salt of the one before it). A sealed file is 16 bytes larger per 64 KiB.
- **Visible in System.** Backups shows whether encryption is on and tags each sealed backup; `GET /v1/system/backups` returns `encryption_enabled` and each backup's `encryption`; the setup checks have a *Backup encryption* line that suggests the key until it is set.

### Security
- Closes the accepted review finding **R2-5** (backups were not encrypted and held `secret.key`).

### Upgrading
- Nothing to do: without the key, backups are made as before. To turn it on, set `REPOMEMO_BACKUP_KEY` and restart; the next backup is sealed, older ones stay plain until they are deleted.
- **Keep the passphrase apart from the backups, in a password manager.** It is stored nowhere in them, and without it a sealed backup cannot be restored. Changing it seals new backups only; keep the old one for as long as you keep the old backups.

## V0.2.41 (2026-10-16)

A workspace archive can be imported as a new workspace.

### Added
- **Import a workspace** on the dashboard, for owners and administrators of an organization. Choose an archive made by *Export this workspace*, **check it first** (it is read completely and nothing is written), see what it holds and any warning, name the new workspace and import it. You become its owner.
- **The check** verifies the manifest and format version, the data against its checksum, every stored file against its hash and size, and that the records agree with each other (references, folder tree, states). It also tells you if the archive is too large for the server or for a workspace's storage allowance. A damaged archive, one from a newer RepoMemo or one whose records disagree is refused with the reason, and a failed import leaves no workspace behind.
- **Everything is linked as before:** folders, each file's lifecycle and history, memory cards with their citations, tasks with checklists, comments and the workspace's AI policy, with their dates. A cited passage is found again once its file is indexed; if it cannot be found, the citation points at the whole file and the report says how many. Searching works as soon as a file is indexed, in the background.
- **Nobody is credited by account.** An archive can be written by anyone, so its authors are not believed: comments, tasks and history belong to the person who imports, and the original name is kept in the text. Links to a folder, repository or tracker are not recreated; their files arrive as plain evidence.
- **`POST /v1/workspaces/import?organization_id=&dry_run=&name=`** takes the archive as the request body. It is received into a temporary file in the data folder (not into memory) and is not subject to the request timeout.
- **`REPOMEMO_MAX_IMPORT_MB`** (default 2048) is the largest archive accepted.

### Upgrading
- Nothing to do. If a reverse proxy sits in front of the server, allow `POST /v1/workspaces/import` a body as large as your biggest archive and do not buffer it (the production guide shows the nginx location).
- Importing makes a copy; importing the same archive twice makes two workspaces.

## V0.2.40 (2026-10-16)

A workspace can be exported as one file.

### Added
- **Export this workspace** (Settings, for owners and administrators). It downloads a compressed archive (`repomemo-<name>-<date>.tar.gz`, readable with any archive tool) holding a `manifest.json`, a `workspace.json` and the original files.
- **What is in it:** the evidence with its original files (a file stored once is in the archive once), folders, each file's lifecycle state, owner, review note and history, memory cards with their citations, tasks with their checklists, comments, and the workspace's own settings. The manifest lists the counts, every file's checksum and a checksum of the data.
- **Citations travel with the card.** A citation names its file and, for a passage, its place and a fingerprint of its text, so the passage can be found again after the file is indexed on another server. Citations whose passage or file no longer exists are left out and counted in the manifest.
- **People are a name and an address,** never an account of this server. Anonymized accounts stay "Former member" with no address.
- **Nothing secret leaves.** AI provider keys, repository and tracker tokens and webhook secrets are not in the archive, and links to a repository or tracker are not carried (their files arrive as plain evidence).
- **`GET /v1/workspaces/{id}/export`** streams the archive while it is built, so a large workspace does not fill the server's memory. A workspace with a stored file missing from the server's disk is refused with a plain message (`409`) instead of being exported incomplete. The export is recorded in the workspace's activity (`workspace_exported`).

### Upgrading
- Nothing to do. Importing an archive into a new workspace comes in V0.2.41; earlier versions of a file, saved searches, assistant chats and the activity feed are not part of the archive.

## V0.2.39 (2026-10-15)

Mentions are suggested as you type them.

### Added
- **`@` suggestions in comments.** Typing `@` in the discussion of a file offers the workspace's members; typing more narrows them (by the start of a name, of one of its words or of the address, then anywhere in either). Arrow keys move, Enter or Tab chooses, Esc closes. Choosing one writes their address after the `@`, which is what a mention has always been, so they are notified when the comment is saved.
- **Only members of the workspace are suggested,** and only to people who can comment (members, administrators and owners; viewers see no suggestions). Yourself, people whose account was deactivated and anonymized accounts are left out, and at most eight are offered at a time.
- **`GET /v1/workspaces/{id}/mention-suggestions?q=`** answers with `{id, display_name, email}` for each.

### Upgrading
- Nothing to do. Comments written with a typed `@address` work as before.

## V0.2.38 (2026-10-15)

Earlier versions of a file stay readable.

### Added
- **Version history.** Uploading a new version of a file (the *New version* action on its page) keeps the content it replaces. A file with more than one version shows a *Version history* panel: each version with who uploaded it, when it became current and its size. The first upload's author is not recorded.
- **Read any version.** *View text* shows what a version said, as search reads it: the file's own text, or the text extracted from a Word, Excel, PowerPoint, PDF, OneNote or Outlook file. An image or a binary file says it has no text to show.
- **Compare versions.** *What changed* compares a version with the one before it, and *Compare with current* compares an earlier one with the latest. The comparison is line by line, with removed and added lines marked and a few unchanged lines around each change; long unchanged stretches are folded. Only the first 4,000 lines of each version are compared.
- **API:** `GET /v1/artifacts/{id}/versions` (the list, newest first), `GET /v1/artifacts/{id}/versions/{n}` (the text of version n), `GET /v1/artifacts/{id}/compare?from=&to=` (members of the workspace; the comparison counts as a heavy request).
- Migration 0036 (`artifact_versions`).

### Changed
- Earlier versions' stored content is no longer collected as unreferenced: it is kept for as long as the file exists, and counts in the workspace's storage use. Deleting the file lets it go.

### Upgrading
- Nothing to do. Files uploaded as new versions before this release have no history: their next new version starts it (the version before it is kept from then on).

## V0.2.37 (2026-10-15)

One list of what waits for a person.

### Added
- **Review queue:** a new *Review* section in every workspace lists, in one place, the files marked *needs review*, the memory cards whose evidence changed, disappeared or was retired, and the open health findings. Each item says why it is waiting and since when, and opens where it is resolved: the file's page, the card's page or Health. The oldest wait first; findings, which have no date, follow.
- **It empties as things are resolved:** the list is computed from those places each time, so marking a file verified or active again, confirming or editing the card, or acting on or dismissing the finding takes it off. It refreshes by itself when workspace activity changes it.
- **`GET /v1/workspaces/{id}/review`** (members of the workspace): `{items, evidence_count, memory_card_count, finding_count}`. Each item has a `kind` (`evidence`, `memory_card`, `health_finding`), a title, the reason, where it lives, the file to open, when it started waiting and who can resolve it (`writer` or `admin`).
- A stale memory card is listed once, as the card, and not again as a *Stale memory* finding. Findings a person has dismissed or acted on stay out, as on the Health page.
- The command palette lists the new section.

### Upgrading
- Nothing to do. No migration; nothing is stored for the queue.

## V0.2.36 (2026-10-15)

C# and PHP are read like the other languages.

### Added
- **C# and PHP files are split along their declarations.** A class, struct, record, interface, enum, function or method is not cut in half; a large class is split by method, and a namespace that holds one large class is split inside it. Each passage records its scope in its heading (`App > Store`).
- **Their symbols are searchable.** C# classes, structs, records, interfaces, enums, methods and constructors; PHP classes, interfaces, traits, enums, functions and methods. They appear in *Code symbols* in Retrieval, on the file's page and in the assistant's *Find a symbol*.
- **`.cs` and `.php` files can be uploaded** and are indexed as C# and PHP. PHP files are read with the grammar that also understands the HTML around `<?php … ?>`.

### Changed
- Namespaces, classes and other containers that hold a single, large member are now split along that member too (before, such a container was cut by size). Every language benefits.
- The indexer version is 4, so every file already indexed is indexed once more in the background after the upgrade. Passages whose text is unchanged keep their identity, their embeddings and the links that point at them.

### Upgrading
- Nothing to do. C# and PHP files already in a workspace through a repository are re-indexed in the background and gain their symbols.

## V0.2.35 (2026-10-15)

Go and Java are read like Rust and Python.

### Added
- **Go and Java files are split along their declarations.** A function, a method, a struct, an interface, a class or a record is not cut in half: small neighbours are grouped, a large class is split by method, and each passage records its scope in its heading (`Store.Get` for a Go method, `Store > lookupEntry` for a Java class). Comments stay attached to the declaration below them.
- **Their symbols are searchable.** Go functions, methods (named after their receiver type), structs and interfaces; Java classes, records, interfaces, annotation types, enums, methods and constructors. They appear in *Code symbols* in Retrieval, on the file's page and in the assistant's *Find a symbol*.
- **`.go` and `.java` files can be uploaded** (the file picker and folder uploads accept them) and are indexed as Go and Java. Repositories already read them as such.

### Changed
- The indexer version is 3, so every file already indexed is indexed once more in the background after the upgrade. Passages whose text is unchanged keep their identity, their embeddings and the links that point at them.

### Upgrading
- Nothing to do. Go and Java files already in a workspace through a repository are re-indexed in the background and gain their symbols.

## V0.2.34 (2026-10-15)

A keyboard command palette.

### Added
- **Command palette:** press **Ctrl K** (or **⌘ K**) on any signed-in page, or use the search button in the header. Type to jump to a workspace, one of the open workspace's sections (People and Workspace settings only for those who administer it), a file by its title or path, a memory card, the dashboard, notifications, your profile or, for system administrators, a System page. Arrow keys move, Enter runs, Esc closes.
- **Assistant actions from the palette:** on a workspace page the typed text can be sent as *Ask the assistant*, *Find files*, *Search content* or *Find the symbol*, or the assistant can be asked for a workspace overview. The assistant dock opens, runs the request in the current chat and shows the reply; with nothing typed the action is only selected, ready for the question.
- Files come from the open workspace's list (read when the palette opens) and memory cards from its search, both with your own access: nothing is listed that the page itself would not show you.

### Upgrading
- Nothing to do. No server change.

## V0.2.33 (2026-10-15)

Search reaches every workspace you can read.

### Added
- **Search all workspaces** on the dashboard: one box searches the evidence of every workspace the account can read. Each result names its workspace, shows the snippet with the matched words highlighted, and opens the file in its own workspace.
- **`POST /v1/search`** (`{query, workspace_ids?, lifecycle_statuses?, limit?}`; 20 results by default, at most 50). Each workspace is searched on its own: its role check is made for the caller, outdated and superseded evidence is left out unless asked for, and a workspace the caller cannot read is left out without being named, even when asked for.
- **Each workspace's AI policy decides whether the query is matched by meaning.** Where *AI features* are kept to other roles, that workspace is searched by keywords only and the query is never sent to its embedding provider; the answer says so for that workspace (`by_meaning`, `note`).
- **Merged by score.** A workspace's scores are relative to its own best match, so every workspace's best result ties at the top and the others follow by score, then by their rank in their workspace. At most 30 workspaces are read per search (`not_searched` counts the rest); one that cannot be searched is skipped with a note instead of failing the search.

### Upgrading
- Nothing to do. The search counts as one heavy request against the account's quota, not one request per workspace.

## V0.2.32 (2026-10-15)

The assistant remembers the conversation.

### Added
- **Earlier turns go with each question.** Inside a chat, the assistant is reminded of what was said before: the newest earlier turns that fit a budget of about 1,500 tokens (at most 8, each answer cut to 700 characters; a turn is kept whole or left out) are sent along with an *Ask a question* request. A follow-up such as *and what happens when it fails?* is searched together with the turn before it and the titles of the files it was about, so it finds what it refers to, and the model is shown the turns only to tell what "it" and "that file" mean, never as evidence: citations still point at the numbered passages.
- **"Summarize it" and "summarize that file"** summarize the single file the latest turn was about (a file that was found, summarized or, failing that, cited). When that turn listed several files nothing is guessed: the assistant asks which one.
- **Replies say what they read:** *read 2 earlier messages* appears in the reply's header when earlier turns went with the question (`context_turns` on the reply).
- The history is always taken from the stored conversation of the person asking. A `history` sent by a client is ignored, and a new chat starts with none.

### Upgrading
- Nothing to do. Chats that already exist are remembered from their stored turns. A follow-up that goes to the AI provider now carries up to about 1,500 more tokens of context, which counts as one request against the same hourly quota.

## V0.2.31 (2026-10-15)

Connectors follow their tracker within seconds, and behave when it pushes back.

### Added
- **Webhooks for connectors.** A workspace administrator sets one up from a connector's *Webhook* button in Settings › Issues and pull requests: the payload address and a secret are shown once. `POST /v1/webhooks/connectors/{id}` takes GitHub (`X-Hub-Signature-256`, events `issues`, `issue_comment`, `pull_request`, `pull_request_review`, `pull_request_review_comment`), GitLab (`X-Gitlab-Token`, *Issue*, *Note* and *Merge Request* hooks) and any sender that signs with HMAC-SHA256 (`X-RepoMemo-Signature`). The rules are the repository webhooks': constant-time checks, the same `401` for anything unverifiable, limits per address and per connector, one sync queued per call.
- **`Retry-After` is honoured.** A tracker that asks to wait up to 30 seconds is waited out inside the sync (twice at most). A longer wait (`Retry-After`, GitHub's `X-RateLimit-Reset`, GitLab's `RateLimit-Reset`) **pauses the connector** until then: it shows *Paused until …* with the tracker's reason, is not asked again before that (manual syncs answer `409`), and resumes by itself.
- **A refused token pauses the connector** and says so: *Paused until the token is replaced*. Background syncs stop; *Sync now* may still try once; replacing the token (tried against the tracker first) clears the pause.
- The repository webhooks now share this code and its limits; their behaviour is unchanged.

### Upgrading
- Nothing to do. The server must be reachable from the tracker for webhooks to arrive; polling stays on as a safety net.

## V0.2.30 (2026-10-15)

A file shows the issues and pull requests that name it.

### Added
- **Mentioned in / Mentions** on the page of a repository file, an issue and a pull request. A file lists the issues and pull requests whose text names it, one of its symbols, or the commit that last changed it; an issue or pull request lists the files it names.
- **What counts as a mention:** a **path** (`src/ledger.rs`, or `ledger.rs` when only one file in the workspace has that name), a **symbol in backticks** (`` `post_refund()` ``, `` `Ledger::post_refund` ``; one with more than three definitions is too common to link), and a **commit** (a hash of 7 characters or more, or a pull request's own commits) that is the last change of a file. A name that could mean several files is left alone: nothing is guessed.
- **Kept current:** mentions are recomputed when a connector syncs something and when a repository syncs, so a file added after the issue was written still gets linked. `GET /v1/artifacts/{id}/mentions` (members of the workspace) returns both directions.
- Migration 0035 (`artifact_mentions`).

### Upgrading
- Nothing to do. Links appear after the next sync of a connector that changed something or of a repository.

## V0.2.29 (2026-10-15)

GitLab joins GitHub.

### Added
- **GitLab projects** (gitlab.com or self-hosted) connect from the same place as GitHub repositories: Settings › Issues and pull requests, with the project's address (groups and subgroups included, such as `https://gitlab.example.com/platform/ledger/api`). The tracker is told by the address (a host with `gitlab` in its name is GitLab) or chosen in the new *Tracker* field.
- **The same model as GitHub:** each issue, with its notes, and each **merge request**, with its commits, inline review comments and discussion, is one searchable, citable artifact. System notes ("changed the description") are left out. Closed issues and closed merge requests are *outdated*, reopened ones active, merged ones stay active.
- **Incremental:** issues and merge requests each have their own `updated_after` cursor, oldest update first, with the same request budget per sync as GitHub.
- **The token goes in `PRIVATE-TOKEN`,** to the project's own host only; the host must be on `REPOMEMO_GIT_ALLOWED_HOSTS` like GitHub's. Routes accept an optional `provider` (`github` or `gitlab`).

### Upgrading
- Nothing to do. For GitLab, name its host in `REPOMEMO_GIT_ALLOWED_HOSTS`. A token with the `read_api` scope is enough.

## V0.2.28 (2026-10-15)

GitHub pull requests become evidence, next to the issues.

### Added
- **Pull requests are kept by the same connector** as the repository's issues (Settings › Issues and pull requests). Each one, with its description, **commits** (id, first line, author), **reviews** (reviewer, verdict, text), **inline review comments** (file and line) and conversation, is one artifact (type *pr*): searchable, citable and linkable to memory cards.
- **State:** an open pull request is active, a **merged** one stays active (it describes what is in the code), and one **closed without merging** is *outdated*, like a closed issue; a reopened one is active again. The connector counts a merged pull request with the closed items.
- **The commit ids are kept** in the item's metadata (`commits`), which is what V0.2.30 links to files and commits.
- **A request budget per sync (500).** A pull request takes five requests, so a large tracker is read over several syncs: the sync stops, says *more to fetch*, and resumes from where it stopped.
- Existing connectors read pull requests from the next sync on; the first one may need several syncs on a large tracker.

### Upgrading
- Nothing to do. A token that could read issues reads pull requests too (a fine-grained token also needs *Pull requests: read*).

## V0.2.27 (2026-10-15)

GitHub issues become evidence.

### Added
- **Settings › Issues**: connect a GitHub repository's issues (github.com or GitHub Enterprise Server) to a workspace, with an optional read-only token (needed for a private repository or a higher rate limit). *Check access* shows the repository and its open issues before anything is stored.
- **Each issue, with its comments, is one artifact** (type *issue*): title, state, author, labels, assignees, link, body and discussion as markdown. It is indexed like any file, so it is searchable, citable in Ask answers and linkable to memory cards.
- **Incremental syncs:** each sync asks the tracker only for what changed since the last one, oldest first, so an interrupted sync resumes. It runs when connected, on *Sync now*, and on the repository polling interval. Pull requests listed among the issues are left alone for now.
- **Closed issues are *outdated*;** a reopened one is active again; a *verified* issue that changed is flagged *needs review*.
- Routes `GET/POST /v1/workspaces/{id}/connectors`, `POST …/connectors/check`, `GET/DELETE /v1/connectors/{id}`, `POST …/sync`, `PUT …/token`. The token is stored sealed with the repository tokens and never returned.
- **Outside rules:** the API host must be on `REPOMEMO_GIT_ALLOWED_HOSTS` (add `api.github.com`, or the Enterprise host; unset, connectors are off), `https` only (plain `http` for this machine only), redirects and next-page links to other hosts are not followed, answers are limited in size and time.
- Migration 0034 (`connector_items`).

### Upgrading
- Nothing to do. To use connectors, add `api.github.com` (or the Enterprise host) to `REPOMEMO_GIT_ALLOWED_HOSTS`.

## V0.2.26 (2026-10-15)

A citation can open the text it was made from.

### Added
- **View at commit.** A citation of a repository file names the commit its text is from. Next to it, in Ask answers, AI overviews and the assistant's evidence, *View at 1a2b3c4* opens the file as it was at that commit, labelled with the commit, its summary, author and date, and says whether it is **not the current version** (and which commit the workspace holds now). The cited lines are marked.
- **`GET /v1/artifacts/{id}/at/{commit}`** (any member of the workspace) returns that text with the commit's details. It reads the server's copy of the repository (the folder, or the partial clone of a remote one) with `git cat-file`: the commit must be a hex id of 7 to 64 characters, files over 1 MiB and files that are not text are refused, and a commit that is no longer in the copy (a force-push) or a file over the clone's size limit says so.

### Upgrading
- Nothing to do.

## V0.2.25 (2026-10-15)

A repository's `CODEOWNERS` shows on its files.

### Added
- **Owner hints from `CODEOWNERS`.** Each sync reads the repository's `CODEOWNERS` (`.github/CODEOWNERS`, `CODEOWNERS`, `docs/CODEOWNERS` or `.gitlab/CODEOWNERS`, the first found) and shows the owners it names for a file on the file's page: *CODEOWNERS suggests @payments, lead@example.com*, next to where the review owner is set. It is a hint only: the handles are names on the host, not people of the workspace, and nothing is assigned or notified from it.
- **Matching follows what GitHub and GitLab document:** the last matching pattern wins; `*` and `?` stay inside one path segment and `**` crosses them; a pattern with a `/` in front or in the middle is anchored to the root and one without matches at any depth; a pattern ending in `/` (or in a plain name) covers the directory it names; a pattern with no owners clears ownership; GitLab `[Section]` lines and their default owners are understood. Negation and escapes are not.
- Files whose owners change are updated on the next sync, including when `CODEOWNERS` is edited or removed. Files over 3 MiB of `CODEOWNERS` are ignored.

### Upgrading
- Nothing to do. Owners appear after the next sync of each repository.

## V0.2.24 (2026-10-15)

Who last changed a file, and when.

### Added
- **Last author and date for every repository file**, taken from the repository's history. They show on the file's page (*last changed by Ana · 3 Mar 2026 in 1a2b3c4*), in the repository's file list and under each search result that comes from a repository file.
- **One `git log` pass per sync** over the latest 20,000 commits finds the newest commit that touched each path; it also corrects the author after a revert, which changes the author without changing the file. Files untouched for more than 20,000 commits show no author. Renames count as a change of the new path.
- The author is the commit's *author* (not the committer) and is kept with the file (migration 0033: `last_author`, `last_authored_at`, `last_change_commit` on `repo_files`, and in the artifact's metadata). It is filled in by the first sync after upgrading.

### Upgrading
- Nothing to do. Run a sync (or wait for the next poll) to fill in authors for repositories linked before.

## V0.2.23 (2026-10-15)

A push can ask for a sync at once.

### Added
- **Push webhooks** for any linked repository (local or remote). A workspace administrator sets one up in Settings › Repositories › Edit › *Push webhooks*; the payload address and a secret are shown once (*Make a new secret* retires the old one, *Turn off* removes it). Hosts call `POST /v1/webhooks/repositories/{id}` when someone pushes, and the repository is synced within seconds instead of at the next poll.
- **Three senders are understood:** GitHub (`X-Hub-Signature-256`, an HMAC-SHA256 of the body; `ping` is answered, `push` syncs), GitLab (`X-Gitlab-Token`, the secret itself, with *Push Hook* events) and any other sender that signs the body with HMAC-SHA256 in `X-RepoMemo-Signature: sha256=<hex>`.
- **What makes a call count:** it must be signed right. Every comparison is constant time; a call that is unsigned, signed wrongly or for another body, or for a repository that does not exist or has no webhook, is answered `401` the same way. Failed calls are limited per client address (20 a minute) and verified ones per repository (60 a minute). Pushes to tags, to a branch the link does not follow, and events that are not pushes are accepted and ignored. One push queues one sync; a sync already running absorbs it.
- The repository shows whether a webhook is on and when a signed call last arrived. Polling stays on as a safety net.
- The secret is stored sealed like provider keys (migration 0032, `repo_webhooks`) and is never returned again.

### Upgrading
- Nothing to do. The server must be reachable from the host that sends the webhooks (`REPOMEMO_PUBLIC_URL` gives the full payload address).

## V0.2.22 (2026-10-15)

A remote clone can no longer take unbounded room or time.

### Added
- **A size cap and a time cap on every clone and fetch of a remote repository.** The size of the server's copy is measured twice a second while git runs; a transfer that goes over the cap, or past the time limit, is stopped at once and the person is told which limit it hit. A stopped clone is removed; a stopped fetch leaves the previous copy in place and removes its half-written packs.
- **Two settings** in System › Settings › Limits, changeable while the server runs: *Repository clone size* (`REPOMEMO_REPO_CLONE_MAX_MB`, 2048 MiB by default, `0` for no cap) and *Repository clone time* (`REPOMEMO_REPO_CLONE_TIMEOUT_MINUTES`, 30 by default). Before, the time cap was a fixed 30 minutes and there was no size cap.
- **The copies count in the workspace's storage allowance** like its files do: uploads and new notes are refused when files and copies together would go over, and a sync is not started once they are at the allowance.
- **System › Overview › Storage** shows how many remote copies the server keeps, their total size, the cap each one stops at, and the five largest.

### Upgrading
- Nothing to do. A workspace with large remote repositories may now be at its allowance: raise *Storage per workspace* or the repositories' branch rules, or remove a link (its copy is deleted with it).

## V0.2.21 (2026-10-15)

Link a repository by its `ssh` address, with a deploy key.

### Added
- **Repository links accept `ssh://git@host/owner/repo.git` and `git@host:owner/repo.git`**, on a host listed in `REPOMEMO_GIT_ALLOWED_HOSTS` (`host:port` for a port other than 22). The link needs a **deploy key** (a private key without a passphrase, read-only on the host) and the **host's public key**, as printed by `ssh-keyscan -t ed25519 host`.
- **The host key is pinned:** the connection only goes through to a host that presents exactly the pasted key. An unknown or changed key is refused, with a message that says so; nothing is trusted on first use, and the server's own `known_hosts` and `ssh` configuration are not read.
- **The deploy key never reaches a command line or the log:** it is stored sealed (like tokens), written to a private file that exists only for the length of one git command and is then removed, and passed to `ssh` as `-i`. `ssh` runs with the agent, passwords, forwarding and the user's configuration off. The host key is public and kept with the link.
- `PUT /v1/repositories/{id}/deploy-key` replaces the key (and, when given, the pinned host key); the new key is tried against the host before it replaces the old one. A key cannot be removed from an `ssh` link, only replaced. The change is recorded in the workspace activity.
- Clone, fetch, polling for new commits and everything after them work as for `https`.

### Upgrading
- Nothing to do. `ssh` links need an `ssh` client on the server's `PATH` (it comes with git on Windows and with OpenSSH elsewhere).

## V0.2.20 (2026-10-15)

Link a repository by its `https` address.

### Added
- **Repository links accept an `https` address** (such as `https://github.com/acme/app.git`) next to a folder on the server, in Settings › Repositories and in `POST /v1/workspaces/{id}/repositories` and `/check`. The host must be on `REPOMEMO_GIT_ALLOWED_HOSTS` (V0.2.19). *Check access* asks the host once (`git ls-remote`) and shows the branch and commit; nothing is cloned until the first sync.
- **An optional read-only token** for a private repository (`token`, and `token_username` when the host wants an account name). It is stored sealed, bound to its link, and never returned: the repository only shows *with a token* or *no token*. `PUT` and `DELETE /v1/repositories/{id}/token` replace or remove it; a new token is tried against the host before it replaces the old one. Both are recorded in the workspace activity.
- **How it is fetched:** the server keeps a bare, partial clone of the branch under `remote-repos/` in its data folder (blobs over 1 MiB, which are never indexed, stay on the host). git runs with an empty environment, `https` only, no redirects, object checks on, no submodules and no hooks; the token reaches it through the server's own program as `GIT_ASKPASS`, never in an address or an argument, and git's messages are shown with it removed. Fetching, sync, renames, overview, indexing and polling for new commits (one `ls-remote`, no fetch) work as for a local repository.
- **git must be patched** (2.50.1 or later, or a fixed maintenance release) for the CVE-2025-48384 submodule flaw; an older git is refused with that message.
- Removing a link removes the server's copy.

### Not yet
- `ssh` addresses are refused with a message: deploy keys come in V0.2.21. Clone size and time limits are V0.2.22; until then a transfer is stopped after 30 minutes.

### Upgrading
- Nothing to do. Remote links need `REPOMEMO_GIT_ALLOWED_HOSTS`. The server's own program doubles as git's credential helper, so no extra file is installed.

## V0.2.19 (2026-10-15)

Groundwork for linking remote repositories. Nothing is linked yet: that is V0.2.20.

### Added
- **`REPOMEMO_GIT_ALLOWED_HOSTS`**: the hosts remote repository links may use, comma separated (`github.com`, `git.example.com:2222`, `*.corp.example`, or `*`). **Unset turns remote links off**, like `REPOMEMO_REPO_ROOTS` does for local ones. It shows in the server checks (*Remote repository hosts*) and in System › Overview.
- **One address check, before any network call:** only `https` and `ssh` (including the `git@host:owner/repo` form); no credentials in the address (a token or key is entered separately); no `git://`, `http://`, `file://`, local paths, git transports such as `ext::`, options, whitespace or control characters; a valid host that is on the list. Link-local and cloud-metadata addresses are never allowed, and a private address must be named in the list. A refusal says which hosts the server allows, whether or not the address exists.
- **A credential store for each repository link**, for a read-only token (`https`) or a deploy key (`ssh`): sealed with the same key as AI provider keys, bound to its link (a sealed value copied onto another link does not open), removed with the link. It is never returned by the API or written to the log, and the opened value cannot be printed by accident.
- Migration 0031 (`repo_credentials`).

### Upgrading
- Nothing to do. Remote links stay off until `REPOMEMO_GIT_ALLOWED_HOSTS` is set. git resolves host names itself, so for names that point at private addresses the list is advisory: enforce it with an egress rule or a proxy on the server.

## V0.2.18 (2026-10-15)

Open registration no longer reveals who has an account.

### Fixed
- **Creating an account from the sign-up form answers `202` with the same words for a new address and for one that already has an account**, when registration is open, mail is set up and `REPOMEMO_PUBLIC_URL` is known. Before, a known address was refused with `409`, which told anyone which addresses had accounts (review M4, residual).
- **A new address** gets its account, created with the password just chosen, and a confirmation link by mail; the person then signs in (the sign-up form switches to sign in and shows the message). **A known address** is not touched: its owner gets a message that someone tried to register with it, at most twice an hour, with a link to sign in or to *Forgot your password?*. Deactivated accounts get no message.
- The password is hashed in both cases, so the time taken is close; it is not made identical.
- Registrations with an invitation, the first account of a server, and servers where mail is not set up (or has no public address) are unchanged: they answer `201` with a session, and `409` for a known address, since nothing else can carry the difference.

### Upgrading
- Nothing to do. A script that registers accounts on an open server with mail on now gets `202` and no session: it signs in afterwards. Registration stays closed by default.

## V0.2.17 (2026-10-15)

Anonymizing a deactivated account.

### Added
- **System › Users › Anonymize account** for a deactivated account (system administrators), and `users anonymize <email> --yes` in the console; `POST /v1/system/users/{id}/anonymize` with `{confirm}`, the account's address typed to confirm. It cannot be undone, and is audited as `user_anonymized` by the account's id.
- **What goes, for good:** the address, the name, the password, the profile picture, every membership, the notices, notification preferences, the digest record, assistant conversations, the mail waiting for the address (sent ones lose the address and subject) and the invitations made out to it. Tasks assigned to it become unassigned.
- **What stays:** everything it wrote (comments, tasks, notes, lifecycle changes), credited to **Former member**. The account row stays, so nothing that points at it blocks anything (risk 12 closed). Its address becomes an unreachable placeholder.
- **Summaries and audit details** that name the address, or the name on the account's own actions, are rewritten (*a former member*, *Former member*).
- An anonymized account is shown as *Anonymized* in System › Users and cannot be reactivated.
- Migration 0030 (`users.anonymized_at`).

### Upgrading
- Nothing to do.
- **What anonymizing does not reach:** log files and backups made earlier keep the address, as do other people's words and documents that mention the person. Log files age out with their retention setting.

## V0.2.16 (2026-10-15)

A deactivated account cannot sign in or use a token, and comes back when reactivated.

### Added
- **System › Users › Deactivate account / Reactivate account** (system administrators), and the console commands `users deactivate <email> --yes` and `users reactivate <email> --yes`; `POST /v1/system/users/{id}/deactivate` and `/reactivate` (a session or a `system` token). Both are audited as `user_deactivated` and `user_reactivated`.
- **What deactivating ends, in one step:** every session and refresh token, every personal access token, the open event streams of the account, its pending file links, the password reset and verification links waiting for it, and the invitations it sent. Access tokens already issued stop working at once.
- **What stays:** everything the account wrote and every membership. It shows in System › Users as *Deactivated*, with no active sessions.
- **Refused:** deactivating your own account, the last active system administrator, and an account that still owns an organization or a workspace (the answer names them: transfer the ownership first).
- **Sign-in says why**, but only after the right password was given: *This account has been deactivated*. A wrong password answers as for any account, so nothing is revealed about who has one. No password reset link is created or mailed for a deactivated account, and it gets no notification mail or digest.
- A deactivated account cannot be added to an organization or a workspace until it is reactivated.
- Migration 0029 (`users.deactivated_at`).

### Upgrading
- Nothing to do. Every account stays active. Reactivating does not bring back the sessions, tokens or invitations that ended.

## V0.2.15 (2026-10-14)

A daily digest by email.

### Added
- **Profile › Notifications › a summary by mail, once a day** (off until asked for; `daily_digest` with `mail` or `none`). A person who asked for it gets one message that counts the in-app notices they have not read since their last summary and lists the newest ten, with a link to the notifications page.
- **At most one a day, and only when something happened:** a person is not sent another within 24 hours of the last, and no message is made when nothing unread came after it (the next one then covers everything since the last summary). People whose address is unconfirmed get none.
- It goes through the outbox like every mail, so it needs mail set up and `REPOMEMO_PUBLIC_URL`. A person who wants the digest instead of single mails sets *a task is assigned* and *a mention* to *in the app only*.
- Migration 0028 (`mail_digests`: when each person was last sent one). Asking for the digest wakes the sender at once.

### Upgrading
- Nothing to do. Nobody gets a digest until they turn it on.

## V0.2.14 (2026-10-14)

Notification preferences.

### Added
- **Profile › Notifications:** for each kind of notice a person can choose *in the app and by mail* (the default), *in the app only*, *by mail only* or *nowhere*. The kinds that can be chosen are a task assigned to you and a mention in a comment; both the in-app notice and its mail honour the choice, decided when the event happens.
- **Security notices cannot be turned off:** password reset notices always reach the app and the mail (`PUT` for them is refused).
- `GET` and `PUT /v1/profile/notification-preferences`, and migration 0027 (`notification_preferences`; no row means both).
- Notification mails now point to the profile for the choice.

### Upgrading
- Nothing to do: everyone starts at *in the app and by mail*, which is what V0.2.12 did.

## V0.2.13 (2026-10-14)

Email verification.

### Added
- **Unverified accounts are held back.** When registration is open and mail is set up, an account made without an invitation can sign in but cannot create an organization or a workspace until it has used the link mailed to its address (`403` with the reason). The link works once and for 24 hours; the profile page can send another (three an hour). Registration stays closed by default, so this only applies once an operator opens it.
- **What proves an address:** a verification link, or accepting an invitation sent to it (an invitation counts as verification, also for people who register from one). The first account of a server is verified.
- **Changing the address** (Profile › Email address, `PUT /v1/profile/email`) asks for the current password, refuses an address another account uses, and starts the new address unverified with a link sent to it. A link made for the old address stops working.
- **Nothing is withheld without mail:** when mail is not set up nobody could verify, so accounts keep working as they did; the profile still shows whether the address is confirmed.
- The page `/verify-email#<token>`, `POST /v1/auth/verify-email`, `POST /v1/profile/email/verify`, and migration 0026 (`users.email_unverified_at`, `email_verifications`). Every account that exists stays verified.

### Upgrading
- Nothing to do. If you open registration and set up mail, people who register get a confirmation mail and are held back from creating things until they confirm.

## V0.2.12 (2026-10-14)

Mentions and task assignments are mailed.

### Added
- **A mail with each notice that asks for attention:** being assigned a task and being mentioned (`@email`) in an evidence comment. The message says who did it, what it is about and carries a link to the task board or the file. It goes through the outbox, only when mail is set up and `REPOMEMO_PUBLIC_URL` is known.
- **Once per event, never to the person who caused it:** a notice and its mail are made in the same step, so changing a task without changing its assignee sends nothing, and assigning a task to yourself or mentioning yourself sends nothing. Titles are cleaned for the subject, so a crafted title adds no header.
- The in-app notices are unchanged and still the record; the mail says so. Choosing, per kind, what arrives where comes with V0.2.14.

### Upgrading
- Nothing to do. People in workspaces start receiving these mails as soon as mail is switched on.

## V0.2.11 (2026-10-14)

"Forgot your password?" by email.

### Added
- **Get a link by email** on the sign-in page, and `POST /v1/auth/forgot-password {email}`. The answer is the same for every address: the same status (`202`) and the same words whether the address has an account or not, whether mail is set up or not, and whether the address's limit was reached; the work happens after the answer, so the time it takes says nothing either. Only the sign-in limits of the client address can refuse (`429`).
- **When the address belongs to an account and mail is set up**, a link goes through the outbox: one-time, valid for **1 hour** (administrators' links stay at 24 hours), and using it ends every session of the account, lifts lockouts and revokes personal access tokens, as every reset link does. An earlier unused link of the account stops working. The account also gets an in-app notification (`password_reset_issued`).
- **Limits:** at most three mails an hour per address (past it, nothing is sent and the answer does not change), on top of the per-client-address sign-in limits.

### Upgrading
- Nothing to do. Without mail set up (System › Settings › Mail) and `REPOMEMO_PUBLIC_URL`, nothing is sent and the page still says an administrator can issue a link in System › Users.

## V0.2.10 (2026-10-14)

Invitations are sent by email.

### Changed
- **Creating an invitation mails it.** When mail is set up (V0.2.9) and the server knows its public address (`REPOMEMO_PUBLIC_URL`), the invited address receives a message that names who invited them, where to, in which role, the link and the date it expires. It goes through the outbox, so a slow mail server never holds the request. The link is still returned and shown to copy: the answer's new `mail` field says what became of the message (`queued`, `not_set_up`, `no_public_address`, `failed`), and the page says so.
- **A crafted name adds nothing to the mail.** The inviter's and the workspace's or organization's names are cleaned for the subject (control characters, such as a line break, become spaces; long names are cut), and the message builder refuses any header value with a control character anyway.
- The outbox listing never shows a body, and a sent message's body is emptied, so a link in a mail is not kept.

### Upgrading
- Nothing to do. Without `REPOMEMO_PUBLIC_URL` no link can be put in a mail: set it (the production guide already asks for it).

## V0.2.9 (2026-10-14)

Mail delivery: the groundwork every mail feature after it stands on.

### Added
- **System › Settings › Mail** (system administrators; app administrators read): the mail server, port, connection security (TLS from the first byte, or STARTTLS), user name, password, sender address and name, and a switch. The password is stored sealed like AI provider keys and is never returned: the page only knows that one is saved, and leaving the field empty keeps it. Saving is audited (`mail_settings_changed`, without the password) and needs a signed-in session or a `system` token.
- **An outbox with retries.** Features put messages in `mail_outbox` and return; one background task hands due messages to the server. A temporary refusal or a network problem is tried again after 1, 5 and 30 minutes, 2 and 6 hours, then the message is marked failed with the reason; a permanent refusal fails at once. A message that was sent or given up on keeps no body (it can hold a one-time link), and finished messages are removed after 30 days. Messages a stopped server was sending are due again at the next start. A slow or dead mail server therefore never holds a request.
- **Send test mail** (page, `POST /v1/system/mail/test`, console `mail test <address> --yes`): queues a message to yourself or an address and answers at once; the result is on its outbox entry.
- **Console:** `mail status` (server, sender, outbox counts) and `mail test`.
- **The new `repomemo-mail` crate**: a plain-text message builder in which no header can carry a line break or other control character (a crafted name or subject is refused, not turned into a header), non-ASCII text is encoded, and an SMTP client that speaks TLS only. It checks the certificate against the public roots plus `REPOMEMO_SMTP_CA_FILE`, resolves the name itself and refuses link-local, cloud-metadata and private addresses unless the host is in `REPOMEMO_SMTP_ALLOWED_HOSTS`, and has a timeout at every step. A server that does not offer STARTTLS is refused rather than used in plain text, and the password is never sent before TLS.
- A server check *Outgoing mail* in the setup wizard, and migration 0025.

### Upgrading
- Mail is off until an administrator sets it up. Allow the server's outbound connection to the mail port; a mail server on a private address needs `REPOMEMO_SMTP_ALLOWED_HOSTS`.
- Nothing sends mail yet: invitations and reset links are still passed on by hand until V0.2.10 and V0.2.11.

## V0.2.8 (2026-10-14)

The knowledge map and Workspace Health recompute only what a change touched.

### Changed
- **A change to one file no longer recomputes the workspace.** The map and the health check used to read every vector of the workspace again whenever anything in it changed. They now keep, in memory, each file's passage counts and its direction in meaning space, and each comparison of files, and redo only the files written since: one edited file costs its own passages and one row of comparisons. The two share what they read, so opening the map warms the health check.
- At 50,000 passages (a debug build, so every figure is an upper bound): a full computation of the map and health took 5.0 s, the same after one file changed took 233 ms (4.6%), and asking again with nothing changed took 0.2 s. `cargo test -p repomemo-api --release map_and_health_at_scale -- --ignored --nocapture` now prints the third figure and fails when a one-file change costs a tenth or more of the full run.
- The answers are unchanged: a test compares the map and the health after a change with those of a cold computation. The similarity threshold is found by selection instead of sorting every pair.

### Added
- `StorageEngine::vector_stamps`, a journal of which file's passages or vectors were written, and targeted reads of passage counts and centroids for named files. Nothing is stored: after a restart, with another embedding model, or for a workspace whose centroids would take over 64 MiB, the first request simply does the whole work once.

### Upgrading
- Nothing to do. No migration.

## V0.2.7 (2026-10-14)

Workspace commands in the admin console.

### Added
- **`workspace reindex <workspace> --yes`**: re-indexes the files of a workspace that the current indexer has not indexed, as a background job (the same pass as `POST /v1/workspaces/{id}/index`, which answers `202` since V0.2.6). The dry run says how many files it would take.
- **`repo sync <repository> --yes`**: syncs a linked repository now, as a background job. The dry run shows its state and when it last synced; a repository that is syncing is a `409`.
- **`embeddings rebuild <workspace> --yes`**: removes a workspace's stored vectors, whatever model made them, and embeds every passage again with its embedding provider. The dry run reports the vectors it would remove, the passages, the model, the provider calls and whether that exceeds the workspace's daily allowance of background AI calls; a workspace without an embedding provider, or with a pass already running, is refused. Search by meaning is partial until the pass has run; keyword search is not affected.
- **`workspace list` and `repo list [--workspace w]`** to find what the commands above take. A workspace or repository is given by its id, or by its name when only one has it.
- Each of these is for **system administrators** (app administrators are refused), is a dry run without `--yes`, and leaves a `console_command` line with the exact command in the audit trail. They work from `repomemo-console` and from System › Console.

### Upgrading
- Nothing to do. `StorageEngine::clear_workspace_embeddings` and the core's `embedding_rebuild_plan`, `clear_embeddings` and `stale_artifact_count` back the new commands.

## V0.2.6 (2026-10-14)

Manual re-indexing no longer holds a request open.

### Changed
- **`POST /v1/artifacts/{id}/index` and `POST /v1/workspaces/{id}/index` answer `202 Accepted`** with the running job instead of waiting for the indexing to finish. The work continues in the background; the job's status, progress (files done out of the stale ones, for a workspace) and failure reason are on `GET /v1/jobs/{id}` and on the event streams, and the embedding and the activity entry follow when it completes. A workspace pass can be stopped with `POST /v1/jobs/{id}/cancel`.
- **One whole-workspace pass at a time:** starting a second while one runs answers `409` with the reason.
- The chunks dialog (workspace administrators, on a file's page) follows the job and shows its failure reason, so *Re-index* behaves as before from the person's side.

### Upgrading
- Scripts that call these two endpoints and read the job in the answer must now expect `status: "running"` and poll `GET /v1/jobs/{id}` until it is `completed`, `failed` or `cancelled`. The desktop app is unaffected.

## V0.2.5 (2026-10-14)

The stylesheets, split.

### Changed
- **`styles.css` (6,242 lines) and `blueprint-refinement.css` (1,220) are twelve files in `src/styles/`**, none over 900 lines, imported in their original order by `styles/index.ts` (`main.tsx` imports it). The order is the cascade: the files are successive design passes that override one another, so they are split where the passes and their sections already were (tokens and shared web mode, sidebar, workbench, forms, import and search, artifact detail, the three evidence-instrument blocks, the neutral research canvas, the quiet evidence desk, the blueprint refinement).
- **47 dead rules removed** (about 150 lines): rules whose classes appear nowhere in the web client, the Rust sources or the HTML (old list and member rows, the pipeline marks and recent lists of the retired local screens, a brand mark and a route mark). The content of every other rule and its position are unchanged, so nothing looks different.

### Added
- `npm run styles:check` (also in CI) fails when a stylesheet passes 1,000 lines or `styles/index.ts` does not import every file once, in order.

### Upgrading
- Nothing to do.

## V0.2.4 (2026-10-14)

Rendering tests for the two pages a person opens from a link.

### Added
- **Tests of the invitation page** for each state: being checked; valid for someone with an account (sign in, join, continue to the workspace); valid for someone without one (create the account with the invited address and the invitation as proof); signed in as the invited address (one button to join); signed in as someone else (sign out, no way to join); used, expired and withdrawn links with the server's reason; no reason given; joining failing after signing in (the page stays so it can be retried); the destination of an organization invitation.
- **Tests of the password reset page**: being checked; valid (the account is named, two fields); saving signs this browser in; two different passwords are never sent; the server refusing a password keeps the form; used and expired links; no reason given.
- 51 web tests in all. No product code changed.

### Upgrading
- Nothing to do.

## V0.2.3 (2026-10-14)

The remaining web pages, on the query cache.

### Changed
- **No page loads its own data any more.** The dashboard, profile (with the tasks assigned to you), notifications, evidence detail (file, comments, lifecycle and its history, chunks) and memory card pages read their data through the cache that the workspace page already used. A page you opened before shows what it had at once and refreshes it in the background; details and personal pages always refetch when they open.
- **Live events refresh them.** The evidence page marks its file, comments or lifecycle stale when someone else changes them or indexing finishes, through the cache instead of its own fetches. The memory card page now listens to the workspace stream too: another person editing the card, or evidence changing (which can make a card *need review*), refreshes it once per burst, and a dropped connection refreshes it on reconnect.
- Entries of the evidence and memory card pages live under their workspace in the cache, so a full reload of the workspace page refreshes them with everything else, and the file list and member list are shared with the workspace page instead of fetched again.

### Added
- `staleTime` on `useServerState`, `accountKeys` for what belongs to a person rather than a workspace, and two tests of the cache (32 web tests).

### Upgrading
- Nothing to do.

## V0.2.2 (2026-10-14)

The workspace page, split.

### Changed
- **One component per section.** `SharedWorkspaceDetail` (1,089 lines) is now the shell (320 lines: live events, loading and the layout) and `src/web/workspace/` holds `OverviewSection`, `EvidenceSection`, `RetrievalSection`, `MemorySection`, `TasksSection`, `PeopleSection` and `SettingsSection` (the largest, evidence, is 371 lines). Sections read their server data through the hooks of `workspace/data.ts`, which share the query cache, and receive what every section needs (who is looking, what they may do, the busy flag, a reload) as one `WorkspacePage` object.
- Nothing changes on screen. Filters, drafts, results and the open folder are still there when a person leaves a section and comes back (`sectionState`); they start clean in another workspace, as the page already reset its folder there.
- Tasks and People read the member list when they open (it used to be fetched ahead for every section); the page still reads it ahead, so the list is there at once.

### Added
- Tests of the section state (30 web tests).

### Upgrading
- Nothing to do.

## V0.2.1 (2026-10-14)

Operations: the production guide and the server checks, corrected against the code.

### Changed
- **Server checks say what is missing.** The setup wizard's checks gain *Public address* (HTTPS, which turns HSTS and the `Secure` cookie on) and *Automatic backups*, and *Client addresses* warns when the address is HTTPS but `REPOMEMO_TRUST_PROXY` is not set, because every person would then share the proxy's address for limits and lockouts. Advice appears only where something is missing. The LibreOffice hint names `REPOMEMO_SOFFICE`.
- **The production guide** lists the checks, adds `REPOMEMO_BACKUP_INTERVAL_HOURS` to the settings that must be right, and drops the "to be followed in V0.1.76" status. Every variable, default, route and metric it names was compared with the code: no other difference was found.

### Upgrading
- Nothing to do. A server without HTTPS or backups shows two new warnings in the setup checks.
- The clean-machine install (R6) itself is still the maintainer's: anything it finds becomes the next fix.

## V0.2.0 (2026-10-14)

**Team-ready.** The first minor release: a server a team can install, expose and trust with its knowledge. It is V0.1.76 with the version number changed; no feature was added after the candidate and the candidate's checks found nothing to fix.

What 0.2.0 promises, and the release that delivered each part:

- **Secure by default** (V0.1.64–V0.1.69, V0.1.76): registration is closed unless an operator opens it, invitations onboard people, refresh tokens live in an `HttpOnly` cookie, the browser runs under a strict content security policy, the server only reaches hosts it is told about, and three security passes are closed in writing.
- **Checked on every commit** (V0.1.66, V0.1.71): CI, one version source, formatting, lints and dependency audits.
- **Deployable and recoverable** (V0.1.75): a production guide, backups that restore into a new environment, quotas and a disk floor, OpenMetrics.
- **Memory stays true** (V0.1.72, V0.1.73): cards that say when their evidence changed, search that can put trusted evidence first, symbol search, ownership transfer.
- **Changeable** (V0.1.70, V0.1.71): the web client and the server are split into modules with tests.
- **Fast enough to trust** (V0.1.74, V0.1.76): measured at 50,000 passages in a release build.
- **Upgradable** (V0.1.76): environments from V0.1.40 and later upgrade in place; see [UPGRADING.md](UPGRADING.md).

### Upgrading
- Nothing beyond [UPGRADING.md](UPGRADING.md). Entries V0.1.66–V0.1.76 below are the detailed release notes of this version.
- The version is `0.2.0` in `Cargo.toml`, both `package.json` files, `tauri.conf.json` and the lock files (`npm run version:check`). The tag `v0.2.0` is created by the maintainer after the clean-machine install (R6); until then the build is the release candidate.

## V0.1.76 (2026-10-14)

Release candidate for 0.2.0. **Feature freeze:** from here to 0.2.0 only fixes from this candidate go in.

### Security
- **Third security pass** over everything changed since V0.1.65 ([review §12](mindmap/security/2026-10-10-security-review.md#12-third-pass-2026-10-14-release-candidate-v0176)): no high and no medium finding. Two low findings are fixed: **repository syncs obey the storage allowance and the disk floor** (a linked repository could grow a workspace past them), and **the key file in a backup is owner-only on Windows**. Four low-impact items are accepted in writing.
- The review's status table is closed: every finding is fixed, mitigated or accepted.

### Added
- **An upgrade test.** The schemas of V0.1.40, V0.1.52 and V0.1.65 are rebuilt from the migration files, filled with data, and opened by the current engine: migrations apply, data and full-text search survive, and the integrity and foreign key checks pass. It runs with `cargo test`, so CI runs it.
- **Performance runs** for the map and health (`cargo test -p repomemo-api --release at_scale -- --ignored --nocapture`) next to the storage one. Results at 50,000 passages, release build: meaning search p95 112 ms (first search, which builds the index, 6.0 s), keyword search p95 63 ms, figures p95 6 ms, file query 1.4 ms, knowledge map 0.5 s and health 0.4 s on first load.
- **Documentation:** [UPGRADING.md](UPGRADING.md), the generated [API reference](mindmap/technical/api-reference.md) (`node scripts/list-routes.mjs`) and the [authentication and authorization](mindmap/technical/auth-and-authorization.md) page.
- `node scripts/smoke-test.mjs <address>` checks a running server the way a clean install should behave (health, headers, the web app, and with an administrator's credentials the overview, metrics and a backup).

### Upgrading
- Nothing to do beyond [UPGRADING.md](UPGRADING.md). Repository syncs are refused, with the reason in the repository's activity, while the workspace is at its storage allowance or the disk is under its floor.

## V0.1.75 (2026-10-14)

Operations: what an operator needs before trusting the server.

### Added
- **Backup and restore.** System › Jobs & maintenance › **Back up now**, the console (`backup list`, `backup create --yes`, `backup restore <backup> <environment> --yes`, `backup delete <backup> --yes`) and `GET/POST /v1/system/backups`, `DELETE /v1/system/backups/{name}`, `POST /v1/system/backups/{name}/restore`. A backup is a folder in `<data folder>/backups/` with a consistent snapshot of the database (`VACUUM INTO`, taken while the server runs), the stored files, the key file and a manifest; each backup links the files the one before it had, so only new files cost space. A restore never touches the running environment: it copies a backup into a **new environment folder**, opens it to prove it works and says how many workspaces, files and people it holds; attach it from the environment menu.
- **Automatic backups.** `REPOMEMO_BACKUP_INTERVAL_HOURS` (0, off, by default; also a setting) makes the maintenance loop back up by itself when the newest backup is older, and `REPOMEMO_BACKUP_KEEP` (7) bounds how many are kept.
- **Quotas that protect the disk.** Storage per workspace (`REPOMEMO_WORKSPACE_STORAGE_MB`, 20,480 MiB: `413 workspace_full`), uploads per person per hour (`REPOMEMO_UPLOADS_PER_HOUR`, 600: `429`), workspaces owned per person (`REPOMEMO_WORKSPACES_PER_PERSON`, 25: `409 workspace_limit`) and a free-disk floor (`REPOMEMO_DISK_FLOOR_MB`, 1,024 MiB: uploads answer `507 disk_low`). All are settings in System › Settings (group *Limits*); `0` turns one off. System › Overview shows free disk space and the allowances.
- **Metrics.** `GET /v1/system/metrics` in the OpenMetrics text format: version, uptime, requests and errors, accounts, workspaces, files, passages, vectors, jobs by status, database and log size, free disk, queues, vector index memory, abandoned document readings, backups and the time of the last one. It needs a system administrator's session or a personal access token with the `system` scope.
- **A production guide**, [mindmap/technical/operations.md](mindmap/technical/operations.md): the settings that must be right, nginx and Caddy configurations with timeouts and event streams, systemd and Windows service definitions, readiness checks, limits, backups and the restore drill, monitoring with alerts, the LibreOffice sandbox, upgrades and a troubleshooting table.

### Upgrading
- Nothing changes in the database. Uploads over a workspace's 20 GiB allowance, or while less than 1 GiB is free, are refused from now on: raise the settings or set them to `0` for the old behaviour.
- Backups are kept next to the data, so copy the `backups` folder to another machine (the guide shows how). They hold `secret.key`: protect them like the data.

## V0.1.74 (2026-10-14)

Performance and robustness.

### Added
- **A dashboard in one request.** `GET /v1/dashboard` returns the figures of every workspace the person can open (files, active tasks, memory cards). The dashboard page used to ask for the full metrics of each workspace.
- **Limits on heavy requests.** Search, symbol search, figures, the knowledge map, Workspace Health and the file query each allow a person `REPOMEMO_HEAVY_REQUESTS_PER_MINUTE` requests a minute (240 by default, `0` is unlimited; also a setting in System › Settings). Beyond it the answer is `429` with `Retry-After`.
- **A daily allowance for background AI.** Image descriptions and embedding batches the server starts on its own are counted per workspace per day (`REPOMEMO_BACKGROUND_AI_PER_DAY`, 2,000 by default, `0` is unlimited; also a setting). A workspace that used it up pauses until midnight UTC: embedding resumes by itself, an image waits for the next indexing retry, and the job says why.
- **A time budget for document readers.** Reading a Word, Excel, PowerPoint, PDF, OneNote or Outlook file stops waiting after `REPOMEMO_DOCUMENT_TIMEOUT_SECONDS` (60 by default, `0` for no limit); the file is reported as failed with the reason.
- **`REPOMEMO_VECTOR_INDEX_MB`** (512 by default, `0` turns it off) sets the memory for in-memory vector indexes. System › Overview lists it with the document time limit.
- A performance run for operators: `cargo test -p repomemo-storage --release performance_at_scale -- --ignored --nocapture` builds a workspace of `REPOMEMO_PERF_PASSAGES` passages (50,000 by default) and prints search, figures and query timings.

### Changed
- **Meaning search no longer scans the database.** The vectors of a workspace are kept in memory as one matrix, built on the first search, kept current by the embedding queue and checked against the database on every search. A workspace whose vectors do not fit `REPOMEMO_VECTOR_INDEX_MB` is scanned as before.
- **Figures and the file query are computed by the database.** `GET …/metrics` uses SQL aggregates instead of loading every file; `POST …/artifacts/query` filters, counts and pages in SQL (`X-Total-Count` when paged). Text matching ignores case for ASCII letters only (it used to ignore case for every script).
- **The knowledge map and Workspace Health are kept while a workspace is unchanged.** They are recomputed when files, passages, vectors, memory cards, links, evidence states, health decisions or providers change, and at the latest after ten minutes.
- File links for "Open in Office" are valid for **60 seconds** (they were 5 minutes) and belong to the client that first used them (see Security).

### Security
- **Heavy endpoints are rate limited and cached, parsers have a time budget (M11).** A reading that overruns is abandoned, not killed: it ends by itself, and new readings are refused while four abandoned ones are still running. There is no memory cap on a reading.
- **Background AI is counted (M12).** See the daily allowance above.
- **File links are not reusable (L1).** A link works for 60 seconds, for the first client that fetches it (the same client may repeat the request for 30 seconds, as Office apps do), and its path is replaced by `/v1/shared-files/[link]/name` in request logs.
- **`secret.key` is owner-only on Windows (L5).** The folder's inherited permissions are removed and only the account running the server keeps access (`icacls`); on Unix an older file with wider permissions is set to `0600`. Failing to do so is logged, and `REPOMEMO_SECRET_KEY` avoids the file altogether.

### Upgrading
- Links to open a file in Office that were issued before the upgrade stop working; open the file again.
- A backup or service account other than the server's own can no longer read `secret.key` on Windows. Give it access, or keep the key in `REPOMEMO_SECRET_KEY`.
- Scripts that make more than 240 searches or reports a minute as one person need a higher `REPOMEMO_HEAVY_REQUESTS_PER_MINUTE`.
- A workspace that embeds more than 2,000 batches (about 64,000 passages) in one day pauses until the next day; raise `REPOMEMO_BACKGROUND_AI_PER_DAY` or set `0`.

## V0.1.73 (2026-10-14)

Evidence, ownership and repositories, phase 1b.

### Added
- **Ownership transfer.** An owner can hand a workspace (People) or an organization (Organization administration) to another member with **Make owner**; the previous owner stays on as an administrator. Recorded in the workspace activity and the System audit. `POST /v1/workspaces/{ws}/transfer-ownership` and `POST /v1/organizations/{org}/transfer-ownership`, from a signed-in session.
- **Folders can be moved** (`POST /v1/workspaces/{ws}/folders/{f}/move`, menu *Move to folder…*), not into themselves or below themselves, within the five levels, and not onto a name that is taken.
- **Several files at once.** *Select files* in the Evidence ledger moves, marks verified, needs review or outdated, or deletes the chosen files together (`POST /v1/workspaces/{ws}/artifacts/bulk`, up to 200). Files that cannot be handled (repository files, files already gone) are reported and do not stop the others.
- **New version of a file** (`POST /v1/artifacts/{a}/versions`, *New version* on the file page): the content is replaced in place, so comments, folder, state and the memory cards that cite it stay; passages whose text did not change keep their identity and their links. It must be the same kind of file.
- **Repository renames with edits.** A file renamed and edited in one commit stays the same artifact (git's rename detection, `-M50%`), with its comments, memory links and history.
- **The commit on results and citations.** Search results, citations and Ask answers for a repository file say which commit their text is from.
- **Console:** `users reset-link <email> --yes`, `tokens list [--user email]`, and `logs --follow` (also `--after <sequence>`).

### Security
- **Nobody can leave an organization while owning one of its workspaces (N8):** removing such a member answers `409` and names the workspaces.

### Upgrading
- No data is changed by this release. A workspace that an earlier removal left without an owner stays as it is: ownership can only be handed over by an owner.

## V0.1.72 (2026-10-14)

Memory and search you can trust.

### Added
- **Memory cards say when their evidence moved.** A card whose cited passage was changed or removed, whose cited file was marked outdated or superseded, or whose cited file was updated after the card was written shows **Needs review** in the list and a panel with the reasons on the card, where it can be confirmed as it is. Workspace Health has a new detector, *Stale memory* (create task, dismiss).
- **Evidence of a card can be edited.** `PUT /v1/memory-cards/{id}` takes `citations`; the edit form lists what the card cites with remove and add.
- **Search by trust.** Search takes an evidence state, a folder (and the folders below it) and an updated-between date range. Outdated and superseded evidence is hidden unless asked for, in Search, Ask and the assistant alike, and verified evidence ranks a little higher (10 %, never above 1).
- **Code symbols on the web.** `GET /v1/workspaces/{ws}/symbols?q=` and a *Code symbols* box on the Retrieval page; the assistant answers "where is parse_config defined" and "find the function …" from the same table.
- **Live Map and Health.** Both pages refresh themselves, once per burst, when evidence, memory or repositories change or a job finishes.

### Changed
- `SearchRequest` has `lifecycle_statuses`, `folder_id`, `modified_after` and `modified_before` (all optional); `MemoryCardSummary.needs_review` and `MemoryCardDetail.review` are new fields.

### Upgrading
- Search no longer returns passages of files marked outdated or superseded by default. Ask for them with the evidence state *Outdated and superseded* or *Every state*.

## V0.1.71 (2026-10-14)

Server foundation: the base the product work after it builds on. Behaviour changes are limited to the items below.

### Changed
- **The server is split by area.** `apps/server/src/lib.rs` (9,000 lines with its tests) is the router and state, with the handlers in `auth`, `access_control`, `profile`, `organizations`, `workspaces`, `artifacts`, `collaboration`, `retrieval`, `ai`, `streams`, `requests`, `errors` and `config`, and its tests in `tests/`. No module is over 1,500 lines.
- **The storage crate is split by area.** `crates/storage/src/lib.rs` (6,200 lines) keeps the engine, the row types and the helpers; its methods live in `users`, `organizations`, `activity`, `collaboration`, `artifacts`, `search`, `memory`, `jobs` and `providers`.
- **Typed errors.** What a person caused (a record that does not exist, input that is not acceptable, a name that is taken) is created with `not_found!`, `invalid!` and `conflict!` in `repomemo-domain` and carries its kind. The HTTP layer answers from the kind, no longer from the words of the message, and every other failure is a generic 500 with the detail in the log. Handlers that returned storage errors as 400 (token creation, folders) now answer with the right status (review L6).
- **One writer connection and a pool of readers.** Writes and transactions go through a single SQLite connection (writers queue instead of failing with "database is locked"); reads use their own read-only connections (`REPOMEMO_DB_READ_CONNECTIONS`, default 4) and never wait for a write.
- Indexing and embedding concurrency are settings: `REPOMEMO_INDEX_CONCURRENCY` (default 2) and `REPOMEMO_EMBEDDING_CONCURRENCY` (default 1). System › Overview lists them.
- **The code base is formatted with rustfmt and clean under clippy** (`-D warnings`), and CI fails otherwise. License and duplicate-crate checks run in CI as well.
- `apps/worker`, a placeholder that did nothing, is removed.
- **Dependencies.** `quick-xml` 0.41 in the document readers closes two advisories about hostile XML (RUSTSEC-2026-0194, -0195), and `cargo update` brings `anyhow`, `rustls` and the rest to their patched releases, so `cargo deny` is clean for the whole workspace. One advisory is ignored with its reason in `deny.toml`: `rsa` is linked by `jsonwebtoken` but unused, since tokens are HS256.

### Security
- **Request timeouts (N7).** Every route except the live event streams answers within `REPOMEMO_REQUEST_TIMEOUT_SECONDS` (default 120; `408` after that). A client has `REPOMEMO_HEADER_TIMEOUT_SECONDS` (default 30) to send its request headers, which ends slow-header connections. One account can hold `REPOMEMO_MAX_EVENT_STREAMS_PER_USER` live streams (default 10; `429` after that). Part of M11.

### Upgrading
- A request that takes longer than two minutes (a very large upload over a slow link) needs a higher `REPOMEMO_REQUEST_TIMEOUT_SECONDS`.
- Scripts that matched error texts of the API should use status codes and the `error.code` field.

## V0.1.70 (2026-10-14)

Web client foundation: the base every later product release builds on. No change in what the app does.

### Changed
- **`SharedWebApp.tsx` is split by page.** The 2,800-line file is now the app shell (about 340 lines) plus modules under `src/web/`: the dashboard and workspace home, profile, notifications, organization pages, workspace detail, artifact and memory card pages, shared constants, formatting helpers and the router.
- **A typed router** (`src/web/router.ts`): `parseRoute` turns an address into a `Route` (workspace and section, artifact, memory card, System page, not found) and `routePath` builds it back, instead of matching `pathname.split("/")` by hand.
- **Server state lives in a cache** (TanStack Query, `src/lib/queryClient.ts`). A workspace page shows cached data at once and refreshes it in the background. Live events and mutations mark only what they touched as stale (ledger, memory cards, tasks, figures), so one change no longer refetches the whole page; a stream resync refetches everything once.
- **Heavy panels load on demand:** the System area, the rich note editor, the knowledge map, the document viewer, workspace health and the repository pages are separate bundles fetched when first shown.
- **Types from the Rust source.** `npm run types:generate` writes `src/generated/domain.ts` from `crates/domain`; `src/types.ts` re-exports the 70 types that match and keeps the 28 the client shapes differently. CI fails when the generated file is out of date (`npm run types:check`).
- The local desktop app is labelled **Local edition**: it is frozen, with security fixes only (roadmap decision 3).

### Added
- **Web client tests** (Vitest and Testing Library, `npm run test:web`, in CI): session renewal through the cookie, live event parsing, Markdown safety (no images, link targets), image preview rules, the upload file filter, routes, and the query cache.


Host exposure (bundle S3): the last two high findings.

### Security
- **AI providers only where the operator allows (H2).** Without `REPOMEMO_AI_ALLOWED_HOSTS`, a provider may sit only on this machine (`127.0.0.1`, `localhost`, `::1`) or on `openrouter.ai`. List your own hosts to allow others, or `*` to allow every host (link-local and cloud-metadata addresses stay refused). Names are resolved by the server and every address is checked at connect time, so a name cannot be re-pointed at the server's own network after it was accepted; with the built-in list a name must resolve to a public address.
- **Provider error bodies are not echoed (H2).** What a provider answers with stays in the server log (System › Logs, AI category); the message shown has the status and a hint only.
- **Repository links are off until the operator names folders (H3).** A server without `REPOMEMO_REPO_ROOTS` refuses every link and sync with one message; outside the configured folders the answer is the same whether or not the path exists.
- **Git runs isolated (L8).** On a server, git ignores the system and global configuration, allows no transport protocol and does not recurse into submodules. The repository folders, and folders in `REPOMEMO_GIT_SAFE_DIRECTORIES`, are exempt from git's ownership check.
- **LibreOffice runs without the server's secrets (M10).** Every `REPOMEMO_*` variable and anything named like a key, token or password is removed from its environment, and `REPOMEMO_SOFFICE_WRAPPER` runs it inside a sandbox command of the operator's (no network, no data folder). System › Overview and the server check show its version and whether it is sandboxed.

### Upgrading
- Provider URLs outside this machine and OpenRouter stop working until listed in `REPOMEMO_AI_ALLOWED_HOSTS`. A provider on another machine in your network must be listed by name.
- Repository links stop working (and linked repositories stop syncing) until `REPOMEMO_REPO_ROOTS` names the folders that hold them.
- Git no longer reads the service account's `git config --global`: a `safe.directory` set there is ignored. Use `REPOMEMO_GIT_SAFE_DIRECTORIES`. Git 2.32 or later is needed for the isolation to be complete.

## V0.1.68 (2026-10-14)

Browser hardening (bundle S4).

### Security
- **The refresh token is out of reach of scripts (M2).** The web app's refresh token travels in an `HttpOnly; SameSite=Strict` cookie (`repomemo_refresh`, path `/v1/auth`, `Secure` when `REPOMEMO_PUBLIC_URL` is HTTPS) instead of `localStorage`. Only requests with `X-RepoMemo-Client: web` are affected; the console and scripts still use the token in the response body. The web app and the API must share a site (one host behind the proxy; different ports are fine). Tokens kept by an earlier version are exchanged once on the next visit.
- **A content security policy for the built web app (M2).** `npm run build` puts it in `index.html` and writes the header value, with `frame-ancestors 'none'`, to `dist/content-security-policy.txt` for the host to send: scripts from the app only, requests to itself and the API, images from itself, `blob:` and `data:`, frames only for `blob:` previews, no plugins.
- **A strict policy for the desktop shell (M6)** in `tauri.conf.json`: its own files and the local IPC only, no remote connections.
- **No remote images in Markdown (M5).** AI answers, notes, memory cards and repository summaries show an image as a note with its text and address instead of fetching it, so a prompt-injected image cannot send workspace content out in a URL.
- **External links show where they go (L9).** The target host is shown next to the link, which opens in a new tab with `noopener noreferrer`.
- **Invitation and reset secrets leave the address bar (L3).** The link page reads the fragment once and removes it from the address and the history entry.

### Changed
- The API allows credentialed cross-origin calls from the origins in `REPOMEMO_ALLOWED_ORIGIN` (needed for the cookie when the web app is on another port).

### Upgrading
- Serve the web app and the API from one host (a reverse proxy routing `/v1` and `/health` to the server), or at least one site; a cookie-less setup across different sites can no longer renew sessions.
- Send the header from `dist/content-security-policy.txt` from the host that serves the web app, and rebuild the web app with `VITE_REPOMEMO_API_URL` set so the policy names your API.

## V0.1.67 (2026-10-14)

Safe defaults and identity (bundle S2).

### Security
- **Registration is closed by default (M1).** `REPOMEMO_ALLOW_REGISTRATION` now defaults to `false`; people join with invitation links, and the first account of a new server can still register. `/health` no longer says whether registration is open (I2).
- **Adding someone no longer reveals whether they have an account (M4).** The member forms add only people who are already in the organization (or workspace); anyone else gets the same `409 invitation_required` answer, whether or not the address has an account, and the web app offers an invitation link. Nobody is added to an organization or workspace without accepting.
- **App administrators read, system administrators change (M8).** App administrators keep the System pages, jobs and maintenance. Settings, roles, other people's sessions, lockouts, reset links, raw log files and detaching the environment need a system administrator, and workspace activity records are hidden from app administrators in the logs.
- **A `system` scope for personal access tokens (N2 residual).** Only administrators can create one. Changes in the System area (sign people out, unlock, run maintenance, console `--yes`) need a signed-in session or a `system` token; read-and-write tokens no longer reach them. Migration 0024.
- **Transport checks (M9).** The server refuses to listen beyond loopback unless `REPOMEMO_PUBLIC_URL` is `https://…`, `REPOMEMO_TRUST_PROXY` is on, or `REPOMEMO_ALLOW_INSECURE_HTTP` says the network is trusted. An HTTPS public address turns on `Strict-Transport-Security`. `repomemo-console` warns when it talks plain `http://` to another machine.
- **Client addresses behind a proxy (M3).** `X-Forwarded-For` counts only when the TCP peer is loopback or listed in `REPOMEMO_TRUSTED_PROXIES`, and is read from the right, so a client cannot choose its own address.
- **Lockouts no longer lock owners out (L10).** A sign-in that carries a session this browser still holds gets past a lockout other people caused.
- **The account hears about reset links (L4):** a notification when an administrator creates one and another when it is used.
- **Weak passwords refused (L7):** the most common long passwords, passwords of fewer than five different characters, and passwords containing the email name.

### Changed
- The member forms say "is not in this organization yet. Create an invite link for them below." instead of asking whether an account exists.

### Upgrading
- A server that relied on open registration must set `REPOMEMO_ALLOW_REGISTRATION=true` (or turn it on in System › Settings). A value saved in System › Settings is kept as it was.
- A server bound beyond loopback over plain HTTP stops at startup until it has `REPOMEMO_PUBLIC_URL`, `REPOMEMO_TRUST_PROXY` or `REPOMEMO_ALLOW_INSECURE_HTTP=true`.
- Scripts that added existing accounts straight to a workspace must create an invitation instead.
- Scripts that ran System changes with a read-and-write token need a `system` token.

## V0.1.66 (2026-10-13)

Checkable releases: the first step of the road to V0.2.0.

### Added
- **Continuous integration** (`.github/workflows/ci.yml`) on every push to `main` and every pull request. Blocking: versions agree, Rust tests on Linux and Windows, the web type check, and security advisories for the server's dependencies (`cargo deny`, policy in `deny.toml`) and the web client's production dependencies (`npm audit`). Reported but not yet blocking: rustfmt and clippy, licenses and duplicate crates, and advisories that only reach the desktop shell.
- **One version source.** `npm run version:set -- <version>` (`scripts/set-version.mjs`) sets `Cargo.toml`, `Cargo.lock`, both `package.json` files, `package-lock.json` and `tauri.conf.json` together; `npm run version:check` (also in CI) fails when they disagree.
- **Performance timings.** Search, Ask, workspace metrics, the artifact query, the knowledge map and health checks record how long they took and how much they handled (`timing` target), with the workspace. Turn on **System › Settings › Logging › Performance** at *debug* to see them in System › Logs, filterable by workspace. New setting: `log_performance_level`.
- This changelog.

### Changed
- The version reported by System › Overview and the console is correct again: it had stayed at 0.1.60 since V0.1.60.
- `PRODUCT.md` describes the self-hosted team product instead of the desktop-only one. `docs/IMPLEMENTATION_TRACKER.md` is retired in favour of the roadmap and this changelog.

## V0.1.65 (2026-10-13)

The security second pass ([review §11](mindmap/security/2026-10-10-security-review.md#11-second-pass-2026-10-13)).

### Security
- **Stored XSS through image previews closed (N1, high).** Stored and served content types now come from the file name, never from the uploader (also I3); HTML, scripts and unknown types are served as plain text. Image previews open in a tab of their own only for raster images.
- Run-time settings changes need a signed-in session; personal access tokens can still run dry runs (N2).
- A workspace or organization can no longer lose its owner through the member list or an organization role change (N3).
- Someone downgraded to viewer can no longer edit or delete their earlier comments or tasks; saved searches are deleted by their author or an administrator (N4).
- Using a password reset link also revokes the account's personal access tokens (N5).
- `POST /v1/workspaces/{ws}/index` (synchronous, whole workspace) is for administrators (N6).

### Upgrading
- Scripts that changed settings with a personal access token must use a signed-in console (`repomemo-console` with a password) instead.

## V0.1.64 (2026-10-13)

Security ship-blockers for onboarding (bundle S1).

### Security
- Adding someone to one workspace, directly or by invitation, gives that workspace only; their organization shows on their dashboard as a guest entry (H1).
- Registering an address listed in `REPOMEMO_SYSTEM_ADMIN_EMAILS` grants nothing; only accounts that exist at startup are promoted (H4).
- What personal access tokens may reach is marked on the handlers instead of a path list (L2).
- Invitation and reset links carry their secret in the URL fragment (`/invite#…`, `/reset-password#…`) and the web app sends no referrer; older path links still open (L3).
- Only `VITE_` and `TAURI_ENV_` environment variables reach the web bundle (M7).

### Upgrading
- People added to a single workspace before V0.1.64 kept the organization membership they received then, and with it every workspace of the organization. Review organization members once and remove anyone who should only see one workspace.

## V0.1.63 (2026-10-13)

Onboarding without an account (migration 0023).

### Added
- **Invitation links** for one email address and role, single-use, valid 7 days, withdrawable, usable even when registration is closed.
- **Password reset links** issued in System › Users, one-time, valid 24 hours.
- **Personal access tokens** (`rmp_…`, read-only or read and write, 7–365 days) for scripts, CI and `repomemo-console` (`REPOMEMO_TOKEN`).
