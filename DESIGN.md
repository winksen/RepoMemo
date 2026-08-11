---
name: RepoMemo Evidence Instrument
description: A precision technical workbench that makes local evidence, provenance, and system state legible.
colors:
  trace-green: "#0a735f"
  trace-green-hover: "#075c4d"
  trace-green-soft: "#dcece6"
  signal-amber: "#a65f08"
  signal-amber-soft: "#f5e7cf"
  lab-paper: "#f8faf7"
  instrument-well: "#eef2ef"
  anodized-aluminum: "#d8dedb"
  graphite-ink: "#18201f"
  calibration-steel: "#56615c"
  hairline: "#c7cfca"
typography:
  display:
    fontFamily: "Fjalla One, Arial Narrow, sans-serif"
    fontSize: "24px"
    fontWeight: 400
    lineHeight: 1.05
    letterSpacing: "normal"
  body:
    fontFamily: "Recursive Variable, Segoe UI, sans-serif"
    fontSize: "11px"
    fontWeight: 400
    lineHeight: 1.55
    letterSpacing: "normal"
  label:
    fontFamily: "JetBrains Mono, Cascadia Code, Consolas, monospace"
    fontSize: "8px"
    fontWeight: 600
    lineHeight: 1.4
    letterSpacing: "0.08em"
rounded:
  trace: "2px"
  control: "3px"
  plane: "4px"
  lamp: "999px"
spacing:
  micro: "4px"
  compact: "8px"
  module: "12px"
  panel: "16px"
  section: "24px"
components:
  button-primary:
    backgroundColor: "{colors.trace-green}"
    textColor: "#ffffff"
    rounded: "{rounded.control}"
    padding: "0 12px"
    height: "34px"
  button-primary-hover:
    backgroundColor: "{colors.trace-green-hover}"
    textColor: "#ffffff"
    rounded: "{rounded.control}"
  input:
    backgroundColor: "{colors.instrument-well}"
    textColor: "{colors.graphite-ink}"
    rounded: "{rounded.control}"
    padding: "0 10px"
    height: "37px"
  panel:
    backgroundColor: "{colors.lab-paper}"
    textColor: "{colors.graphite-ink}"
    rounded: "{rounded.plane}"
    padding: "{spacing.panel}"
  nav-active:
    backgroundColor: "{colors.trace-green-soft}"
    textColor: "{colors.trace-green-hover}"
    rounded: "{rounded.control}"
---

# Design System: RepoMemo

## Overview

**Creative North Star: "The Evidence Instrument"**

RepoMemo feels like a precision instrument used in a daylight engineering lab: calm, durable, exact, and built to expose how a result was produced. Its visual grammar comes from logic analyzers and test benches rather than terminals or generic dashboards. Channels, traces, registration marks, instrument labels, and measured divisions turn provenance into the visible structure of the interface.

This is an Operate system. Expression never obscures the current task, source identity, status, or next action. The visual world is carried by disciplined topology and purposeful state signals—not decorative circuitry, sci-fi glow, or control-panel cosplay.

**Key Characteristics:**

- Restrained aluminum, paper, and graphite fields with scarce circuit-green and signal-amber state color.
- Dense information organized into registered channels, ledgers, and evidence planes.
- Square technical geometry softened only where a touch target or focus treatment benefits.
- Provenance drawn as structure: paths, traces, source markers, and indexed stages.
- Motion that behaves like an instrument reading settling into place.

## Colors

The palette is deliberately restrained. Neutral instrument surfaces own nearly the entire screen; Trace Green identifies trustworthy local/ready state, selection, focus, and the primary action; Signal Amber marks processing or attention.

### Primary

- **Trace Green:** selected navigation, ready/local state, focus, and primary actions.
- **Trace Green Hover:** active control response and strong green text on pale surfaces.
- **Trace Green Soft:** selected rows, ready-state fields, and calm focus context.

### Secondary

- **Signal Amber:** active processing, warnings, and the one state requiring immediate attention.
- **Signal Amber Soft:** the background field behind that attention state.

### Neutral

- **Lab Paper:** the brightest reading and inspection plane.
- **Instrument Well:** form fields, quiet modules, and inset task areas.
- **Anodized Aluminum:** the shell canvas and registered seams between regions.
- **Graphite Ink:** primary copy, headings, and strong structural marks.
- **Calibration Steel:** secondary copy, paths, and inactive measurements.
- **Hairline:** permanent dividers, trace boundaries, and component outlines.

**The Signal Budget Rule.** Trace Green and Signal Amber communicate state; neither is ambient decoration.

**The Daylight Bench Rule.** Light mode is the default physical scene for long reading beside an editor. Dark mode is a first-class night-bench translation, not a separate identity.

## Typography

