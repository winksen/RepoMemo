import * as ToggleGroupPrimitive from "@radix-ui/react-toggle-group";
import type { ReactNode } from "react";
import { cn } from "../../lib/utils";

export interface SegmentedOption<T extends string> {
  /** A count shown after the label, muted. */
  count?: number;
  icon?: ReactNode;
  /** The visible text; leave it out for an icon-only option and give `name` instead. */
  label?: string;
  /** The accessible name of an icon-only option. */
  name?: string;
  value: T;
}

export interface SegmentedControlProps<T extends string> {
  "aria-label": string;
  className?: string;
  /** Stretch the options to share the width. */
  fill?: boolean;
  /** The arrow keys choose as they move, as tabs do. Otherwise Space or Enter chooses. */
  followFocus?: boolean;
  onValueChange: (value: T) => void;
  options: SegmentedOption<T>[];
  value: T;
}

/**
 * One choice out of a few that are all shown at once, as joined segments: a view switch, a
 * filter, sign in or create account. Arrow keys move between the segments, Home and End go to
 * the ends, and the chosen one cannot be turned off. For many options use a `Dropdown`.
 */
export function SegmentedControl<T extends string>({ "aria-label": ariaLabel, className, fill = false, followFocus = false, onValueChange, options, value }: SegmentedControlProps<T>) {
  return (
    <ToggleGroupPrimitive.Root
      aria-label={ariaLabel}
      className={cn("rm-segmented", fill && "rm-segmented-fill", className)}
      onValueChange={(next) => { if (next) onValueChange(next as T); }}
      type="single"
      value={value}
    >
      {options.map((option) => (
        <ToggleGroupPrimitive.Item aria-label={option.label ? undefined : option.name} className="rm-segmented-item" key={option.value} onFocus={followFocus ? () => onValueChange(option.value) : undefined} value={option.value}>
          {option.icon}
          {option.label}
          {option.count !== undefined ? <span className="rm-segmented-count">{option.count}</span> : null}
        </ToggleGroupPrimitive.Item>
      ))}
    </ToggleGroupPrimitive.Root>
  );
}
