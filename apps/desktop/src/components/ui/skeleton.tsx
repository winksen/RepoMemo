import type { CSSProperties } from "react";
import { cn } from "../../lib/utils";

/**
 * Grey placeholders that stand where content will be, shimmering until it arrives, so a page keeps its shape while it
 * loads instead of showing a spinner on an empty area. They are decoration: each group says what is loading once, as
 * a `status`, and the blocks are hidden from assistive technology. With reduced motion the shimmer is switched off.
 */
export function Skeleton({ className, height, round = false, width }: { className?: string; height?: number | string; round?: boolean; width?: number | string }) {
  const style: CSSProperties = { width, height };
  return <span aria-hidden="true" className={cn("rm-skeleton", round && "rm-skeleton-round", className)} style={style} />;
}

/** Lines of text, the last one shorter. */
export function SkeletonText({ lines = 3 }: { lines?: number }) {
  return <span aria-hidden="true" className="rm-skeleton-text">{Array.from({ length: lines }, (_, index) => <Skeleton height={12} key={index} width={index === lines - 1 && lines > 1 ? "62%" : "100%"} />)}</span>;
}

/** The label is read once ("Loading jobs…"); the rows are what the list will look like: an icon and two lines. */
export function SkeletonList({ className, label, rows = 4 }: { className?: string; label: string; rows?: number }) {
  return <div aria-busy="true" className={cn("rm-skeleton-list", className)} role="status">
    <span className="sr-only">{label}…</span>
    {Array.from({ length: rows }, (_, index) => <div aria-hidden="true" className="rm-skeleton-row" key={index}>
      <Skeleton height={32} round width={32} />
      <span><Skeleton height={13} width={`${52 + ((index * 17) % 30)}%`} /><Skeleton height={11} width={`${30 + ((index * 23) % 28)}%`} /></span>
    </div>)}
  </div>;
}

/** A row of figures' worth of cards. */
export function SkeletonCards({ count = 3, label }: { count?: number; label: string }) {
  return <div aria-busy="true" className="rm-skeleton-cards" role="status">
    <span className="sr-only">{label}…</span>
    {Array.from({ length: count }, (_, index) => <div aria-hidden="true" className="rm-skeleton-card" key={index}><Skeleton height={12} width="45%" /><Skeleton height={26} width="35%" /><Skeleton height={11} width="70%" /></div>)}
  </div>;
}

/** A whole page before its content: a heading, a line under it, and the shape of the body. */
export function PageSkeleton({ label = "Loading the page", variant = "list" }: { label?: string; variant?: "list" | "cards" }) {
  return <section aria-busy="true" className="shared-page-content rm-skeleton-page" role="status">
    <span className="sr-only">{label}…</span>
    <div aria-hidden="true" className="rm-skeleton-heading"><Skeleton height={26} width={220} /><Skeleton height={13} width="min(420px, 70%)" /></div>
    <div aria-hidden="true" className="rm-skeleton-body">
      {variant === "cards" ? <div className="rm-skeleton-cards">{Array.from({ length: 4 }, (_, index) => <div className="rm-skeleton-card" key={index}><Skeleton height={12} width="45%" /><Skeleton height={26} width="35%" /><Skeleton height={11} width="70%" /></div>)}</div> : null}
      <div className="rm-skeleton-list">{Array.from({ length: 6 }, (_, index) => <div className="rm-skeleton-row" key={index}>
        <Skeleton height={32} round width={32} />
        <span><Skeleton height={13} width={`${52 + ((index * 17) % 30)}%`} /><Skeleton height={11} width={`${30 + ((index * 23) % 28)}%`} /></span>
      </div>)}</div>
    </div>
  </section>;
}

/** The whole signed-in frame as placeholders, while the session and the lists are fetched: the bar, the rail and a page. */
export function AppSkeleton({ label }: { label: string }) {
  return <div aria-busy="true" className="shared-home" role="status">
    <span className="sr-only">{label}…</span>
    <header aria-hidden="true" className="shared-home-header"><img alt="" className="shared-brand-full" src="/RM-logofull.svg" /><span className="rm-skeleton-bar-actions"><Skeleton height={32} round width={32} /><Skeleton height={32} round width={32} /><Skeleton height={32} width={92} /></span></header>
    <div aria-hidden="true" className="shared-home-frame">
      <aside className="shared-home-rail"><Skeleton height={13} width={96} /><div className="rm-skeleton-rail">{Array.from({ length: 6 }, (_, index) => <Skeleton height={28} key={index} width={`${70 + ((index * 11) % 26)}%`} />)}</div></aside>
      <main className="shared-home-content"><PageSkeleton label={label} variant="cards" /></main>
    </div>
  </div>;
}
