import { useRef, useState } from "react";
import { IconCheck as Check, IconCopy as Copy } from "@tabler/icons-react";
import { Button } from "./ui/button";
import { Dialog } from "./ui/dialog";
import { showToast } from "./ui/toast";

/**
 * Shows a link or token that the server returns only once: it is not stored
 * in readable form, so closing this dialog loses it. Copy puts it on the
 * clipboard; the field can also be selected by hand.
 */
export function SecretDialog({ description, onClose, title, value }: { description: string; onClose: () => void; title: string; value: string }) {
  const [copied, setCopied] = useState(false);
  const field = useRef<HTMLInputElement>(null);

  async function copy() {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      showToast("success", "Copied to the clipboard.");
    } catch {
      // Without clipboard access (plain http, or a refused permission), select it for a manual copy.
      field.current?.select();
      showToast("warning", "Copy is not available here. The text is selected: press Ctrl+C.");
    }
  }

  return <Dialog className="rm-dialog-wide" description={description} footer={<Button onClick={onClose} type="button" variant="main">Done</Button>} onClose={onClose} open title={title}>
    <div className="rm-secret-field">
      <input aria-label={title} className="rm-input" onFocus={(event) => event.currentTarget.select()} readOnly ref={field} spellCheck={false} value={value} />
      <Button onClick={() => void copy()} type="button" variant="secondary">{copied ? <Check size={16} /> : <Copy size={16} />} {copied ? "Copied" : "Copy"}</Button>
    </div>
    <p className="rm-secret-warning">This is shown only once. Keep it somewhere safe before closing; it cannot be displayed again.</p>
  </Dialog>;
}

/** The full address of a page of this web app, such as an invitation link. */
export function appLink(path: string): string {
  return `${window.location.origin}${path}`;
}
