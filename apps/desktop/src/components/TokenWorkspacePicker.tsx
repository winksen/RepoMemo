// Choose the workspaces an API token is limited to (or, for a new token, whether it is limited at all).
import { Checkbox } from "./ui/checkbox";
import type { SharedWorkspace } from "../types";

export function TokenWorkspacePicker({ chosen, onChange, workspaces }: { chosen: string[]; onChange: (ids: string[]) => void; workspaces: SharedWorkspace[] }) {
  return <fieldset className="rm-token-workspaces">
    <legend>Workspaces this token may reach</legend>
    {workspaces.length ? workspaces.map((entry) => <Checkbox
      checked={chosen.includes(entry.workspace.id)}
      key={entry.workspace.id}
      label={entry.workspace.name}
      onCheckedChange={(checked) => onChange(checked ? [...chosen, entry.workspace.id] : chosen.filter((id) => id !== entry.workspace.id))}
    />) : <p className="shared-muted-copy">You are not in any workspace yet.</p>}
  </fieldset>;
}
