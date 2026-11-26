import { LockKey } from "@phosphor-icons/react";
import type { Folder } from "../../types";
import { Checkbox } from "../../components/ui/checkbox";
import { PanelHeading } from "../../components/ui/panel-heading";

/** The folder's path from the top level, for the list. */
function pathOf(folder: Folder, byId: Map<string, Folder>): string {
  const names = [folder.name];
  let cursor = folder.parent_id ? byId.get(folder.parent_id) : undefined;
  while (cursor) {
    names.unshift(cursor.name);
    cursor = cursor.parent_id ? byId.get(cursor.parent_id) : undefined;
  }
  return names.join(" / ");
}

/**
 * The folders CAIRN does not read (V0.9.93): its search, read, list and analysis tools, its chats across workspaces and the MCP endpoint
 * never return what is in a closed folder or the folders inside it. Ask outside CAIRN is unchanged. A workspace administrator chooses.
 */
export function ClosedFolders({ closed, folders, onChange }: { closed: string[]; folders: Folder[]; onChange: (closed: string[]) => void }) {
  if (!folders.length) return null;
  const byId = new Map(folders.map((folder) => [folder.id, folder]));
  const entries = folders.map((folder) => ({ folder, path: pathOf(folder, byId) })).sort((left, right) => left.path.localeCompare(right.path));
  return <section className="shared-settings-group">
    <PanelHeading description="CAIRN does not read what is in them" icon={LockKey} title="Folders closed to CAIRN" />
    {entries.map(({ folder, path }) => <Checkbox checked={closed.includes(folder.id)} description={!closed.includes(folder.id) && folder.closed_to_cairn ? "Closed because a folder above it is" : undefined} key={folder.id} label={path} onCheckedChange={(on) => onChange(on ? [...closed, folder.id] : closed.filter((id) => id !== folder.id))} />)}
  </section>;
}
