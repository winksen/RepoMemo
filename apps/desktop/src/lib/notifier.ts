/**
 * How the client tells a person something while its window is in the background (V0.4.53). A browser has none: its tab shows the
 * notices when it is looked at. The desktop app installs one that shows a system notification and marks the app's icon.
 */
export interface Notifier {
  /** Shows a system notification; answers whether it was shown (a person who has not allowed them gets none). */
  notify: (notice: { title: string; body: string }) => Promise<boolean>;
  /** Marks the app's icon with how much waits; zero takes the mark away. */
  badge: (count: number) => Promise<void>;
}

let installed: Notifier | null = null;

export function setNotifier(next: Notifier | null) {
  installed = next;
}

export function currentNotifier(): Notifier | null {
  return installed;
}
