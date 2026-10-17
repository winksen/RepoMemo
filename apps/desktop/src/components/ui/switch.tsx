import * as SwitchPrimitive from "@radix-ui/react-switch";
import type { ReactNode } from "react";
import { cn } from "../../lib/utils";

export interface SwitchProps {
  "aria-label"?: string;
  checked: boolean;
  className?: string;
  disabled?: boolean;
  id?: string;
  /** Text beside the switch, for example its state; clicking it toggles the switch. */
  label?: ReactNode;
  onCheckedChange: (checked: boolean) => void;
}

/** An on and off setting. Give it a `label` or an `aria-label`, or tie a `<label htmlFor>` to its `id`. */
export function Switch({ "aria-label": ariaLabel, checked, className, disabled, id, label, onCheckedChange }: SwitchProps) {
  const control = (
    <SwitchPrimitive.Root
      aria-label={ariaLabel}
      checked={checked}
      className={cn("rm-switch", label ? undefined : className)}
      disabled={disabled}
      id={id}
      onCheckedChange={onCheckedChange}
    >
      <SwitchPrimitive.Thumb className="rm-switch-thumb" />
    </SwitchPrimitive.Root>
  );
  if (!label) return control;
  return <label className={cn("rm-switch-field", className)} data-disabled={disabled ? "" : undefined}>{control}<span>{label}</span></label>;
}
