import { useCallback, useEffect, useState } from "react";
import { Question } from "@phosphor-icons/react";
import { listSharedArtifactsPage, listSharedMemoryCards, listSharedSearchFeedback, resolveSharedSearchFeedback } from "../lib/sharedApi";
import type { SearchFeedback } from "../types";
import { Button } from "./ui/button";
import { Combobox } from "./ui/combobox";
import { PanelHeading } from "./ui/panel-heading";
import { showToast } from "./ui/toast";
import { apiMessage } from "../web/format";
import { formatDateValue } from "../lib/locale";

const when = (value: string) => formatDateValue(value);

type Option = { label: string; value: string };

/**
 * Search requests (V0.6.25): what people looked for and did not find, or a result that did not help. Everyone who can read the
 * workspace sees the requests they sent and what answered them; owners and administrators see all of them and answer with a
 * file or a memory card, or decline. The query text is removed 30 days after a request is answered (180 days if it is not).
 */
export function SearchFeedbackPanel({ accessToken, canAnswer, workspaceId }: { accessToken: string; canAnswer: boolean; workspaceId: string }) {
  const [items, setItems] = useState<SearchFeedback[] | null>(null);
  const [options, setOptions] = useState<{ cards: Option[]; files: Option[] } | null>(null);
  const [chosen, setChosen] = useState<Record<string, { kind: "memory_card" | "evidence"; id: string }>>({});
  const [busy, setBusy] = useState<string | null>(null);

  const load = useCallback(async () => {
    try { setItems(await listSharedSearchFeedback(accessToken, workspaceId)); } catch (error) { setItems([]); showToast("error", apiMessage(error, "The search requests could not be read.")); }
  }, [accessToken, workspaceId]);
  useEffect(() => { void load(); }, [load]);

  const hasOpen = items?.some((item) => item.status === "open") ?? false;
  useEffect(() => {
    if (!canAnswer || !hasOpen || options) return;
    Promise.all([listSharedMemoryCards(accessToken, workspaceId), listSharedArtifactsPage(accessToken, workspaceId, { limit: 200, offset: 0 })])
      .then(([cards, files]) => setOptions({ cards: cards.map((card) => ({ label: card.title, value: card.id })), files: files.items.map((file) => ({ label: file.title, value: file.id })) }))
      .catch(() => setOptions({ cards: [], files: [] }));
  }, [accessToken, canAnswer, hasOpen, options, workspaceId]);

  async function answer(item: SearchFeedback, resolution: "memory_card" | "evidence" | "declined") {
    setBusy(item.id);
    try {
      const target = resolution === "declined" ? undefined : chosen[item.id]?.id;
      await resolveSharedSearchFeedback(accessToken, workspaceId, item.id, { resolution, targetId: target });
      showToast("success", resolution === "declined" ? "Declined." : "Marked as answered.");
      await load();
    } catch (error) { showToast("error", apiMessage(error, "That could not be saved.")); } finally { setBusy(null); }
  }

  if (!items || items.length === 0) return null;
  return (
    <section aria-labelledby="search-requests-title" className="shared-settings-group">
      <PanelHeading description={canAnswer ? "from your team" : "yours"} icon={Question} id="search-requests-title" title="Search requests" />
      <ul aria-label="Search requests" className="shared-version-list">
        {items.map((item) => <li key={item.id}>
          <div>
            <strong>{item.query}</strong>
            <small>{item.kind === "no_answer" ? "Nothing useful found" : `A result did not help${item.artifact_title ? `: ${item.artifact_title}` : ""}`}{item.asked_by ? ` · ${item.asked_by.display_name}` : ""} · {when(item.created_at)}{item.note ? ` · ${item.note}` : ""}</small>
            <small>{item.status === "open" ? "Open" : item.status === "declined" ? "Declined" : `Answered with ${item.resolution_kind === "memory_card" ? "a memory card" : "evidence"}${item.resolution_title ? `: ${item.resolution_title}` : ""}`} · removed {when(item.expires_at)}</small>
          </div>
          {canAnswer && item.status === "open" ? <div className="shared-version-actions">
            <Combobox aria-label={`Memory card that answers “${item.query}”`} noneLabel="Choose a memory card" onValueChange={(id) => setChosen((current) => ({ ...current, [item.id]: { kind: "memory_card", id } }))} options={options?.cards ?? []} placeholder="Memory card" value={chosen[item.id]?.kind === "memory_card" ? chosen[item.id].id : ""} />
            <Combobox aria-label={`File that answers “${item.query}”`} noneLabel="Choose a file" onValueChange={(id) => setChosen((current) => ({ ...current, [item.id]: { kind: "evidence", id } }))} options={options?.files ?? []} placeholder="File" value={chosen[item.id]?.kind === "evidence" ? chosen[item.id].id : ""} />
            <Button busy={busy === item.id} disabled={busy !== null || !chosen[item.id]?.id} onClick={() => void answer(item, chosen[item.id].kind)} type="button" variant="main">Mark answered</Button>
            <Button disabled={busy !== null} onClick={() => void answer(item, "declined")} type="button" variant="secondary">Decline</Button>
          </div> : null}
        </li>)}
      </ul>
    </section>
  );
}
