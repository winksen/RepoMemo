# RepoMemo UI Design And Visual System

This guide is the product-facing design source of truth for RepoMemo. It should be used before implementing or changing UI in `apps/desktop`.

RepoMemo is a private, local-first memory workspace for technical teams. The UI should feel like a compact technical workbench: calm, fast, trustworthy, and built for repeated daily use. It should not feel like a generic AI dashboard, a marketing site, or a chat box wrapped around files.

## Design Thesis

RepoMemo should look like a serious archive console for code and decisions.

The visual language combines:

- a dense desktop-app layout
- warm editorial surfaces for memory and documents
- Spotify-like block separation where each major region owns its own rounded space
- graphite and cool grey neutrals for chrome, panels, rows, and navigation
- blue only as a restrained highlight for primary actions, links, focus, and active affordances
- restrained semantic accents for state, provenance, and retrieval confidence

The result should feel quiet and opinionated: more Raycast, Linear, GitHub, and native desktop utility than SaaS landing page.

## Experience Principles

### 1. Local-first confidence

Users should always understand where data lives, what has been indexed, and whether AI or cloud services are involved. Storage paths, provider mode, indexing state, and citation source should be visible where they matter.

### 2. Dense, not cramped

RepoMemo is for technical work. Favor compact rows, side panels, keyboardable lists, and clear information hierarchy. Avoid oversized hero sections, decorative cards, large empty marketing space, and one-item-per-screen layouts.

### 3. Retrieval before generation

Search results, artifact context, line ranges, symbols, and citations are first-class UI. AI answers are useful only when the underlying evidence stays inspectable.

### 4. Progressive disclosure

Show the short answer first, then make details easy to expand: import logs, chunk metadata, symbol outlines, provider settings, relationship evidence, and indexing diagnostics.

### 5. Dark and light are equal citizens

Do not design light mode first and invert it later. Every token must have a deliberate light and dark value. Dark mode should use layered surfaces that become lighter with elevation, not pitch-black panels with neon accents.

### 6. Stylish by restraint

The app should get its character from color discipline, typography, icon rhythm, density, and small interaction details. Avoid gradient blobs, glassmorphism, purple-blue AI gradients, decorative bokeh, novelty shadows, and illustrations that do not explain real product state.

## Visual Direction

### Personality

Use these words as a design filter:

- precise
- grounded
- archival
- technical
- compact
- private
- quietly premium

Avoid these impressions:

- chatbot-first
- crypto dashboard
- whimsical productivity toy
- generic admin template
- glowing AI assistant
- beige notebook app

### First-viewport Goal

The first screen should immediately communicate:

- which workspace is open
- what sources/artifacts exist
- what is searchable or still indexing
- what action is most useful next

For the current skeleton, the useful next action is workspace creation or selection. In later phases, it becomes import, search, or ask with citations.

## Design Tokens

Use semantic tokens in UI code. Component CSS should refer to roles such as `--color-surface` and `--color-text-muted`, not raw hex values.

### Theme Tokens

