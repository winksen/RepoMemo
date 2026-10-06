import * as DialogPrimitive from "@radix-ui/react-dialog";
import type { ReactNode } from "react";
import { Button } from "./button";

/** A centered modal with a title, optional description and a footer of actions. */
export function Dialog({
  children,
  className,
  description,
  footer,
  onClose,
  open,
  title,
}: {
  children?: ReactNode;
  className?: string;
  description?: ReactNode;
  footer: ReactNode;
  onClose: () => void;
  open: boolean;
  title: string;
}) {
  return (
    <DialogPrimitive.Root onOpenChange={(next) => { if (!next) onClose(); }} open={open}>
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay className="rm-dialog-overlay" />
        <DialogPrimitive.Content className={className ? `rm-dialog ${className}` : "rm-dialog"}>
          <DialogPrimitive.Title className="rm-dialog-title">{title}</DialogPrimitive.Title>
          {description ? <DialogPrimitive.Description className="rm-dialog-description">{description}</DialogPrimitive.Description> : null}
          {children ? <div className="rm-dialog-body">{children}</div> : null}
          <div className="rm-dialog-footer">{footer}</div>
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
}

export function DialogCancel({ label = "Cancel", onClick }: { label?: string; onClick: () => void }) {
  return <Button onClick={onClick} type="button" variant="secondary">{label}</Button>;
}
