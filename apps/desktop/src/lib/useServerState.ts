import { useQuery } from "@tanstack/react-query";
import { useCallback, useRef, type SetStateAction } from "react";
import { queryClient } from "./queryClient";

/**
 * Server data held in the query cache but used like `useState`: the value is
 * the cached data (or `initial` before it arrives), and the setter writes to
 * the cache, so handlers that update a list after a mutation keep working
 * while everything else (loading, refetching, sharing between pages,
 * invalidation) comes from the cache.
 */
export function useServerState<T>(options: {
  key: readonly unknown[];
  fetch: () => Promise<T>;
  initial: T;
  enabled?: boolean;
  /** A page that must not show an old answer for long asks for `0`: the cached value shows at once and is refetched when the page opens. */
  staleTime?: number;
}) {
  const { key, fetch, enabled = true, staleTime } = options;
  const initial = useRef(options.initial);
  const query = useQuery({ queryKey: key, queryFn: fetch, enabled, ...(staleTime === undefined ? {} : { staleTime }) });
  const keyText = JSON.stringify(key);
  const set = useCallback((next: SetStateAction<T>) => {
    queryClient.setQueryData<T>(JSON.parse(keyText) as unknown[], (current) => (
      typeof next === "function" ? (next as (value: T) => T)(current ?? initial.current) : next
    ));
  }, [keyText]);
  return [query.data ?? initial.current, set, query] as const;
}
