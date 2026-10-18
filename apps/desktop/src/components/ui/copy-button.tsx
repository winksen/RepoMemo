import { IconCheck, IconCopy } from "@tabler/icons-react";
import { useEffect, useRef, useState } from "react";
import { Button } from "./button";
import { ICON_SIZE } from "./icon";
import { IconButton } from "./icon-button";
import { showToast } from "./toast";

export interface CopyButtonProps {
  className?: string;
  /** The accessible name and tooltip, for example "Copy commit hash". */
  label: string;
  /** Runs when the clipboard is not available, for example to select the text for a manual copy. */
  onUnavailable?: () => void;
  /** Visible text for a button with a word ("Copy"); leave it out for an icon-only button. */
  text?: string;
  /** What to put on the clipboard. */
  value: string;
}

/** How long "Copied" stays, in milliseconds. */
const COPIED_FOR = 2000;

/**
 * Copies a value. The icon becomes a check (and the word "Copied") for two seconds; when the
 * browser refuses, a toast says so. Every copy action of the app uses this one button.
 */
export function CopyButton({ className, label, onUnavailable, text, value }: CopyButtonProps) {
  const [copied, setCopied] = useState(false);
  const timer = useRef<number | undefined>(undefined);
  useEffect(() => () => window.clearTimeout(timer.current), []);

  async function copy() {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      window.clearTimeout(timer.current);
      timer.current = window.setTimeout(() => setCopied(false), COPIED_FOR);
    } catch {
      // Without clipboard access (plain http, or a refused permission) the text is left to copy by hand.
      onUnavailable?.();
      showToast("warning", "Copy is not available here. Select the text and press Ctrl+C.");
    }
  }

  const icon = copied ? <IconCheck size={ICON_SIZE.md} /> : <IconCopy size={ICON_SIZE.md} />;
  return (
    <>
      {text !== undefined
        ? <Button className={className} onClick={() => void copy()} type="button" variant="secondary">{icon} {copied ? "Copied" : text}</Button>
        : <IconButton className={className} label={copied ? "Copied" : label} onClick={() => void copy()} size="sm">{icon}</IconButton>}
      <span aria-live="polite" className="sr-only" role="status">{copied ? "Copied" : ""}</span>
    </>
  );
}
