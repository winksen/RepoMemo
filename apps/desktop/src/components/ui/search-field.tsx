import { IconSearch, IconX } from "@tabler/icons-react";
import { forwardRef, useRef, type InputHTMLAttributes, type KeyboardEvent } from "react";
import { cn } from "../../lib/utils";
import { ICON_SIZE } from "./icon";
import { IconButton } from "./icon-button";

export interface SearchFieldProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "onChange" | "type" | "value"> {
  "aria-label": string;
  /** Escape clears the text. Turn it off where Escape already closes something, such as a dialog. */
  clearOnEscape?: boolean;
  onValueChange: (value: string) => void;
  value: string;
}

/** A text field for filtering a list: a search icon, a clear button once there is text, and Escape to clear. */
export const SearchField = forwardRef<HTMLInputElement, SearchFieldProps>(
  ({ className, clearOnEscape = true, onKeyDown, onValueChange, value, ...props }, forwardedRef) => {
    const own = useRef<HTMLInputElement | null>(null);
    const setRef = (node: HTMLInputElement | null) => {
      own.current = node;
      if (typeof forwardedRef === "function") forwardedRef(node);
      else if (forwardedRef) forwardedRef.current = node;
    };
    function clear() {
      onValueChange("");
      own.current?.focus();
    }
    function handleKeyDown(event: KeyboardEvent<HTMLInputElement>) {
      onKeyDown?.(event);
      if (event.defaultPrevented || event.key !== "Escape" || !clearOnEscape || !value) return;
      event.preventDefault();
      event.stopPropagation();
      clear();
    }
    return (
      <div className={cn("rm-search-field", className)}>
        <IconSearch aria-hidden="true" className="rm-search-field-icon" size={ICON_SIZE.md} />
        <input
          autoComplete="off"
          className="rm-input rm-search-field-input"
          onChange={(event) => onValueChange(event.target.value)}
          onKeyDown={handleKeyDown}
          ref={setRef}
          type="text"
          value={value}
          {...props}
        />
        {value ? <IconButton appearance="plain" className="rm-search-field-clear" label="Clear search" onClick={clear} size="sm" tabIndex={-1}><IconX size={ICON_SIZE.sm} /></IconButton> : null}
      </div>
    );
  },
);
SearchField.displayName = "SearchField";
