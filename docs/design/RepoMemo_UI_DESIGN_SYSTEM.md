# RepoMemo UI Design And Visual System

This guide is the product-facing design source of truth for RepoMemo. Read it before implementing or changing any UI in `apps/desktop`. Per-screen layout specs live in [`RepoMemo_PAGE_BLUEPRINTS.md`](./RepoMemo_PAGE_BLUEPRINTS.md). Reference mockups live in [`mockups/`](./mockups/).

RepoMemo is a private, local-first memory workspace for technical teams. The UI should feel like a compact technical workbench: calm, fast, trustworthy, and built for repeated daily use. It must not feel like a generic AI dashboard, a marketing site, or a chat box wrapped around files.

> **v2 note for implementers.** This revision fixes the issues that made the first build look flat and generic: (1) dark-mode surfaces were near-identical near-black greys, so the "Spotify blocks" never separated from the canvas; (2) empty states consumed entire panels; (3) the accent was defined but barely used; (4) there was no real monospace, so a code/archive tool had no technical typographic signal. The token tables, elevation model, and density rules below are the corrected target. When the existing `styles.css` disagrees with this document, this document wins.

## Design Thesis

RepoMemo should look like a serious archive console for code and decisions.

The visual language combines:

- a dense desktop-app layout
- cool graphite neutrals for chrome, panels, rows, and navigation
- **Spotify-like block separation**: each major region is its own bounded, rounded surface that floats clearly above a darker canvas, with a clear purpose and header
- a single calm technical **blue** accent, used only for primary actions, links, focus, and active affordances
- restrained semantic accents for state, provenance, and retrieval confidence
- a real monospace face for the technical "truth" tokens: paths, hashes, line ranges, symbols, and code

The result should feel quiet and opinionated: more Linear, Raycast, GitHub, and native desktop utility than SaaS landing page.

## Experience Principles

### 1. Local-first confidence
Users should always understand where data lives, what has been indexed, and whether AI or cloud services are involved. Storage paths, provider mode, indexing state, and citation source must stay visible where they matter.

### 2. Dense, not cramped
RepoMemo is for technical work. Favor compact rows, side panels, keyboardable lists, and clear hierarchy. Avoid oversized heroes, decorative cards, and one-item-per-screen layouts. **Empty states are compact and do work** (see Empty States) — they never blank out a whole panel.

### 3. Retrieval before generation
Search results, artifact context, line ranges, symbols, and citations are first-class UI. AI answers are useful only when the underlying evidence stays inspectable.

### 4. Progressive disclosure
Show the short answer first, then make details easy to expand: import logs, chunk metadata, symbol outlines, provider settings, relationship evidence, indexing diagnostics.

### 5. Dark and light are equal citizens
Every token has a deliberate light and dark value. Dark mode uses layered surfaces that get **lighter** with elevation — never pitch-black panels with neon accents.

### 6. Stylish by restraint
Character comes from surface layering, color discipline, typography, icon rhythm, density, and small interaction details — not from gradient blobs, glassmorphism, purple-blue AI gradients, bokeh, novelty shadows, or decorative illustration.

### Personality filter
Aim for: precise, grounded, archival, technical, compact, private, quietly premium.
Avoid: chatbot-first, crypto dashboard, whimsical productivity toy, generic admin template, glowing AI assistant, beige notebook app.

## Design Tokens

Use semantic tokens in UI code. Component CSS refers to roles such as `--color-surface` and `--color-text-muted`, never raw hex values (token definitions are the only place hex appears).

### Theme Tokens

