import type { ReactNode } from "react";
import { Button } from "./button";
import { SkeletonList } from "./skeleton";

export function FeedbackNotice({ kind, title, detail, action, onAction, rows = 3 }: {
  kind: "loading" | "partial" | "error";
  title: string;
  detail?: ReactNode;
  action?: string;
  onAction?: () => void;
  /** How many placeholder rows stand in for the content while `kind` is `loading`. */
  rows?: number;
}) {
  if (kind === "loading") return <SkeletonList className="rm-feedback-loading" label={title} rows={rows} />;
  return <div className={`rm-feedback-notice rm-feedback-${kind}`} role={kind === "error" ? "alert" : "status"}>
    <div><strong>{title}</strong>{detail ? <p>{detail}</p> : null}</div>
    {action && onAction ? <Button onClick={onAction} type="button" variant="secondary">{action}</Button> : null}
  </div>;
}
