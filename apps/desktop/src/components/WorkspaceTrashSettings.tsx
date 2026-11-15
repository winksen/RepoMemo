import { useCallback, useEffect, useState } from "react";
import { IconLoader2 as Loader, IconTrash as Trash } from "@tabler/icons-react";
import { listSharedTrash, purgeSharedTrashItem, restoreSharedTrashItem } from "../lib/sharedApi";
import type { TrashItem } from "../types";
import { Button } from "./ui/button";
import { PanelHeading } from "./ui/panel-heading";
import { useConfirm } from "./ui/confirm-dialog";
import { showToast } from "./ui/toast";
import { ICON_SIZE } from "./ui/icon";
import { apiMessage } from "../web/format";

const when = (value: string) => {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleDateString(undefined, { dateStyle: "medium" });
};

/**
 * What was deleted lately and can still be put back (V0.6.15): memory cards with their history, and files that were
 * uploaded or written here. Items stay 30 days. Anyone who can edit restores; only an administrator removes one for good.
 */
export function WorkspaceTrashSettings({ accessToken, canPurge, onRestored, workspaceId }: { accessToken: string; canPurge: boolean; onRestored?: () => void; workspaceId: string }) {
  const [items, setItems] = useState<TrashItem[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const confirm = useConfirm();

  const load = useCallback(async () => {
    try { setItems(await listSharedTrash(accessToken, workspaceId)); } catch (error) { setItems([]); showToast("error", apiMessage(error, "The trash could not be read.")); }
  }, [accessToken, workspaceId]);
  useEffect(() => { void load(); }, [load]);

  async function restore(item: TrashItem) {
    setBusy(item.id);
    try {
      const result = await restoreSharedTrashItem(accessToken, workspaceId, item.id);
      const notes = [result.skipped_citations ? `${result.skipped_citations} citation${result.skipped_citations === 1 ? "" : "s"} left out because the cited source is gone` : "", result.widened_citations ? `${result.widened_citations} passage citation${result.widened_citations === 1 ? "" : "s"} came back as the whole file` : ""].filter(Boolean);
      showToast("success", `Restored ${item.title}.${notes.length ? ` ${notes.join("; ")}.` : ""}`);
      await load();
      onRestored?.();
    } catch (error) { showToast("error", apiMessage(error, "That could not be restored.")); } finally { setBusy(null); }
  }

  async function purge(item: TrashItem) {
    const agreed = await confirm({ action: "Remove for good", description: `${item.title} and, for a card, its history and approvals will be removed permanently. This cannot be undone.`, title: "Remove from the trash?" });
    if (!agreed) return;
    setBusy(item.id);
    try { await purgeSharedTrashItem(accessToken, workspaceId, item.id); showToast("success", "Removed for good."); await load(); } catch (error) { showToast("error", apiMessage(error, "That could not be removed.")); } finally { setBusy(null); }
  }

  return (
    <section className="shared-settings-group">
      <PanelHeading description="kept 30 days" icon={Trash} title="Trash" />
      <p className="shared-muted-copy">Deleted memory cards come back with their revisions, approvals and review date; deleted uploaded files come back with the cards that cited them, indexed again. Comments, tasks and earlier versions of a file are not kept, and nothing from a repository or connector is.</p>
      {items === null ? <p className="shared-muted-copy"><Loader className="rm-spin" size={ICON_SIZE.sm} /> Reading the trash…</p> : items.length === 0 ? <p className="shared-muted-copy">The trash is empty.</p> : <ul aria-label="Deleted items" className="shared-version-list">
        {items.map((item) => <li key={item.id}>
          <div><strong>{item.title}</strong><small>{item.kind === "memory_card" ? "Memory card" : "File"} · deleted {when(item.deleted_at)}{item.deleted_by ? ` by ${item.deleted_by.display_name}` : ""} · removed for good after {when(item.expires_at)}</small></div>
          <Button busy={busy === item.id} disabled={busy !== null} onClick={() => void restore(item)} type="button" variant="secondary">Restore</Button>
          {canPurge ? <Button disabled={busy !== null} onClick={() => void purge(item)} type="button" variant="secondary">Remove for good</Button> : null}
        </li>)}
      </ul>}
    </section>
  );
}