```css
:root {
  color-scheme: light;

  /* Neutrals — cool graphite. canvas is the void behind blocks; surfaces lift above it. */
  --color-bg:            #eaedf2; /* outer app gutter */
  --color-canvas:        #e2e6ec; /* inter-block spacing layer (the "void") */
  --color-surface:       #ffffff; /* major blocks, dialogs, menus */
  --color-surface-2:     #f3f5f8; /* rows, inputs, secondary panels */
  --color-surface-3:     #e8ebf0; /* selected rows, nested metadata, elevated detail */
  --color-surface-hover: #eef1f5; /* neutral hover for grey-first controls */
  --color-code-surface:  #f6f8fa; /* code / terminal blocks */

  --color-text:        #16191f;
  --color-text-muted:  #5b626d;
  --color-text-subtle: #8a909b;
  --color-border:        #dce0e7;
  --color-border-strong: #c3c9d3;

  /* Accent — calm technical blue. Fill passes AA for white text. */
  --color-accent:        #2f6fed; /* primary fill (white text ≥4.5:1) */
  --color-accent-hover:  #2057c9;
  --color-accent-soft:   #e9f0ff; /* tinted bg: active mark, soft selected */
  --color-accent-text:   #1d54bf; /* links / inline accent text on light surfaces */
  --color-on-accent:     #ffffff;

  /* Semantic — meaning only. Solid badge fills; AA-safe with white text.
     *-soft values stay for inline highlights (e.g. <mark>), not for badges. */
  --color-info:          #1d54bf;  --color-info-soft:     #e5eefc;
  --color-success:       #1f7a4d;  --color-success-soft:  #dcf0e4;
  --color-warning:       #8a5310;  --color-warning-soft:  #f7e9cf;
  --color-danger:        #b23a2a;  --color-danger-soft:   #f7ddd7;
  --color-discovery:     #6b4aa3;  --color-discovery-soft:#ebe4f7;

  /* Focus is a distinct warm hue so it never reads as accent or selection */
  --color-focus: #e08321;

  /* Elevation: blocks lift with shadow only — never a visible border.
     See "No bordered blocks" rule below. */
  --shadow-block:    0 1px 2px rgba(20, 26, 38, 0.05), 0 8px 24px rgba(20, 26, 38, 0.08);
  --shadow-floating: 0 16px 48px rgba(20, 26, 38, 0.18);
}

:root[data-theme="dark"] {
  color-scheme: dark;

  /* Neutrals — cool graphite, NOT pure black. Each step is clearly lighter. */
  --color-bg:            #0a0c10; /* outer gutter (darkest) */
  --color-canvas:        #0e1116; /* inter-block spacing layer */
  --color-surface:       #161a21; /* major blocks — clearly lifted from canvas */
  --color-surface-2:     #1d222b; /* rows, inputs, secondary panels */
  --color-surface-3:     #262c37; /* selected rows, nested, elevated detail */
  --color-surface-hover: #222834; /* neutral hover */
  --color-code-surface:  #0f1318; /* code / terminal blocks (allowed darker than surface) */

  --color-text:        #f2f4f8;
  --color-text-muted:  #a7afbd;
  --color-text-subtle: #6d7585;
  --color-border:        #2b313c;
  --color-border-strong: #3b4250;

  /* Accent — fill stays blue-600 for white-text AA; text/link role goes lighter. */
  --color-accent:        #2563eb; /* primary fill (white text ≥5:1) */
  --color-accent-hover:  #3b82f6;
  --color-accent-soft:   #16243f; /* tinted bg: active mark, soft selected */
  --color-accent-text:   #7eb0ff; /* links / inline accent text on dark surfaces */
  --color-on-accent:     #ffffff;

  /* Dark theme: badge fills stay saturated enough to read as semantic
     while keeping white text at AA. *-soft values are only for inline marks. */
  --color-info:          #2563eb;  --color-info-soft:     #16263f;
  --color-success:       #2c8a5e;  --color-success-soft:  #133124;
  --color-warning:       #b56b16;  --color-warning-soft:  #382712;
  --color-danger:        #c0432e;  --color-danger-soft:   #3f1d18;
  --color-discovery:     #7a59c2;  --color-discovery-soft:#2c2147;

  --color-focus: #f2b35c;

  /* Elevation: blocks lift through the surface step + soft dark shadow.
     No visible border, no inset ring. The brightness contrast against canvas
     is what makes blocks read as floating. */
  --shadow-block:    0 12px 32px rgba(0, 0, 0, 0.5);
  --shadow-floating: 0 24px 64px rgba(0, 0, 0, 0.6);
}
```

### Color Roles

