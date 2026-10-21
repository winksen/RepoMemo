---
name: RepoMemo Neutral Research Canvas
description: A light, composed productivity workspace that places a local knowledge task beside its inspectable context.
colors:
  canvas: "#f2f2f2"
  surface: "#ffffff"
  surface-subtle: "#fafafa"
  ink: "#262626"
  ink-muted: "#616161"
  divider: "#e6e6e6"
  action-blue: "#1228ce"
  action-blue-soft: "#f0f3ff"
  suggestion-wash: "#f4e7d2"
typography:
  display:
    fontFamily: "Fjalla One, Arial Narrow, sans-serif"
    fontSize: "30px"
    fontWeight: 400
    lineHeight: 1.12
    letterSpacing: "-0.025em"
  body:
    fontFamily: "Plus Jakarta Sans Variable, Segoe UI, system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.55
rounded:
  control: "8px"
  plane: "12px"
sizes:
  control-height: "36px"
  control-height-touch: "40px"
  icon-button-compact: "32px"
spacing:
  compact: "8px"
  module: "16px"
  panel: "24px"
  section: "32px"
---

# Design System: RepoMemo

## Overview

**Creative North Star: "Neutral Research Canvas"**

RepoMemo is a quiet working surface for understanding local project material. It follows the practical productivity composition approved from the Rox reference: a narrow icon rail, one generous primary canvas, and a fixed contextual note. The product rejects a system-monitor dashboard; the task is always more visually important than the application's machinery.

The reader should be able to select a workspace, search or ask a question, and inspect the supporting local context without scanning colored telemetry. Provenance is represented by meaningful source names, citations, paths, and concise counts.

## Color and tone

Light mode is the product's default. The physical scene is a developer working through a project on a normal desktop display, with enough contrast for long reading and no themed ambient field. White is the working surface; a neutral grey (saturation 0, in both themes) establishes gutters and inactive controls; thin grey dividers define regions.

Blue (`--color-accent`, `hsl(233 84% 44%)`) is the only expressive color, as shipped; the green accent that older stylesheets declared is dead and gone. It denotes keyboard focus, a direct primary action, and an active tab. Green, amber, and red remain semantic exception colors only; they never brand broad regions. A warm suggestion wash may appear only behind actionable assistant recommendations, never as general decoration.

## Typography

Body text is Plus Jakarta Sans (with Segoe UI and the system sans as fallbacks) in a straightforward product-editor rhythm; page titles use the compact display face, Fjalla One. Supporting copy is 14px and deliberately spacious. Monospace is limited to file paths, identifiers, ranges, and code.

- Page title: 28–32px / display face / -0.025em.
- Panel title: 16px / 600.
- Body: 14px / 1.55, at 65–75ch where reading matters.
- Metadata: 12px / 400, neutral gray.

## Layout

Desktop uses a 64px icon rail, a flexible primary canvas, and a 400px contextual right pane. The global title bar is light, compact, and divided from content by a single hairline. The primary and contextual columns scroll independently when needed. On narrow screens, the rail becomes a horizontal icon strip and contextual panes move below the primary task.

The Workspace route uses the central canvas to browse and create workspaces. Its right note gives local context and one next action; it deliberately omits diagrams, pipeline meters, and repeated system totals. Search, Ask, Summary, Artifacts, Memory, and Settings retain this task-plus-context relationship.

## Components

Controls (text fields, dropdowns and buttons) are 36px tall, 40px on touch screens (narrow windows), with a 2px border, softly rounded at 8px, and communicate through text and icons rather than colored blocks. They take their size from the control tokens (`--control-height`, `--control-height-touch`, `--control-radius`, `--control-border`, `--control-padding-x`), never from a pixel value of their own. Surfaces use one faint divider or a very soft offset shadow, never both by default. Selected rows use neutral surfaces with stronger ink or a neutral boundary. Never pair accent-colored text or icons with an accent-tint background on a component or button—at rest, selected, or hovered. Reserve accent backgrounds for solid primary actions and accent color for focus and restrained status signals. User-supplied logo artwork remains black and white; do not recolor it with the accent. Suggested actions can use the reserved warm wash and a small trailing action control. Inputs are white with a subtle border and blue focus outline.

### Shared UI primitives

