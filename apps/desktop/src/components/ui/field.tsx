import { cloneElement, isValidElement, useId, useState, type FormEvent, type ReactElement, type ReactNode } from "react";
import { fieldMessage, type CheckedControl } from "../../lib/fieldMessage";
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
 *
 * A required or malformed control (`required`, `type="email"`, `minLength`, a range…) also marks
 * itself: when the form is submitted the browser refuses it, and the field shows its own message
 * instead of the browser's bubble, takes focus if it is the first, and clears as soon as it is valid.
 * `invalid` and `message` still win, for a rule only the page knows.
 */
export function Field({ children, className, hint, htmlFor, invalid: forcedInvalid = false, label, message: forcedMessage, required }: FieldProps) {
  const generated = useId();
  const [checked, setChecked] = useState<string | null>(null);
  const own = isValidElement<{ id?: string }>(children) ? children.props.id : undefined;
  const id = htmlFor ?? own ?? generated;
  const invalid = forcedInvalid || checked !== null;
  const message = forcedInvalid ? forcedMessage : checked;
  const hintId = hint ? `${id}-hint` : undefined;
  const messageId = invalid && message ? `${id}-message` : undefined;
  const describedBy = [hintId, messageId].filter(Boolean).join(" ") || undefined;
  function refused(event: FormEvent<HTMLElement>) {
    const target = event.target as CheckedControl;
    // Replaces the browser's bubble, which would also move focus to the first refused control.
    event.preventDefault();
    setChecked(fieldMessage(target, label));
    if (target.form?.querySelector(":is(input, select, textarea):invalid") === target) target.focus();
  }
  function clearWhenValid(event: FormEvent<HTMLElement>) {
    if (checked !== null && (event.target as CheckedControl).validity?.valid !== false) setChecked(null);
  }
  const control: FieldControlProps = {
    "aria-describedby": describedBy,
    "aria-invalid": invalid ? true : undefined,
    id,
    required: required || undefined,
  };
  return (
    <div
      className={cn("rm-field", className)}
      data-invalid={invalid ? "" : undefined}
      onChangeCapture={clearWhenValid}
      onInputCapture={clearWhenValid}
      onInvalidCapture={refused}
    >
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