| Role | Use |
| --- | --- |
| `bg` | App background / outer gutter. |
| `canvas` | The void behind blocks; the inter-block gap color. Must be darker (dark mode) / cooler-darker (light) than `surface` so blocks read as floating. |
| `surface` | Major blocks, dialogs, menus, list containers. |
| `surface-2` | Rows, inputs, secondary panels, hover backgrounds. |
| `surface-3` | Selected rows, nested metadata strips, elevated detail zones. |
| `surface-hover` | Neutral hover for grey-first controls and rows. |
| `code-surface` | Code blocks, evidence snippets, terminal-like areas. The only surface allowed darker than the page in dark mode. |
| `text` / `text-muted` / `text-subtle` | Primary / secondary metadata / lowest-emphasis helper. |
| `border` / `border-strong` | Default dividers / focused sections, selected items, table headers. |
| `accent` | Primary action fill, focus target, small active highlights. |
| `accent-text` | Links and inline accent text (lighter in dark for contrast). |
| `info` | Indexing, processing, neutral progress. |
| `success` | Completed imports, available indexes, verified providers. |
| `warning` | Incomplete indexing, provider fallback, recoverable issues. |
| `danger` | Failed import, destructive actions, permission errors. |
| `discovery` | New capability, AI suggestion, relationship insight. |
| `focus` | Keyboard focus ring only (warm, deliberately not the accent). |

Use semantic colors only for meaning. Green = success/completion, red = risk/failure — never decoration or "importance."

### Elevation & Block Anatomy (the Spotify-block model)

This is the single most important visual rule and the main fix from v1.

```
┌─ canvas (the dark void; this is the gap between blocks) ──────────────┐
│  ┌─ block (surface, radius 12, shadow-block, NO BORDER) ───────────┐  │
│  │  header strip:  [icon] EYEBROW LABEL            [actions →]      │  │
│  │                 Block title                                      │  │
│  │  ───────────────────────────────────────────────── (hairline)   │  │
│  │  body: rows on surface-2 / nested on surface-3                   │  │
│  └─────────────────────────────────────────────────────────────────┘  │
│  (8–12px canvas gap)                                                   │
│  ┌─ next block ────────────────────────────────────────────────────┐  │
└────────────────────────────────────────────────────────────────────────┘
```

Layering contract:

- **Each major block sits on `surface`**, radius `12px`, with `--shadow-block` and **no border** (see the "No bordered blocks" rule below). The block must be visibly lighter than the `canvas` gap around it in both themes.
- **Inside a block**, rows/inputs/sub-panels use `surface-2`; selected/nested/elevated use `surface-3`.
- The `canvas` color fills the gaps between blocks and behind everything; it is darker than every block.
- Block separation comes from **the surface step + the drop shadow**, never from a stroke. If a block doesn't read as lifted, raise the surface contrast or the shadow — do not add a border.
- Never place a component darker than the page background except `code-surface`.
- Keep blue out of large backgrounds. Blue is buttons, links, focus, and small active marks only.

### No bordered blocks (hard rule)

Major blocks, sub-panels, and nested rows are defined by **fill + shadow**, not strokes. Concretely:

- **Never** use `border` or `box-shadow: inset 0 0 0 1px …` on a `surface`/`surface-2`/`surface-3` container.
- Rows and inputs get their separation from a darker fill against the block they sit on (`surface-2` on `surface`, `surface-3` on `surface-2`), not from a 1px stroke.
- Inputs in focus replace their background lift with the focus ring; they still do not gain a permanent border.
- Hairline dividers (`1px` `--color-border`) are allowed *between sections inside a single block* (e.g. between Settings groups) — they are dividers, not box outlines.
- Code blocks and evidence snippets use `--color-code-surface` fill — also no border.
- Floating overlays (menus, popovers, dialogs) use `--shadow-floating` only — also no border.

This is what keeps the app reading as "Spotify-style floating blocks" instead of a bordered admin template.

Verify with a quick test: squint at any screen. You should see distinct rectangular "islands" of differing brightness. If the page looks like one flat sheet, the canvas/surface contrast is too low — that was the v1 failure.

## Typography

Two families, both bundled locally via `@fontsource` (never fetch remote fonts at runtime):

- **Outfit** — all UI: brand, titles, labels, body, metadata.
- **JetBrains Mono** — technical truth tokens only: file paths, content hashes, line ranges (`lines 12–18`), code symbols, code/evidence snippets, command names, and storage roots.

```css
--font-ui:    "Outfit", system-ui, sans-serif;
--font-brand: "Outfit", system-ui, sans-serif;
--font-mono:  "JetBrains Mono", ui-monospace, SFMono-Regular, Menlo, monospace;
```

Add the dependency: `@fontsource/jetbrains-mono` (weights 400, 500, 700), imported once at app entry alongside Outfit.