The web client uses shadcn's source-owned component pattern. Components live in
`apps/desktop/src/components/ui/`, remain editable in this repository, and use
Radix only for interaction behavior. There are three button treatments and one
icon shape, and no others:

- `Button variant="main"`: the single decisive action in a group.
- `Button variant="secondary"`: every other action (the default).
- `Button variant="danger"`: the decisive action of a destructive confirmation.
- `IconButton`: a button with one icon and a required label.

`Dropdown` has one accessible visual treatment for all selects: its trigger
takes the width of its content (at least 160 px) in a toolbar and fills its
field in a form; its list is at most 320 px tall and scrolls inside the window;
it opens above a dialog. `Input` and `Textarea` own all text-entry fields: a
calm neutral boundary at rest and a solid blue boundary on hover and focus.
Control-state transitions use the shared ease-in-out curve; buttons never move
position on hover. A `Button` is `type="button"` unless it says `type="submit"`.
Do not introduce another button or dropdown treatment without updating this
system deliberately.

### Layers

Every `z-index` in the stylesheets is one of the layer tokens of
`styles/01-tokens-and-shared-web.css` (0, -1 and auto are the only literals),
so a menu can never open behind the thing that holds it:

| Token | Value | For |
|---|---|---|
| `--layer-raised` | 1 | a part lifted above its neighbours (table headers, map controls) |
| `--layer-sticky` | 10 | a menu attached to a control (the file type filter) |
| `--layer-float` | 20 | the assistant dock's button on narrow screens |
| `--layer-sheet` | 800 | the backdrop of the assistant sheet, and `--layer-sheet-panel` 801 for its panel |
| `--layer-overlay` | 900 | the backdrop of a dialog |
| `--layer-dialog` | 901 | dialogs |
| `--layer-popover` | 950 | dropdown lists, action menus, mention lists: above dialogs |
| `--layer-toast` | 1000 | toasts |

### Icons

Every icon next to text takes one of three sizes from `ICON_SIZE`
(`components/ui/icon.ts`): `sm` 14px in dense rows and badges, `md` 16px next to
text and inside controls, `lg` 18px in panel headings. Larger icons are
illustrations (empty states, setup steps, file and folder tiles, avatars) and
keep their own size.

### Building blocks

Shared pieces beyond the primitives, in `apps/desktop/src/components/ui/` with
their styles in `styles/14-building-blocks.css`. Use them instead of building a
page's own version, and add to this list when a new one is added.

- **`Field`** (and `FieldRow`): a label tied to its control, an optional muted
  hint and, while the field is invalid, a one-line message under it. A required
  or malformed field is marked `aria-invalid` (only its border turns red) and
  says why in that line; it is a field state, never a banner or a tinted fill.
  Server errors stay toasts. `FieldRow` puts fields side by side and stacks
  them on narrow screens. See "Forms" below for when a field marks itself.
- **`Checkbox`**: a 16 px box (accent when checked), its label and an optional muted
  description, built on Radix; the label toggles it. Use it for a choice in a
  list or a form; use `Switch` for a setting that applies at once.
- **`Switch`**: an on and off setting, a 36 by 20 px track with a round thumb,
  built on Radix. Give it a `label` beside it (for example "On" or "Off"), an
  `aria-label`, or a `<label htmlFor>` tied to its `id`. It does not replace a
  `Checkbox`: use a checkbox to pick items, a switch for a state.
- **`Button` with `busy`**: while work is in progress a spinner replaces the
  button's icon, the label stays, the button is disabled and `aria-busy` is
  set. Use it on the button that started the work instead of a hand-made
  spinner. `Spinner` is the same turning ring for inline use.
- **`IconButton`**: a square button with one icon and a required `label` (its
  accessible name), 32 px (`size="sm"`, in rows and menus) or the control
  height (`md`, beside fields). `appearance="plain"` is bare until hovered;
  `pressed` marks a toggle that is on. An icon-only button is always an
  `IconButton`, never a `Button` with an `aria-label`. It is the third button
  treatment next to `main` and `secondary` (decision 18).
- **`Tooltip`**: a short neutral label (inverted greys) above a control, on
  hover and on keyboard focus, built on Radix. `IconButton` shows its `label`
  in one. Use it instead of a `title` attribute to explain a control; wrap a
  disabled control in a `span` so the tooltip can still open.
