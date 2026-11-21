const EVENT = "repomemo:new-workspace";

/** Asks the app to open the *New workspace* dialog, with an organization already chosen when one is known. */
export function openNewWorkspace(organizationId?: string) {
  window.dispatchEvent(new CustomEvent<{ organizationId?: string }>(EVENT, { detail: { organizationId } }));
}

/** Listens for `openNewWorkspace`; returns the function that stops listening. */
export function onNewWorkspaceRequest(listener: (organizationId?: string) => void): () => void {
  const handler = (event: Event) => listener((event as CustomEvent<{ organizationId?: string }>).detail?.organizationId);
  window.addEventListener(EVENT, handler);
  return () => window.removeEventListener(EVENT, handler);
}
