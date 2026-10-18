import { useCallback, useEffect, useRef, useState } from "react";
import { showToast } from "../components/ui/toast";
import { apiMessage } from "../web/format";

export interface AsyncActionOptions<T> {
  /** Shown as a success toast when the action finishes; a function receives what the action returned. */
  success?: string | ((result: T) => string);
  /** Shown as an error toast when the action fails and the error has no message of its own. */
  failure?: string;
}

/**
 * Runs a mutation with a busy flag, a success toast and an error toast. `run` resolves to `true` when
 * the action finished and `false` when it failed (the toast has already been shown). Actions that
 * overlap keep the flag on until the last one is done, and nothing is set after the component left.
 */
export function useAsyncAction() {
  const [pending, setPending] = useState(0);
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);

  const run = useCallback(async <T,>(action: () => Promise<T>, options: AsyncActionOptions<T> = {}): Promise<boolean> => {
    setPending((count) => count + 1);
    try {
      const result = await action();
      if (options.success) showToast("success", typeof options.success === "function" ? options.success(result) : options.success);
      return true;
    } catch (error) {
      showToast("error", options.failure ? apiMessage(error, options.failure) : apiMessage(error));
      return false;
    } finally {
      if (mounted.current) setPending((count) => count - 1);
    }
  }, []);

  return { isBusy: pending > 0, run };
}
