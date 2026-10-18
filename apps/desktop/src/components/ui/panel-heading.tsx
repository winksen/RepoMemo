import type { TablerIcon } from "@tabler/icons-react";
import type { ReactNode } from "react";
import { cn } from "../../lib/utils";
import { ICON_SIZE } from "./icon";

export interface PanelHeadingProps {
  /** Buttons or a status element at the end of the heading. */
  actions?: ReactNode;
  className?: string;
  /** A muted note at the end of the heading: a count, a state or a short qualifier. */
  description?: ReactNode;
  /** A Tabler icon component; it is drawn at the heading's size. */
  icon?: TablerIcon;
  /** The id of the heading element, for `aria-labelledby`. */
  id?: string;
  /** 2 for a panel, 3 for a part of a panel. The page itself has the h1. */
  level?: 2 | 3;
  title: ReactNode;
}

/** The heading of a panel: an icon, the title, an optional muted note and optional actions. */
export function PanelHeading({ actions, className, description, icon: Icon, id, level = 2, title }: PanelHeadingProps) {
  const Heading = level === 3 ? "h3" : "h2";
  return (
    <div className={cn("rm-panel-heading", className)}>
      <div>
        {Icon ? <Icon size={level === 3 ? ICON_SIZE.md : ICON_SIZE.lg} /> : null}
        <Heading id={id}>{title}</Heading>
      </div>
      {description !== undefined && description !== null && description !== false ? <span className="rm-panel-heading-description">{description}</span> : null}
      {actions}
    </div>
  );
}
