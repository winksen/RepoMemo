import { useRef } from "react";
import { Button } from "./ui/button";
import { CopyButton } from "./ui/copy-button";
import { Dialog } from "./ui/dialog";

/**
 * Shows a link or token that the server returns only once: it is not stored
 * in readable form, so closing this dialog loses it. Copy puts it on the
 * clipboard (selecting the field when the browser refuses); it can also be selected by hand.
 */
export function SecretDialog({ description, onClose, title, value }: { description: string; onClose: () => void; title: string; value: string }) {
  const field = useRef<HTMLInputElement>(null);

  return <Dialog size="lg" description={description} footer={<Button onClick={onClose} type="button" variant="main">Done</Button>} onClose={onClose} open title={title}>
    <div className="rm-secret-field">
      <input aria-label={title} className="rm-input" onFocus={(event) => event.currentTarget.select()} readOnly ref={field} spellCheck={false} value={value} />
      <CopyButton label={`Copy ${title}`} onUnavailable={() => field.current?.select()} text="Copy" value={value} />
    </div>
    <p className="rm-secret-warning">This is shown only once. Keep it somewhere safe before closing; it cannot be displayed again.</p>
  </Dialog>;
}

/** The full address of a page of this web app, such as an invitation link. */
export function appLink(path: string): string {
  return `${window.location.origin}${path}`;
}
