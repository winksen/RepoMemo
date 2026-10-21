import { useEffect, useState } from "react";
import { IconLoader2 as Loader, IconPackageExport as PackageExport } from "@tabler/icons-react";
import { save } from "@tauri-apps/plugin-dialog";
import { listWorkspaces } from "../lib/repomemoApi";
import { copyPreparedArchive, discardPreparedArchive, type PreparedArchive, prepareLocalArchive, rememberHandoff } from "../lib/handoff";
import type { Workspace } from "../types";
import { Button } from "./ui/button";
import { Dialog, DialogCancel } from "./ui/dialog";
import { Dropdown } from "./ui/dropdown";
import { showToast } from "./ui/toast";
import { ICON_SIZE } from "./ui/icon";
import { apiMessage, formatBytes } from "../web/format";

/** What an archive holds, as a sentence. */
export function describeCounts(archive: PreparedArchive): string {
  const { counts } = archive;
  const part = (count: number, one: string, many: string) => `${count.toLocaleString()} ${count === 1 ? one : many}`;
  return [
    part(counts.artifacts, "file", "files"),
    part(counts.folders, "folder", "folders"),
    part(counts.memory_cards, "memory card", "memory cards"),
    part(counts.citations, "citation", "citations"),
    part(counts.tasks, "task", "tasks"),
  ].join(", ");
}

/**
 * *Move a workspace to a server* in the local workspace of the desktop app (V0.2.50). The workspace is written as an
 * archive, the file a server's export makes, and the person goes on to the server to import it: the server checks the
 * file and shows what is in it before it writes anything.
 */
export function LocalMoveToServer({ onGoToServer }: { onGoToServer: () => void }) {
  const [open, setOpen] = useState(false);
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [workspaceId, setWorkspaceId] = useState("");
  const [archive, setArchive] = useState<PreparedArchive | null>(null);
  const [isBusy, setIsBusy] = useState(false);

  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    listWorkspaces()
      .then((list) => { if (!cancelled) { setWorkspaces(list); setWorkspaceId((current) => current || list[0]?.id || ""); } })
      .catch((error: unknown) => showToast("error", error instanceof Error ? error.message : String(error)));
    return () => { cancelled = true; };
  }, [open]);

  function close() {
    setOpen(false);
    setArchive(null);
  }

  async function prepare() {
    setIsBusy(true);
    try {
      setArchive(await prepareLocalArchive(workspaceId));
    } catch (error) {
      showToast("error", typeof error === "string" ? error : apiMessage(error, "The workspace could not be prepared."));
    } finally { setIsBusy(false); }
  }

  async function saveCopy() {
    if (!archive) return;
    try {
      const destination = await save({ defaultPath: archive.file_name, filters: [{ name: "RepoMemo workspace archive", extensions: ["gz"] }] });
      if (!destination) return;
      await copyPreparedArchive(archive, destination);
      showToast("success", "Saved a copy of the archive.");
    } catch (error) {
      showToast("error", typeof error === "string" ? error : apiMessage(error, "The copy could not be saved."));
    }
  }

  function goToServer() {
    if (!archive) return;
    rememberHandoff(archive);
    close();
    onGoToServer();
  }

  async function discard() {
    if (archive) await discardPreparedArchive(archive).catch(() => undefined);
    rememberHandoff(null);
    close();
  }

  return <>
    <Button className="rm-desktop-move" onClick={() => setOpen(true)} type="button" variant="secondary"><PackageExport size={ICON_SIZE.sm} /> Move a workspace to a server</Button>
    <Dialog
      size="lg"
      description={archive
        ? `${archive.workspace_name} is ready: ${describeCounts(archive)} (${formatBytes(archive.size_bytes)}).`
        : "The workspace is written as an archive, the file a server exports. On the server you check what is in it and import it as a new workspace; this computer's copy is not changed or removed."}
      footer={archive ? <>
        <Button onClick={() => void discard()} type="button" variant="secondary">Discard the archive</Button>
        <Button onClick={() => void saveCopy()} type="button" variant="secondary">Save a copy…</Button>
        <Button onClick={goToServer} type="button" variant="main">Go to the server to import it</Button>
      </> : <>
        <DialogCancel onClick={close} />
        <Button disabled={isBusy || !workspaceId} onClick={() => void prepare()} type="button" variant="main">{isBusy ? <Loader className="spin" size={ICON_SIZE.md} /> : null} Prepare the archive</Button>
      </>}
      onClose={close}
      open={open}
      title={archive ? "Ready to move" : "Move a workspace to a server"}
    >
      {archive ? <p className="shared-muted-copy">Nothing has left this computer yet. The next step connects to your server (the address you give is the only place the archive goes) and asks you to confirm the import. People in the archive are credited to you on the server, with their names kept in the text. AI provider settings and keys are not moved; the server indexes the files again.</p>
        : workspaces.length ? <label className="shared-dialog-field">Workspace<Dropdown aria-label="Workspace to move" onValueChange={setWorkspaceId} options={workspaces.map((workspace) => ({ label: workspace.name, value: workspace.id }))} value={workspaceId} /></label>
        : <p className="shared-muted-copy">There is no local workspace to move.</p>}
    </Dialog>
  </>;
}
