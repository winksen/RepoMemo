import { Button } from "./ui/button";

/** A server and a plug that has come away from it: the connection is broken. Drawn in the theme's greys. */
function BrokenConnection() {
  return (
    <svg aria-hidden="true" className="shared-unreachable-art" fill="none" viewBox="0 0 280 140" xmlns="http://www.w3.org/2000/svg">
      <rect fill="var(--color-surface-2)" height="116" rx="58" width="116" x="82" y="12" />
      <g stroke="var(--color-text)" strokeLinecap="round" strokeLinejoin="round" strokeWidth="3">
        <rect fill="var(--color-surface)" height="30" rx="7" width="76" x="28" y="34" />
        <rect fill="var(--color-surface)" height="30" rx="7" width="76" x="28" y="74" />
        <path d="M104 49h18M104 89h18" strokeDasharray="2 7" />
        <path d="M176 70h-16M176 58v24M192 58v24" />
        <path d="M192 70h40" strokeDasharray="2 7" />
        <path d="M232 50l16 20-16 20" />
      </g>
      <g fill="var(--color-text)">
        <circle cx="42" cy="49" r="3.5" />
        <circle cx="42" cy="89" r="3.5" />
      </g>
      <g stroke="var(--color-text-muted)" strokeLinecap="round" strokeWidth="3">
        <path d="M137 40l-6-10M149 36v-12M161 40l6-10" />
      </g>
    </svg>
  );
}

/**
 * Shown instead of the sign-in page when the server cannot be reached at all: signing in would only fail, and a wrong
 * password or a missing account is not what is wrong. The page starts again from the beginning, so whatever the app
 * assumed while it could not ask the server (its setup, its session) is asked again.
 */
export function ServerUnreachable() {
  return (
    <main className="shared-unreachable">
      <section aria-labelledby="server-unreachable-title" className="shared-unreachable-card" role="alert">
        <img alt="RepoMemo" className="shared-brand-full shared-unreachable-logo" src="/RM-logofull.svg" />
        <BrokenConnection />
        <h1 id="server-unreachable-title">We can&apos;t connect to the server right now</h1>
        <p>Check your connection and try again in a moment. If it keeps happening, contact your administrator.</p>
        <Button onClick={() => window.location.reload()} type="button" variant="main">Try again</Button>
      </section>
    </main>
  );
}
