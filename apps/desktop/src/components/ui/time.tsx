import { formatRelativeTime, formatTime } from "../../web/format";
import { Tooltip } from "./tooltip";

/**
 * A moment as a relative time ("3h ago"), with the exact date and time on hover and focus. The element
 * is a `<time>` carrying the ISO value, so assistive technology reads the exact moment.
 */
export function Time({ value }: { value: string }) {
  return <Tooltip content={formatTime(value)}><time dateTime={value}>{formatRelativeTime(value)}</time></Tooltip>;
}
