import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import { forwardRef } from "react";
import { cn } from "../../lib/utils";
import { Spinner } from "./spinner";

export const buttonVariants = cva("rm-button", {
  variants: {
    variant: {
      main: "rm-button-main",
      secondary: "rm-button-secondary",
    },
  },
  defaultVariants: {
    variant: "secondary",
  },
});

/** `type` defaults to `button`; only a form's one submit button says `type="submit"`. */
export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  asChild?: boolean;
  /** Work is in progress: a spinner replaces the icon, the label stays and the button is disabled. */
  busy?: boolean;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  ({ asChild = false, busy = false, children, className, disabled, type, variant, ...props }, ref) => {
    const Comp = asChild ? Slot : "button";
    return (
      <Comp
        aria-busy={busy || undefined}
        className={cn(buttonVariants({ variant }), className)}
        disabled={disabled || busy}
        ref={ref}
        // A plain button unless it says submit, so a button inside a form never submits it by accident.
        type={asChild ? type : (type ?? "button")}
        {...props}
      >
        {asChild ? children : <>{busy ? <Spinner /> : null}{children}</>}
      </Comp>
    );
  },
);
Button.displayName = "Button";
