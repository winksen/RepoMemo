import { useEffect } from "react";
import { showToast } from "../components/ui/toast";
import { apiMessage } from "../web/format";

/** Shows an error toast when a query fails: the error's message, or `fallback` when it has none. */
export function useErrorToast(error: unknown, fallback: string) {
  useEffect(() => {
    if (error) showToast("error", apiMessage(error, fallback));
  }, [error]);
}
