import { forwardRef } from "react";
import { submitOnShortcut } from "../../lib/submitShortcut";
import { cn } from "../../lib/utils";

export type TextareaProps = React.TextareaHTMLAttributes<HTMLTextAreaElement>;

/**
 * The shared text-field treatment for longer form values. Ctrl+Enter (Cmd+Enter on macOS)
 * submits the form the field is in, unless the field's own `onKeyDown` handled the key.
 */
export const Textarea = forwardRef<HTMLTextAreaElement, TextareaProps>(({ className, onKeyDown, ...props }, ref) => (
  <textarea
    className={cn("rm-textarea", className)}
    onKeyDown={(event) => {
      onKeyDown?.(event);
      if (!event.defaultPrevented) submitOnShortcut(event);
    }}
    ref={ref}
    {...props}
  />
));
Textarea.displayName = "Textarea";
