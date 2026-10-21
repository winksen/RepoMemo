import { useCallback, useEffect, useRef } from "react";
import { useConfirm } from "../components/ui/confirm-dialog";
import { addNavigationGuard } from "../web/router";

export interface UnsavedChangesOptions {
  /** True for an address where the page keeps the draft (another section of the same workspace), so nothing is lost. */
  keepsOn?: (to: string) => boolean;
  /** What would be lost, for the question: "your changes" when omitted. */
  subject?: string;
}

/**
 * Stops a person from losing a form by accident. While `isDirty`, moving to another page asks
 * to discard through `useConfirm`, and closing or reloading the tab shows the browser's own warning.
 * It returns `canLeave()` for the form's own exits (Cancel, closing its dialog): `true` when
 * nothing is unsaved or the person agreed to discard.
 */
export function useUnsavedChanges(isDirty: boolean, options: UnsavedChangesOptions = {}) {
  const confirm = useConfirm();
  const latest = useRef({ confirm, isDirty, options });
  latest.current = { confirm, isDirty, options };

  const ask = useCallback(async (to?: string) => {
    const { confirm: ask, isDirty: dirty, options: current } = latest.current;
    if (!dirty || (to !== undefined && current.keepsOn?.(to))) return true;
    return ask({ action: "Discard changes", danger: true, description: `${capitalize(current.subject ?? "your changes")} will be lost.`, title: "Discard unsaved changes?" });
  }, []);

  useEffect(() => addNavigationGuard((to) => ask(to)), [ask]);
  useEffect(() => {
    if (!isDirty) return;
    const warn = (event: BeforeUnloadEvent) => { event.preventDefault(); event.returnValue = ""; };
    window.addEventListener("beforeunload", warn);
    return () => window.removeEventListener("beforeunload", warn);
  }, [isDirty]);

  return useCallback(() => ask(), [ask]);
}

function capitalize(text: string) {
  return text.charAt(0).toUpperCase() + text.slice(1);
}
