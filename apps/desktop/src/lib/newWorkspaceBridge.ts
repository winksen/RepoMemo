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

const ORGANIZATION_EVENT = "repomemo:new-organization";

/** Asks the app to open the *New organization* dialog. */
export function openNewOrganization() {
  window.dispatchEvent(new Event(ORGANIZATION_EVENT));
}

/** Listens for `openNewOrganization`; returns the function that stops listening. */
export function onNewOrganizationRequest(listener: () => void): () => void {
  window.addEventListener(ORGANIZATION_EVENT, listener);
  return () => window.removeEventListener(ORGANIZATION_EVENT, listener);
}
