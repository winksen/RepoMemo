/**
 * Unsaved drafts of notes and memory cards (V0.6.5). A draft is kept only in this tab's session storage, so it survives
 * a reload or a failed save but never leaves the browser, is never published by itself and is gone when the tab closes.
 * Each key names the server and the person, so another account or another server in the same tab cannot read it, and
 * `clearAllDrafts` runs when the session ends.
 */

const PREFIX = "repomemo.draft.";

export type DraftKind = "note" | "memory-card";

export interface DraftScope {
  /** The server's address, as the client reaches it. */
  server: string;
  userId: string;
}

export interface Draft<T> {
  /** When the draft was last written, for the person to judge how old it is. */
  savedAt: string;
  /** What the draft was based on (a revision number), so a later conflict is still detected when it is restored. */
  base: number | null;
  value: T;
}

function key(scope: DraftScope, kind: DraftKind, id: string) {
  return `${PREFIX}${encodeURIComponent(scope.server)}|${scope.userId}|${kind}|${id}`;
}

function storage(): Storage | null {
  try {
    return window.sessionStorage;
  } catch {
    return null;
  }
}

export function saveDraft<T>(scope: DraftScope, kind: DraftKind, id: string, value: T, base: number | null = null) {
  try {
    const draft: Draft<T> = { savedAt: new Date().toISOString(), base, value };
    storage()?.setItem(key(scope, kind, id), JSON.stringify(draft));
  } catch {
    // storage full or unavailable: the draft is simply not kept
  }
}

export function loadDraft<T>(scope: DraftScope, kind: DraftKind, id: string): Draft<T> | null {
  try {
    const raw = storage()?.getItem(key(scope, kind, id));
    if (!raw) return null;
    const draft = JSON.parse(raw) as Draft<T>;
    return draft && typeof draft.savedAt === "string" && "value" in draft ? draft : null;
  } catch {
    return null;
  }
}

export function discardDraft(scope: DraftScope, kind: DraftKind, id: string) {
  try {
    storage()?.removeItem(key(scope, kind, id));
  } catch {
    // nothing to do
  }
}

/** Removes every draft this tab holds: sign-out, a session that ended, a change of server. */
export function clearAllDrafts() {
  const store = storage();
  if (!store) return;
  try {
    for (const name of Object.keys(store)) if (name.startsWith(PREFIX)) store.removeItem(name);
  } catch {
    // nothing to do
  }
}