- **`Button variant="danger"`**: the solid decisive action of a destructive
  confirmation (`--color-danger` fill, white text). Use it only for the one
  button that deletes or removes something for good in a dialog's footer; the
  row button that opens that dialog stays `secondary`.
- **`ConfirmDialog` and `useConfirm`**: a question in the app's own dialog, never
  `window.confirm`. `const confirm = useConfirm(); if (!(await confirm({ title,
  description, action, danger }))) return;` resolves `true` on the decisive
  action and `false` on Cancel, Escape or a click outside. Name the action after
  what it does ("Delete", not "OK") and set `danger` when it cannot be undone.
  `ConfirmProvider` is mounted once in `SharedWebApp`.
- **`Dialog`**: the one modal: a title, an optional description, a body and a
  footer of actions, built on Radix. Its `size` is `sm` (400 px, a question),
  `md` (520 px, a small form; the default when it has a body) or `lg` (720 px,
  long content such as a version or the palette). It focuses its first writable
  field when it opens, Escape closes it, focus returns to what opened it, and
  the footer puts Cancel before the decisive action. A dialog with fields takes
  `onSubmit`, so Enter in a field runs the decisive action, and that footer
  button says `type="submit"`. Never build a modal of your own.
- **`ActionMenu`**: the "more actions" button of a row or card (`IconButton` plus
  a Radix menu); a destructive item is marked `destructive` and opens a
  confirmation rather than acting at once.
- **`Pagination`** (with `usePagedList`): a list that can pass 24 rows shows 24 a
  page, with "1–24 of 60" and previous and next buttons under it; it is hidden
  while everything fits. A filter returns the list to its first page.
- **`LoadingState`** and **`Spinner`**: a turning ring and one muted line with
  `role="status"` for something still loading. Word the label `Loading <thing>`
  (`Loading jobs`); the component adds the ellipsis. Use `Spinner` alone inside
  a button or a line of text.
- **`EmptyState`**: what a list shows when it has nothing in it: an optional
  illustration-sized icon, a title, one muted sentence and at most one action,
  centred in its panel. Say what to do next, not only that nothing is there.
- **`Badge`**: a short label for a state, role or kind. Neutral surface, border
  and grey text; a `tone` (`success`, `warning`, `danger`, `info`) only adds a
  small status dot, never a tinted fill with same-hue text. An optional `icon`
  goes before the text. Do not invent a tag or chip class.
- **`SegmentedControl`**: one choice out of a few that are all shown at once,
  as joined segments (Radix Toggle Group): sign in or create account, a view
  switch, a filter, the sheets of a workbook. Arrow keys move between the
  segments and the chosen one cannot be turned off. Options have a label, an
  optional icon and count, or only an icon with a `name`. For many options use
  a `Dropdown`.
- **`PanelHeading`**: the heading of a panel: a Tabler icon, the title (an `h2`;
  `level={3}` for a part of a panel), an optional muted `description` at the end
  (a count or a state) and optional `actions` (buttons, a status). Panel titles
  are 16 px at most; the page title is the `PageHeader`.
- **`PageHeader`**: the one header of a page: the title (the page's only `h1`,
  28 to 32 px), a short description, actions at the end and an optional back
  link. The dashboard, workspace, artifact, memory card, profile, notifications
  and System pages use it; a panel inside a page uses `PanelHeading`.
- **`Table`** (and the `rm-table` styles): the one look for a data table: a box
  that scrolls sideways on narrow screens, a header that stays in view past
  640 px, a hover on each row and numbers (`className="rm-table-number"`)
  right-aligned in tabular figures. Use `<Table>` around a `<thead>` and a
  `<tbody>`; do not style a table of your own.
- **`Combobox`**: chooses one item out of many by typing part of its name (a
  Radix Popover with a search field and a list of matches; arrow keys move,
  Enter chooses, Escape closes). It looks like a `Dropdown`. Use a `Dropdown` for
  a handful of options and a `Combobox` where scrolling the whole list would be
  slow: files, people, tasks.
- **`SearchField`**: the text field for filtering a list: a search icon, a clear
  button once there is text and Escape to clear it. Use it for a filter that
  narrows a list as you type; a search that needs a button to run stays a
  `Field` with an `Input`.
