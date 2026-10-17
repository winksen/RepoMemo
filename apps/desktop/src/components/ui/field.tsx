import { cloneElement, isValidElement, useId, type ReactElement, type ReactNode } from "react";
import { cn } from "../../lib/utils";

/** What `Field` hands a control so the label, the hint and the message are tied to it. */
export interface FieldControlProps {
  "aria-describedby"?: string;
  "aria-invalid"?: true;
  id: string;
  required?: boolean;
}

export interface FieldProps {
  children: ReactElement | ((control: FieldControlProps) => ReactNode);
  className?: string;
  /** A muted line under the control that says what to enter. */
  hint?: ReactNode;
  /** The id of the control; one is generated when none is given. */
  htmlFor?: string;
  /** Marks the control `aria-invalid` and shows `message` under it. */
  invalid?: boolean;
  label: ReactNode;
  /** The one-line field message shown while the field is invalid. */
  message?: ReactNode;
  required?: boolean;
}

/**
 * A label, one control, an optional muted hint and, while `invalid`, a one-line message.
 * The control is a single element (cloned with the id and ARIA attributes) or a function that
 * receives them. A mistake in a field is a field state, never a banner; server errors stay toasts.
 */
export function Field({ children, className, hint, htmlFor, invalid = false, label, message, required }: FieldProps) {
  const generated = useId();
  const own = isValidElement<{ id?: string }>(children) ? children.props.id : undefined;
  const id = htmlFor ?? own ?? generated;
  const hintId = hint ? `${id}-hint` : undefined;
  const messageId = invalid && message ? `${id}-message` : undefined;
  const describedBy = [hintId, messageId].filter(Boolean).join(" ") || undefined;
  const control: FieldControlProps = {
    "aria-describedby": describedBy,
    "aria-invalid": invalid ? true : undefined,
    id,
    required: required || undefined,
  };
  return (
    <div className={cn("rm-field", className)} data-invalid={invalid ? "" : undefined}>
      <label className="rm-field-label" htmlFor={id}>{label}</label>
      {typeof children === "function" ? children(control) : cloneElement(children, control)}
      {hint ? <small className="rm-field-hint" id={hintId}>{hint}</small> : null}
      {messageId ? <small className="rm-field-message" id={messageId}>{message}</small> : null}
    </div>
  );
}

/** Puts fields side by side; they stack once a row would be narrower than about 220 px. */
export function FieldRow({ children, className }: { children: ReactNode; className?: string }) {
  return <div className={cn("rm-field-row", className)}>{children}</div>;
}
