import { useEffect, useSyncExternalStore } from "react";
import { IconAlertCircle, IconAlertTriangle, IconCircleCheck, IconInfoCircle, IconX } from "@tabler/icons-react";

export type ToastKind = "error" | "success" | "warning" | "info";

interface ToastItem {
  id: number;
  kind: ToastKind;
  message: string;
}

const DISMISS_MS: Record<ToastKind, number> = { error: 7000, success: 4500, warning: 6000, info: 4500 };

let toasts: ToastItem[] = [];
let nextId = 1;
const listeners = new Set<() => void>();
const timers = new Map<number, number>();

function emit() {
  toasts = [...toasts];
  listeners.forEach((listener) => listener());
}

function startTimer(id: number, delay: number) {
  window.clearTimeout(timers.get(id));
  timers.set(id, window.setTimeout(() => dismissToast(id), delay));
}

export function dismissToast(id: number) {
  window.clearTimeout(timers.get(id));
  timers.delete(id);
  toasts = toasts.filter((item) => item.id !== id);
  emit();
}

/** Shows a toast. An identical visible toast is refreshed instead of duplicated. */
export function showToast(kind: ToastKind, message: string) {
  const existing = toasts.find((item) => item.kind === kind && item.message === message);
  if (existing) {
    startTimer(existing.id, DISMISS_MS[kind]);
    return;
  }
  const id = nextId++;
  toasts = [...toasts, { id, kind, message }].slice(-4);
  emit();
  startTimer(id, DISMISS_MS[kind]);
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => { listeners.delete(listener); };
}

/** Declarative bridge: shows a toast whenever `message` becomes a non-empty value. */
export function Toast({ kind, message }: { kind: ToastKind; message: string | null | undefined }) {
  useEffect(() => {
    if (message) showToast(kind, message);
  }, [kind, message]);
  return null;
}

const ICONS = { error: IconAlertCircle, success: IconCircleCheck, warning: IconAlertTriangle, info: IconInfoCircle } as const;

/** Mount once near the app root. Renders the bottom-right toast stack. */
export function ToastViewport() {
  const items = useSyncExternalStore(subscribe, () => toasts);
  return (
    <div aria-label="Notifications" className="rm-toast-viewport" role="region">
      {items.map((item) => {
        const Icon = ICONS[item.kind];
        return (
          <div
            className={`rm-toast rm-toast-${item.kind}`}
            key={item.id}
            onMouseEnter={() => window.clearTimeout(timers.get(item.id))}
            onMouseLeave={() => startTimer(item.id, 2500)}
            role={item.kind === "error" ? "alert" : "status"}
          >
            <Icon className="rm-toast-icon" size={18} />
            <p>{item.message}</p>
            <button aria-label="Dismiss notification" className="rm-toast-close" onClick={() => dismissToast(item.id)} type="button"><IconX size={14} /></button>
          </div>
        );
      })}
    </div>
  );
}
