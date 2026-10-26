const KEY = "repomemo.desktop.updates";

/** Whether the person turned on looking for updates at the project's releases (V0.4.56). Off until they do. */
export function readUpdateChoice(): boolean {
  try {
    return window.localStorage.getItem(KEY) === "on";
  } catch {
    return false;
  }
}

export function writeUpdateChoice(on: boolean) {
  try {
    window.localStorage.setItem(KEY, on ? "on" : "off");
  } catch { /* only a convenience */ }
}
