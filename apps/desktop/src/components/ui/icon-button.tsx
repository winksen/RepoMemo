import { forwardRef } from "react";
import { cn } from "../../lib/utils";
import { Button, type ButtonProps } from "./button";

export interface IconButtonProps extends Omit<ButtonProps, "asChild" | "variant"> {
  /** `outline` has the secondary button's border; `plain` is bare until hovered (menus, rows). */
  appearance?: "outline" | "plain";
  /** The button's name: icon-only buttons have no text, so it is required. */
  label: string;
  /** Marks a toggle that is on (`aria-pressed`). */
  pressed?: boolean;
  /** 32 px (`sm`, in rows and menus) or 36 px (`md`, beside fields, the control height). */
  size?: "sm" | "md";
}

/** A square button holding one icon, with a required name. */
export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(
  ({ appearance = "outline", className, label, pressed, size = "md", title, ...props }, ref) => (
    <Button
      aria-label={label}
      aria-pressed={pressed}
      className={cn("rm-icon-button", `rm-icon-button-${size}`, appearance === "plain" && "rm-icon-button-plain", className)}
      ref={ref}
      title={title ?? label}
      variant="secondary"
      {...props}
    />
  ),
);
IconButton.displayName = "IconButton";