**Display Font:** Fjalla One (with Arial Narrow fallback)
**Body Font:** Recursive Variable (with Segoe UI fallback)
**Label/Mono Font:** JetBrains Mono (with Cascadia Code and Consolas fallbacks)

**Character:** Fjalla One reads like an engraved instrument name without turning the entire product into condensed type. Recursive carries readable interface copy at desktop density. JetBrains Mono aligns paths, counts, timestamps, and machine state.

### Hierarchy

- **Display** (400, 24px, 1.05): workspace and active tool names.
- **Headline** (400, 22px, 1.12): panel-level orientation where the display face is warranted.
- **Title** (600–650, 12–16px, 1.3): sections, results, and task transitions.
- **Body** (400, 11px, 1.55): operational copy with an effective maximum measure of 72 characters.
- **Label** (600, 8px, 0.08em): terse metadata, channel labels, and system state.

**The Mono Is Evidence Rule.** Monospace identifies paths, identifiers, counts, timestamps, code, and machine state; it never replaces the readable interface voice.

## Layout

The shell is a registered three-zone workbench: a 178px channel rail, a flexible task/evidence workbench, and a 224px status spine. At widths below 1380px the rail collapses to 68px; below 1160px the status spine yields to the main task; below 900px navigation becomes a horizontal instrument strip.

Inside the Workspaces surface, a compact ledger occupies roughly one-third of the workbench and the evidence plane owns the rest. Other task views preserve the same relation through a main pane and evidence rail. Spacing follows a 4px base rhythm. Permanent regions share edges rather than floating independently.

## Elevation & Depth

The system is flat by default. Adjacent material tones, inset instrument wells, crisp one-pixel rules, and inner registration marks establish depth. Floating shadows are reserved for temporary overlays and dialogs.

**The Registered Plane Rule.** Permanent regions share edges and dividers. They do not float as unrelated cards over the app background.

## Shapes

Trace nodes use 2px corner easing, controls use 3px, and full planes use 4px. Status lamps are circular because they represent indicator hardware. Large soft pills and uniformly rounded cards are outside this world.

Trace lines and registration ticks are functional: they connect a real count, source, citation, stage, or selected item. The Evidence Path may use a single measurement axis; generic two-axis grid wallpaper is not part of the system.

## Components

### Buttons

- **Shape:** compact instrument control (3px radius, 34px default height).
- **Primary:** Trace Green field, white label, and one action-forward icon.
- **Hover / Focus:** deepen to Trace Green Hover; use a two-pixel translucent green focus outline.
- **Secondary:** Lab Paper or transparent field with a Hairline boundary.

### Status Lamps

- **Style:** a 7px circular light with a two-pixel semantic-color surround.
- **State:** green is ready/local, amber is processing/attention, red is unavailable/error, steel is inactive.

### Cards / Containers

- **Corner Style:** registered plane (4px) or trace node (2px).
- **Background:** Lab Paper for reading, Instrument Well for inset tasks.
- **Shadow Strategy:** none at rest.
- **Border:** one-pixel Hairline.
- **Internal Padding:** 12px modules and 16px panels.

### Inputs / Fields

- **Style:** Instrument Well field, Graphite Ink text, Hairline stroke, 3px radius.
- **Focus:** Lab Paper field, Trace Green stroke, and translucent two-pixel outline.
- **Disabled:** Calibration Steel text with no misleading active signal.

### Navigation

The channel rail uses readable Recursive labels and Tabler line icons. Active state receives Trace Green Soft, a green boundary, and a one-pixel registration mark. At compact widths the icons remain and labels leave the visual layout while accessible names remain intact.

### Evidence Path

Input trace nodes converge on Indexed Evidence and branch to Symbols and Memory. Every count is live workspace data. Trace motion settles once on entry and is disabled under reduced-motion preference.

### Status Spine

The right spine reports real database, storage, indexing, provider, and next-action state. It never displays synthetic telemetry and disappears before it would compress the primary task below a useful width.

## Do's and Don'ts

### Do:

- **Do** use channel structure to connect sources, indexing stages, results, and citations.
- **Do** make current selection, local/cloud state, and the next safe action obvious.
- **Do** preserve full technical identifiers with wrapping, truncation plus disclosure, or responsive reflow.
- **Do** use one trace-settling motion for major state changes and respect reduced-motion preferences.
- **Do** let empty states teach the first useful local action.

### Don't:

- **Don't** recreate a generic SaaS dashboard with nested rounded cards and decorative badges.
- **Don't** turn RepoMemo into a neon terminal, cyberpunk HUD, or sci-fi control panel.
- **Don't** use green or amber where no state is being communicated.
- **Don't** make all interface copy uppercase or monospace.
- **Don't** hide provenance behind hover-only affordances or visual effects.
