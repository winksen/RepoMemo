import { SharedWebApp } from "./SharedWebApp";
import { DesktopApp } from "./components/DesktopApp";
import { isDesktopRuntime } from "./lib/desktop";

/**
 * One client for everything: the browser opens the web client, and the desktop window opens it against a
 * server it is told about (V0.2.49). The local workspace of the earlier desktop app is gone (V0.3.5).
 */
export function App() {
  return isDesktopRuntime ? <DesktopApp /> : <SharedWebApp />;
}
