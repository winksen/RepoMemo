---
name: RepoMemo Neutral Research Canvas
description: A light, composed productivity workspace that places a local knowledge task beside its inspectable context.
colors:
  canvas: "#f7f8fa"
  surface: "#ffffff"
  surface-subtle: "#fafbfc"
  ink: "#17181a"
  ink-muted: "#6f747c"
  divider: "#e6e9ee"
  action-blue: "#2563eb"
  action-blue-soft: "#eff6ff"
  suggestion-wash: "#f4e7d2"
typography:
  display:
    fontFamily: "Inter, ui-sans-serif, system-ui, sans-serif"
    fontSize: "30px"
    fontWeight: 700
    lineHeight: 1.12
    letterSpacing: "-0.03em"
  body:
    fontFamily: "Inter, ui-sans-serif, system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.55
rounded:
  control: "8px"
  plane: "12px"
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

Light mode is the product's default. The physical scene is a developer working through a project on a normal desktop display, with enough contrast for long reading and no themed ambient field. White is the working surface; a very cool gray establishes gutters and inactive controls; thin cool-gray dividers define regions.

Blue is the only expressive color. It denotes the current selection, keyboard focus, a direct primary action, and an active tab. Green, amber, and red remain semantic exception colors only; they never brand broad regions. A warm suggestion wash may appear only behind actionable assistant recommendations, never as general decoration.

## Typography

Use a system sans stack with a straightforward product-editor rhythm. Headlines are bold and compact, while supporting copy is 14px and deliberately spacious. Monospace is limited to file paths, identifiers, ranges, and code.

- Page title: 28–32px / 700 / -0.03em.
- Panel title: 16px / 600.
- Body: 14px / 1.55, at 65–75ch where reading matters.
- Metadata: 12px / 400, neutral gray.

## Layout

Desktop uses a 64px icon rail, a flexible primary canvas, and a 400px contextual right pane. The global title bar is light, compact, and divided from content by a single hairline. The primary and contextual columns scroll independently when needed. On narrow screens, the rail becomes a horizontal icon strip and contextual panes move below the primary task.

The Workspace route uses the central canvas to browse and create workspaces. Its right note gives local context and one next action; it deliberately omits diagrams, pipeline meters, and repeated system totals. Search, Ask, Summary, Artifacts, Memory, and Settings retain this task-plus-context relationship.

## Components

Controls are 36–40px tall, softly rounded at 8px, and communicate through text and icons rather than colored blocks. Surfaces use one faint divider or a very soft offset shadow, never both by default. Selected rows use a pale blue field. Suggested actions can use the reserved warm wash and a small trailing action control. Inputs are white with a subtle border and blue focus outline.

### Shared UI primitives

The web client uses shadcn's source-owned component pattern. Components live in
`apps/desktop/src/components/ui/`, remain editable in this repository, and use
Radix only for interaction behavior. `Button` has exactly two variants:
`main` for the single decisive action in a group and `secondary` for every
other action. `Dropdown` has one accessible visual treatment for all selects.
Do not introduce a third button or dropdown treatment without updating this
system deliberately.

## Do's and don'ts

- Do make the primary task the largest, brightest surface.
- Do expose local evidence in a calm companion pane instead of permanent telemetry.
- Do reserve blue for action, current state, and focus.
- Don't use dark themes, grids, terminal motifs, traced diagrams, or dashboard metric strips as the default experience.
- Don't repeat workspace state in a graph, metrics row, and readiness tracker.
- Don't use all-caps technical labels or monospace as ordinary interface copy.
