import { useSyncExternalStore } from "react";

/** Screens narrower than this get the stacked page, the folded rail and the assistant as a sheet (V0.2.48). Keep in step with the `max-width` of `styles/13-narrow-screens.css`. */
export const NARROW_QUERY = "(max-width: 900px)";

/** Whether a media query matches now, following it as the window changes. False where the browser cannot say. */
export function useMediaQuery(query: string): boolean {
  return useSyncExternalStore(
    (notify) => {
      if (typeof window.matchMedia !== "function") return () => undefined;
      const list = window.matchMedia(query);
      list.addEventListener("change", notify);
      return () => list.removeEventListener("change", notify);
    },
    () => typeof window.matchMedia === "function" && window.matchMedia(query).matches,
    () => false,
  );
}
