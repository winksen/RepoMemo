// One provider sign-in or link attempt: the provider page opens in the browser (the system browser on desktop), this
// window polls for the single-use result and always offers a way to reopen the page, cancel, or start again.
import { useEffect, useRef, useState } from "react";
import { IconExternalLink as ExternalLink, IconLogin2 as Login } from "@tabler/icons-react";
import { openExternal, isDesktopRuntime } from "../lib/desktop";
import { pollProviderFlow, SharedApiError, type ProviderFlow, type TokenResponse } from "../lib/sharedApi";
import { apiMessage } from "../web/format";
import { Button } from "./ui/button";
import { Field } from "./ui/field";
import { Input } from "./ui/input";
import { showToast } from "./ui/toast";
import { ICON_SIZE } from "./ui/icon";
import { Toast } from "./ui/toast";

const POLL_MS = 2000;

export function ProviderSignIn({
  label,
  start,
  onDone,
  keepSession = true,
}: {
  label: string;
  start: () => Promise<ProviderFlow>;
  onDone: (session: TokenResponse) => void;
  /** False when the result only proves a link and must not replace the current session. */
  keepSession?: boolean;
}) {
  const [flow, setFlow] = useState<ProviderFlow | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [code, setCode] = useState("");
  const [confirming, setConfirming] = useState(false);
  const alive = useRef(true);
  const done = useRef(onDone);
  done.current = onDone;
  useEffect(() => () => { alive.current = false; }, []);

  useEffect(() => {
    if (!flow) return;
    const deadline = Date.now() + flow.expires_in * 1000 + POLL_MS;
    let stopped = false;
    const timer = window.setInterval(async () => {
      if (stopped) return;
      if (Date.now() > deadline) {
        stopped = true;
        window.clearInterval(timer);
        setFlow(null);
        setError("The provider page expired. Start again, or use local sign-in.");
        return;
      }
      try {
        const session = await pollProviderFlow(flow, keepSession);
        if (!session || stopped) return;
        stopped = true;
        window.clearInterval(timer);
        if (alive.current) { setFlow(null); done.current(session); }
      } catch (problem) {
        if (stopped) return;
        stopped = true;
        window.clearInterval(timer);
        if (alive.current) { setFlow(null); setError(apiMessage(problem, "Provider sign-in failed. Try again or use local sign-in.")); }
      }
    }, POLL_MS);
    return () => { stopped = true; window.clearInterval(timer); };
  }, [flow, keepSession]);

  // The code is shown only on the provider's return page, so a sign-in someone else started cannot be finished here.
  async function confirm() {
    if (!flow) return;
    setConfirming(true);
    try {
      const session = await pollProviderFlow(flow, keepSession, code);
      if (session && alive.current) { setFlow(null); setCode(""); done.current(session); }
    } catch (problem) {
      if (problem instanceof SharedApiError && problem.status === 409) showToast("error", problem.message);
      else if (alive.current) { setFlow(null); setCode(""); setError(apiMessage(problem, "Provider sign-in failed. Try again or use local sign-in.")); }
    } finally { if (alive.current) setConfirming(false); }
  }

  function open(url: string) {
    if (isDesktopRuntime) void openExternal(url).catch(() => undefined);
    else window.open(url, "_blank", "noopener");
  }

  async function begin() {
    setBusy(true);
    setError(null);
    try {
      const next = await start();
      setFlow(next);
      open(next.authorization_url);
    } catch (problem) {
      setError(apiMessage(problem, "Provider sign-in could not start."));
    } finally {
      setBusy(false);
    }
  }

  return <div className="shared-auth-provider">
    <Toast kind="error" message={error} />
    {flow ? <form onSubmit={(event) => { event.preventDefault(); void confirm(); }}>
      <p role="status">
        Finish signing in on the provider page{isDesktopRuntime ? " in your browser" : ""}. It then shows an eight-digit confirmation code: type it here.{" "}
        <Button onClick={() => open(flow.authorization_url)} type="button" variant="secondary"><ExternalLink size={ICON_SIZE.sm} /> Open it again</Button>
      </p>
      <Field label="Confirmation code" hint="Only type a code from a page you opened here yourself."><Input autoComplete="one-time-code" inputMode="numeric" maxLength={9} onChange={(event) => setCode(event.target.value)} value={code} /></Field>
      <Button busy={confirming} disabled={code.trim().length < 8} type="submit" variant="main">Finish signing in</Button>{" "}
      <Button onClick={() => { setFlow(null); setCode(""); setError("Provider sign-in was cancelled."); }} type="button" variant="secondary">Cancel</Button>
    </form> : <Button busy={busy} onClick={() => void begin()} type="button" variant="secondary"><Login size={ICON_SIZE.md} /> {label}</Button>}
  </div>;
}
