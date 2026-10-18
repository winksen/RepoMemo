import * as TooltipPrimitive from "@radix-ui/react-tooltip";
import type { ReactElement, ReactNode } from "react";

/**
 * Names or explains the one control it wraps, on hover and on keyboard focus. The child must be
 * an element that takes a ref (a button, an `IconButton`, a `span`). Wrap a disabled control in a
 * `span` so the tooltip can still open. Use it instead of a `title` attribute.
 */
export function Tooltip({ children, content }: { children: ReactElement; content: ReactNode }) {
  return (
    <TooltipPrimitive.Provider delayDuration={400} skipDelayDuration={150}>
      <TooltipPrimitive.Root>
        <TooltipPrimitive.Trigger asChild>{children}</TooltipPrimitive.Trigger>
        <TooltipPrimitive.Portal>
          <TooltipPrimitive.Content className="rm-tooltip" collisionPadding={8} side="top" sideOffset={6}>{content}</TooltipPrimitive.Content>
        </TooltipPrimitive.Portal>
      </TooltipPrimitive.Root>
    </TooltipPrimitive.Provider>
  );
}
