/** Where the command palette sends a person: enough to list it again, and nothing it could be tricked into running. */
export interface RecentItem {
  /** The palette entry's id, so the same place is listed once. */
  id: string;
  label: string;
  hint?: string;
  /** An address of the web app, such as `/workspaces/w1/artifacts/a1`. */
  to: string;
  /** The workspace it belongs to; it is listed only while the person can still read that workspace. */
  workspaceId: string;
}

/** Recent places kept for one person. */
export const MAX_RECENTS = 8;
const KEY_PREFIX = "repomemo.palette.recent.";
/** An address of the app and nothing else: a path, never a scheme or another site. */
const APP_PATH = /^\/[A-Za-z0-9_\-./%]*$/;

function key(userId: string) {
  return `${KEY_PREFIX}${userId}`;
}

function valid(value: unknown): value is RecentItem {
  if (!value || typeof value !== "object") return false;
  const item = value as Partial<RecentItem>;
  return typeof item.id === "string" && item.id.length > 0 && item.id.length <= 200
    && typeof item.label === "string" && item.label.length > 0 && item.label.length <= 300
    && (item.hint === undefined || (typeof item.hint === "string" && item.hint.length <= 300))
    && typeof item.to === "string" && APP_PATH.test(item.to) && !item.to.startsWith("//") && item.to.length <= 300
    && typeof item.workspaceId === "string" && item.workspaceId.length > 0;
}

/** The recent places of this person, newest first; none when nothing is kept or storage is not available. */
export function readRecents(userId: string): RecentItem[] {
  if (!userId) return [];
  try {
    const parsed: unknown = JSON.parse(window.localStorage.getItem(key(userId)) ?? "[]");
    return Array.isArray(parsed) ? parsed.filter(valid).slice(0, MAX_RECENTS) : [];
  } catch {
    return [];
  }
}

/** Puts a place at the top of this person's recent places, once, and keeps the newest few. */
export function rememberRecent(userId: string, item: RecentItem): void {
  if (!userId || !valid(item)) return;
  try {
    const next = [item, ...readRecents(userId).filter((existing) => existing.id !== item.id)].slice(0, MAX_RECENTS);
    window.localStorage.setItem(key(userId), JSON.stringify(next));
  } catch { /* only a convenience */ }
}

/** Forgets this person's recent places. */
export function clearRecents(userId: string): void {
  try { window.localStorage.removeItem(key(userId)); } catch { /* only a convenience */ }
}
