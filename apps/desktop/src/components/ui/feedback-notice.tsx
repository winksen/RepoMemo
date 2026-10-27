import type { ReactNode } from "react";
import { Button } from "./button";

export function FeedbackNotice({ kind, title, detail, action, onAction }: {
  kind: "loading" | "partial" | "error";
  title: string;
  detail?: ReactNode;
  action?: string;
  onAction?: () => void;
}) {
  return <div className={`rm-feedback-notice rm-feedback-${kind}`} role={kind === "error" ? "alert" : "status"}>
    <div><strong>{title}</strong>{detail ? <p>{detail}</p> : null}</div>
    {action && onAction ? <Button onClick={onAction} type="button" variant="secondary">{action}</Button> : null}
  </div>;
}
