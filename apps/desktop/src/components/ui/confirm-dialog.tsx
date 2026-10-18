import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { Button } from "./button";
import { Dialog, DialogCancel } from "./dialog";

export interface ConfirmOptions {
  /** The decisive button, named after what it does ("Delete", "Hand over"); `Confirm` when omitted. */
  action?: string;
  /** Styles the decisive button as a destructive action. */
  danger?: boolean;
  description?: ReactNode;
  title: string;
}

/** A question in the app's own dialog: Cancel on the left, the decisive action on the right. */
export function ConfirmDialog({ action = "Confirm", danger = false, description, onCancel, onConfirm, open, title }: ConfirmOptions & { onCancel: () => void; onConfirm: () => void; open: boolean }) {
  return (
    <Dialog
      description={description}
      footer={<><DialogCancel onClick={onCancel} /><Button autoFocus onClick={onConfirm} type="button" variant={danger ? "danger" : "main"}>{action}</Button></>}
      onClose={onCancel}
      open={open}
      title={title}
    />
  );
}

type Confirm = (options: ConfirmOptions) => Promise<boolean>;
const ConfirmContext = createContext<Confirm | null>(null);

/** Hosts the one confirmation dialog of the app; mount it once, above every page. */
export function ConfirmProvider({ children }: { children: ReactNode }) {
  const [options, setOptions] = useState<ConfirmOptions | null>(null);
  const pending = useRef<((answer: boolean) => void) | null>(null);

  const settle = useCallback((answer: boolean) => {
    pending.current?.(answer);
    pending.current = null;
    setOptions(null);
  }, []);
  const confirm = useCallback<Confirm>((next) => {
    pending.current?.(false);
    return new Promise<boolean>((resolve) => { pending.current = resolve; setOptions(next); });
  }, []);
  useEffect(() => () => pending.current?.(false), []);

  return (
    <ConfirmContext.Provider value={confirm}>
      {children}
      {options ? <ConfirmDialog {...options} onCancel={() => settle(false)} onConfirm={() => settle(true)} open /> : null}
    </ConfirmContext.Provider>
  );
}

/**
 * `const confirm = useConfirm(); if (!(await confirm({ title, description, action, danger }))) return;`
 * Resolves `true` on the decisive action and `false` on Cancel, Escape or a click outside.
 */
export function useConfirm(): Confirm {
  const confirm = useContext(ConfirmContext);
  if (!confirm) throw new Error("useConfirm is only available inside ConfirmProvider.");
  return confirm;
}
