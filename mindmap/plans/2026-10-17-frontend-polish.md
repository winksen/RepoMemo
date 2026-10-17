# Frontend polish: V0.2.57 → V0.2.129

> **Status:** planned on 2026-10-17, not started. **Line:** after V0.2.56, before `V0.3.0`.
> **For:** the model or person implementing it, one release at a time, in order.
> **Related:** [Roadmap, track K](../ROADMAP.md#track-k-frontend-polish-v0257v02129) · [Release index](../RELEASES_0.2.md) · [DESIGN.md](../../DESIGN.md) · [Technical mindmap §9](../TECHNICAL_MINDMAP.md)

The web client works, but it does not look like one product. Fields are 34, 38, 40 or 42 px tall depending on the page. Their borders are 0, 1 or 2 px, and their corners are rounded at 3, 6, 8 or 10 px. Dropdowns open behind dialogs, and long lists run off the screen. Checkboxes are stretched like text fields, and every page builds its own headings, badges, tabs, loading text and confirmations.

This track **does not change the look**. The palette, type, spacing and components that exist stay. It makes every page use them the same way, fills the gaps with a few missing building blocks, and leaves the React code easier to change.

---

## 1. What the audit found

A static reading of `apps/desktop/src` (web client) on 2026-10-17. Line numbers are as of that date.

| # | Finding | Evidence |
|---|---|---|
| A1 | **Dropdowns open behind dialogs.** The list of a `Dropdown` has `z-index: 50`; a dialog has 901. Five files put a `Dropdown` inside a `Dialog` | `styles/01-tokens-and-shared-web.css:796`, `styles/11-blueprint-primitives.css:667-668`; `ApiTokensPanel`, `ConnectorsPanel`, `LocalMoveToServer`, `SystemAdminPanel`, `web/Profile` |
| A2 | **Long dropdown lists run off the screen.** `.rm-dropdown-content` has `overflow: hidden` and no maximum height | `styles/01…:796` |
| A3 | **Every dropdown is full width.** `.rm-dropdown-trigger { width: 100% }` is the default, so a dropdown in a toolbar or filter row stretches | `styles/01…` (`.rm-dropdown-trigger`) |
| A4 | **Global element rules fight the primitives.** `input`, `textarea` and `select` are restyled in five stylesheets. The rules disagree: radius 6 (04), 3 (06), 8 (09, 10) and 10 (11); border 0 (04), 1 px (06, 10) and 2 px (11); backgrounds `surface-2` (04), `#fff` (09, 10) and `surface` (11). Dark-theme copies repeat them (11) | `styles/04…`, `06…`, `09…`, `10…`, `11…` (83 element rules that size form controls) |
| A5 | **Page rules override the shared field.** Sign-in, setup and create-workspace fields are 42 px; member and memory forms are 38 px with a 1 px border; create-workspace is fixed at 260 px | `styles/01…` (`.shared-auth-form input`, `.shared-member-form input`, `.shared-memory-form select`, `.shared-create-workspace input`) |
| A6 | **Checkboxes are styled as text fields** by the global `input` rules, then patched in six places one by one | `.shared-ai-provider-toggle input[type=checkbox]`, `.shared-artifact-type-menu-panel input`, `.shared-task-checklist-list input`, `.provider-toggle input`, `.filter-options input` |
| A7 | **Neutrals are tinted green**, against DESIGN.md's "No tinted neutrals" (saturation 0) | `styles/01…:6-18` (`--color-bg: #e2e7e4`, `--color-surface: #f8faf7`, `--color-text: #18201f`…), dark theme `:56-68` |
| A8 | **A second accent.** `--workspace-accent: #147d72` and six other `--workspace-*` colours next to `--color-accent` | `styles/03-workspace-workbench-and-forms.css:4-26`, used at `:293`, `:448` |
| A9 | **Hard-coded colours.** 96 hex or rgb values in `10…`, 54 in `09…`, 20 in `11…`, plus a danger red `#c9372c` that exists only in dialog footers | `styles/09…`, `10…`, `11…:690-691` |
| A10 | **Control heights disagree with DESIGN.md** (36–40 px). Fields and dropdowns are 34 px; icon squares are 28, 32, 34, 36 or 44 px | `styles/10…` (`.rm-input`), `01…:354`, `:466`, `11…:695`, `:750`, `:810` |
| A11 | **173 `<Button>` elements have no `type`.** Inside a form they submit it | `grep '<Button' --include=*.tsx` (components, web) |
| A12 | **Seven native `window.confirm()` calls** instead of the app's dialog | `web/ArtifactDetail.tsx:180`, `:222`, `web/MemoryCardDetail.tsx:162`, `web/Organization.tsx:65`, `web/workspace/PeopleSection.tsx:51`, `SettingsSection.tsx:60`, `TasksSection.tsx:76` |
| A13 | **One raw `<select>`** in the memory card page | `web/MemoryCardDetail.tsx:172` |
| A14 | **About 25 different "Loading…" sentences** and no shared loading or empty state | `grep Loading` (components, web) |
| A15 | **No shared field.** 27 form classes lay out labels their own way: 63 labels wrap their control, 25 have a class, 3 use `htmlFor` | `grep '<form'`, `grep '<label'` |
| A16 | **Hand-built pieces repeated:** 59 `shared-panel-heading` blocks, 6 kinds of tab or segmented switch, 10 badge or tag classes, 6 hand-made toggles, several "option list" styles (palette, mentions, folder picker, assistant chats) | `grep className` (components, web) |
| A17 | **Duplicated helpers.** `errorMessage` in 9 files and `formatDate`, `formatBytes` or `formatTime` in 8, next to `web/format.ts` | `ApiTokensPanel`, `AssistantPanel`, `ConnectorsPanel`, `InviteMembers`, `InvitePage`, `PasswordResetPage`, `PlatformSetup`, `RepositoriesPanel`, `ServerSetup`, `SystemAdminPanel`, `VerifyEmailPage`, `WorkspaceHealthPanel`, `DocumentViewer`, `LanguageChart` |
| A18 | **Large components.** `SystemAdminPanel` (904 lines, 36 `useState`), `RepositoriesPanel` (882), `KnowledgeMapPanel` (530), `ServerSetup` (377, 21 `useState`) | `wc -l` |
| A19 | **Data loading outside the query cache.** `useEffect` plus `.then` in `SystemAdminPanel` (11), `KnowledgeMapPanel` (8), `RepositoriesPanel` (6), `AssistantPanel` (5), `ConnectorsPanel` (4), `DocumentViewer` (4); only 11 files guard against a late answer | `grep useEffect` |
| A20 | **Icon sizes 12–26 px** in nine values (16 ×202, 15 ×70, 18 ×65, 14 ×35, 17 ×22…) | `grep 'size={'` |
| A21 | **DESIGN.md and the code disagree** on the accent (blue in DESIGN.md, green in the tokens) and on the height of controls | `DESIGN.md` §Color and tone, §Components; `styles/01…:20` |

---

## 2. Decisions to agree before V0.2.57

| # | Decision | Proposal |
|---|---|---|
| 15 | **Lift the feature freeze for this track** (the roadmap froze features at V0.2.52) | Yes: this track adds no product feature, only building blocks and fixes. V0.2.129 repeats the release checks before the tag |
| 16 | **The accent colour** (A21) | Keep the green accent the product has shipped with; V0.2.128 corrects DESIGN.md. Changing to blue would be a new look |
| 17 | **Neutrals at saturation 0** (A7) | Yes, as DESIGN.md already requires: the same lightness, the hue removed. The change is subtle on screen |
| 18 | **More than two button treatments** (DESIGN.md allows only `main` and `secondary`) | Add `danger` (a destructive confirmation) and the `IconButton` shape (one icon, a required label). Both are already in the code as one-off styles; this makes them deliberate |
| 19 | **Where form mistakes are shown** | API errors stay toasts (CLAUDE.md). A required or malformed field is marked `aria-invalid` with a one-line hint under the field. That is a field state, not a banner: no colour fill, only the field's border and a short muted line |

---

## 3. How to work through this track

- **One release per row, in order.** Each changes one thing. A later release relies on the earlier ones (fields need `Field`, migrations need the split files).
- **The look stays.** Use the existing tokens, sizes and components. Where two existing styles disagree, keep the one DESIGN.md describes, or the most common one, and say which in the commit.
- **Errors and successes are toasts** (`showToast`, `<Toast>`), never inline banners. Backgrounds and text use neutral tokens only.
- **Checks before each commit:** `npm run typecheck`, `npm run styles:check`, `npm run a11y:check`, `npm run types:check`, `npm run version:check`. **Do not** start the dev server, build, run Vitest or `cargo test` (CLAUDE.md): write or update the tests in `apps/desktop/src/test/`; CI runs them. Say in the commit when a test was written but not run.
- **Every release:** `npm run version:set -- 0.2.N`; a `CHANGELOG.md` entry; its row in [RELEASES_0.2.md](../RELEASES_0.2.md) set to ✅ with the date and what was built; its box in the [roadmap](../ROADMAP.md) ticked; [TECHNICAL_MINDMAP.md](../TECHNICAL_MINDMAP.md) §9 when a convention of the web client changes; [DESIGN.md](../../DESIGN.md) when a building block is added.
- **Commit** with exactly this shape, never pushed, no trailer:

```
Update V0.2.N

- What changed, in one line
- A second line when needed
- Updated changelog, roadmap, mindmaps and release index
```

---

## 4. The releases

**Type:** 🐞 fix · 🔧 enhancement · ✨ new building block · 🧹 refactor · 🧪 test · 📘 update (docs, process)

### K1. Foundations: tokens, layers and the cascade (V0.2.57–V0.2.68)

Fix the causes before touching pages.

#### V0.2.57 · 🐞 · Dropdowns and menus open above dialogs (A1)
- **Change:** a layer scale in `styles/01…` (`--layer-raised: 1`, `--layer-sticky: 10`, `--layer-overlay: 900`, `--layer-dialog: 901`, `--layer-popover: 950`, `--layer-toast: 1000`). Use it for `.rm-dropdown-content`, `.rm-action-menu`, `.rm-dialog-overlay`, `.rm-dialog`, the toast viewport, the map tooltip and the mention list. Dropdowns and menus sit at `--layer-popover`.
- **Files:** `styles/01…`, `styles/11…`, `styles/12…`, `components/ui/toast.tsx` if it sets its own.
- **Done when:** no literal `z-index` above 10 remains outside the scale. A dropdown in the token dialog of Profile › API tokens opens on top of the dialog.
- **Commit:** `- Dropdown lists and action menus open above dialogs: one layer scale for raised content, sticky headers, dialogs, popovers and toasts`

#### V0.2.58 · 🐞 · Long dropdown lists scroll inside the window (A2)
- **Change:** `.rm-dropdown-content` gets `max-height: min(320px, var(--radix-select-content-available-height))`, and `.rm-dropdown-viewport` gets `overflow-y: auto`. Add Radix `ScrollUpButton` and `ScrollDownButton` in `components/ui/dropdown.tsx`.
- **Done when:** the System › Settings level dropdown and the Retrieval language filter stay inside the window when opened near the bottom.
- **Commit:** `- Long dropdown lists stay inside the window and scroll`

#### V0.2.59 · 🐞 · Dropdowns size to their content unless a form stretches them (A3)
- **Change:** `.rm-dropdown-trigger` defaults to `width: auto` with a `min-width: 160px`. Forms make it full width through the field layout (V0.2.69); until then, add `width: 100%` only to the form containers that need it.
- **Done when:** the dropdowns in toolbars and filter rows (Tasks, Retrieval, System logs) no longer stretch across the row.
- **Commit:** `- Dropdowns take the width of their content in toolbars and filter rows; forms still stretch them`

#### V0.2.60 · 🔧 · One size for every control (A10)
- **Change:** tokens `--control-height: 36px`, `--control-height-touch: 40px`, `--control-radius: 8px`, `--control-border: 2px`, `--control-padding-x: 12px`. `.rm-input`, `.rm-dropdown-trigger` and `.rm-button` use them; `13-narrow-screens.css` switches to the touch height.
- **Done when:** text fields, dropdowns and buttons are the same height on every page (36 px, 40 px on narrow screens).
- **Commit:** `- Fields, dropdowns and buttons share one height, radius and border from new control tokens (36 px, 40 px on touch screens, as DESIGN.md says)`

#### V0.2.61 · 🔧 · True neutral greys (A7, decision 17)
- **Change:** set every neutral token in both themes (`bg`, `canvas`, `surface*`, `code-surface`, `text*`, `border*`) to saturation 0, keeping its lightness. Leave the accent and status colours as they are.
- **Done when:** `npm run a11y:check` (contrast) still passes; no neutral token has a hue.
- **Commit:** `- Backgrounds, borders and text are true neutral greys in both themes (same lightness, no hue)`

#### V0.2.62 · 🔧 · One accent: the `--workspace-*` colours retired (A8)
- **Change:** replace `--workspace-accent*`, `--workspace-line*`, `--workspace-inset` and `--workspace-selected` with the existing `--color-*` tokens and remove them.
- **Done when:** `grep -- --workspace- styles` finds nothing.
- **Commit:** `- The workspace pages use the one accent and the shared neutral tokens; the second teal palette is removed`

#### V0.2.63 · 🔧 · Hard-coded colours become tokens (A9)
- **Change:** in `09…`, `10…`, `11…` and `12…`, replace hex and rgb values with tokens. The dialog footer's danger red becomes `--color-danger`. Keep values that are tokens' own definitions in `01…`.
- **Done when:** the only hex colours outside `01…` are inside `:root` token blocks, and dark theme fields are no longer white.
- **Commit:** `- Colours in the later stylesheets come from tokens: no white fields in the dark theme, one danger red`

#### V0.2.64 · 🐞 · Retired desktop styles kept to the desktop edition (A4)
- **Change:** the global element rules in `04…` and `06…` (`input`, `textarea`, `select`, `.paste-language`, `.workspace-create-controls`, `.search-input-wrap`, `.filter-options`, `.artifact-search`) serve the retired local edition (`App.tsx`). Nest them under a `.rm-local-edition` class that `LocalDesktopApp` puts on its root.
- **Done when:** the web client gets no style from these rules. The local edition looks as before (the maintainer checks it by hand).
- **Commit:** `- Styles of the retired local edition apply only inside it; the web client's fields no longer inherit them`

#### V0.2.65 · 🐞 · Duplicate global field rules removed (A4)
- **Change:** delete the `input, textarea, select` rules and their dark copies in `09…`, `10…` and `11…`. Fields are styled only by `.rm-input`, `.rm-textarea` and `.rm-dropdown-trigger`, plus a minimal base reset in `01…` (font inherit, colour, `box-sizing`).
- **Done when:** no stylesheet outside the local-edition scope styles a bare `input`, `textarea` or `select` except the base reset.
- **Commit:** `- Fields are styled only by their shared classes: the competing global rules for inputs, text areas and selects are removed`

#### V0.2.66 · 🐞 · Page rules no longer resize the shared field (A5)
- **Change:** remove the height, border, padding, background and radius overrides of `.shared-auth-form`, `.shared-setup-form`, `.shared-create-workspace`, `.shared-member-form`, `.shared-memory-form`, `.shared-note-form`, `.shared-search-form` and `.shared-comment-form`. Keep only layout (width, grid placement, `min-height` of text areas).
- **Done when:** sign-in, setup, members, memory, notes and search fields look like every other field.
- **Commit:** `- Sign-in, setup, member, memory, note and search fields are the shared field: the page-specific sizes and borders are removed`

#### V0.2.67 · 🐞 · Checkboxes, radios and file inputs reset (A6)
- **Change:** a base rule in `01…` gives `input[type=checkbox]`, `input[type=radio]` and `input[type=file]` their natural size and the accent colour (`accent-color`). Remove the six per-place patches.
- **Done when:** every checkbox is 16 px and none stretches.
- **Commit:** `- Checkboxes, radio buttons and file pickers keep their natural size everywhere; six local patches removed`

#### V0.2.68 · 🔧 · Three icon sizes (A20)
- **Change:** `ICON_SIZE = { sm: 14, md: 16, lg: 18 }` in `components/ui/icon.ts`. Replace 12, 13, 15, 17, 20, 22 and 25 with the nearest of the three, except page illustrations (avatars, empty states) which keep their size.
- **Done when:** `grep 'size={1[2357]}'` finds only documented exceptions.
- **Commit:** `- Icons come in three sizes (14, 16, 18) from one constant`

### K2. Building blocks (V0.2.69–V0.2.90)

Each adds one shared component in `components/ui/`, documents it in DESIGN.md, and uses it in one or two places to prove it. Pages move to it in K4.

#### V0.2.69 · ✨ · `Field` (A15, decision 19)
- **Change:** `<Field label hint required invalid htmlFor>` renders the label, the control, an optional muted hint and, when `invalid`, a one-line field message (decision 19). `FieldRow` puts fields side by side and stacks them on narrow screens. Generate the control id when none is given.
- **Done when:** used in Profile › Personal details. Each field has a label tied to its control, and the visual is unchanged.
- **Commit:** `- A shared Field (label, control, hint, field message) and FieldRow, first used on the profile page`

#### V0.2.70 · ✨ · `Checkbox`
- **Change:** `<Checkbox checked onCheckedChange label description>` (Radix Checkbox), styled with tokens.
- **Done when:** used in the evidence type filter (`shared-artifact-type-menu-panel`).
- **Commit:** `- A shared Checkbox with a label and description, first used in the evidence type filter`

#### V0.2.71 · ✨ · `Switch` for on and off settings
- **Change:** `<Switch>` (Radix Switch) for settings that apply at once. It replaces the hand-made `shared-ai-provider-toggle` in one place to start.
- **Done when:** used in System › Settings for boolean settings.
- **Commit:** `- A shared Switch for settings that turn on and off, first used in System › Settings`

#### V0.2.72 · 🐞 · Buttons no longer submit forms by accident (A11)
- **Change:** `Button` defaults to `type="button"`. Forms say `type="submit"` on their one submit button; check every `<form>` for it.
- **Done when:** every form submits from its submit button and Enter, and no other button submits.
- **Commit:** `- Buttons are plain buttons unless they say submit; every form names its submit button`

#### V0.2.73 · ✨ · `Button` shows that it is working
- **Change:** a `busy` prop: a spinner replaces the icon, the label stays, the button is disabled and `aria-busy` is set.
- **Done when:** used on the sign-in and save buttons of Profile.
- **Commit:** `- Buttons can show that they are working (spinner, disabled, aria-busy)`

#### V0.2.74 · ✨ · `IconButton` (decision 18)
- **Change:** a square button holding one icon, 32 px (`size="sm"`) or 36 px, with a required `label` (used as `aria-label`). It replaces `rm-action-trigger`, `RefreshButton` and the 34 px squares in the organization and evidence view switches.
- **Done when:** every icon-only button in those places is an `IconButton`.
- **Commit:** `- A shared IconButton (one icon, a required name, two sizes) for icon-only actions`

#### V0.2.75 · ✨ · `Tooltip`
- **Change:** Radix Tooltip on `IconButton` (its label), and wherever a `title` attribute explains a control.
- **Done when:** icon-only buttons show their name on hover and on keyboard focus.
- **Commit:** `- Icon-only buttons name themselves in a tooltip on hover and keyboard focus`

#### V0.2.76 · ✨ · `danger` button (decision 18)
- **Change:** `variant="danger"` uses `--color-danger` (solid fill, white text, as DESIGN.md allows for the decisive action), replacing `.rm-dialog-footer .rm-button-danger`.
- **Done when:** destructive confirmations use it.
- **Commit:** `- A danger button for destructive confirmations, replacing a one-off dialog style`

#### V0.2.77 · ✨ · `ConfirmDialog` and `useConfirm` (A12)
- **Change:** `const confirm = useConfirm(); if (!(await confirm({ title, description, action, danger }))) return;` backed by `Dialog`. It replaces the two confirms in `ArtifactDetail` and the one in `MemoryCardDetail`.
- **Done when:** those three deletions confirm in the app's dialog.
- **Commit:** `- Confirmations in the app's own dialog (useConfirm), first for deleting evidence, comments and memory cards`

#### V0.2.78 · 🔧 · No more native confirmations (A12)
- **Change:** `useConfirm` in `Organization` (ownership), `PeopleSection` (ownership), `SettingsSection` (delete workspace) and `TasksSection` (delete task).
- **Done when:** `grep window.confirm` finds nothing in the web client.
- **Commit:** `- Ownership transfers and deletions of workspaces and tasks confirm in the app's dialog`

#### V0.2.79 · ✨ · `LoadingState` (A14)
- **Change:** `<LoadingState label="Loading jobs" />` (spinner, one muted line, `role="status"`) and an inline `<Spinner>`. One wording rule: "Loading <thing>…".
- **Done when:** used on the System pages.
- **Commit:** `- A shared loading state with one wording rule, first used on the System pages`

#### V0.2.80 · ✨ · `EmptyState`
- **Change:** `<EmptyState icon title description action>` for a list with nothing in it: muted, centred in its panel, one sentence, at most one action.
- **Done when:** used for tasks, memory cards and notifications.
- **Commit:** `- A shared empty state (icon, title, one sentence, one action), first for tasks, memory and notifications`

#### V0.2.81 · ✨ · `Badge` (A16)
- **Change:** `<Badge tone="neutral|success|warning|danger|info">`: neutral surface and text with a small status dot for tones (no tinted fill with same-hue text, per DESIGN.md). It replaces `rm-system-tag`, `rm-system-role`, `shared-review-badge`, `rm-console-tag`, `workspace-context-chip` and `rm-stack-chip`.
- **Done when:** those classes are gone.
- **Commit:** `- A shared Badge (neutral, with a status dot) replacing six tag and chip styles`

#### V0.2.82 · ✨ · `SegmentedControl` (A16)
- **Change:** Radix ToggleGroup styled as one segmented control. It replaces `shared-auth-switch`, `shared-activity-filter`, `shared-artifact-view-switch`, `shared-sheet-tabs` and `rm-desktop-switch`.
- **Done when:** the five switches share one look and arrow-key behaviour.
- **Commit:** `- A shared segmented control replacing five hand-made switches and tab strips`

#### V0.2.83 · ✨ · `PanelHeading` (A16)
- **Change:** `<PanelHeading icon title description actions>` for the 59 `shared-panel-heading` blocks, with the heading level as a prop.
- **Done when:** `grep shared-panel-heading` finds only the component's own CSS.
- **Commit:** `- A shared panel heading (icon, title, description, actions) replacing 59 hand-built ones`

#### V0.2.84 · ✨ · `PageHeader`
- **Change:** one header for pages (`shared-page-heading`, `shared-detail-heading`): title, description, actions and an optional back link. Title sizes follow DESIGN.md (page 28–32 px, panel 16 px).
- **Done when:** the dashboard, workspace, artifact, memory card, profile, notifications, organization and System pages use it.
- **Commit:** `- One page header (title, description, actions, back link) on every page`

#### V0.2.85 · ✨ · Table styles
- **Change:** an `rm-table` class (`components/ui/table.tsx` for the markup): sticky header, numbers right-aligned in tabular figures, a row hover, and a wrapper that scrolls sideways on narrow screens. It replaces `rm-map-coverage` and the other table styles.
- **Done when:** the System users, jobs, audit, map coverage and repository tables share it.
- **Commit:** `- One table style for the System, map and repository tables`

#### V0.2.86 · ✨ · `Combobox` for long lists (A13)
- **Change:** a searchable picker (Radix Popover plus a filtered list) for choosing one item out of many. It replaces the raw `<select>` of "Add a file…" in `MemoryCardDetail`.
- **Done when:** a card's evidence is picked by typing part of a file name.
- **Commit:** `- A searchable picker for long lists, replacing the raw select that adds evidence to a memory card`

#### V0.2.87 · 🔧 · The searchable picker where lists are long
- **Change:** use `Combobox` for *Replacement evidence* (artifact lifecycle, `web/ArtifactDetail.tsx`), a task's *Related evidence* and *Open a task checklist* (`web/workspace/TasksSection.tsx`).
- **Done when:** none of these lists needs scrolling through every file.
- **Commit:** `- Replacement evidence, a task's related evidence and the checklist picker use the searchable picker`

#### V0.2.88 · ✨ · `SearchField`
- **Change:** a text field with a search icon, a clear button and Escape to clear. It replaces the evidence filter, the System users filter, the repository file filter and the palette field.
- **Done when:** those four filters look and behave alike.
- **Commit:** `- A shared search field (icon, clear button, Escape) for every list filter`

#### V0.2.89 · ✨ · `CopyButton`
- **Change:** copies a value, says "Copied" for two seconds and shows a toast on failure. It replaces the copy code in `SecretDialog`, the invitation links, the commit hashes and the file paths.
- **Done when:** every copy action behaves the same.
- **Commit:** `- One copy button with the same feedback everywhere`

#### V0.2.90 · 🔧 · One option-list style
- **Change:** the command palette, mention suggestions, folder picker and assistant chat list share one `rm-option-list` style (row height, highlight, keyboard focus).
- **Done when:** their four separate rule sets are gone.
- **Commit:** `- The palette, mention suggestions, folder picker and chat list share one list style`

### K3. React foundations (V0.2.91–V0.2.101)

Prepare the code the K4 migrations touch, so they move clean code.

#### V0.2.91 · 🧹 · Formatting helpers in one place (A17)
- **Change:** move `formatDate`, `formatTime`, `formatBytes` and `formatDuration` into `web/format.ts` with tests, and delete the local copies (8 files). Add a `<Time value>` component showing a relative time with the exact time in its tooltip.
- **Done when:** `grep 'function format'` finds them only in `web/format.ts`.
- **Commit:** `- Dates, sizes and durations are formatted by one module; a Time component shows relative times with the exact time on hover`

#### V0.2.92 · 🧹 · One error message helper (A17)
- **Change:** `apiMessage(error, fallback)` in `web/format.ts` replaces the nine local `errorMessage` functions.
- **Done when:** `grep 'function errorMessage'` finds nothing.
- **Commit:** `- One helper turns an error into the message a toast shows; nine local copies removed`

#### V0.2.93 · 🧹 · `useAsyncAction`
- **Change:** a hook that runs a mutation with a busy flag, a success toast and an error toast. It replaces the repeated `setIsBusy(true); try … finally` blocks, starting with Profile and Tasks.
- **Done when:** both use the hook and behave as before.
- **Commit:** `- A hook for mutations (busy flag, success and error toasts), first used on the profile and task pages`

#### V0.2.94 · 🧹 · `useFormState`
- **Change:** a small hook holding a form's values, the initial values, `isDirty` and `reset`. It is used by V0.2.120's guard and by the K4 migrations.
- **Done when:** Profile › Personal details uses it.
- **Commit:** `- A small form-state hook (values, dirty flag, reset)`

#### V0.2.95 · 🧹 · System area split (A18)
- **Change:** `components/SystemAdminPanel.tsx` becomes `web/system/` with one file per page (overview, usage, users, settings, logs, jobs, audit, backups) and their shared pieces.
- **Done when:** no file in `web/system/` is over 300 lines and behaviour is unchanged.
- **Commit:** `- The System area is one file per page instead of one 900-line component`

#### V0.2.96 · 🧹 · Repositories split (A18)
- **Change:** `RepositoriesPanel.tsx` becomes `web/repositories/` (list, link form, settings, repository page, file browser).
- **Done when:** no file is over 300 lines and behaviour is unchanged.
- **Commit:** `- The repositories pages are split into list, link form, settings, repository page and file browser`

#### V0.2.97 · 🧹 · Setup wizard split (A18)
- **Change:** `ServerSetup.tsx` (21 `useState`) becomes one component per step with a shared step frame.
- **Done when:** each step owns its state and the wizard behaves as before.
- **Commit:** `- The setup wizard is one component per step`

#### V0.2.98 · 🧹 · Knowledge map split (A18)
- **Change:** `KnowledgeMapPanel.tsx` becomes the pipeline, coverage table, graph and link table.
- **Done when:** behaviour is unchanged.
- **Commit:** `- The knowledge map page is split into pipeline, coverage, graph and link table`

#### V0.2.99 · 🧹 · System pages load through the cache (A19)
- **Change:** the System pages use `useServerState` and the query cache instead of `useEffect` plus `.then`. Live job events invalidate the jobs query.
- **Done when:** no System page sets state from a request that answers after it was left.
- **Commit:** `- The System pages load through the query cache; an answer that arrives after leaving a page is ignored`

#### V0.2.100 · 🧹 · Repositories and connectors load through the cache (A19)
- **Change:** the same for the repositories and connectors pages.
- **Commit:** `- The repositories and connectors pages load through the query cache`

#### V0.2.101 · 🧹 · Map, document viewer and assistant load through the cache (A19)
- **Change:** the same for the knowledge map, the document viewer and the assistant's chat list.
- **Done when:** `grep -c useEffect` drops in each file and no request result lands after unmount.
- **Commit:** `- The knowledge map, document viewer and assistant chat list load through the query cache`

### K4. Pages on the shared pieces (V0.2.102–V0.2.118)

Each release moves one area to `Field`, `Checkbox`, `Switch`, `LoadingState`, `EmptyState`, `PanelHeading` and the other building blocks, and deletes the CSS that only that area used. **Done when** for each: the area's forms have labelled `Field`s of one height, its dropdowns behave as above, and its now-unused CSS is removed.

| Release | Area | Commit line |
|---|---|---|
| **V0.2.102** | Sign-in, registration, invitation, password reset and email verification pages | `- Sign-in, registration, invitation, reset and verification pages use the shared fields, buttons and states` |
| **V0.2.103** | Setup wizard and environment menu | `- The setup wizard and the environment menu use the shared fields, switches and states` |
| **V0.2.104** | Profile (details, password, picture, notification preferences, API tokens) | `- The profile page and API tokens use the shared fields, switches and states` |
| **V0.2.105** | Organization page (rename, members, invitations, ownership) | `- The organization page uses the shared fields, badges and states` |
| **V0.2.106** | Workspace settings (rename, AI providers, AI policy, repository links) | `- Workspace settings use the shared fields and switches` |
| **V0.2.107** | People section (members, invitations, activity) | `- The people section uses the shared fields, badges and segmented control` |
| **V0.2.108** | Evidence section (notes, uploads, folders, filters, bulk bar) | `- The evidence section's forms, filters and bulk actions use the shared pieces` |
| **V0.2.109** | Retrieval (search filters, saved searches, Ask, symbols) | `- Retrieval filters, saved searches, Ask and symbol search use the shared pieces` |
| **V0.2.110** | Tasks (create, filters, card, checklist) | `- Tasks use the shared fields, checkboxes and states` |
| **V0.2.111** | Memory section and memory card page | `- The memory section and card page use the shared fields, picker and states` |
| **V0.2.112** | Artifact page (rename, lifecycle, comments, versions) | `- The artifact page's forms use the shared pieces` |
| **V0.2.113** | Repositories (link check, settings, repository page) | `- Repository forms and pages use the shared pieces` |
| **V0.2.114** | Connectors | `- Connector forms use the shared pieces` |
| **V0.2.115** | Mail settings | `- Mail settings use the shared fields and states` |
| **V0.2.116** | System › Settings (setting rows, numbers, read-only state) | `- System settings rows use the shared fields and switches` |
| **V0.2.117** | System › Users, Logs, Jobs, Audit toolbars (filters, date range) | `- System toolbars and filters use the shared search field, dropdowns and date fields` |
| **V0.2.118** | Assistant dock and composer, command palette | `- The assistant and the command palette use the shared field and option list` |

### K5. Behaviour you can rely on (V0.2.119–V0.2.125)

#### V0.2.119 · 🔧 · Dialogs behave the same
- **Change:** a dialog focuses its first field, Enter submits it, Escape closes it, and its footer puts Cancel before the decisive action. Widths are `sm` 400 px, `md` 520 px and `lg` 720 px through a `size` prop instead of one-off classes.
- **Done when:** every dialog follows the rule (list them in the commit).
- **Commit:** `- Every dialog focuses its first field, submits on Enter, closes on Escape and has one of three widths`

#### V0.2.120 · 🔧 · Unsaved changes are not lost
- **Change:** with `useFormState`, leaving a dirty form (route change, closing its dialog) asks to discard through `useConfirm`. Applies to System › Settings, the note editor, memory card editing and workspace settings.
- **Commit:** `- Leaving a form with unsaved changes asks first (settings, notes, memory cards, workspace settings)`

#### V0.2.121 · 🔧 · Ctrl or Cmd plus Enter sends
- **Change:** in comments, notes, Ask, the assistant and memory statements, Ctrl+Enter (Cmd+Enter on macOS) submits. A muted hint says so.
- **Commit:** `- Ctrl+Enter (Cmd+Enter) sends comments, notes, questions and memory statements`

#### V0.2.122 · 🔧 · Required fields before the request (decision 19)
- **Change:** required and malformed fields (email, URL, number ranges) are marked by `Field` and block the submit with a field message. API errors stay toasts.
- **Commit:** `- Required and malformed fields are marked before anything is sent; server errors stay toasts`

#### V0.2.123 · 🔧 · Long lists page
- **Change:** use the shared `Pagination` where a list can exceed 24 rows (notifications, tasks per column if long, memory cards, System users, jobs, audit, repository files).
- **Commit:** `- Long lists show 24 rows a page with the shared pagination`

#### V0.2.124 · 🔧 · Every change says it happened
- **Change:** go through every mutation and give each a success toast (or a visible state change) and an error toast. Remove duplicate and silent ones.
- **Done when:** the commit lists each mutation and its feedback.
- **Commit:** `- Every change confirms itself with a toast or a visible state, and every failure with an error toast`

#### V0.2.125 · 🔧 · Focus lands somewhere sensible
- **Change:** after a dialog closes, focus returns to what opened it. After a deletion, focus moves to the next item or the list heading. After a route change it is already handled (V0.2.47).
- **Commit:** `- Focus returns to the opener after a dialog and moves to the next item after a deletion`

### K6. Keep it that way (V0.2.126–V0.2.129)

#### V0.2.126 · 🧪 · Tests for the building blocks
- **Change:** tests in `src/test/` for `Field` (label and control tied, field message), `Checkbox`, `Switch`, `Dropdown` (renders into a portal, keyboard), `ConfirmDialog` and `useConfirm`, `LoadingState`, `EmptyState`, `Badge`, `SegmentedControl`, `Combobox` (filtering), `SearchField` (Escape), `CopyButton` and `useFormState`. Written, not run (CI runs them).
- **Commit:** `- Tests for every shared building block (written; CI runs them)`

#### V0.2.127 · 🧪 · A style check that stops the drift
- **Change:** extend `scripts/check-styles.mjs` to fail on:
  - a bare `input`, `select`, `textarea` or `button` selector outside the base reset and the local-edition scope;
  - a hex or rgb colour outside a `:root` token block;
  - a `z-index` that is not a layer token;
  - a control height that is not a control token.
- **Done when:** `npm run styles:check` passes on the tree and fails on a planted example (described in the commit).
- **Commit:** `- The style check refuses bare element styles, raw colours, raw layers and raw control heights`

#### V0.2.128 · 📘 · DESIGN.md matches the code (A21, decision 16)
- **Change:** update DESIGN.md:
  - the accent is green, as shipped;
  - controls are 36 px (40 on touch screens);
  - the layer scale, and that neutrals have saturation 0;
  - the button treatments: `main`, `secondary`, `danger` and `IconButton`;
  - the building blocks and when to use each;
  - the field-message rule;
  - the icon sizes.

  Update the technical mindmap §9.
- **Commit:** `- DESIGN.md describes the interface as it is built: accent, sizes, layers, buttons and building blocks`

#### V0.2.129 · 📘 · Release check after the polish track
- **Change:**
  - run the documentation, style, accessibility, type and version checks;
  - go once through the clean-install runbook's web steps (the maintainer);
  - update the release notes for V0.3.0 with this track;
  - restore the feature freeze until the tag.
- **Commit:** `- Release checks repeated after the frontend polish track; the freeze is back on until V0.3.0`

---

## 5. Not in this track

- **A new visual identity** (palette, type, spacing scale): decision 16 keeps the shipped look.
- **The retired local edition** (`App.tsx`): only its styles are fenced in (V0.2.64).
- **New product features**: they wait for after V0.3.0.
