import type { ReactNode } from "react";

/** A form control that the browser can check: the fields of a `Field`. */
export type CheckedControl = HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement;

/**
 * The one-line message for a control the browser refused to submit, worded for the field instead of
 * the browser's own bubble: what is missing, or what to change. `label` names the field when it is text.
 */
export function fieldMessage(control: CheckedControl, label: ReactNode): string {
  const { validity } = control;
  const name = typeof label === "string" ? label : "This field";
  const input = control as HTMLInputElement;
  if (validity.valueMissing) return `${name} is required.`;
  if (validity.typeMismatch) {
    if (input.type === "email") return "Enter an email address such as name@example.com.";
    if (input.type === "url") return "Enter a web address that starts with http:// or https://.";
    return "Enter a valid value.";
  }
  if (validity.badInput) return "Enter a number.";
  if (validity.rangeUnderflow || validity.rangeOverflow) {
    const { max, min } = input;
    if (min !== "" && max !== "") return `Enter a value from ${min} to ${max}.`;
    return validity.rangeUnderflow ? `Enter a value of ${min} or more.` : `Enter a value of ${max} or less.`;
  }
  if (validity.stepMismatch) return "Enter a whole number.";
  if (validity.tooShort) return `Use at least ${input.minLength} characters.`;
  if (validity.tooLong) return `Use at most ${input.maxLength} characters.`;
  if (validity.patternMismatch) return input.title || "Use the format asked for.";
  return control.validationMessage || "Check this field.";
}
