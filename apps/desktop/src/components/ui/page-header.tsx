import type { ReactNode } from "react";
import { cn } from "../../lib/utils";

export interface PageHeaderProps {
  /** Buttons at the end of the header. */
  actions?: ReactNode;
  /** A link to the page this one belongs to, shown above the title. */
  back?: { label: string; onClick: () => void };
  className?: string;
  /** One or two sentences under the title. */
  description?: ReactNode;
  /** A decorative icon before the title (a workspace's icon). */
  icon?: ReactNode;
  /** What the page belongs to (a workspace's name): shown before the title and a dot. Without a `title` it is the title. */
  scope?: string;
  /** The page's title: the one `h1` of the page. */
  title?: ReactNode;
}

/** The header of a page: the title, a short description, actions and an optional back link. */
export function PageHeader({ actions, back, className, description, icon, scope, title }: PageHeaderProps) {
  return (
    <div className={cn("rm-page-header", className)}>
      <div>
        {back ? <button className="rm-page-header-back" onClick={back.onClick} type="button">‹ {back.label}</button> : null}
        <h1 className={scope && title ? "rm-page-header-scoped" : undefined}>
          {icon ? <span aria-hidden="true" className="rm-page-header-icon">{icon}</span> : null}
          {scope && title ? <><span className="rm-page-header-scope">{scope}<span aria-hidden="true"> · </span></span>{title}</> : title || scope}
        </h1>
        {description ? <p>{description}</p> : null}
      </div>
      {actions ? <div className="rm-page-header-actions">{actions}</div> : null}
    </div>
  );
}
