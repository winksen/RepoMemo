import { type FormEvent } from "react";
import { IconSearch as Search, IconShieldLock as Shield } from "@tabler/icons-react";
import { createSharedMemoryCard, searchSharedMemoryCards } from "../../lib/sharedApi";
import { type MemoryCardSummary } from "../../types";
import { Button } from "../../components/ui/button";
import { Dropdown } from "../../components/ui/dropdown";
import { Input } from "../../components/ui/input";
import { Textarea } from "../../components/ui/textarea";
import { showToast } from "../../components/ui/toast";
import { apiMessage, artifactCitation } from "../format";
import { useArtifacts, useMemoryCards } from "./data";
import { type WorkspacePage, sectionState } from "./page";

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
            <div className="shared-memory-section"><div className="shared-panel-heading"><div><Shield size={18} /><h2>Team memory</h2></div><span>{memoryResults?.length ?? memoryCards.length}</span></div><form className="shared-search-form shared-memory-search" onSubmit={runMemorySearch}><Input onChange={(event) => setMemoryQuery(event.target.value)} placeholder="Search team memory" value={memoryQuery} /><Button disabled={isSubmitting} type="submit" variant="secondary">Search</Button></form>{(memoryResults ?? memoryCards).length ? <div className="shared-memory-list">{(memoryResults ?? memoryCards).map((card) => <article key={card.id}><Button className="shared-record-link" onClick={() => onOpenMemoryCard(card.id)} type="button" variant="secondary"><strong>{card.title}{card.needs_review ? <span className="shared-review-badge" title="Evidence this card cites changed, disappeared or was retired"> · Needs review</span> : null}</strong><span>{card.body_excerpt}</span></Button></article>)}</div> : <p className="shared-muted-copy">{memoryResults ? "No memory cards match that search." : "No durable memory cards yet."}</p>}{canWrite ? <form className="shared-memory-form" onSubmit={addMemory}><Input onChange={(event) => setMemoryTitle(event.target.value)} placeholder="Memory title" required value={memoryTitle} /><Textarea onChange={(event) => setMemoryBody(event.target.value)} placeholder="A concise durable fact…" required value={memoryBody} /><div className="shared-memory-evidence"><span>Evidence links</span>{memoryArtifactIds.length ? <ul aria-label="Linked evidence">{memoryArtifactIds.map((artifactId) => { const linked = artifacts.find((artifact) => artifact.id === artifactId); return <li key={artifactId}><span>{linked?.title ?? "Missing evidence"}</span><button aria-label={`Remove ${linked?.title ?? "evidence"}`} onClick={() => setMemoryArtifactIds((current) => current.filter((id) => id !== artifactId))} type="button">×</button></li>; })}</ul> : null}<Dropdown aria-label="Add an evidence link" onValueChange={(value) => { if (value !== "__add__") setMemoryArtifactIds((current) => current.includes(value) ? current : [...current, value]); }} options={[{ label: memoryArtifactIds.length ? "Add another evidence link" : "Add an evidence link (optional)", value: "__add__" }, ...artifacts.filter((artifact) => !memoryArtifactIds.includes(artifact.id)).map((artifact) => ({ label: artifact.title, value: artifact.id }))]} value="__add__" /></div><Button disabled={isSubmitting} type="submit" variant="secondary">Save memory</Button></form> : null}</div>
  );
}
