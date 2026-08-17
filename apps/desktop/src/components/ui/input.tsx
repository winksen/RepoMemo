import { forwardRef } from "react";
import { cn } from "../../lib/utils";

export type InputProps = React.InputHTMLAttributes<HTMLInputElement>;

/** The shared text-field treatment for short form values. */
export const Input = forwardRef<HTMLInputElement, InputProps>(({ className, ...props }, ref) => (
  <input className={cn("rm-input", className)} ref={ref} {...props} />
));
Input.displayName = "Input";
