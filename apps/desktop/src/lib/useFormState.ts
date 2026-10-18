import { useCallback, useEffect, useMemo, useRef, useState } from "react";

function sameValues<T extends object>(first: T, second: T) {
  const keys = Object.keys(first) as Array<keyof T>;
  return keys.length === Object.keys(second).length && keys.every((key) => Object.is(first[key], second[key]));
}

/**
 * The values of a form, the values it started from, whether they differ and a way back.
 *
 * `initial` is what the form shows before anyone types: when it changes (the saved record was
 * loaded, or updated by a save) the form starts over from the new values, so the page needs no
 * effect that copies a record into its fields. `set` changes one field, `patch` several.
 */
export function useFormState<T extends Record<string, unknown>>(initial: T) {
  const [baseline, setBaseline] = useState(initial);
  const [values, setValues] = useState(initial);
  const latest = useRef(initial);
  latest.current = initial;

  const changed = !sameValues(baseline, initial);
  useEffect(() => {
    if (!changed) return;
    setBaseline(latest.current);
    setValues(latest.current);
  }, [changed]);

  const set = useCallback(<K extends keyof T>(field: K, value: T[K]) => setValues((current) => ({ ...current, [field]: value })), []);
  const patch = useCallback((changes: Partial<T>) => setValues((current) => ({ ...current, ...changes })), []);
  const reset = useCallback(() => setValues(baseline), [baseline]);
  const isDirty = useMemo(() => !sameValues(values, baseline), [values, baseline]);

  return { values, initial: baseline, isDirty, set, patch, reset };
}
