import { useEffect, useRef } from "react";

/** Id of the element that holds the page; the skip link and the focus move after a route change land here. */
export const MAIN_CONTENT_ID = "main-content";

/** Waits (the page may still be loading) and then puts the keyboard at the start of the page. */
function focusMain(attempts = 30): () => void {
  let timer = 0;
  const attempt = (left: number) => {
    const main = document.getElementById(MAIN_CONTENT_ID);
    if (main) {
      main.focus({ preventScroll: true });
      return;
    }
    if (left > 0) timer = window.setTimeout(() => attempt(left - 1), 100);
  };
  attempt(attempts);
  return () => window.clearTimeout(timer);
}

/**
 * After a move to another page: names it in the tab (`<title>`), says so to a
 * screen reader, and puts keyboard focus at the start of the page, so the
 * person is not left on a control that is gone. A change of section inside the
 * same page (`pageKey` unchanged) is announced but keeps focus on the tab that
 * was pressed.
 */
export function RouteAnnouncer({ pageKey, title }: { pageKey: string; title: string }) {
  const previousKey = useRef<string | null>(null);

  useEffect(() => {
    document.title = `${title} · RepoMemo`;
  }, [title]);

  useEffect(() => {
    const first = previousKey.current === null;
    const moved = previousKey.current !== pageKey;
    previousKey.current = pageKey;
    // The first page after sign-in or a reload keeps the browser's own starting point.
    if (first || !moved) return;
    return focusMain();
  }, [pageKey]);

  return <div aria-atomic="true" aria-live="polite" className="sr-only" role="status">{title}</div>;
}
