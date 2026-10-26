import type { ReactNode } from "react";
import { ThemeToggle } from "./SharedLayout";

/** One line of the trust list beside a sign-in form: a heading with its icon, and a sentence. */
export type TrustPoint = { icon: ReactNode; title: string; text: string };

/**
 * The frame of the pages that ask who you are or which server (V0.4.45): the brand and an introduction on one side, the form on the
 * other. The sign-in page and the desktop app's connect screen are both made of it, so they look alike and change together.
 */
export function AuthFrame({
  introLabel,
  eyebrow,
  title,
  copy,
  trust,
  formLabel,
  children,
}: {
  introLabel: string;
  eyebrow: string;
  title: string;
  copy: string;
  trust: TrustPoint[];
  formLabel: string;
  children: ReactNode;
}) {
  return (
    <main className="shared-auth-canvas">
      <ThemeToggle className="shared-auth-theme-toggle" />
      <section aria-label={introLabel} className="shared-auth-intro">
        <div className="shared-brand"><img alt="RepoMemo" className="shared-brand-full" src="/RM-logofull.svg" /></div>
        <p className="shared-eyebrow">{eyebrow}</p>
        <h1>{title}</h1>
        <p className="shared-intro-copy">{copy}</p>
        <dl className="shared-trust-list">
          {trust.map((point) => <div key={point.title}><dt>{point.icon} {point.title}</dt><dd>{point.text}</dd></div>)}
        </dl>
      </section>

      <section aria-label={formLabel} className="shared-auth-form-region">
        <div className="shared-auth-form-shell">{children}</div>
      </section>
    </main>
  );
}
