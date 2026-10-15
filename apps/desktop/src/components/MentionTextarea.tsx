import { type KeyboardEvent, useEffect, useId, useRef, useState } from "react";
import { listSharedMentionSuggestions } from "../lib/sharedApi";
import type { SharedUser } from "../types";
import { Textarea, type TextareaProps } from "./ui/textarea";

const SUGGESTION_DELAY_MS = 150;

/** The `@word` the caret is at the end of, if any: where it starts and what follows the `@`. */
export function mentionAt(text: string, caret: number): { start: number; query: string } | null {
  const match = /(^|\s)@([^\s@]*)$/.exec(text.slice(0, caret));
  return match ? { start: caret - match[2].length - 1, query: match[2] } : null;
}

/**
 * A comment field that suggests workspace members while an `@` is being
 * typed. Choosing one writes their address after the `@`, which is what a
 * mention is: they are then notified when the comment is saved. Only members
 * of the workspace are suggested (the server answers for those who can
 * comment).
 */
export function MentionTextarea({ accessToken, onValueChange, value, workspaceId, ...rest }: Omit<TextareaProps, "onChange" | "value"> & {
  accessToken: string;
  onValueChange: (value: string) => void;
  value: string;
  workspaceId: string;
}) {
  const field = useRef<HTMLTextAreaElement>(null);
  const listId = useId();
  const [trigger, setTrigger] = useState<{ start: number; query: string } | null>(null);
  const [suggestions, setSuggestions] = useState<SharedUser[]>([]);
  const [active, setActive] = useState(0);
  const open = trigger !== null && suggestions.length > 0;

  useEffect(() => {
    if (!trigger) { setSuggestions([]); return; }
    let cancelled = false;
    const timer = window.setTimeout(() => {
      listSharedMentionSuggestions(accessToken, workspaceId, trigger.query)
        .then((found) => { if (!cancelled) { setSuggestions(found); setActive(0); } })
        .catch(() => { if (!cancelled) setSuggestions([]); });
    }, SUGGESTION_DELAY_MS);
    return () => { cancelled = true; window.clearTimeout(timer); };
  }, [accessToken, workspaceId, trigger?.start, trigger?.query]);

  function follow(element: HTMLTextAreaElement) {
    setTrigger(mentionAt(element.value, element.selectionStart ?? element.value.length));
  }

  function choose(user: SharedUser) {
    if (!trigger || !user.email) return;
    const element = field.current;
    const caret = element?.selectionStart ?? value.length;
    const inserted = `@${user.email} `;
    onValueChange(`${value.slice(0, trigger.start)}${inserted}${value.slice(caret)}`);
    setTrigger(null);
    setSuggestions([]);
    const position = trigger.start + inserted.length;
    window.requestAnimationFrame(() => {
      element?.focus();
      element?.setSelectionRange(position, position);
    });
  }

  function onKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    rest.onKeyDown?.(event);
    if (!open || event.defaultPrevented) return;
    if (event.key === "ArrowDown") { event.preventDefault(); setActive((current) => (current + 1) % suggestions.length); }
    else if (event.key === "ArrowUp") { event.preventDefault(); setActive((current) => (current - 1 + suggestions.length) % suggestions.length); }
    else if ((event.key === "Enter" || event.key === "Tab") && !event.nativeEvent.isComposing) { event.preventDefault(); choose(suggestions[active]); }
    else if (event.key === "Escape") { event.preventDefault(); setTrigger(null); }
  }

  return <div className="shared-mention-field">
    <Textarea
      {...rest}
      aria-activedescendant={open ? `${listId}-${active}` : undefined}
      aria-autocomplete="list"
      aria-controls={open ? listId : undefined}
      aria-expanded={open}
      aria-haspopup="listbox"
      onBlur={(event) => { rest.onBlur?.(event); window.setTimeout(() => setTrigger(null), 120); }}
      onChange={(event) => { onValueChange(event.target.value); follow(event.target); }}
      onClick={(event) => follow(event.currentTarget)}
      onKeyDown={onKeyDown}
      onKeyUp={(event) => { if (event.key.startsWith("Arrow") && !open) follow(event.currentTarget); }}
      ref={field}
      role="combobox"
      value={value}
    />
    {open ? <ul aria-label="People you can mention" className="shared-mention-list" id={listId} role="listbox">
      {suggestions.map((user, index) => <li key={user.id} role="presentation">
        <button aria-selected={index === active} className={index === active ? "active" : ""} id={`${listId}-${index}`} onMouseDown={(event) => { event.preventDefault(); choose(user); }} onMouseMove={() => setActive(index)} role="option" tabIndex={-1} type="button">
          <strong>{user.display_name}</strong><small>{user.email}</small>
        </button>
      </li>)}
    </ul> : null}
  </div>;
}
