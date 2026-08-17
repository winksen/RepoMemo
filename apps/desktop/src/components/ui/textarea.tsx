import { forwardRef } from "react";
import { cn } from "../../lib/utils";

export type TextareaProps = React.TextareaHTMLAttributes<HTMLTextAreaElement>;

/** The shared text-field treatment for longer form values. */
export const Textarea = forwardRef<HTMLTextAreaElement, TextareaProps>(({ className, ...props }, ref) => (
  <textarea className={cn("rm-textarea", className)} ref={ref} {...props} />
));
Textarea.displayName = "Textarea";
