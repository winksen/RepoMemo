import type { ReactNode } from "react";

/** Content that opens and closes smoothly (a height and fade transition) instead of appearing and vanishing; closed, it is skipped by focus and screen readers. */
export function Collapse({ children, open }: { children: ReactNode; open: boolean }) {
  const closed = open ? {} : { inert: "" };
  return <div aria-hidden={open ? undefined : true} className="rm-collapse" data-open={open ? "true" : "false"} {...closed}><div>{children}</div></div>;
}
