import * as SelectPrimitive from "@radix-ui/react-select";
import { IconCheck, IconChevronDown } from "@tabler/icons-react";
import { cn } from "../../lib/utils";

export interface DropdownOption {
  label: string;
  value: string;
  disabled?: boolean;
}

export interface DropdownProps {
  "aria-label"?: string;
  className?: string;
  disabled?: boolean;
  id?: string;
  onValueChange: (value: string) => void;
  options: DropdownOption[];
  placeholder?: string;
  value?: string;
}

/** A single calm, keyboard-accessible select treatment for RepoMemo forms. */
export function Dropdown({
  className,
  disabled,
  id,
  onValueChange,
  options,
  placeholder,
  value,
  ...props
}: DropdownProps) {
  return (
    <SelectPrimitive.Root disabled={disabled} onValueChange={onValueChange} value={value}>
      <SelectPrimitive.Trigger className={cn("rm-dropdown-trigger", className)} id={id} {...props}>
        <SelectPrimitive.Value placeholder={placeholder} />
        <SelectPrimitive.Icon className="rm-dropdown-icon"><IconChevronDown size={16} /></SelectPrimitive.Icon>
      </SelectPrimitive.Trigger>
      <SelectPrimitive.Portal>
        <SelectPrimitive.Content className="rm-dropdown-content" position="popper" sideOffset={6}>
          <SelectPrimitive.Viewport className="rm-dropdown-viewport">
            {options.map((option) => (
              <SelectPrimitive.Item className="rm-dropdown-item" disabled={option.disabled} key={option.value} value={option.value}>
                <SelectPrimitive.ItemText>{option.label}</SelectPrimitive.ItemText>
                <SelectPrimitive.ItemIndicator className="rm-dropdown-check"><IconCheck size={15} /></SelectPrimitive.ItemIndicator>
              </SelectPrimitive.Item>
            ))}
          </SelectPrimitive.Viewport>
        </SelectPrimitive.Content>
      </SelectPrimitive.Portal>
    </SelectPrimitive.Root>
  );
}
