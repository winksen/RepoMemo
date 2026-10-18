import { forwardRef, type HTMLAttributes, type ReactNode } from "react";
import { cn } from "../../lib/utils";

export type BadgeTone = "neutral" | "success" | "warning" | "danger" | "info";

export interface BadgeProps extends HTMLAttributes<HTMLSpanElement> {
  /** A small icon before the text. */
  icon?: ReactNode;
  /** `neutral` has no dot; the others add a small status dot. The fill and the text stay grey. */
  tone?: BadgeTone;
}

/** A short label for a state, role or kind: neutral surface and text, with a status dot for a tone. */
export const Badge = forwardRef<HTMLSpanElement, BadgeProps>(({ children, className, icon, tone = "neutral", ...props }, ref) => (
  <span className={cn("rm-badge", className)} data-tone={tone === "neutral" ? undefined : tone} ref={ref} {...props}>
    {icon}
    {children}
  </span>
));
Badge.displayName = "Badge";
