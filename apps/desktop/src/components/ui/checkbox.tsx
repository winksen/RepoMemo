import * as CheckboxPrimitive from "@radix-ui/react-checkbox";
import type { ReactNode } from "react";
import { cn } from "../../lib/utils";

export interface CheckboxProps {
  checked: boolean;
  className?: string;
  /** A muted second line under the label. */
  description?: ReactNode;
  disabled?: boolean;
  id?: string;
  /** What the checkbox is for; the whole label is clickable. */
  label: ReactNode;
  onCheckedChange: (checked: boolean) => void;
}

/** A 16 px checkbox with its label and an optional description; the label toggles it. */
export function Checkbox({ checked, className, description, disabled, id, label, onCheckedChange }: CheckboxProps) {
  return (
    <label className={cn("rm-checkbox", className)} data-disabled={disabled ? "" : undefined}>
      <CheckboxPrimitive.Root
        checked={checked}
        className="rm-checkbox-box"
        disabled={disabled}
        id={id}
        onCheckedChange={(next) => onCheckedChange(next === true)}
      >
        <CheckboxPrimitive.Indicator className="rm-checkbox-mark" />
      </CheckboxPrimitive.Root>
      <span className="rm-checkbox-text">
        <span className="rm-checkbox-label">{label}</span>
        {description ? <small className="rm-checkbox-description">{description}</small> : null}
      </span>
    </label>
  );
}