```css
:root {
  color-scheme: light;

  --color-bg: #e9edf2;
  --color-canvas: #dfe4ea;
  --color-surface: #f9fafb;
  --color-surface-2: #eef1f5;
  --color-surface-3: #e0e5ec;
  --color-surface-hover: #e7ebf0;
  --color-text: #171a1f;
  --color-text-muted: #5f6670;
  --color-text-subtle: #8b929d;
  --color-border: #d1d7df;
  --color-border-strong: #b8c0ca;

  --color-accent: #2f6fed;
  --color-accent-hover: #255dcc;
  --color-accent-soft: #e8f0ff;
  --color-accent-text: #2454b8;
  --color-on-accent: #ffffff;

  --color-info: #315a93;
  --color-info-soft: #dfe9f8;
  --color-success: #2b744f;
  --color-success-soft: #dff1e6;
  --color-warning: #965613;
  --color-warning-soft: #f8ead1;
  --color-danger: #ad3a2b;
  --color-danger-soft: #f8ded8;
  --color-discovery: #6b4aa3;
  --color-discovery-soft: #ebe4f7;

  --color-focus: #d58535;
  --shadow-raised: 0 12px 34px rgba(32, 35, 31, 0.12);
  --shadow-floating: 0 24px 70px rgba(32, 35, 31, 0.18);
}

:root[data-theme="dark"] {
  color-scheme: dark;

  --color-bg: #050505;
  --color-canvas: #0a0a0a;
  --color-surface: #121212;
  --color-surface-2: #1c1c1c;
  --color-surface-3: #2a2a2a;
  --color-surface-hover: #242424;
  --color-text: #f5f5f5;
  --color-text-muted: #b8b8b8;
  --color-text-subtle: #777777;
  --color-border: #2d2d2d;
  --color-border-strong: #3a3a3a;

  --color-accent: #60a5fa;
  --color-accent-hover: #93c5fd;
  --color-accent-soft: #1f2a3a;
  --color-accent-text: #9ecbff;
  --color-on-accent: #08111f;

  --color-info: #7ea7e0;
  --color-info-soft: #1c304d;
  --color-success: #72c596;
  --color-success-soft: #173522;
  --color-warning: #e2a64f;
  --color-warning-soft: #3c2b14;
  --color-danger: #eb7a61;
  --color-danger-soft: #451f19;
  --color-discovery: #b59af0;
  --color-discovery-soft: #302449;

  --color-focus: #ffb86c;
  --shadow-raised: 0 12px 34px rgba(0, 0, 0, 0.28);
  --shadow-floating: 0 24px 70px rgba(0, 0, 0, 0.42);
}
```

### Color Roles

| Role | Use |
| --- | --- |
| `bg` | App background and page base. |
| `canvas` | Broad workbench area behind panels. |
| `surface` | Primary panels, dialogs, menus, and list containers. |
| `surface-2` | Rows, inputs, secondary panels, and hover backgrounds. |
| `surface-3` | Selected rows, nested metadata strips, and elevated detail zones. |
| `surface-hover` | Neutral hover state for grey-first controls and rows. |
| `text` | Primary readable content. |
| `text-muted` | Secondary metadata, descriptions, timestamps. |
| `text-subtle` | Disabled or lowest-emphasis helper text. |
| `border` | Default dividers and component outlines. |
| `border-strong` | Focused sections, selected items, table headers. |
| `accent` | Primary action, links, focus, and small active highlights only. |
| `info` | Indexing, processing, neutral progress. |
| `success` | Completed imports, available indexes, verified providers. |
| `warning` | Incomplete indexing, provider fallback, recoverable issues. |
| `danger` | Failed import, destructive actions, permission errors. |
| `discovery` | New capability, AI suggestion, relationship insight. |

Use semantic colors only for meaning. A green button means success or completion, not decoration. A red badge means risk or failure, not "important."

### Layering Rules

Light mode:

- use `bg` as the outer app gutter and `canvas` as the inter-block spacing layer
- let major zones sit on `surface` with rounded corners and modest elevation
- use `surface-2`, `surface-3`, and `surface-hover` for rows and nested controls
- keep blue out of large backgrounds; use it for buttons, links, focus, and small active marks

Dark mode:

- base layer starts near black at `bg`
- raised layers get lighter: `canvas`, `surface`, `surface-2`, `surface-3`
- avoid components darker than the page background unless they are code blocks or terminal-like surfaces
- keep the app grey-first; blue remains a highlight, not the dominant theme

This follows the practical layering model used by mature systems such as Carbon, adapted to RepoMemo's palette.

## Typography

RepoMemo uses Outfit everywhere, bundled locally through `@fontsource/outfit`. Do not fetch remote fonts by default, including Google Fonts at runtime.

### Font Stacks

```css
--font-ui: "Outfit", system-ui, sans-serif;
--font-brand: "Outfit", system-ui, sans-serif;
--font-mono: "Outfit", system-ui, sans-serif;
```

### Type Scale