### Type Scale

| Token | Size / line-height | Weight | Use |
| --- | --- | --- | --- |
| `text-xs` | 11px / 16px | 500–700 | badges, timestamps, metadata |
| `text-sm` | 12px / 16px | 400–700 | labels, compact controls, eyebrow labels |
| `text-md` | 14px / 20px | 400–700 | default UI body |
| `text-lg` | 16px / 22px | 600–700 | section titles, row titles |
| `title-sm` | 19px / 26px | 650–750 | panel/block headers |
| `title-md` | 24px / 32px | 650–750 | page/workspace title |
| `title-lg` | 32px / 40px | 650–750 | rare welcome title |

Mono runs ~1px visually larger than Outfit at the same px; step mono down one notch where it sits inline with UI text (e.g. paths in a 14px row use 13px mono).

Rules:
- Sentence case for all UI text. Letter spacing `0`.
- Eyebrow labels (the small text above a title, e.g. "Local retrieval") are `text-sm`, `--color-text-muted`, weight 600, sentence case. **Do not use all-caps** for new labels.
- Use weight and the mono/sans split — not novelty typefaces — to separate wordmark, titles, metadata, and technical tokens.
- Preserve text selection on paths, code, snippets, and answers.

## Spacing, Radius, Density

4px grid with a few half-steps for icon alignment.

| Token | Value | Use |
| --- | ---: | --- |
| `space-1` | 2px | hairline alignment |
| `space-2` | 4px | tight icon/text gap |
| `space-3` | 6px | compact internal gap |
| `space-4` | 8px | default small gap / inter-block gap (narrow) |
| `space-5` | 10px | icon button padding / inter-block gap (default) |
| `space-6` | 12px | compact component padding |
| `space-8` | 16px | block internal rhythm |
| `space-10` | 20px | section rhythm |
| `space-12` | 24px | page gutter |
| `space-16` | 32px | large page gutter |

Control heights:

| Component | Height |
| --- | ---: |
| Icon button | 32px |
| Compact button | 34px |
| Default button / input | 38px |
| Search / command field | 44px |
| Compact list row | 40px |
| Rich list row | 56px |
| Table row | 36px |
| Sidebar item | 36px |

Radius:
- `4px` tags, checkboxes, table cells, code chips
- `6px` buttons, inputs, menu items, list rows
- `8px` nested panels, evidence snippets, repeated item rows
- `12px` major app blocks (this is the Spotify-block radius — was 10, now 12)
- `999px` status pills, avatars, progress capsules

No cards-inside-cards. Separate sections with layout, spacing, subtle backgrounds, and hairline dividers — not nested rounded piles.

## Layout System

### Desktop Shell

Three-zone model with canvas gaps between blocks:

```
Outer gutter (bg)
  Left rail block        Main workbench           Detail / context block
  (surface)         ->   (header block +     ->   (surface)
  workspace/nav          content block)            preview/citations/metadata
```

Widths:
- left rail: `248px` compact, `280px` comfortable
- main workbench: flexible, min `520px`
- right detail panel: `340px`–`440px`
- page gutter: `24px` desktop, `16px` narrow
- inter-block gap: `10px` default (`8px` narrow)
- major block radius: `12px`

Scrolling contract:
- the app shell is viewport-bound; never let `body`, `#root`, or the top-level grid become the primary scroll container
- the sidebar is a fixed/sticky block and does not scroll with main content
- the header/status region is its own rounded block, separated by the canvas gap
- each main block owns its own overflow; lists/previews scroll internally; no nested page-level scrolling

### Navigation

Vertical, compact, icon + label:

`Workspaces · Import · Artifacts · Search · Ask · Memory cards · Settings`

- Active = neutral selected background (`surface-3`) + text/icon emphasis. **No left-border / left-stripe indicator.**
- Disabled (future-phase) items are dimmed with a tooltip explaining availability.
- Group the nav into two clusters with a hairline + small label: **Library** (Workspaces, Import, Artifacts) and **Intelligence** (Search, Ask, Memory cards), with Settings pinned to the bottom near the storage-root strip. This is a key Spotify-style "clear section purpose" cue.

### Workbench Header (its own block)

