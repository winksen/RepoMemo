import { useEffect, useRef, useState } from "react";
import { IconDatabase as Database, IconKey as Key, IconLoader2 as Loader } from "@tabler/icons-react";
import { copySystemBackups, createSystemBackup, deleteSystemBackup, getSystemBackups, restoreSystemBackup } from "../../lib/sharedApi";
import type { BackupsOverview } from "../../types";
import { ActionMenu } from "../../components/ui/action-menu";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { Table } from "../../components/ui/table";
import { PanelHeading } from "../../components/ui/panel-heading";
import { Dialog, DialogCancel } from "../../components/ui/dialog";
import { Input } from "../../components/ui/input";
import { LoadingState } from "../../components/ui/loading-state";
import { showToast } from "../../components/ui/toast";
import { ICON_SIZE } from "../../components/ui/icon";
import { apiMessage, formatBytes, formatTime } from "../format";
import { count, Facts, useSystemData } from "./shared";
import { systemKeys } from "../../lib/queryClient";

/** Backups of the environment, and restoring one into a new environment folder. */
export function BackupsPanel({ accessToken, canChange }: { accessToken: string; canChange: boolean }) {
  // While a backup or restore runs, the list is read again every two seconds.
  const [overview, , reload, setOverview] = useSystemData<BackupsOverview>(systemKeys.backups(), () => getSystemBackups(accessToken), { failure: "Backups could not be listed.", refreshMs: (current) => current?.status.running ? 2_000 : false });
  const [restoring, setRestoring] = useState<string | null>(null);
  const [deleting, setDeleting] = useState<string | null>(null);
  const [environment, setEnvironment] = useState("");
  const [isBusy, setIsBusy] = useState(false);
  const wasRunning = useRef(false);
  const running = overview?.status.running ?? false;

  useEffect(() => {
    // Say how a backup or restore ended, once, when it was running before.
    if (wasRunning.current && !running && overview?.status.message) showToast(overview.status.succeeded ? "success" : "error", overview.status.message);
    wasRunning.current = running;
  }, [running, overview]);

  async function backUp() {
    setIsBusy(true);
    try { setOverview(await createSystemBackup(accessToken)); }
    catch (error) { showToast("error", apiMessage(error, "The backup could not start.")); }
    finally { setIsBusy(false); }
  }
  async function copyNow() {
    setIsBusy(true);
    try { setOverview(await copySystemBackups(accessToken)); }
    catch (error) { showToast("error", apiMessage(error, "The copy could not start.")); }
    finally { setIsBusy(false); }
  }
  async function restore() {
    if (!restoring) return;
    setIsBusy(true);
    try { setOverview(await restoreSystemBackup(accessToken, restoring, environment.trim())); setRestoring(null); setEnvironment(""); }
    catch (error) { showToast("error", apiMessage(error, "The restore could not start.")); }
    finally { setIsBusy(false); }
  }
  async function remove() {
    if (!deleting) return;
    setIsBusy(true);
    try { await deleteSystemBackup(accessToken, deleting); setDeleting(null); showToast("success", "The backup was deleted."); await reload(); }
    catch (error) { showToast("error", apiMessage(error, "The backup could not be deleted.")); }
    finally { setIsBusy(false); }
  }

  const status = overview?.status;
  return <section className="shared-settings-group">
    <PanelHeading description={running ? `${status?.operation ?? "backup"} running…` : overview ? `${overview.backups.length} kept` : "loading"} icon={Database} title="Backups" />
    <p className="shared-muted-copy">A backup holds a consistent copy of the database, the stored files and the key file, made while the server runs. It sits in the data folder, so copy it to another machine for it to survive a lost disk, or set REPOMEMO_BACKUP_COPY_DIR and the server copies each backup there and checks the copy. Unless it is encrypted, protect it like the data; with REPOMEMO_BACKUP_KEY set, every file in it is sealed and restoring needs that same key, which is kept nowhere in the backup. Restoring never touches the running environment: it creates a new one, which you attach from the environment menu.</p>
    <Facts items={[
      ["Folder", <code key="dir">{overview?.directory ?? "—"}</code>],
      ["Automatic backups", overview ? (overview.interval_hours ? `every ${overview.interval_hours} h, ${overview.keep} kept` : `off, ${overview.keep} kept (Settings › Automatic backups)`) : "—"],
      ["Second copy", overview ? (overview.copy_directory ? <><code key="copy">{overview.copy_directory}</code>, each backup checked after it is copied</> : "off: set REPOMEMO_BACKUP_COPY_DIR to a folder on another disk or a share") : "—"],
      ["Encryption", overview ? (overview.encryption_enabled ? "on: new backups are sealed with REPOMEMO_BACKUP_KEY" : "off: set REPOMEMO_BACKUP_KEY to seal new backups") : "—"],
      ["Free disk", overview?.disk_free_bytes != null ? formatBytes(overview.disk_free_bytes) : "unknown"],
      ["Last backup or restore", status?.finished_at ? `${formatTime(status.finished_at)} · ${status.message ?? ""}` : "none since the server started"],
    ]} />
    {canChange ? <div><Button disabled={isBusy || running} onClick={() => void backUp()} type="button" variant="main">{running ? <Loader className="spin" size={ICON_SIZE.md} /> : <Database size={ICON_SIZE.md} />} Back up now</Button>{overview?.copy_directory && overview.backups.some((backup) => !backup.copy) ? <Button disabled={isBusy || running} onClick={() => void copyNow()} type="button" variant="secondary">Copy to the second location now</Button> : null}</div> : <p className="shared-muted-copy">Only system administrators make, restore and delete backups.</p>}
    {overview?.backups.length ? <Table>
      <thead><tr><th scope="col">Backup</th><th scope="col">Made</th><th scope="col">Version</th><th scope="col">Files</th><th scope="col">Database</th><th scope="col">Key</th>{overview.copy_directory ? <th scope="col">Second copy</th> : null}{canChange ? <th scope="col"><span className="sr-only">Actions</span></th> : null}</tr></thead>
      <tbody>{overview.backups.map((backup) => <tr key={backup.name}>
        <th scope="row">{backup.name}<Badge>{backup.trigger}</Badge>{backup.encryption ? <Badge>encrypted</Badge> : null}</th>
        <td>{formatTime(backup.created_at)}</td><td>{backup.version}</td>
        <td>{count(backup.blob_count)} · {formatBytes(backup.blob_bytes)}{backup.missing_blobs ? ` · ${count(backup.missing_blobs)} missing` : ""}</td>
        <td>{formatBytes(backup.database_bytes)}</td><td>{backup.includes_key ? "included" : "not in it"}</td>
        {overview.copy_directory ? <td>{backup.copy ? `checked ${formatTime(backup.copy.copied_at)}` : "waiting"}</td> : null}
        {canChange ? <td><ActionMenu items={[
          { label: "Restore as a new environment…", onSelect: () => { setEnvironment(""); setRestoring(backup.name); } },
          { label: "Delete", onSelect: () => setDeleting(backup.name), destructive: true },
        ]} label={`Actions for ${backup.name}`} /></td> : null}
      </tr>)}</tbody>
    </Table> : overview ? <p className="shared-muted-copy">No backups yet.</p> : <LoadingState label="Loading backups" />}
    <Dialog description="The backup is copied into a new environment folder under workspace-data and checked: it must open. The running environment is not changed. Attach the new one from the environment menu when you want to use it." footer={<><DialogCancel onClick={() => setRestoring(null)} /><Button disabled={isBusy || !environment.trim()} onClick={() => void restore()} type="button" variant="main">{isBusy ? <Loader className="spin" size={ICON_SIZE.md} /> : null}Restore</Button></>} onClose={() => setRestoring(null)} open={Boolean(restoring)} title={`Restore ${restoring ?? "the backup"}`}>
      <label className="rm-system-inline">New environment name<Input aria-label="New environment name" onChange={(event) => setEnvironment(event.target.value)} placeholder="restored-2026-10" value={environment} /></label>
    </Dialog>
    <Dialog description="The backup is deleted from the backups folder. Other backups that share its stored files keep them." footer={<><DialogCancel onClick={() => setDeleting(null)} /><Button disabled={isBusy} onClick={() => void remove()} type="button" variant="main">{isBusy ? <Loader className="spin" size={ICON_SIZE.md} /> : null}Delete backup</Button></>} onClose={() => setDeleting(null)} open={Boolean(deleting)} title={`Delete ${deleting ?? "the backup"}?`} />
  </section>;
}
