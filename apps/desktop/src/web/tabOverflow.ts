import { type RefObject, useEffect, useLayoutEffect, useRef, useState } from "react";

/** Room kept for the *More* tab when it has not been measured yet. */
const MORE_FALLBACK_WIDTH = 96;

/**
 * Which tabs fit in `available` pixels. The open tab always stays; the others keep their order and the first one that does not
 * fit, with everything after it, falls behind *More*. `alwaysMore` reserves room for *More* even when every tab fits (it holds
 * sections the template keeps out of the row).
 */
export function fitTabs(keys: string[], widths: ReadonlyMap<string, number>, available: number, moreWidth: number, activeKey: string | undefined, alwaysMore: boolean): Set<string> {
  const width = (key: string) => widths.get(key) ?? 0;
  const total = keys.reduce((sum, key) => sum + width(key), 0);
  if (!alwaysMore && total <= available) return new Set(keys);
  let budget = available - moreWidth;
  const visible = new Set<string>();
  if (activeKey && keys.includes(activeKey)) {
    visible.add(activeKey);
    budget -= width(activeKey);
  }
  for (const key of keys) {
    if (key === activeKey) continue;
    if (width(key) > budget) break;
    visible.add(key);
    budget -= width(key);
  }
  return visible;
}

/**
 * Keeps a row of tabs inside its safe zone: the tabs that do not fit are left out of the row (the caller lists them in its
 * *More* menu). `ref` is the row (the same one the tab indicator uses) and its parent sets the room. Returns the keys to
 * draw, or `null` while every tab is drawn to be measured (and always when there is no layout to measure).
 */
export function useOverflowTabs(ref: RefObject<HTMLElement>, keys: string[], activeKey: string | undefined, alwaysMore: boolean): Set<string> | null {
  const widths = useRef(new Map<string, number>());
  const [visible, setVisible] = useState<Set<string> | null>(null);
  const signature = keys.join("|");

  const compute = (): Set<string> | null => {
    const row = ref.current;
    const parent = row?.parentElement;
    if (!row || !parent) return null;
    const style = getComputedStyle(parent);
    const available = parent.clientWidth - (parseFloat(style.paddingLeft) || 0) - (parseFloat(style.paddingRight) || 0);
    const more = row.querySelector<HTMLElement>('[data-tab-key="__more"]');
    if (more && more.offsetWidth > 0) widths.current.set("__more", more.offsetWidth);
    if (available <= 0 || keys.some((key) => !widths.current.has(key))) return null;
    return fitTabs(keys, widths.current, available, widths.current.get("__more") ?? MORE_FALLBACK_WIDTH, activeKey, alwaysMore);
  };

  // A different set of tabs is measured again from scratch.
  useLayoutEffect(() => { setVisible(null); }, [signature]);

  // While every tab is drawn, measure them and decide.
  useLayoutEffect(() => {
    if (visible !== null) return;
    const row = ref.current;
    if (!row) return;
    row.querySelectorAll<HTMLElement>("[data-tab-key]").forEach((tab) => widths.current.set(tab.dataset.tabKey ?? "", tab.offsetWidth));
    const next = compute();
    if (next) setVisible(next);
  });

  // The window or the open tab changed: decide again with the widths already known.
  useEffect(() => {
    const parent = ref.current?.parentElement;
    if (!parent) return;
    const decide = () => setVisible((current) => {
      if (current === null) return current;
      const next = compute();
      if (!next) return null;
      return next.size === current.size && [...next].every((key) => current.has(key)) ? current : next;
    });
    decide();
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(decide);
    observer?.observe(parent);
    return () => observer?.disconnect();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeKey, alwaysMore, signature]);

  return visible;
}
