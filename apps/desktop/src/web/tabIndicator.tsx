import { type ReactNode, useEffect, useLayoutEffect, useRef, useState } from "react";

/**
 * The line under the active tab as one element that slides from tab to tab (ease in, ease out) instead of being
 * drawn on each tab. Put `ref` on the row that holds the tabs (the row must be positioned), mark the active tab with
 * `aria-current="page"`, and render `indicator` last inside the row. It measures again when tabs change size, order or
 * which one is active, and does not animate the first time it appears, so it never slides in from the edge.
 */
export function useTabIndicator(): { ref: React.RefObject<HTMLDivElement>; indicator: ReactNode } {
  const ref = useRef<HTMLDivElement>(null);
  const [box, setBox] = useState<{ left: number; width: number } | null>(null);
  const [animated, setAnimated] = useState(false);

  useLayoutEffect(() => {
    const row = ref.current;
    if (!row) return;
    const measure = () => {
      const tab = row.querySelector<HTMLElement>('[aria-current="page"]');
      const next = tab ? { left: tab.offsetLeft, width: tab.offsetWidth } : null;
      setBox((current) => current && next && current.left === next.left && current.width === next.width ? current : next);
    };
    measure();
    const resize = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(measure);
    resize?.observe(row);
    row.querySelectorAll("button").forEach((button) => resize?.observe(button));
    const mutation = new MutationObserver(measure);
    mutation.observe(row, { attributes: true, attributeFilter: ["aria-current"], childList: true, subtree: true });
    return () => { resize?.disconnect(); mutation.disconnect(); };
  }, []);

  // Moves made after the first frame are animated.
  useEffect(() => {
    const frame = requestAnimationFrame(() => setAnimated(true));
    return () => cancelAnimationFrame(frame);
  }, []);

  const indicator = box ? <span aria-hidden="true" className="shared-tab-indicator" data-animated={animated ? "true" : "false"} style={{ transform: `translateX(${box.left}px)`, width: box.width }} /> : null;
  return { ref, indicator };
}
