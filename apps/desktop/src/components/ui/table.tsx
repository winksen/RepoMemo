import { forwardRef, type HTMLAttributes, type ReactNode } from "react";
import { cn } from "../../lib/utils";

export interface TableProps extends HTMLAttributes<HTMLDivElement> {
  /** Content placed over the table inside the scrolling box, such as a hover tooltip. */
  overlay?: ReactNode;
}

/**
 * The markup of a data table: a box that scrolls sideways on narrow screens and, past a
 * height, keeps the header in view, around a `<table className="rm-table">`. Give the content
 * a `<thead>` with `scope="col"` headers and a `<tbody>`; numbers take `className="rm-table-number"`.
 */
export const Table = forwardRef<HTMLDivElement, TableProps>(({ children, className, overlay, ...props }, ref) => (
  <div className={cn("rm-table-box", className)} ref={ref} {...props}>
    <table className="rm-table">{children}</table>
    {overlay}
  </div>
));
Table.displayName = "Table";
