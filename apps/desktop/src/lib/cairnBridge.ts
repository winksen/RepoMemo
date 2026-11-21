const EVENT = "repomemo:cairn-quick-ask";

/** Asks the app to open CAIRN's quick-question dialog. */
export function openQuickAsk() {
  window.dispatchEvent(new Event(EVENT));
}

/** Listens for `openQuickAsk`; returns the function that stops listening. */
export function onQuickAskRequest(listener: () => void): () => void {
  window.addEventListener(EVENT, listener);
  return () => window.removeEventListener(EVENT, listener);
}
