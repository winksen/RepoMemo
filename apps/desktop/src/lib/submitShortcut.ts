import type { KeyboardEvent } from "react";

/** True on macOS and iOS, where the shortcut is Cmd+Enter instead of Ctrl+Enter. */
function isApplePlatform(): boolean {
  if (typeof navigator === "undefined") return false;
  return /Mac|iPhone|iPad/i.test(navigator.platform || navigator.userAgent);
}

/** Ctrl+Enter, or Cmd+Enter on macOS, outside an input method's composition. */
export function isSubmitShortcut(event: { ctrlKey: boolean; isComposing?: boolean; key: string; metaKey: boolean }): boolean {
  return event.key === "Enter" && (isApplePlatform() ? event.metaKey : event.ctrlKey) && !event.isComposing;
}

/** The key combination as the muted hint writes it: "Ctrl+Enter" or "Cmd+Enter". */
export function submitShortcutLabel(): string {
  return isApplePlatform() ? "Cmd+Enter" : "Ctrl+Enter";
}

/** The muted line under a field that sends with the shortcut: "Ctrl+Enter sends." */
export function submitHint(action = "sends"): string {
  return `${submitShortcutLabel()} ${action}.`;
}

/** Submits the form that holds the field the key was pressed in. Returns whether it did. */
export function submitOnShortcut(event: KeyboardEvent<HTMLElement>): boolean {
  if (!isSubmitShortcut({ ctrlKey: event.ctrlKey, isComposing: event.nativeEvent.isComposing, key: event.key, metaKey: event.metaKey })) return false;
  const form = (event.currentTarget as HTMLTextAreaElement).form;
  if (!form) return false;
  event.preventDefault();
  form.requestSubmit();
  return true;
}