Shows: workspace name + phase eyebrow, indexing/search state badges, the primary command/search affordance, a compact provider/privacy indicator, and the theme toggle. It is a control surface, not a hero — keep it ≤ 80px tall.

### Main Content Patterns

| View | Pattern |
| --- | --- |
| Workspaces | list + detail summary |
| Import | source list + job progress + log drawer |
| Artifacts | filterable list + preview/detail |
| Search | command field + ranked results + filters + evidence preview |
| Ask | prompt + retrieved context + answer + citations |
| Memory cards | dense list / card grid + source links |
| Settings | grouped forms with explicit save state |

## Component Guidance

### Buttons
- **Primary** (one per view): `accent` fill, `on-accent` text.
- **Secondary**: `surface-2` + `border`, neutral text.
- **Ghost**: transparent, hover → `surface-hover`.
- **Destructive**: `danger` only on destructive confirmation.
- **Icon-only**: 32px, tooltip + accessible label required.

Labels are verbs: `Create`, `Import folder`, `Reindex`, `Save memory`.

### Inputs & Forms
- default height `38px`, search field `44px`
- visible label above (or inline for dense settings rows)
- helper text only when it prevents an error; validation near the field
- focus ring uses `--color-focus` (warm), `2px`, offset `2px`
- inputs sit on `surface-2` with a `--ring-inset`; focus swaps the inset for the focus ring
- for path/provider settings, favor copy buttons, reveal buttons, and validation badges

### Search & Command
Search is the core interaction, not a secondary page.

Field anatomy: leading search icon → query text → optional scope token (`workspace` / `docs` / `code` / `memory`) → keyboard hint (`⏎`) on the right.

Result rows show: title, path (mono), highlighted snippet, artifact type, indexed state, and rank/confidence when useful. Highlighted match uses `mark` → `accent-text` on `accent-soft`.

### Tables & Lists
- fixed row heights; sticky headers for long lists
- timestamps/status are right-aligned or visually secondary
- selected rows use neutral background change + text weight (+ optional dot/icon); **never a left stripe**
- row actions appear on hover and keyboard focus

### Panels / Blocks
Use `surface`, no default visible border (the `--ring-inset` hairline does the edge), `12px` radius, `--shadow-block`. Reserve `--shadow-floating` for overlays, command palettes, dialogs, and menus.

### Empty States (compact, working — the v1 fix)
Empty states must **not** blank out a panel. Constrain them and make them productive.

- Max height ~`160px`; left-aligned or compact-centered, never a giant centered icon filling 400px.
- One `20px` icon, a one-line title, one line of guidance, and a primary action.
- Where possible, replace "nothing here" with **useful adjacent content**: recent searches, last import summary, index status, or suggested next step.

Good: `No artifacts indexed yet.` + `Import a folder to make this workspace searchable.` + `[Import folder]`.
For Search before first query, show a compact "Ready to search" strip **plus** recent queries and index coverage — not a full-panel placeholder.
Avoid: mascots, abstract illustration, vague AI promises, full-product paragraphs.

### Status Badges

Compact, role-based, paired with text and (where space allows) an icon. Never color-only.

`Ready · Indexing · Partial · Failed · Local · Cloud off · AI enabled`

**Style rule (hard):** semantic badges use a **solid colored fill with white text**, not the "soft tinted background + colored text" pattern that reads as generic AI / startup-template UI.

- **Neutral badge** — `surface-2` background, `text` color. Used for non-semantic metadata (counts, types, modes, generic chips).
- **Semantic badges** — solid fill from the role color (`--color-success`, `--color-warning`, `--color-danger`, `--color-info`, `--color-discovery`) with **`#ffffff` text** (or `--color-on-accent` for the accent badge). The token values are chosen so white text passes AA on all of them in both themes.
- Keep them small (24px height, 12px text, semibold). The fill is what carries meaning — never lean on color-only.
- Do **not** introduce a "soft" variant for badges. The `*-soft` tokens stay reserved for inline-text highlights (e.g. the search-result `mark`) and for the very rare large surface that needs a tinted background, never for badges.

Example: an `Indexed` badge is a solid green pill with white text; a `Failed` badge is a solid red pill with white text; a `Stored` badge is a solid amber/warning pill with white text. A neutral `markdown` chip stays grey on grey.

