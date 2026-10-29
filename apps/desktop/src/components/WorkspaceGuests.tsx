// Guests (people in a workspace who are not in its organization) and when their access ends.
import { useCallback, useEffect, useState } from "react";
import { IconClockHour4 as Clock } from "@tabler/icons-react";
import { request } from "../lib/sharedApi";
import { apiMessage, formatTime } from "../web/format";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { PanelHeading } from "./ui/panel-heading";
import { Table } from "./ui/table";
import { showToast } from "./ui/toast";

interface Guest { user_id: string; email: string | null; display_name: string; role: string; expires_at: string | null; expired: boolean }

export function WorkspaceGuests({ accessToken, workspaceId }: { accessToken: string; workspaceId: string }) {
  const [guests, setGuests] = useState<Guest[] | null>(null);
  const [days, setDays] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState<string | null>(null);
  const load = useCallback(async () => {
    try { setGuests((await request<{ guests: Guest[] }>(`/v1/workspaces/${workspaceId}/guests`, {}, accessToken)).guests); }
    catch (error) { showToast("error", apiMessage(error, "The guests could not be loaded.")); }
  }, [accessToken, workspaceId]);
  useEffect(() => { void load(); }, [load]);
  if (!guests?.length) return null;
  async function save(guest: Guest, expires: string | null) {
    setBusy(guest.user_id);
    try {
      await request(`/v1/workspaces/${workspaceId}/guests/${guest.user_id}/expiry`, { method: "PUT", body: JSON.stringify({ expires_at: expires }) }, accessToken);
      setDays((current) => ({ ...current, [guest.user_id]: "" }));
      await load();
      showToast("success", expires ? `${guest.display_name}'s access now ends at the end of ${expires}.` : `${guest.display_name}'s access no longer ends.`);
    } catch (error) { showToast("error", apiMessage(error, "The end date could not be saved.")); }
    finally { setBusy(null); }
  }
  return <section aria-label="Guests" className="shared-settings-group">
    <PanelHeading description={guests.length} icon={Clock} level={3} title="Guests" />
    <p className="shared-muted-copy">Guests are in this workspace without being in its organization. After the end date they cannot read, search, export or receive events, and their tokens stop reaching it. Renew by choosing a later day (up to a year ahead).</p>
    <Table>
      <thead><tr><th scope="col">Guest</th><th scope="col">Role</th><th scope="col">Access</th><th scope="col">End date</th></tr></thead>
      <tbody>{guests.map((guest) => <tr key={guest.user_id}>
        <th scope="row">{guest.display_name}{guest.email ? ` · ${guest.email}` : ""}</th>
        <td>{guest.role}</td>
        <td>{guest.expires_at ? (guest.expired ? `Ended ${formatTime(guest.expires_at)}` : `Ends ${formatTime(guest.expires_at)}`) : "Does not end"}</td>
        <td>
          <Input aria-label={`End date for ${guest.display_name}`} onChange={(event) => setDays((current) => ({ ...current, [guest.user_id]: event.target.value }))} type="date" value={days[guest.user_id] ?? ""} />{" "}
          <Button busy={busy === guest.user_id} disabled={!days[guest.user_id]} onClick={() => void save(guest, days[guest.user_id] ?? null)} type="button" variant="secondary">{guest.expires_at ? "Renew" : "Set"}</Button>
          {guest.expires_at ? <Button disabled={busy === guest.user_id} onClick={() => void save(guest, null)} type="button" variant="secondary">Remove end date</Button> : null}
        </td>
      </tr>)}</tbody>
    </Table>
  </section>;
}
