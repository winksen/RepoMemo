const SIGNED_IN_KEY = "repomemo.signedInAt";

/** Remembers when this browser last signed in, for the rail's session line. Storage can be blocked; nothing depends on it. */
export function rememberSignIn() {
  try { window.localStorage.setItem(SIGNED_IN_KEY, new Date().toISOString()); } catch { /* the rail then shows no time */ }
}

export function forgetSignIn() {
  try { window.localStorage.removeItem(SIGNED_IN_KEY); } catch { /* nothing to forget */ }
}

export function lastSignIn(): Date | null {
  try {
    const stored = window.localStorage.getItem(SIGNED_IN_KEY);
    const date = stored ? new Date(stored) : null;
    return date && !Number.isNaN(date.getTime()) ? date : null;
  } catch { return null; }
}
