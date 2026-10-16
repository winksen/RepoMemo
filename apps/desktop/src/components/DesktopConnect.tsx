import { useState } from "react";
import type { FormEvent } from "react";
import { IconArrowRight as ArrowRight, IconKey as Key, IconLoader2 as Loader, IconServer as Server, IconShieldLock as Shield } from "@tabler/icons-react";
import { configureDesktopApi, getSharedHealth } from "../lib/sharedApi";
import { desktopServer } from "../lib/desktop";
import { normalizeServerAddress } from "../lib/serverAddress";
import { pendingHandoff } from "../lib/handoff";
import { ThemeToggle } from "./SharedLayout";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { showToast } from "./ui/toast";

/** Points this client at a server the shell has allowed, and checks that a RepoMemo server answers there. */
export async function pointAtServer(origin: string) {
  const { fetch: desktopFetch } = await import("@tauri-apps/plugin-http");
  // The shell allows the chosen server's address only; a redirect to anywhere else is not followed.
  configureDesktopApi(origin, ((input: RequestInfo | URL, init?: RequestInit) => desktopFetch(input, { ...init, maxRedirections: 0 })) as typeof fetch);
}

function reason(error: unknown): string {
  const text = typeof error === "string" ? error : error instanceof Error ? error.message : "";
  return text.trim();
}

/**
 * The first screen of the desktop app: it asks for the address of a RepoMemo server (V0.2.49). The shell
 * checks the address and lets this window call that server and no other; the page then asks the server for
 * its health, and only a server that answers is kept for the next start.
 */
export function DesktopConnect({ onConnected, onUseLocal }: { onConnected: () => void; onUseLocal: () => void }) {
  const [address, setAddress] = useState("");
  const [isConnecting, setIsConnecting] = useState(false);
  const waiting = pendingHandoff();

  async function connect(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const typed = normalizeServerAddress(address);
    if (!typed.ok) {
      showToast("error", typed.message);
      return;
    }
    setIsConnecting(true);
    try {
      // The shell has the last word on the address.
      const origin = await desktopServer.prepare(address).catch((error: unknown) => { throw new Error(reason(error) || "That address cannot be used."); });
      await pointAtServer(origin);
      let health: Awaited<ReturnType<typeof getSharedHealth>>;
      try {
        health = await getSharedHealth();
      } catch (error) {
        throw new Error(`Nothing answered at ${origin}${reason(error) ? ` (${reason(error)})` : ""}. Check the address, that the server is running and that this computer can reach it.`);
      }
      if (typeof health?.status !== "string" || typeof health?.authentication !== "string") {
        throw new Error(`${origin} answered, but it does not look like a RepoMemo server.`);
      }
      await desktopServer.remember();
      onConnected();
    } catch (error) {
      showToast("error", reason(error) || "The server could not be reached.");
    } finally {
      setIsConnecting(false);
    }
  }

  return (
    <main className="shared-auth-canvas">
      <ThemeToggle className="shared-auth-theme-toggle" />
      <section aria-label="RepoMemo desktop" className="shared-auth-intro">
        <div className="shared-brand"><img alt="RepoMemo" className="shared-brand-full" src="/RM-logofull.svg" /></div>
        <p className="shared-eyebrow">Desktop app</p>
        <h1>Your team's evidence, on your desktop.</h1>
        <p className="shared-intro-copy">The desktop app opens your RepoMemo server and shows the same pages as the browser. Your workspaces, memory and permissions stay on the server.</p>
        <dl className="shared-trust-list">
          <div><dt><Shield size={16} /> One server</dt><dd>The app calls only the server you choose here, and cannot be sent to another site.</dd></div>
          <div><dt><Key size={16} /> Your session</dt><dd>Kept in this computer's credential store (Windows Credential Manager, the macOS Keychain or the system keyring), not in a file.</dd></div>
        </dl>
      </section>

      <section aria-label="Choose a server" className="shared-auth-form-region">
        <div className="shared-auth-form-shell">
          <div className="shared-auth-heading">
            <p className="shared-eyebrow">Connect</p>
            <h2>Which server?</h2>
            <p>Enter the address your administrator gave you, such as <code>repomemo.example.com</code>. Use <code>https://</code>; plain <code>http://</code> is only accepted for a server on your own network.</p>
          </div>
          {waiting ? <p className="shared-auth-hint" role="status">Your workspace <strong>{waiting.workspace_name}</strong> is ready to move. Connect to the server you want it on, sign in, and the dashboard's <em>Import a workspace</em> checks it for you.</p> : null}
          <form className="shared-auth-form" onSubmit={connect}>
            <label>Server address<Input autoCapitalize="none" autoComplete="url" autoCorrect="off" autoFocus inputMode="url" onChange={(event) => setAddress(event.target.value)} placeholder="https://repomemo.example.com" required spellCheck={false} type="text" value={address} /></label>
            <Button disabled={isConnecting} type="submit" variant="main">
              {isConnecting ? <Loader className="spin" size={17} /> : <ArrowRight size={17} />}
              Connect
            </Button>
          </form>
          <p className="shared-auth-hint">
            Just looking around? <Button onClick={onUseLocal} type="button" variant="secondary"><Server size={15} /> Use the local workspace</Button> It stays on this computer only. It is retired: it gets security fixes only and will be removed in a 0.3.x release, so move your work to a server.
          </p>
        </div>
      </section>
    </main>
  );
}
