import { appLinkFor } from "../lib/appLink";
import { platform } from "../lib/platform";
import { sharedApiUrl } from "../lib/sharedApi";

/**
 * *Open in the app* (V0.4.54): a link that hands a page of this client to the RepoMemo desktop app, which shows the same page for the
 * same server. Shown in a browser only; the app has no use for it. A system without the app does nothing with it.
 */
export function OpenInAppLink({ path, label = "Open in the app", className }: { path: string; label?: string; className?: string }) {
  if (!platform.opensInApp) return null;
  const href = appLinkFor(sharedApiUrl || window.location.origin, path);
  if (!href) return null;
  return <a className={className} href={href} title="Needs the RepoMemo desktop app on this computer">{label}</a>;
}
