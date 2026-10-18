import { IconLoader2 } from "@tabler/icons-react";
import { cn } from "../../lib/utils";
import { ICON_SIZE } from "./icon";

/** A small turning ring for work in progress, inline with text or inside a button. */
export function Spinner({ className, size = ICON_SIZE.md }: { className?: string; size?: number }) {
  return <IconLoader2 aria-hidden="true" className={cn("rm-spinner spin", className)} size={size} />;
}