| Token | Size / line-height | Weight | Use |
| --- | --- | --- | --- |
| `text-xs` | 11px / 16px | 500-700 | badges, timestamps, metadata |
| `text-sm` | 12px / 16px | 400-700 | labels, compact controls |
| `text-md` | 14px / 20px | 400-700 | default UI body |
| `text-lg` | 16px / 22px | 600-700 | section titles, row titles |
| `title-sm` | 20px / 28px | 650-750 | panel headers |
| `title-md` | 26px / 34px | 650-750 | page/workspace title |
| `title-lg` | 34px / 42px | 650-750 | rare empty-state or welcome title |

Rules:

- Use sentence case for UI text.
- Keep letter spacing at `0`.
- Use Outfit weights and scale, not different typefaces, to separate wordmark, titles, metadata, and body text.
- Use `font-mono` only as a semantic token for paths, hashes, command names, code symbols, and line ranges; it still resolves to Outfit for visual consistency.
- Do not use all-caps labels except legacy eyebrow text that already exists; prefer small sentence-case labels going forward.

## Spacing, Radius, And Density

Use a 4px spacing grid with a few half-step values for icon alignment.

| Token | Value | Use |
| --- | ---: | --- |
| `space-0` | 0 | reset |
| `space-1` | 2px | hairline alignment |
| `space-2` | 4px | tight icon/text gap |
| `space-3` | 6px | compact internal gap |
| `space-4` | 8px | default small gap |
| `space-5` | 10px | icon button padding |
| `space-6` | 12px | compact component padding |
| `space-8` | 16px | panel internal rhythm |
| `space-10` | 20px | section rhythm |
| `space-12` | 24px | page gutter minimum |
| `space-16` | 32px | large page gutter |

Recommended control heights:

| Component | Height |
| --- | ---: |
| Icon button | 32px |
| Compact button | 34px |
| Default button/input | 38px |
| Search/command field | 42px |
| Compact list row | 40px |
| Rich list row | 56px |
| Table row | 36px |
| Sidebar item | 36px |

Radius:

- `4px` for tags, checkboxes, table cells, code chips
- `6px` for buttons, inputs, menu items, list rows
- `8px` for panels, dialogs, repeated cards
- `999px` only for status pills, avatars, and progress capsules

Do not use rounded card piles or cards inside cards. If sections need separation, use layout, spacing, subtle backgrounds, and dividers.

## Layout System

### Desktop Shell

Use a stable three-zone model as the app grows, with Spotify-like spacing between each major block:

```text
Outer gutter
  Left rail block      Main workbench block       Detail/context block
  Workspace/nav   ->   import/search/lists   ->   preview/citations/metadata
```

Recommended widths:

- left rail: `240px` compact, `280px` comfortable
- main workbench: flexible, minimum `520px`
- right detail panel: `340px` to `440px`
- page gutter: `24px` desktop, `16px` narrow
- inter-block gap: `8px` to `12px`
- major block radius: `10px` to `12px`

Scrolling contract:

- the app shell is viewport-bound; do not allow `body`, `#root`, or the top-level app grid to become the primary scroll container
- the sidebar is a fixed/sticky block and should not scroll with main content
- the top header/status region is its own rounded block, separated from the content blocks by the same inter-block gap
- each main section is its own rounded block and owns overflow when its content exceeds the available height
- lists, previews, reports, and detail panes may scroll internally; avoid nested page-level scrolling

The current two-column skeleton can evolve into this model without visual churn.

### Navigation

Primary navigation should stay vertical and compact:

- Workspaces
- Import
- Artifacts
- Search
- Ask
- Memory cards
- Settings

Navigation items use an icon plus label. Active state uses a neutral selected background plus text/icon emphasis. Do not use a left-border active indicator. Disabled items should explain availability with a tooltip when possible.

### Workbench Header

The header should show:

- workspace name
- indexing/search state
- primary command or search field
- a compact provider/privacy indicator when AI is enabled

Do not make the header a hero. It is a control surface.

### Main Content Patterns

Use list/table/detail patterns:

- Workspaces: list + detail summary
- Imports: source list + job progress + log drawer
- Artifacts: filterable table/list + preview
- Search: command field + ranked results + filters + preview
- Ask: prompt + retrieved context + answer + citations
- Memory cards: compact cards/list + source links
- Settings: grouped forms with clear save state

