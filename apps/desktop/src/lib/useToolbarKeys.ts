import { useEffect, useRef, type FocusEvent, type KeyboardEvent } from "react";

const BUTTONS = "button:not(:disabled)";

/**
 * The keyboard half of the WAI-ARIA toolbar pattern (V0.3.25): the toolbar is one stop in the tab order, and the
 * arrow keys, Home and End move between its buttons. Give `ref`, `onFocus` and `onKeyDown` to the element that
 * has `role="toolbar"`, with `aria-label` and `aria-orientation`. The button that last had focus is the one Tab
 * comes back to; a button that is disabled is skipped.
 */
export function useToolbarKeys<T extends HTMLElement>(orientation: "horizontal" | "vertical" = "horizontal") {
  const ref = useRef<T>(null);
  const current = useRef(0);
  const buttons = () => Array.from(ref.current?.querySelectorAll<HTMLElement>(BUTTONS) ?? []);

  // After every render, since the buttons may have changed: only the current one is a tab stop.
  useEffect(() => {
    const list = buttons();
    const keep = Math.min(current.current, Math.max(0, list.length - 1));
    list.forEach((button, index) => { button.tabIndex = index === keep ? 0 : -1; });
  });

  function onFocus(event: FocusEvent<T>) {
    const index = buttons().indexOf(event.target as HTMLElement);
    if (index < 0) return;
    current.current = index;
    buttons().forEach((button, at) => { button.tabIndex = at === index ? 0 : -1; });
  }

  function onKeyDown(event: KeyboardEvent<T>) {
    if (event.altKey || event.ctrlKey || event.metaKey) return;
    const list = buttons();
    const at = list.indexOf(event.target as HTMLElement);
    if (at < 0 || list.length === 0) return;
    const [back, forward] = orientation === "horizontal" ? ["ArrowLeft", "ArrowRight"] : ["ArrowUp", "ArrowDown"];
    let next = at;
    if (event.key === forward) next = (at + 1) % list.length;
    else if (event.key === back) next = (at - 1 + list.length) % list.length;
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = list.length - 1;
    else return;
    event.preventDefault();
    list[next].focus();
  }

  return { ref, onFocus, onKeyDown };
}
