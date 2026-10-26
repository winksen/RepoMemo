import { useState } from "react";
import type { FormEvent } from "react";
import { IconArrowRight as ArrowRight, IconKey as Key, IconShieldLock as Shield } from "@tabler/icons-react";
import { getSharedHealth } from "../lib/sharedApi";
import { pointAtServer } from "../lib/desktopConnection";
import { checkServerVersion, versionAdvice } from "../lib/versionCheck";
import { desktopServer } from "../lib/desktop";
import { normalizeServerAddress } from "../lib/serverAddress";
import { AuthFrame } from "./AuthFrame";
import { Button } from "./ui/button";
import { Field } from "./ui/field";
import { Input } from "./ui/input";
import { showToast } from "./ui/toast";
import { ICON_SIZE } from "./ui/icon";
import { apiMessage } from "../web/format";

function reason(error: unknown): string {
  const text = typeof error === "string" ? error : apiMessage(error, "");
  return text.trim();
}

/**
 * The first screen of the desktop app: it asks for the address of a RepoMemo server (V0.2.49). The shell
 * checks the address and lets this window call that server and no other; the page then asks the server for
 * its health, and only a server that answers is kept for the next start.
 */
export function DesktopConnect({ onConnected }: { onConnected: () => void }) {
  const [address, setAddress] = useState("");
  const [isConnecting, setIsConnecting] = useState(false);

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
      const verdict = checkServerVersion(health.version);
      if (!verdict.ok) throw new Error(versionAdvice(verdict, true).body);
      await desktopServer.remember();
      onConnected();
    } catch (error) {
      showToast("error", reason(error) || "The server could not be reached.");
    } finally {
      setIsConnecting(false);
    }
  }

  return (
    <AuthFrame
      copy="The desktop app opens your RepoMemo server and shows the same pages as the browser. Your workspaces, memory and permissions stay on the server."
      eyebrow="Desktop app"
      formLabel="Choose a server"
      introLabel="RepoMemo desktop"
      title="Your team's evidence, on your desktop."
      trust={[
        { icon: <Shield size={ICON_SIZE.md} />, title: "One server", text: "The app calls only the server you choose here, and cannot be sent to another site." },
        { icon: <Key size={ICON_SIZE.md} />, title: "Your session", text: "Kept in this computer's credential store (Windows Credential Manager, the macOS Keychain or the system keyring), not in a file." },
      ]}
    >
      <div className="shared-auth-heading">
        <p className="shared-eyebrow">Connect</p>
        <h2>Which server?</h2>
        <p>Enter the address your administrator gave you, such as <code>repomemo.example.com</code>. Use <code>https://</code>; plain <code>http://</code> is only accepted for a server on your own network.</p>
      </div>
      <form className="shared-auth-form" onSubmit={connect}>
        <Field label="Server address" required><Input autoCapitalize="none" autoComplete="url" autoCorrect="off" autoFocus inputMode="url" onChange={(event) => setAddress(event.target.value)} placeholder="https://repomemo.example.com" spellCheck={false} type="text" value={address} /></Field>
        <Button busy={isConnecting} type="submit" variant="main">
          <ArrowRight size={ICON_SIZE.md} />
          Connect
        </Button>
      </form>
    </AuthFrame>
  );
}