## Component Guidance

### Buttons

Variants:

- Primary: one per view; use `accent`.
- Secondary: neutral surface with border.
- Ghost: transparent, hover surface only.
- Destructive: danger only for destructive confirmation.
- Icon-only: must have tooltip and accessible label.

Button labels should be verbs: `Create`, `Import folder`, `Reindex`, `Save memory`.

### Inputs And Forms

Inputs should be compact and calm:

- default height `38px`
- visible label above or inline for dense settings rows
- helper text only when it prevents an error
- validation messages near the field
- focus ring uses `--color-focus`

For path and provider settings, favor copy buttons, reveal buttons, and validation status badges.

### Search And Command

Search is the core interaction, not a secondary page.

Search field anatomy:

- leading search icon
- query text
- optional scope token, such as `workspace`, `docs`, `code`, or `memory`
- keyboard hint on the right

Search results should show:

- title
- path/source
- snippet with highlighted match
- artifact type
- updated/indexed state
- confidence or rank when useful

### Tables And Lists

Use tables for comparison and scanning. Use lists for ranked retrieval and conversational context.

Rules:

- fixed row heights where possible
- sticky headers for long lists
- timestamp and status columns are right aligned or visually secondary
- selected rows use neutral background changes, text weight, and optional compact dots or icons; never use a left-side stripe or left border
- row actions appear on hover and keyboard focus

### Panels

Panels hold real tools or repeated item groups. They should use:

- `surface`
- no default visible border
- `10px` to `12px` radius for major app blocks
- low or no shadow inside the app canvas

Use shadows mainly for overlays, command palettes, dialogs, and floating menus.

### Empty States

Empty states should be useful, not cute.

Good:

- "No artifacts indexed yet."
- "Import a folder to make this workspace searchable."
- primary action: `Import folder`

Avoid:

- large mascots
- abstract illustrations
- vague AI promises
- paragraphs explaining the entire product

### Status Badges

Badges are compact, role-based, and paired with text:

- `Ready`
- `Indexing`
- `Partial`
- `Failed`
- `Local`
- `Cloud off`
- `AI enabled`

Status should not rely on color alone. Include icon or text where space allows.

### Citations

Citations are product-critical.

Citation chips should include:

- artifact title
- path or source
- line range when available
- confidence or retrieval rank if useful

Clicking a citation opens the artifact preview at the referenced chunk. Hover can reveal the full path.

### Memory Cards

Memory cards are durable knowledge, not decorative cards.

Each memory card should show:

- title
- short body excerpt
- source type: user, summary, answer
- linked artifacts/chunks
- last verified or updated timestamp

Use cards only when the card itself is a meaningful object. For browsing many memory cards, also provide a dense list mode.

### AI Answer Surface

The answer UI must preserve trust:

- retrieved context appears before or beside the generated answer
- answer includes citation chips inline or immediately below relevant claims
- insufficient context is a valid final state
- provider and mode are visible: local, cloud, no AI
- save-to-memory requires explicit user action

Do not present AI output as the source of truth.

## Iconography

Use `@tabler/icons-react` for all app icons.

Rules:

- `16px` for dense metadata and table actions
- `18px` for nav and standard buttons
- `20px` for panel headers
- `24px` for brand mark or empty state
- stroke width between `1.75` and `2`; use Tabler's `stroke` prop when tuning weight
- no mixed icon families
- icon-only buttons need tooltips and accessible labels

Use icons to clarify object type and action, not as decoration.

## Motion And Interaction

Motion should make the app feel responsive without feeling theatrical.

Durations:

- hover/focus: `100ms`
- small state changes: `140ms`
- panel open/close: `180ms`
- modal/command palette: `220ms`

Easing:

```css
--ease-standard: cubic-bezier(0.2, 0, 0, 1);
--ease-emphasis: cubic-bezier(0.2, 0, 0, 1);
```

Rules:

- respect `prefers-reduced-motion`
- never animate layout in long lists during indexing
- use skeletons for known shapes
- use progress indicators for jobs that can take more than one second

