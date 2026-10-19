import type { ReactNode } from "react";
import { IconCircleCheck as CircleCheck, IconLock as Lock, IconLogout as Logout } from "@tabler/icons-react";
import type { SharedSession } from "../../types";
import { ThemeToggle } from "../../components/SharedLayout";
import { Button } from "../../components/ui/button";
import { ICON_SIZE } from "../../components/ui/icon";
import { STEPS } from "./steps";

/** The page every step sits in: the header, the list of steps with where the setup stands, and the panel of the current step. */
export function StepFrame({ children, onSignOut, session, stepIndex }: { children: ReactNode; onSignOut?: () => void; session?: SharedSession; stepIndex: number }) {
  return <main className="rm-setup">
    <header className="rm-setup-header">
      <div className="shared-brand"><img alt="RepoMemo" className="shared-brand-full" src="/RM-logofull.svg" /></div>
      <div className="rm-setup-header-actions">
        {session ? <span className="rm-setup-signed-in">Signed in as {session.user.email ?? session.user.display_name}</span> : null}
        {session && onSignOut ? <Button onClick={onSignOut} type="button" variant="secondary"><Logout size={ICON_SIZE.sm} /> Sign out</Button> : null}
        <ThemeToggle />
      </div>
    </header>
    <div className="rm-setup-body">
      <nav aria-label="Setup steps" className="rm-setup-nav">
        <p className="shared-eyebrow">Server setup</p>
        <ol className="rm-setup-steps">
          {STEPS.map((entry, index) => {
            const state = index < stepIndex ? "done" : index === stepIndex ? "current" : "upcoming";
            return <li aria-current={state === "current" ? "step" : undefined} className={`rm-setup-step ${state}`} key={entry.key}>
              <span className="rm-setup-step-mark" aria-hidden="true">{state === "done" ? <CircleCheck size={22} /> : entry.icon}</span>
              <span>{entry.label}<small>{state === "done" ? "Done" : state === "current" ? "In progress" : "To do"}</small></span>
            </li>;
          })}
        </ol>
        <p className="rm-setup-nav-note"><Lock size={ICON_SIZE.sm} /> This page is only available until the setup is finished. It never shows again afterwards.</p>
      </nav>
      <section className="rm-setup-panel" aria-live="polite">
        {children}
      </section>
    </div>
  </main>;
}

export function StepHeading({ eyebrow, title, children }: { eyebrow: string; title: string; children: ReactNode }) {
  return <div className="shared-auth-heading rm-setup-heading"><p className="shared-eyebrow">{eyebrow}</p><h2>{title}</h2><p>{children}</p></div>;
}
