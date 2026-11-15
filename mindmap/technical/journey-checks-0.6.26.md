# V0.6.26 journey checks

Prepared 2026-11-17. Baseline: V0.6.25. These are the manual checks that [V0.6.26](../RELEASES_0.6.md) leaves to the
maintainer. **None has been performed**: no browser, screen reader or device was used, and nothing was built or run, so
every result cell is pending. Static checks (accessibility rules, contrast pairs, styles) pass and say nothing about
what a person experiences.

## What this release changed

- Hiding or showing the first-value checklist keeps keyboard focus on the button that replaces the one pressed (it used to
  be lost to the page).
- At 640 px and narrower: My work rows wrap long titles; the checklist state drops under its step; the reviewer form uses the full
  width; version, trash and request lists stack their actions under the text.
- The other 0.6 panels already use the shared button, dialog, field and toast components, which carry their own keyboard
  behaviour.

## The three journeys

1. **First value.** Sign in to an empty workspace; read *Getting started*; add a file; see *Make it searchable* become done;
   save a memory card that cites it; hide the checklist and bring it back.
2. **My work.** Open the dashboard; read *My work*; open a task, a review and a mention from the list; come back.
3. **Review.** Open a card that needs review; read *What changed*; open the file; remove the citation that no longer holds
   (the card keeps a revision); set a reviewer and a date; approve the revision; open *Revision history* and compare.

## Record the result

Use one row per journey and condition. Write *pass*, or what stopped you. A failure becomes a numbered fix row, not a note here.

| Journey | Keyboard only (Tab, Shift+Tab, Enter, Space, Escape) | 200% zoom | 360 px wide | Light theme | Dark theme | After a lost connection and retry |
|---|---|---|---|---|---|---|
| First value | pending | pending | pending | pending | pending | pending |
| My work | pending | pending | pending | pending | pending | pending |
| Review | pending | pending | pending | pending | pending | pending |

What to look for: focus is always visible and never lost after pressing a button that disappears; the order of Tab follows the page;
a dialog opens with focus inside it and returns focus to what opened it; nothing needs sideways scrolling; text and controls stay
readable in both themes; after a reconnect the card or form you were in is still there and an unsaved edit is offered back
(V0.6.5), not lost.
