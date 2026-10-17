import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import { forwardRef } from "react";
import { cn } from "../../lib/utils";

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
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  ({ asChild = false, className, type, variant, ...props }, ref) => {
    const Comp = asChild ? Slot : "button";
    // A plain button unless it says submit, so a button inside a form never submits it by accident.
    return <Comp className={cn(buttonVariants({ variant }), className)} ref={ref} type={asChild ? type : (type ?? "button")} {...props} />;
  },
);
Button.displayName = "Button";
