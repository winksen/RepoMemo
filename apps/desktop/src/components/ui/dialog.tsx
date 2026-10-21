import * as DialogPrimitive from "@radix-ui/react-dialog";
import { useRef, type FormEvent, type ReactNode } from "react";
import { Button } from "./button";

/** `sm` 400 px (a question), `md` 520 px (a small form), `lg` 720 px (long content). */
export type DialogSize = "sm" | "md" | "lg";

const FIRST_FIELD = "input:not([type=hidden]):not([disabled]):not([readonly]), textarea:not([disabled]):not([readonly]), select:not([disabled]), [role=combobox]:not([disabled])";

/**
 * A centered modal with a title, optional description and a footer of actions.
 * It focuses its first field when it opens (Radix focuses the first control otherwise),
 * Escape closes it, and Cancel comes before the decisive action in the footer.
 * Give it `onSubmit` for a dialog with fields: Enter in a field runs it, and the
 * decisive button in the footer says `type="submit"`.
 */
export function Dialog({
  children,
  className,
  description,
  footer,
  onClose,
  onSubmit,
  open,
  size,
  title,
}: {
  children?: ReactNode;
  className?: string;
  description?: ReactNode;
  footer: ReactNode;
  onClose: () => void;
  onSubmit?: () => void;
  open: boolean;
  /** `md` when the dialog has a body, `sm` when it only asks a question. */
  size?: DialogSize;
  title: string;
}) {
  const content = useRef<HTMLDivElement>(null);
  const width = `rm-dialog-${size ?? (children ? "md" : "sm")}`;
  const body = <>
    {children ? <div className="rm-dialog-body">{children}</div> : null}
    <div className="rm-dialog-footer">{footer}</div>
  </>;
  function submit(event: FormEvent) {
    event.preventDefault();
    onSubmit?.();
  }
  return (
    <DialogPrimitive.Root onOpenChange={(next) => { if (!next) onClose(); }} open={open}>
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay className="rm-dialog-overlay" />
        <DialogPrimitive.Content
          className={className ? `rm-dialog ${width} ${className}` : `rm-dialog ${width}`}
          onOpenAutoFocus={(event) => {
            const field = content.current?.querySelector<HTMLElement>(FIRST_FIELD);
            if (!field) return;
            event.preventDefault();
            field.focus();
          }}
          ref={content}
        >
          <DialogPrimitive.Title className="rm-dialog-title">{title}</DialogPrimitive.Title>
          {description ? <DialogPrimitive.Description className="rm-dialog-description">{description}</DialogPrimitive.Description> : null}
          {onSubmit ? <form className="rm-dialog-form" onSubmit={submit}>{body}</form> : body}
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
}

export function DialogCancel({ label = "Cancel", onClick }: { label?: string; onClick: () => void }) {
  return <Button onClick={onClick} type="button" variant="secondary">{label}</Button>;
}
