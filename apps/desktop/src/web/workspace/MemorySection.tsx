import { type FormEvent } from "react";
import { IconSearch as Search, IconShieldLock as Shield } from "@tabler/icons-react";
import { createSharedMemoryCard, searchSharedMemoryCards } from "../../lib/sharedApi";
import { type MemoryCardSummary } from "../../types";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { PanelHeading } from "../../components/ui/panel-heading";
import { EmptyState } from "../../components/ui/empty-state";
import { Pagination } from "../../components/ui/pagination";
import { usePagedList } from "../../lib/usePagedList";
import { Combobox } from "../../components/ui/combobox";
import { Field } from "../../components/ui/field";
import { Input } from "../../components/ui/input";
import { SearchField } from "../../components/ui/search-field";
import { Textarea } from "../../components/ui/textarea";
import { Tooltip } from "../../components/ui/tooltip";
import { showToast } from "../../components/ui/toast";
import { apiMessage, artifactCitation } from "../format";
import { useArtifacts, useMemoryCards } from "./data";
import { type WorkspacePage, sectionState } from "./page";
import { submitHint } from "../../lib/submitShortcut";

export function MemorySection({ page }: { page: WorkspacePage }) {
  const { accessToken, workspace, workspaceId, canWrite, isSubmitting, setIsSubmitting, setError, load, onOpenMemoryCard } = page;
  const useS = sectionState(page.store, "memory");
  const [memoryQuery, setMemoryQuery] = useS("memoryQuery", "");
  const [memoryResults, setMemoryResults] = useS<MemoryCardSummary[] | null>("memoryResults", null);
  const [memoryTitle, setMemoryTitle] = useS("memoryTitle", "");
  const [memoryBody, setMemoryBody] = useS("memoryBody", "");
  const [memoryArtifactIds, setMemoryArtifactIds] = useS<string[]>("memoryArtifactIds", []);
  const [artifacts] = useArtifacts(accessToken, workspaceId);
  const [memoryCards] = useMemoryCards(accessToken, workspaceId);
  const paging = usePagedList(memoryResults ?? memoryCards, memoryResults);
  async function addMemory(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSubmitting(true); setError(null);
    try {
      await createSharedMemoryCard(accessToken, workspace.workspace.id, {
        title: memoryTitle,
        bodyMarkdown: memoryBody,
        source: "Team note",
        citations: memoryArtifactIds.flatMap((artifactId) => artifactCitation(artifacts, artifactId)),
      });
      setMemoryTitle(""); setMemoryBody(""); setMemoryArtifactIds([]);
      await load();
      showToast("success", "Memory saved.");
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function runMemorySearch(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const nextQuery = memoryQuery.trim();
    if (!nextQuery) { setMemoryResults(null); return; }
    setIsSubmitting(true); setError(null);
    try { setMemoryResults(await searchSharedMemoryCards(accessToken, workspace.workspace.id, nextQuery)); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  return (
            <div className="shared-memory-section"><PanelHeading description={memoryResults?.length ?? memoryCards.length} icon={Shield} title="Team memory" /><form className="shared-search-form shared-memory-search" onSubmit={runMemorySearch}><SearchField aria-label="Search team memory" onValueChange={setMemoryQuery} placeholder="Search team memory" value={memoryQuery} /><Button busy={isSubmitting} type="submit" variant="secondary">Search</Button></form>{(memoryResults ?? memoryCards).length ? <div className="shared-memory-list">{paging.rows.map((card) => <article key={card.id}><Button className="shared-record-link" onClick={() => onOpenMemoryCard(card.id)} variant="secondary"><strong>{card.title}{card.needs_review ?  <Tooltip content="Evidence this card cites changed, disappeared or was retired"><Badge tone="warning">Needs review</Badge></Tooltip> : null}</strong><span>{card.body_excerpt}</span></Button></article>)}<Pagination {...paging.props} /></div> : memoryResults ? <EmptyState description="Try other words, or clear the search." icon={<Shield size={25} />} title="No memory cards match" /> : <EmptyState description="Save a fact the team should keep, with links to the evidence behind it." icon={<Shield size={25} />} title="No durable memory cards yet" />}{canWrite ? <form className="shared-memory-form" onSubmit={addMemory}><Field label="Title" required><Input onChange={(event) => setMemoryTitle(event.target.value)} placeholder="Memory title" value={memoryTitle} /></Field><Field hint={submitHint("saves")} label="Statement" required><Textarea onChange={(event) => setMemoryBody(event.target.value)} placeholder="A concise durable fact…" value={memoryBody} /></Field><Field className="shared-memory-evidence" label="Evidence links">{(control) => <>{memoryArtifactIds.length ? <ul aria-label="Linked evidence">{memoryArtifactIds.map((artifactId) => { const linked = artifacts.find((artifact) => artifact.id === artifactId); return <li key={artifactId}><span>{linked?.title ?? "Missing evidence"}</span><button aria-label={`Remove ${linked?.title ?? "evidence"}`} onClick={() => setMemoryArtifactIds((current) => current.filter((id) => id !== artifactId))} type="button">×</button></li>; })}</ul> : null}<Combobox aria-label="Add an evidence link" emptyLabel="No file matches" id={control.id} onValueChange={(value) => { if (value) setMemoryArtifactIds((current) => current.includes(value) ? current : [...current, value]); }} options={artifacts.filter((artifact) => !memoryArtifactIds.includes(artifact.id)).map((artifact) => ({ label: artifact.title, value: artifact.id }))} placeholder={memoryArtifactIds.length ? "Add another evidence link" : "Add an evidence link (optional)"} value="" /></>}</Field><Button busy={isSubmitting} type="submit" variant="secondary">Save memory</Button></form> : null}</div>
  );
}
