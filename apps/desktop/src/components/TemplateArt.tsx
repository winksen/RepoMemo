import type { ReactNode } from "react";

/**
 * A small drawing of what a template is for, shown on its card. Drawn with the theme's tokens so it follows light and dark:
 * greys for the structure, the accent for the one thing that matters. A template this app has no drawing for (a custom
 * one from the operator's folder) gets a neutral grid.
 */
const ART: Record<string, ReactNode> = {
  // Every section in its usual order: a stack of cards.
  general: <>
    <rect fill="var(--color-surface-3)" height="46" rx="8" width="96" x="38" y="34" />
    <rect fill="var(--color-surface)" height="46" rx="8" width="96" x="30" y="26" />
    <rect fill="var(--color-surface)" height="46" rx="8" width="96" x="22" y="18" />
    <path d="M36 34h44M36 44h68M36 54h56" />
    <circle cx="108" cy="32" fill="var(--color-accent)" r="5" stroke="none" />
  </>,
  // Code and the decisions behind it: a window holding code, a branch beside it.
  "software-engineering": <>
    <rect fill="var(--color-surface)" height="64" rx="8" width="112" x="14" y="16" />
    <path d="M14 30h112" />
    <circle cx="24" cy="23" fill="var(--color-text-muted)" r="2" stroke="none" />
    <circle cx="32" cy="23" fill="var(--color-text-muted)" r="2" stroke="none" />
    <path d="M46 44l-8 8 8 8M70 44l8 8-8 8" stroke="var(--color-accent)" />
    <path d="M62 42l-6 20" />
    <circle cx="124" cy="48" fill="var(--color-surface)" r="5" />
    <circle cx="144" cy="64" fill="var(--color-surface)" r="5" />
    <circle cx="144" cy="32" fill="var(--color-accent)" r="5" stroke="none" />
    <path d="M129 48h8q7 0 7-7v-4M129 48h8q7 0 7 7v6" />
  </>,
  // Documents: an open book with a ribbon.
  "knowledge-base": <>
    <path d="M80 28q-22-10-50-6v50q28-4 50 6z" fill="var(--color-surface)" />
    <path d="M80 28q22-10 50-6v50q-28-4-50 6z" fill="var(--color-surface)" />
    <path d="M80 28v50" />
    <path d="M40 36q14-2 28 3M40 46q14-2 28 3M40 56q14-2 28 3M92 39q14-5 28-3M92 49q14-5 28-3" />
    <path d="M110 22v22l6-5 6 5V20" fill="var(--color-accent)" stroke="none" />
  </>,
  // Runbooks and incidents: a checklist and a pulse.
  operations: <>
    <rect fill="var(--color-surface)" height="66" rx="8" width="56" x="16" y="15" />
    <path d="M30 14h28v8H30z" fill="var(--color-surface-3)" />
    <path d="M26 36l4 4 7-8M26 54l4 4 7-8" stroke="var(--color-accent)" />
    <path d="M44 38h18M44 56h18" />
    <path d="M84 52h14l7-22 11 40 8-26h20" stroke="var(--color-accent)" />
    <path d="M84 70h60" stroke="var(--color-surface-3)" />
  </>,
  // Specs and decisions: a decision splitting two ways, a note beside it.
  "product-decisions": <>
    <path d="M60 14l22 18-22 18-22-18z" fill="var(--color-surface)" />
    <path d="M60 50v8M60 58H34v8M60 58h26v8" />
    <rect fill="var(--color-surface)" height="14" rx="4" width="28" x="20" y="66" />
    <rect fill="var(--color-accent)" height="14" rx="4" stroke="none" width="28" x="72" y="66" />
    <rect fill="var(--color-surface-3)" height="34" rx="4" width="32" x="112" y="20" />
    <path d="M118 30h20M118 38h20M118 46h12" />
    <path d="M60 26l-4 6 8 0" stroke="var(--color-accent)" />
  </>,
};

const FALLBACK = <>
  <rect fill="var(--color-surface)" height="26" rx="6" width="46" x="30" y="20" />
  <rect fill="var(--color-surface)" height="26" rx="6" width="46" x="84" y="20" />
  <rect fill="var(--color-surface)" height="26" rx="6" width="46" x="30" y="52" />
  <rect fill="var(--color-accent)" height="26" rx="6" stroke="none" width="46" x="84" y="52" />
</>;

export function TemplateArt({ id }: { id: string }) {
  return (
    <svg aria-hidden="true" className="rm-template-art" fill="none" stroke="var(--color-text-muted)" strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" viewBox="0 0 160 96" xmlns="http://www.w3.org/2000/svg">
      {ART[id] ?? FALLBACK}
    </svg>
  );
}
