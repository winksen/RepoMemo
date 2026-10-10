import { useState } from "react";
import type { FormEvent } from "react";
import {
  IconAlertCircle as AlertCircle,
  IconArrowRight as ArrowRight,
  IconCircleCheck as CircleCheck,
  IconFolder as Folder,
  IconFolderPlus as FolderPlus,
  IconKey as Key,
  IconLoader2 as Loader,
  IconLock as Lock,
  IconRefresh as Refresh,
  IconShieldLock as ShieldLock,
} from "@tabler/icons-react";
import { attachEnvironment, createEnvironment, listEnvironments, SharedApiError } from "../lib/sharedApi";
import type { EnvironmentEntry, PlatformStatus } from "../lib/sharedApi";
import { ThemeToggle } from "./SharedLayout";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { showToast } from "./ui/toast";

function errorMessage(error: unknown, fallback: string): string {
  return error instanceof Error && error.message ? error.message : fallback;
}

const STATUS_LABELS = { valid: "RepoMemo environment", empty: "Empty", invalid: "Cannot be attached" } as const;

/**
 * Shown while the server has no environment attached. The code from the server console unlocks the menu;
 * environments can only be created in, or chosen from, the server's environments folder, and the server verifies a
 * folder's structure before it attaches to it.
 */
export function PlatformSetup({ folder, onAttached, pinned }: { folder: string; onAttached: (status: PlatformStatus) => void; pinned: string | null }) {
  const [code, setCode] = useState("");
  const [environments, setEnvironments] = useState<EnvironmentEntry[] | null>(null);
  const [name, setName] = useState("");
  const [isBusy, setIsBusy] = useState(false);

  async function load() {
    setIsBusy(true);
    try { setEnvironments(await listEnvironments(code)); }
    catch (error) {
      showToast("error", errorMessage(error, "The environments could not be listed."));
      if (error instanceof SharedApiError && error.status === 403) setEnvironments(null);
    }
    finally { setIsBusy(false); }
  }

  async function unlock(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    await load();
  }

  async function attach(entry: EnvironmentEntry) {
    setIsBusy(true);
    try {
      const status = await attachEnvironment(code, entry.name);
      showToast("success", `Attached to ${entry.name}.`);
      onAttached(status);
    } catch (error) { showToast("error", errorMessage(error, "The environment could not be attached.")); }
    finally { setIsBusy(false); }
  }

  async function create(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsBusy(true);
    try {
      const status = await createEnvironment(code, name.trim());
      showToast("success", `Created ${name.trim()} in ${folder}/.`);
      onAttached(status);
    } catch (error) { showToast("error", errorMessage(error, "The environment could not be created.")); }
    finally { setIsBusy(false); }
  }

  return <main className="rm-setup rm-platform">
    <header className="rm-setup-header">
      <div className="shared-brand"><img alt="RepoMemo" className="shared-brand-full" src="/RM-logofull.svg" /></div>
      <div className="rm-setup-header-actions"><ThemeToggle /></div>
    </header>
    <div className="rm-setup-body">
      <section className="rm-setup-panel" aria-live="polite">
        <div className="shared-auth-heading rm-setup-heading">
          <p className="shared-eyebrow">Platform</p>
          <h2>Choose the environment</h2>
          <p>All data of this server (database, files, logs) lives in one environment folder inside <code>{folder}/</code>. Nothing outside that folder can be used.</p>
        </div>
        {pinned
          ? <p className="rm-platform-note" role="note"><AlertCircle size={16} /><span>This server starts with <code>{pinned}</code>, set by <code>REPOMEMO_SERVER_DATA_DIR</code>. The environment you choose here is used until the server restarts, then it goes back to <code>{pinned}</code>. To keep a choice made here, start the server without that variable.</span></p>
          : <p className="rm-platform-note" role="note"><CircleCheck size={16} /><span>The environment you choose here is remembered: the server attaches it again when it restarts, until it is detached in System settings.</span></p>}
        {environments === null ? <>
          <ul className="rm-setup-points">
            <li><ShieldLock size={16} /> Enter the code printed on the server console when it started (or the value of <code>REPOMEMO_SETUP_CODE</code>).</li>
            <li><Lock size={16} /> Until an environment is attached, the server answers nothing else.</li>
          </ul>
          <form className="shared-setup-form" onSubmit={unlock}>
            <label>Code<Input autoComplete="one-time-code" autoFocus onChange={(event) => setCode(event.target.value)} placeholder="ABCD-EFGH-JKLM" required spellCheck={false} value={code} /></label>
            <div className="rm-setup-actions"><span /><Button disabled={isBusy || !code.trim()} type="submit" variant="main">{isBusy ? <Loader className="spin" size={16} /> : <Key size={16} />} Unlock</Button></div>
          </form>
        </> : <>
          <div className="rm-platform-heading"><h3>Existing environments</h3><Button disabled={isBusy} onClick={() => void load()} type="button" variant="secondary"><Refresh size={15} /> Refresh</Button></div>
          {environments.length === 0
            ? <p className="shared-muted-copy">There is no folder in <code>{folder}/</code> yet. Create the first environment below.</p>
            : <ul className="rm-platform-list">{environments.map((entry) => <li className={`rm-platform-item ${entry.status}`} key={entry.name}>
              <span className="rm-platform-icon" aria-hidden="true">{entry.status === "invalid" ? <AlertCircle size={20} /> : entry.status === "valid" ? <CircleCheck size={20} /> : <Folder size={20} />}</span>
              <span className="rm-platform-text"><strong>{entry.name} <small>{STATUS_LABELS[entry.status]}</small></strong><span>{entry.detail}</span></span>
              <Button disabled={isBusy || entry.status === "invalid"} onClick={() => void attach(entry)} type="button" variant="secondary">Use <ArrowRight size={15} /></Button>
            </li>)}</ul>}
          <form className="shared-setup-form rm-platform-create" onSubmit={create}>
            <h3>Create a new environment</h3>
            <label>Name<Input maxLength={48} onChange={(event) => setName(event.target.value)} pattern="[A-Za-z0-9][A-Za-z0-9._\-]*" placeholder="main" required spellCheck={false} value={name} /></label>
            <p className="shared-muted-copy">It is created as <code>{folder}/{name.trim() || "name"}</code>. Letters, digits, "-", "_" and "." only.</p>
            <div className="rm-setup-actions"><span /><Button disabled={isBusy || !name.trim()} type="submit" variant="main">{isBusy ? <Loader className="spin" size={16} /> : <FolderPlus size={16} />} Create and use</Button></div>
          </form>
        </>}
      </section>
    </div>
  </main>;
}