## Dark And Light Mode Implementation

Use a single theme attribute:

```html
<html data-theme="light">
<html data-theme="dark">
```

Recommended setting values:

- `system`
- `light`
- `dark`

Implementation rules:

- default to `system`
- persist the user's choice in app settings
- use `prefers-color-scheme` when mode is `system`
- apply the theme before React paints to avoid flash
- keep syntax highlighting, charts, and file-type icons theme-aware
- test both themes for every new component

## Accessibility And UX Baseline

Required:

- WCAG AA contrast: `4.5:1` for normal text, `3:1` for large text and essential UI marks
- keyboard access for all commands, dialogs, tabs, lists, and menus
- visible focus state on every interactive element
- no color-only status communication
- dialog focus trap and Escape close where appropriate
- reduced-motion support
- hit targets at least `32px`, preferably `36px` or larger in dense areas
- labels for icon-only controls
- preserved text selection for paths, code, snippets, and answers

Content rules:

- use direct, concrete language
- prefer "Indexing paused" over "Something happened"
- mention the user's next action in errors
- keep destructive confirmations specific: "Delete workspace metadata?" not "Are you sure?"

## Product Workflows

### Workspace Creation

Goal: start a local memory container.

UI should show:

- workspace name input
- storage root
- local-only privacy note where relevant
- recently opened workspaces
- create/open success state

Avoid showing future features as primary controls until they are usable.

### Import And Index

Goal: bring files into the memory system.

UI should show:

- accepted source types
- destination workspace
- import progress
- skipped files and reasons
- indexing state by source
- reindex action

Use a job list or activity drawer for detailed logs.

### Search

Goal: find exact technical context fast.

UI should show:

- prominent search field
- filters for source, type, language, and date
- result snippets
- path and line range
- preview pane

Search must work without AI.

### Ask

Goal: synthesize indexed context with citations.

UI should show:

- prompt input
- retrieved context
- provider mode
- generated answer
- citation chips
- save-to-memory action

When context is weak, say so clearly and suggest import/search steps.

### Memory Cards

Goal: preserve important team knowledge.

UI should show:

- concise card title
- body excerpt
- linked evidence
- source and confidence
- update/verify actions

Memory cards should feel durable, not like chat messages.

## CSS Implementation Checklist

Before merging UI work:

- component uses semantic tokens
- light and dark mode both reviewed
- focus state visible
- hover and selected states distinct
- row/control heights are stable
- text does not overflow buttons, cards, or panels
- no raw hex values in component CSS except token definitions
- no selected-state left border or left stripe
- blue appears only as highlight/action/focus/link, not broad panel color
- no decorative gradients, orbs, bokeh, or glass panels
- icons come from `@tabler/icons-react`
- empty/loading/error states exist
- keyboard path is considered

## Future UI Package

When the UI grows beyond the desktop skeleton, consider extracting shared components under:

```text
packages/ui/
  tokens.css
  Button.tsx
  IconButton.tsx
  TextField.tsx
  Badge.tsx
  Panel.tsx
  SearchField.tsx
  DataList.tsx
  CitationChip.tsx
```

Do this only when at least three screens reuse the same patterns.

## References Checked

These systems informed the guide, but RepoMemo should not copy any of them directly:

- Atlassian Design, color roles, design tokens, dark mode, and contrast guidance: https://atlassian.design/foundations/color/
- Carbon Design System, role-based tokens and light/dark layering model: https://carbondesignsystem.com/elements/color/overview/
- Fluent 2, design tokens, spacing, typography, and accessibility-oriented color use: https://fluent2.microsoft.design/
- GitHub Primer, product UI primitives and token naming patterns: https://primer.style/product/primitives/color/
- Radix Themes, 12-step color scales, accent/gray pairing, focus colors, and panel backgrounds: https://www.radix-ui.com/themes/docs/theme/color
- Outfit, RepoMemo's bundled app typeface: https://fonts.google.com/specimen/Outfit
- Tabler Icons, RepoMemo's icon family: https://tabler.io/icons
