import * as PopoverPrimitive from "@radix-ui/react-popover";
import { IconCheck, IconChevronDown } from "@tabler/icons-react";
import { useEffect, useId, useMemo, useState, type KeyboardEvent } from "react";
import { cn } from "../../lib/utils";
import { ICON_SIZE } from "./icon";

export interface ComboboxOption {
  /** A muted second line, searched too (a path next to a title). */
  description?: string;
  label: string;
  value: string;
}

export interface ComboboxProps {
  "aria-label": string;
  className?: string;
  disabled?: boolean;
  /** Shown in the list when nothing matches the typed text. */
  emptyLabel?: string;
  id?: string;
  /** Adds a first option with an empty value that clears the choice, and names it. */
  noneLabel?: string;
  /**
   * For a list too long to hold: called (after a short pause) with the typed text, and with an empty
   * string when the list opens or closes, so the caller can fetch the matches. The options are then
   * shown as given, not filtered again here.
   */
  onSearch?: (query: string) => void;
  onValueChange: (value: string) => void;
  options: ComboboxOption[];
  /** Shown while nothing is chosen. */
  placeholder?: string;
  /** The chosen option's value, or an empty string for none. */
  value: string;
}

/** The most rows drawn at once; the rest appear as the typed text narrows the list. */
const VISIBLE_ROWS = 100;
/** How long typing pauses before `onSearch` asks. */
const SEARCH_DELAY_MS = 250;

/**
 * Chooses one item out of many by typing part of its name. Use a `Dropdown` for a handful of
 * options and a `Combobox` where scrolling the whole list would be slow: files, people, tasks.
 * Arrow keys move through the matches, Enter chooses, Escape closes.
 */
export function Combobox({ "aria-label": ariaLabel, className, disabled, emptyLabel = "Nothing matches", id, noneLabel, onSearch, onValueChange, options: items, placeholder = "Choose…", value }: ComboboxProps) {
  const listId = useId();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const options = useMemo(() => (noneLabel ? [{ label: noneLabel, value: "" }, ...items] : items), [items, noneLabel]);
  // The chosen option is kept, so its label shows although a search no longer lists it.
  const [remembered, setRemembered] = useState<ComboboxOption | undefined>();
  const selected = options.find((option) => option.value === value) ?? (remembered?.value === value && value ? remembered : undefined);
  useEffect(() => {
    if (!onSearch) return;
    const timer = window.setTimeout(() => onSearch(open ? query.trim() : ""), open && query.trim() ? SEARCH_DELAY_MS : 0);
    return () => window.clearTimeout(timer);
  }, [onSearch, open, query]);
  const matches = useMemo(() => {
    const words = query.toLowerCase().split(/\s+/).filter(Boolean);
    if (!words.length || onSearch) return options;
    return options.filter((option) => {
      const haystack = `${option.label} ${option.description ?? ""}`.toLowerCase();
      return words.every((word) => haystack.includes(word));
    });
  }, [onSearch, options, query]);
  const shown = matches.slice(0, VISIBLE_ROWS);

  function choose(option: ComboboxOption | undefined) {
    if (!option) return;
    setRemembered(option);
    onValueChange(option.value);
    setOpen(false);
  }
  function onKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      if (shown.length) setActive((current) => (current + (event.key === "ArrowDown" ? 1 : shown.length - 1)) % shown.length);
    } else if (event.key === "Home" || event.key === "End") {
      event.preventDefault();
      setActive(event.key === "Home" ? 0 : Math.max(shown.length - 1, 0));
    } else if (event.key === "Enter") {
      event.preventDefault();
      choose(shown[active]);
    }
  }

  return (
    <PopoverPrimitive.Root
      onOpenChange={(next) => { setOpen(next); if (next) { setQuery(""); setActive(0); } }}
      open={open}
    >
      <PopoverPrimitive.Trigger
        aria-label={ariaLabel}
        aria-haspopup="listbox"
        className={cn("rm-dropdown-trigger rm-combobox-trigger", className)}
        disabled={disabled}
        id={id}
        type="button"
      >
        <span className={cn("rm-combobox-value", !selected && "placeholder")}>{selected ? selected.label : placeholder}</span>
        <span className="rm-dropdown-icon"><IconChevronDown size={ICON_SIZE.md} /></span>
      </PopoverPrimitive.Trigger>
      <PopoverPrimitive.Portal>
        <PopoverPrimitive.Content align="start" className="rm-combobox-content" sideOffset={6}>
          <input
            aria-activedescendant={shown[active] ? `${listId}-${active}` : undefined}
            aria-controls={listId}
            aria-expanded="true"
            aria-label={`Search: ${ariaLabel}`}
            autoComplete="off"
            className="rm-combobox-search"
            onChange={(event) => { setQuery(event.target.value); setActive(0); }}
            onKeyDown={onKeyDown}
            placeholder="Type to search"
            role="combobox"
            value={query}
          />
          <div aria-label={ariaLabel} className="rm-combobox-list" id={listId} role="listbox">
            {shown.map((option, index) => (
              // a11y-allow: click options are chosen from the search field (aria-activedescendant, Enter); the click is for the mouse
              <div
                aria-selected={option.value === value}
                className="rm-combobox-option"
                data-active={index === active ? "" : undefined}
                id={`${listId}-${index}`}
                key={option.value}
                onClick={() => choose(option)}
                onMouseMove={() => setActive(index)}
                role="option"
              >
                <span className="rm-combobox-option-text">
                  <span>{option.label}</span>
                  {option.description ? <small>{option.description}</small> : null}
                </span>
                {option.value === value ? <IconCheck className="rm-dropdown-check" size={ICON_SIZE.sm} /> : null}
              </div>
            ))}
            {!shown.length ? <p className="rm-combobox-empty">{emptyLabel}</p> : null}
            {matches.length > shown.length ? <p className="rm-combobox-empty">{matches.length - shown.length} more: keep typing to narrow the list</p> : null}
          </div>
        </PopoverPrimitive.Content>
      </PopoverPrimitive.Portal>
    </PopoverPrimitive.Root>
  );
}
