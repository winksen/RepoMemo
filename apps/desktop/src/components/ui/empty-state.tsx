import type { ReactNode } from "react";
import { cn } from "../../lib/utils";

export interface EmptyStateProps {
  /** At most one action, for example the button that creates the first item. */
  action?: ReactNode;
  className?: string;
  /** One sentence on what to do or why the list is empty. */
  description?: ReactNode;
  /** An illustration-sized icon (about 25 px), which keeps its own size. */
  icon?: ReactNode;
  title: string;
}

/** What a list shows when it has nothing in it: muted, centred in its panel. */
export function EmptyState({ action, className, description, icon, title }: EmptyStateProps) {
  return (
    <div className={cn("rm-empty-state", className)}>
      {icon}
      <strong>{title}</strong>
      {description ? <span>{description}</span> : null}
      {action}
    </div>
  );
}
