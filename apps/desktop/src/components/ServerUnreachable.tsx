import { Button } from "./ui/button";

/**
 * Shown instead of the sign-in page when the server cannot be reached at all: signing in would only fail, and a wrong
 * password or a missing account is not what is wrong. The page starts again from the beginning, so whatever the app
 * assumed while it could not ask the server (its setup, its session) is asked again.
 */
export function ServerUnreachable() {
  return (
    <main className="shared-auth-canvas">
      <section aria-labelledby="server-unreachable-title" className="shared-auth-intro" role="alert">
        <div className="shared-brand"><img alt="RepoMemo" className="shared-brand-full" src="/RM-logofull.svg" /></div>
        <p className="shared-eyebrow">No connection</p>
        <h1 id="server-unreachable-title">We can&apos;t connect to the server right now</h1>
        <p className="shared-intro-copy">Check your connection and try again in a moment. If it keeps happening, the server may be down for maintenance: contact your administrator.</p>
        <Button onClick={() => window.location.reload()} type="button" variant="main">Try again</Button>
      </section>
    </main>
  );
}
