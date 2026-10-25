import { type VersionVerdict, versionAdvice } from "../lib/versionCheck";

/**
 * Shown instead of a broken screen when the app and the server are not the same release line (V0.4.21): it says which
 * is which, with both versions, and what to do. The same page serves the browser and the desktop window.
 */
export function VersionMismatch({ verdict, desktop }: { verdict: Extract<VersionVerdict, { ok: false }>; desktop: boolean }) {
  const advice = versionAdvice(verdict, desktop);
  return (
    <main className="shared-auth-canvas">
      <section aria-labelledby="version-mismatch-title" className="shared-auth-intro">
        <div className="shared-brand"><img alt="RepoMemo" className="shared-brand-full" src="/RM-logofull.svg" /></div>
        <p className="shared-eyebrow">Versions do not match</p>
        <h1 id="version-mismatch-title">{advice.title}</h1>
        <p className="shared-intro-copy">{advice.body}</p>
        <dl className="shared-trust-list">
          <div><dt>This app</dt><dd>{verdict.clientVersion}</dd></div>
          <div><dt>The server</dt><dd>{verdict.serverVersion ?? "does not say (earlier than 0.4.3)"}</dd></div>
        </dl>
      </section>
    </main>
  );
}