### Citations (product-critical)
Citation chips include: artifact title, path/source (mono), line range (mono) when available, and rank/confidence when useful. Clicking opens the artifact preview at the referenced chunk; hover reveals the full path. Render the chip on `surface-2` with a `code-chip` mono segment for the line range.

### Memory Cards
Durable knowledge objects, not decorative cards. Each shows: title, short body excerpt, source type (user / summary / answer), linked artifacts/chunks, and last-verified/updated timestamp. Provide both a card grid and a dense list mode for browsing many.

### AI Answer Surface
Preserve trust: retrieved context appears before/beside the generated answer; citation chips sit inline or immediately below claims; "insufficient context" is a valid final state; provider + mode (local / cloud / no AI) are visible; save-to-memory requires explicit user action. Never present AI output as the source of truth.

## Iconography
Use `@tabler/icons-react` for all icons. `16px` dense/table, `18px` nav/buttons, `20px` panel headers, `24px` brand/empty-state. Stroke `1.75`–`2`. No mixed icon families. Icon-only buttons need tooltips + labels. Icons clarify object type and action, not decoration.

## Motion & Interaction
Durations: hover/focus `100ms`, small state `140ms`, panel open/close `180ms`, modal/palette `220ms`.

```css
--ease-standard: cubic-bezier(0.2, 0, 0, 1);
--ease-emphasis: cubic-bezier(0.2, 0, 0, 1);
```

Respect `prefers-reduced-motion`. Never animate layout in long lists during indexing. Use skeletons for known shapes and progress indicators for jobs over ~1s.

## Dark & Light Mode Implementation
Single attribute: `<html data-theme="light|dark">`. Settings: `system | light | dark`, default `system`, persisted, applied before React paints (no flash). Keep syntax highlighting, charts, and file-type icons theme-aware. Test both themes for every component.

## Accessibility & UX Baseline
- WCAG AA: `4.5:1` normal text, `3:1` large text / essential UI marks (token values above are chosen to meet this — primary fills pass white-text AA in both themes).
- keyboard access for all commands, dialogs, tabs, lists, menus; visible focus on every interactive element
- no color-only status; dialog focus trap + Escape close
- hit targets ≥ `32px` (prefer `36px`+ in dense areas)
- reduced-motion support; preserved text selection for paths/code/snippets/answers

Content: direct and concrete. Prefer "Indexing paused" over "Something happened". Name the next action in errors. Destructive confirmations are specific: "Delete workspace metadata?" not "Are you sure?".

## CSS Implementation Checklist
Before merging UI work:
- [ ] component uses semantic tokens; no raw hex outside token blocks
- [ ] **squint test passes**: blocks read as distinct islands above the canvas in both themes
- [ ] light and dark both reviewed
- [ ] focus ring visible and warm (not accent-colored); hover and selected states distinct
- [ ] row/control heights stable; text never overflows buttons/cards/panels
- [ ] no selected-state left border/stripe
- [ ] blue only as action/link/focus/active mark — never a broad panel color
- [ ] paths/hashes/line-ranges/code render in `--font-mono`
- [ ] no decorative gradients, orbs, bokeh, glass panels
- [ ] empty/loading/error states exist and are compact (no full-panel placeholders)
- [ ] icons from `@tabler/icons-react`; keyboard path considered

## Future UI Package
When ≥3 screens reuse the same patterns, extract shared components:

```
packages/ui/
  tokens.css  Button.tsx  IconButton.tsx  TextField.tsx
  Badge.tsx  Block.tsx  SearchField.tsx  DataList.tsx
  CitationChip.tsx  EmptyState.tsx  StatusBadge.tsx
```

## References Checked
These informed the guide; RepoMemo copies none directly.
- Linear — calm dark technical UI, surface layering, density.
- Raycast — block separation and command surfaces.
- Radix Themes color scales (12-step, accent/gray pairing, focus colors): https://www.radix-ui.com/themes/docs/theme/color
- GitHub Primer primitives and token naming: https://primer.style/product/primitives/color/
- Carbon role-based tokens and light/dark layering: https://carbondesignsystem.com/elements/color/overview/
- Atlassian color roles, tokens, dark mode, contrast: https://atlassian.design/foundations/color/
- Outfit (UI face): https://fonts.google.com/specimen/Outfit · JetBrains Mono (technical face): https://www.jetbrains.com/lp/mono/
- Tabler Icons: https://tabler.io/icons