- **`CopyButton`**: copies a value to the clipboard. The icon becomes a check
  (and the word "Copied") for two seconds; if the browser refuses, a toast says
  so and the text is left to copy by hand. Icon-only by default (give it a
  `label`), or a button with a word through `text`. Every copy action uses it.
- **`Time`**: a moment as a relative time ("3h ago") in a `<time>` element,
  with the exact date and time in a tooltip. Dates and sizes in text come from
  `web/format.ts` (`formatDate`, `formatTime`, `formatBytes`, `formatDuration`),
  never from a local function.
- **Option list** (`rm-option-list` with `rm-option` rows): the rows of any list
  you pick from by keyboard or mouse: the command palette, mention suggestions,
  the folder picker and the chat list. One row height (the control height), one
  highlight for hover and for the active row (`aria-selected`, `aria-pressed` or
  `aria-current`), one focus ring. `rm-option-text` stacks a title over a small
  line; `rm-option-row` holds an option next to a menu button.

### Notifications

Errors and success confirmations are toasts, never inline banners. They stack at
the bottom right, dismiss themselves after a few seconds (errors stay longer),
pause while hovered, and can be closed manually. Use `showToast` or `<Toast>`
from `components/ui/toast.tsx`. A toast is a solid fill in its status color
(red error, green success, amber warning, blue info) with white text and a
white icon, and no border. This is the one place a colored fill is used for
status; it still never pairs tinted fills with same-hue text.

Every change confirms itself: a mutation ends in a success toast ("Comment
added.") or in a change the person can see (a card moved to another column, a
secret dialog opening), and a failure in an error toast. Do not add a toast for
the start of work that announces its own end, such as a repository sync.

### Forms

- **Marking mistakes.** A control inside a `Field` that the browser refuses on
  submit (`required`, `type="email"` or `"url"`, `minLength`, a range) marks
  itself: `aria-invalid`, a one-line message under it ("Title is required.",
  "Enter a value from 1 to 65535."), focus on the first one, and it clears as
  soon as the value is valid. Nothing is sent while a field is refused. A rule
  only the page knows is passed as `invalid` and `message`. API errors are
  toasts, never a message under a field.
- **Sending.** Enter in a single-line field submits its form; Ctrl+Enter
  (Cmd+Enter on macOS) submits from a `Textarea` and from the note editor, and a
  muted hint under the field says so (`submitHint()`). A button that sends is
  the form's one `type="submit"` button and shows `busy` while it works.
- **Unsaved changes.** A form that holds work (System settings, a note being
  written, a memory card being edited, a new workspace name) asks "Discard
  unsaved changes?" through `useConfirm` before the page changes, and the browser
  warns before the tab closes (`useUnsavedChanges`). A draft that the page
  keeps while someone moves between sections of a workspace does not ask.
- **Focus.** After a dialog closes, focus is on the control that opened it; after
  a deletion it moves to the next item, else the previous one, else the heading
  of the list. After a move to another page it is at the start of the page.

### No tinted neutrals

Backgrounds, borders and body text are true neutral greys (saturation 0) in both
themes. Do not give surfaces and text a shared hue (blue-grey, green-grey and so
on) and do not pair a color-tinted background with same-hue text; it reads as
generic AI-generated styling. Color appears only as the single accent and the
semantic status colors, used sparingly and never as a tinted fill with matching
text.

This applies to hover and focus states too. When asked to change one property of
a state (for example a danger hover border), change only that property; never add
a tinted background or recolor the text to match.

## Do's and don'ts

- Do make the primary task the largest, brightest surface.
- Do expose local evidence in a calm companion pane instead of permanent telemetry.
- Do reserve blue for action, current state, and focus.
- Do use a building block above before writing a page's own heading, tag, tab, loading line, empty line, dialog or confirmation.
- Do take sizes, colours and layers from the tokens; `npm run styles:check` refuses a new bare element style, raw colour, raw layer or raw control height.
- Don't make the dark theme the default, and don't use grids, terminal motifs, traced diagrams, or dashboard metric strips as the default experience.
- Don't repeat workspace state in a graph, metrics row, and readiness tracker.
- Don't use all-caps technical labels or monospace as ordinary interface copy.
