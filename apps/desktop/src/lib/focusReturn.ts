/**
 * Keeps the keyboard somewhere sensible when what had focus goes away.
 *
 * A dialog gives focus back to the control that opened it. When that control is gone (the row
 * was deleted) or disappears afterwards, focus goes to the item that took its place, else the
 * one before it, else the heading of the list. Menus and dialogs are never remembered: the
 * control that opened them is.
 */

const ITEM = "li, tr, article";
const OVERLAY = '[role="dialog"], [role="alertdialog"], [role="menu"], [role="listbox"]';
const FOCUSABLE = 'a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [role="combobox"]:not([disabled]), [tabindex]:not([tabindex="-1"])';

/** Where a control sits, noted while it exists, to find a neighbour once it does not. */
export interface FocusAnchor {
  element: HTMLElement;
  heading: HTMLElement | null;
  next: Element | null;
  previous: Element | null;
}

export function anchorFor(element: HTMLElement | null): FocusAnchor | null {
  if (!element || element === document.body) return null;
  const item = element.closest<HTMLElement>(ITEM);
  const heading = (item ?? element).closest("section, main, form")?.querySelector<HTMLElement>("h1, h2, h3") ?? null;
  return { element, heading: heading && !item?.contains(heading) ? heading : null, next: item?.nextElementSibling ?? null, previous: item?.previousElementSibling ?? null };
}

function firstFocusable(root: Element): HTMLElement | null {
  return root.matches(FOCUSABLE) ? root as HTMLElement : root.querySelector<HTMLElement>(FOCUSABLE);
}

/** Focuses the anchor's control, or its neighbour, or its heading. Returns whether anything took focus. */
export function restoreFocus(anchor: FocusAnchor): boolean {
  if (anchor.element.isConnected) {
    anchor.element.focus();
    return true;
  }
  for (const neighbour of [anchor.next, anchor.previous]) {
    const target = neighbour?.isConnected ? firstFocusable(neighbour) : null;
    if (target) { target.focus(); return true; }
  }
  if (anchor.heading?.isConnected) {
    anchor.heading.tabIndex = -1;
    anchor.heading.focus();
    return true;
  }
  return false;
}

let last: FocusAnchor | null = null;

if (typeof document !== "undefined") {
  document.addEventListener("focusin", (event) => {
    const target = event.target;
    if (target instanceof HTMLElement && !target.closest(OVERLAY)) last = anchorFor(target);
  });
}

/** The control that has focus outside any dialog or menu: what a dialog opened from. */
export function currentOpener(): FocusAnchor | null {
  if (last?.element.isConnected) return last;
  return anchorFor(document.activeElement instanceof HTMLElement ? document.activeElement : null);
}

/**
 * Moves focus to a neighbour when the control that had it is removed from the page (a deleted row,
 * a removed chip) and nothing else took it. Returns the function that stops watching.
 */
export function watchFocusLoss(): () => void {
  const observer = new MutationObserver(() => {
    const anchor = last;
    if (!anchor || anchor.element.isConnected) return;
    if (document.activeElement && document.activeElement !== document.body) return;
    last = null;
    restoreFocus(anchor);
  });
  observer.observe(document.body, { childList: true, subtree: true });
  return () => observer.disconnect();
}
