/**
 * The one place dates and numbers are formatted for a person (V0.6.27). They follow the browser's language, unless a locale
 * is set explicitly (a test, or a future language setting). Nothing here translates text: see `messages.ts`.
 */

let override: string | null = null;

/** Sets the locale the formatters use; `null` goes back to the browser's. */
export function setLocale(locale: string | null) {
  override = locale;
}

/** The locale in use: the one set, else the browser's, else `undefined`, which lets `Intl` pick its default. */
export function currentLocale(): string | undefined {
  if (override) return override;
  if (typeof navigator !== "undefined" && navigator.language) return navigator.language;
  return undefined;
}

function asDate(value: string | number | Date): Date | null {
  const date = value instanceof Date ? value : new Date(value);
  return Number.isNaN(date.getTime()) ? null : date;
}

/** A date in the person's locale ("3 Mar 2026" in English). A value that is not a date comes back as it was. */
export function formatDateValue(value: string | number | Date, options: Intl.DateTimeFormatOptions = { dateStyle: "medium" }): string {
  const date = asDate(value);
  return date ? new Intl.DateTimeFormat(currentLocale(), options).format(date) : String(value);
}

/** A date with its time in the person's locale ("3 Mar 2026, 14:05"). */
export function formatDateTimeValue(value: string | number | Date): string {
  return formatDateValue(value, { dateStyle: "medium", timeStyle: "short" });
}

/** A short day and month for lists ("3 Mar"), in the person's locale. */
export function formatDayMonth(value: string | number | Date): string {
  return formatDateValue(value, { day: "numeric", month: "short" });
}

/** A number in the person's locale: separators and digits as they are written there. */
export function formatNumber(value: number, options?: Intl.NumberFormatOptions): string {
  return new Intl.NumberFormat(currentLocale(), options).format(value);
}
