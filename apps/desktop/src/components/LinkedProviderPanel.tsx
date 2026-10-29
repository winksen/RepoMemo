// Profile section: which sign-in provider is linked to this account, and the explicit way to link one.
import { useCallback, useEffect, useState } from "react";
import { fetchSignInProvider, listProviderLinks, startProviderLink, type ProviderLink } from "../lib/sharedApi";
import { apiMessage, formatProfileTime } from "../web/format";
import { PanelHeading } from "./ui/panel-heading";
import { showToast } from "./ui/toast";
import { ProviderSignIn } from "./ProviderSignIn";

export function LinkedProviderPanel({ accessToken }: { accessToken: string }) {
  const [provider, setProvider] = useState<string | null>(null);
  const [links, setLinks] = useState<ProviderLink[]>([]);
  const refresh = useCallback(async () => {
    try {
      const [name, linked] = await Promise.all([fetchSignInProvider(), listProviderLinks(accessToken)]);
      setProvider(name.name);
      setLinks(linked.links);
    } catch (problem) { showToast("error", apiMessage(problem, "Linked sign-in providers could not be loaded.")); }
  }, [accessToken]);
  useEffect(() => { void refresh(); }, [refresh]);
  const onDone = useCallback(() => { showToast("success", "Provider linked to your account."); void refresh(); }, [refresh]);
  if (!provider && links.length === 0) return null;
  return <section aria-label="Sign-in provider" className="shared-panel">
    <PanelHeading title="Sign-in provider" />
    {links.length ? <ul>{links.map(([issuer, at]) => <li key={issuer}>{issuer} · linked {formatProfileTime(at)}</li>)}</ul>
      : <p className="shared-muted-copy">No provider is linked. Linking needs you to be signed in here and to sign in at the provider; matching email addresses alone never link an account.</p>}
    {provider && links.length === 0
      ? <ProviderSignIn keepSession={false} label={`Link ${provider}`} onDone={onDone} start={() => startProviderLink(accessToken)} />
      : null}
  </section>;
}
