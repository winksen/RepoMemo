import { cn } from "../../lib/utils";
import { ICON_SIZE } from "./icon";
import { Spinner } from "./spinner";

/**
 * A spinner and one muted line for something that is still loading. Word the label
 * "Loading <thing>" (the component adds the ellipsis), for example `Loading jobs`.
 */
export function LoadingState({ className, label }: { className?: string; label: string }) {
  return <p className={cn("rm-loading-state", className)} role="status"><Spinner size={ICON_SIZE.sm} />{label}…</p>;
}
