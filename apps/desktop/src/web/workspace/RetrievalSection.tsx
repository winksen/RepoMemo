import { type FormEvent, useEffect, useRef, useState } from "react";
import { SafeMarkdown } from "../../components/SafeMarkdown";
import { IconBook2 as Book, IconBrain as Brain, IconCode as Code, IconSearch as Search, IconShieldLock as Shield, IconTrash as Trash } from "@tabler/icons-react";
import { askSharedWorkspace, createSharedSavedSearch, deleteSharedSavedSearch, getSharedArtifact, getSharedArtifactLifecycle, searchSharedSymbols, searchSharedWorkspace, setSharedSavedSearchSharing } from "../../lib/sharedApi";
import { answerExportMarkdown } from "../../lib/answerExport";
import { saveBlob } from "../../lib/saveBlob";
import { type ArtifactType, type AskAnswer, type SymbolSearchResult, type SavedSearch, type SearchResult } from "../../types";
import { Button } from "../../components/ui/button";
import { PanelHeading } from "../../components/ui/panel-heading";
import { Pagination, paginate } from "../../components/ui/pagination";
import { Dropdown } from "../../components/ui/dropdown";
import { EmptyState } from "../../components/ui/empty-state";
import { Field } from "../../components/ui/field";
import { IconButton } from "../../components/ui/icon-button";
import { Input } from "../../components/ui/input";
import { SearchField } from "../../components/ui/search-field";
import { Textarea } from "../../components/ui/textarea";
import { RETRIEVAL_STATES, type RetrievalState } from "../constants";
import { apiMessage, artifactTypeLabel, citationLines, lastChangeLabel } from "../format";
import { EvidenceUsed, renderHighlightedSnippet } from "../common";
import { useFolders, useRetrievalFacets, useSavedSearches, useTemplate } from "./data";
import { retrievalChips } from "../templateAssistant";
import { type WorkspacePage, sectionState } from "./page";
import { ICON_SIZE } from "../../components/ui/icon";
import { showToast } from "../../components/ui/toast";
import { submitHint } from "../../lib/submitShortcut";

export function RetrievalSection({ page }: { page: WorkspacePage }) {
  const { accessToken, workspace, workspaceId, canWrite, isSubmitting, setIsSubmitting, setError, onOpenArtifact, draftMemoryFromAnswer } = page;
  const { section } = page;
  const useS = sectionState(page.store, "retrieval");
  const urlFilters = new URLSearchParams(window.location.search);
  const initialUrlSearch = useRef(urlFilters.has("q"));
  const [semanticAvailable, setSemanticAvailable] = useState<boolean | null>(null);
  const [resultsPage, setResultsPage] = useS("resultsPage", 0);
  const [results, setResults] = useS<SearchResult[]>("results", []);
  const [query, setQuery] = useS("query", urlFilters.get("q") ?? "");
  const [retrievalArtifactType, setRetrievalArtifactType] = useS<ArtifactType | "all">("retrievalArtifactType", (urlFilters.get("type") ?? "all") as ArtifactType | "all");
  const [retrievalLanguage, setRetrievalLanguage] = useS("retrievalLanguage", urlFilters.get("language") ?? "all");
  const [retrievalSourceId, setRetrievalSourceId] = useS("retrievalSourceId", urlFilters.get("source") ?? "all");
  const [retrievalState, setRetrievalState] = useS<RetrievalState>("retrievalState", (Object.prototype.hasOwnProperty.call(RETRIEVAL_STATES, urlFilters.get("state") ?? "") ? urlFilters.get("state") : "current") as RetrievalState);
  const [retrievalFolderId, setRetrievalFolderId] = useS("retrievalFolderId", urlFilters.get("folder") ?? "all");
  const [retrievalAfter, setRetrievalAfter] = useS("retrievalAfter", urlFilters.get("after") ?? "");
  const [retrievalBefore, setRetrievalBefore] = useS("retrievalBefore", urlFilters.get("before") ?? "");
  const [symbolQuery, setSymbolQuery] = useS("symbolQuery", "");
  const [symbolResults, setSymbolResults] = useS<SymbolSearchResult[] | null>("symbolResults", null);
  const [retrievalLimit, setRetrievalLimit] = useS("retrievalLimit", ["10", "20", "50"].includes(urlFilters.get("limit") ?? "") ? urlFilters.get("limit")! : "20");
  const [savedSearchName, setSavedSearchName] = useS("savedSearchName", "");
  const [askQuestion, setAskQuestion] = useS("askQuestion", "");
  const [askAnswer, setAskAnswer] = useS<AskAnswer | null>("askAnswer", null);
  const [askedQuestion, setAskedQuestion] = useS("askedQuestion", "");
  const [retrievalFacets] = useRetrievalFacets(accessToken, workspaceId);
  const [template] = useTemplate(accessToken, workspaceId);
  const typeLabels = template?.labels.artifact_types;
  const chips = retrievalChips(template?.retrieval.type_chips, retrievalFacets?.artifact_types);
  const [folders] = useFolders(accessToken, workspaceId);
  const [savedSearches, setSavedSearches] = useSavedSearches(accessToken, workspaceId, true);
  // A new search, filter, folder or section starts again from the first page.
  useEffect(() => { setResultsPage(0); }, [results]);
  useEffect(() => {
    const url = new URL(window.location.href);
    for (const key of ["q", "type", "language", "source", "state", "folder", "after", "before", "limit"]) url.searchParams.delete(key);
    if (query.trim()) url.searchParams.set("q", query.trim());
    if (retrievalArtifactType !== "all") url.searchParams.set("type", retrievalArtifactType);
    if (retrievalLanguage !== "all") url.searchParams.set("language", retrievalLanguage);
    if (retrievalSourceId !== "all") url.searchParams.set("source", retrievalSourceId);
    if (retrievalState !== "current") url.searchParams.set("state", retrievalState);
    if (retrievalFolderId !== "all") url.searchParams.set("folder", retrievalFolderId);
    if (retrievalAfter) url.searchParams.set("after", retrievalAfter);
    if (retrievalBefore) url.searchParams.set("before", retrievalBefore);
    if (retrievalLimit !== "20") url.searchParams.set("limit", retrievalLimit);
    window.history.replaceState(window.history.state, "", url);
  }, [query, retrievalArtifactType, retrievalLanguage, retrievalSourceId, retrievalState, retrievalFolderId, retrievalAfter, retrievalBefore, retrievalLimit]);
  useEffect(() => { if (initialUrlSearch.current) { initialUrlSearch.current = false; void performSearch(); } }, []);

  async function performSearch() {
    if (!query.trim()) return;
    setIsSubmitting(true); setError(null);
    try {
      const found = await searchSharedWorkspace(accessToken, workspace.workspace.id, {
        query,
        artifactTypes: retrievalArtifactType === "all" ? [] : [retrievalArtifactType],
        languages: retrievalLanguage === "all" ? [] : [retrievalLanguage],
        sourceIds: retrievalSourceId === "all" ? [] : [retrievalSourceId],
        limit: Number(retrievalLimit),
        lifecycleStatuses: RETRIEVAL_STATES[retrievalState].statuses,
        folderId: retrievalFolderId === "all" ? null : retrievalFolderId,
        modifiedAfter: retrievalAfter || null,
        modifiedBefore: retrievalBefore || null,
      });
      setResults(found.results);
      setSemanticAvailable(found.semanticAvailable);
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }
  function runSearch(event: FormEvent<HTMLFormElement>) { event.preventDefault(); void performSearch(); }

  function clearRetrievalFilters() {
    setRetrievalArtifactType("all");
    setRetrievalLanguage("all");
    setRetrievalSourceId("all");
    setRetrievalLimit("20");
    setRetrievalState("current");
    setRetrievalFolderId("all");
    setRetrievalAfter("");
    setRetrievalBefore("");
    setResults([]);
    setSemanticAvailable(null);
  }

  async function runSymbolSearch(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!symbolQuery.trim()) return;
    setIsSubmitting(true); setError(null);
    try {
      setSymbolResults(await searchSharedSymbols(accessToken, workspace.workspace.id, symbolQuery.trim()));
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function saveCurrentSearch(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSubmitting(true); setError(null);
    try {
      const saved = await createSharedSavedSearch(accessToken, workspace.workspace.id, { name: savedSearchName, query, artifactTypes: retrievalArtifactType === "all" ? [] : [retrievalArtifactType], languages: retrievalLanguage === "all" ? [] : [retrievalLanguage], sourceIds: retrievalSourceId === "all" ? [] : [retrievalSourceId], resultLimit: Number(retrievalLimit) });
      setSavedSearches((current) => [saved, ...current]); setSavedSearchName(""); showToast("success", "Private search saved.");
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function applySavedSearch(saved: SavedSearch) {
    setQuery(saved.query); setRetrievalArtifactType(saved.artifact_types[0] ?? "all"); setRetrievalLanguage(saved.languages[0] ?? "all"); setRetrievalSourceId(saved.source_ids[0] ?? "all"); setRetrievalLimit(String(saved.result_limit)); setResults([]);
    setIsSubmitting(true); setError(null);
    try {
      const found = await searchSharedWorkspace(accessToken, workspaceId, { query: saved.query, artifactTypes: saved.artifact_types, languages: saved.languages, sourceIds: saved.source_ids, limit: saved.result_limit });
      setResults(found.results); setSemanticAvailable(found.semanticAvailable);
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function changeSharing(saved: SavedSearch) {
    setIsSubmitting(true); setError(null);
    try {
      const updated = await setSharedSavedSearchSharing(accessToken, saved.id, !saved.shared);
      setSavedSearches((current) => current.map((entry) => entry.id === updated.id ? updated : entry));
      showToast("success", updated.shared ? "Search shared with the workspace." : "Search made private.");
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function removeSavedSearch(saved: SavedSearch) {
    setIsSubmitting(true); setError(null);
    try { await deleteSharedSavedSearch(accessToken, saved.id); setSavedSearches((current) => current.filter((entry) => entry.id !== saved.id)); showToast("success", "Saved search deleted."); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function askEvidence(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!askQuestion.trim()) return;
    setIsSubmitting(true); setError(null);
    const question = askQuestion.trim();
    try { setAskAnswer(await askSharedWorkspace(accessToken, workspace.workspace.id, askQuestion)); setAskedQuestion(question); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function exportAnswer() {
    if (!askAnswer) return;
    setIsSubmitting(true); setError(null);
    try {
      const markdown = await answerExportMarkdown(askedQuestion || "Answer from workspace evidence", askAnswer, async (citation) => {
        const artifact = await getSharedArtifact(accessToken, citation.artifact_id);
        const lifecycle = await getSharedArtifactLifecycle(accessToken, citation.artifact_id);
        return {
          sourceId: artifact.summary.source_id,
          reviewStatus: lifecycle.status,
          headingPath: artifact.chunks.find((chunk) => chunk.id === citation.chunk_id)?.heading_path ?? null,
        };
      });
      await saveBlob(new Blob([markdown], { type: "text/markdown;charset=utf-8" }), "repomemo-answer.md");
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  return (
            <><PanelHeading icon={Search} title="Retrieve" />
            <form className="shared-retrieval-search" onSubmit={runSearch}>
              <SearchField aria-label="Search indexed evidence" onValueChange={setQuery} placeholder="Search indexed evidence" value={query} />
              {chips.length ? <div aria-label="Suggested file types" className="shared-retrieval-chips" role="group">{chips.map((kind) => <Button aria-pressed={retrievalArtifactType === kind} className={retrievalArtifactType === kind ? "active" : ""} key={kind} onClick={() => setRetrievalArtifactType(retrievalArtifactType === kind ? "all" : kind as ArtifactType)} type="button" variant="secondary">{artifactTypeLabel(kind as ArtifactType, typeLabels)}</Button>)}</div> : null}
              <div className="shared-retrieval-filter-row">
                <Field label="File type">{(control) => <Dropdown id={control.id} onValueChange={(value) => setRetrievalArtifactType(value as ArtifactType | "all")} options={[{ label: "All file types", value: "all" }, ...(retrievalFacets?.artifact_types ?? []).map((type) => ({ label: artifactTypeLabel(type, typeLabels), value: type }))]} value={retrievalArtifactType} />}</Field>
                <Field label="Language">{(control) => <Dropdown id={control.id} onValueChange={setRetrievalLanguage} options={[{ label: "All languages", value: "all" }, ...(retrievalFacets?.languages ?? []).map((language) => ({ label: language, value: language }))]} value={retrievalLanguage} />}</Field>
                <Field label="Source">{(control) => <Dropdown id={control.id} onValueChange={setRetrievalSourceId} options={[{ label: "All sources", value: "all" }, ...(retrievalFacets?.sources ?? []).map((source) => ({ label: source.name, value: source.id }))]} value={retrievalSourceId} />}</Field>
                <Field label="Evidence state">{(control) => <Dropdown id={control.id} onValueChange={(value) => setRetrievalState(value as RetrievalState)} options={Object.entries(RETRIEVAL_STATES).map(([value, entry]) => ({ label: entry.label, value }))} value={retrievalState} />}</Field>
                <Field label="Folder">{(control) => <Dropdown id={control.id} onValueChange={setRetrievalFolderId} options={[{ label: "All folders", value: "all" }, ...folders.map((folder) => ({ label: folder.name, value: folder.id }))]} value={retrievalFolderId} />}</Field>
                <Field label="Updated from"><Input onChange={(event) => setRetrievalAfter(event.target.value)} type="date" value={retrievalAfter} /></Field>
                <Field label="Updated to"><Input onChange={(event) => setRetrievalBefore(event.target.value)} type="date" value={retrievalBefore} /></Field>
                <Field label="Results">{(control) => <Dropdown id={control.id} onValueChange={setRetrievalLimit} options={[{ label: "10 results", value: "10" }, { label: "20 results", value: "20" }, { label: "50 results", value: "50" }]} value={retrievalLimit} />}</Field>
                <Button busy={isSubmitting} type="submit" variant="main"><Search size={ICON_SIZE.md} /> Search</Button>
                {results.length ? <Button disabled={isSubmitting} onClick={clearRetrievalFilters} variant="secondary">Clear</Button> : null}
              </div>
            </form>
            <section className="shared-symbol-search"><PanelHeading icon={Code} level={3} title="Code symbols" />
              <form className="shared-retrieval-search" onSubmit={runSymbolSearch}><SearchField aria-label="Function, class or method name" onValueChange={setSymbolQuery} placeholder="Function, class or method name" value={symbolQuery} /><Button busy={isSubmitting} type="submit" variant="secondary">Find definition</Button></form>
              {symbolResults ? (symbolResults.length ? <div className="shared-search-results">{symbolResults.map((hit) => <article key={hit.symbol.id}><Button className="shared-search-result" onClick={() => onOpenArtifact(hit.symbol.artifact_id)} type="button" variant="secondary"><span><strong>{hit.symbol.name}</strong> · {hit.symbol.kind} · Symbol match: “{symbolQuery.trim()}”</span><span>{hit.path}{hit.symbol.start_line ? `:${hit.symbol.start_line}` : ""}</span></Button></article>)}</div> : <EmptyState description="No indexed code defines a symbol named like that." title="No symbol found" />) : null}
            </section>
            <section className="shared-saved-searches">
              <PanelHeading description={savedSearches.length} icon={Book} level={3} title="Saved searches" />
              {savedSearches.length ? <div className="shared-saved-search-list">{savedSearches.map((saved) => <article key={saved.id}>
                <Button className="shared-saved-search-open" onClick={() => void applySavedSearch(saved)} type="button" variant="secondary"><span><strong>{saved.name}</strong><small>{saved.query} · {saved.shared ? "Shared with workspace" : "Private"}</small></span></Button>
                {workspace.role === "owner" ? <Button disabled={isSubmitting} onClick={() => void changeSharing(saved)} type="button" variant="secondary">{saved.shared ? "Make private" : "Share"}</Button> : null}
                {canWrite && (saved.created_by.id === page.session.user.id || workspace.role === "owner" || workspace.role === "admin") ? <IconButton disabled={isSubmitting} label={`Delete ${saved.name}`} onClick={() => void removeSavedSearch(saved)}><Trash size={ICON_SIZE.sm} /></IconButton> : null}
              </article>)}</div> : <EmptyState description="Save a private query and its filters for recurring investigations. An owner can share it with the workspace." title="No saved searches" />}
              {canWrite ? <form className="shared-save-search-form" onSubmit={saveCurrentSearch}><Field label="Name for this search" required><Input onChange={(event) => setSavedSearchName(event.target.value)} placeholder="Search name" value={savedSearchName} /></Field><Button busy={isSubmitting} disabled={!query.trim()} type="submit" variant="secondary">Save private search</Button></form> : null}
            </section>
            <div>{semanticAvailable === false ? <p className="shared-muted-copy" role="status">Semantic search is unavailable for this query. Results use keyword matches only.</p> : null}{results.length ? <div className="shared-search-results"><p className="shared-search-result-count">{results.length} matching evidence {results.length === 1 ? "result" : "results"}</p>{paginate(results, resultsPage).map((result) => <article key={result.chunk_id}><Button className="shared-search-result" onClick={() => onOpenArtifact(result.artifact_id)} type="button" variant="secondary"><span className="shared-search-result-heading"><strong>{result.title}</strong><span>{artifactTypeLabel(result.artifact_type, typeLabels)} · {result.language ?? "Unspecified"} · {result.source_name}</span></span><p>{renderHighlightedSnippet(result.snippet)}</p><span className="shared-search-result-path">{result.match_kind === "keyword_and_semantic" ? "Keyword and semantic match" : result.match_kind === "semantic" ? "Semantic match" : result.match_kind === "keyword" ? `Keyword match: “${query.trim()}”` : "Match type unavailable"} · {result.path}{citationLines(result)}{result.last_author ? ` · last changed by ${lastChangeLabel(result.last_author, result.last_changed_at)}` : ""}</span></Button></article>)}<Pagination onPageChange={setResultsPage} page={resultsPage} total={results.length} /></div> : <EmptyState description="Index one or more artifacts, then search the evidence base from here. Use filters to narrow large workspaces." title="Search the evidence base" />}
              <section className="shared-ask-evidence" aria-live="polite"><div><Brain size={ICON_SIZE.lg} /><h3>Ask your evidence</h3></div><p>Get a citation-backed answer from the indexed workspace context.</p><form onSubmit={askEvidence}><Field hint={submitHint("asks")} label="Question about the evidence" required><Textarea onChange={(event) => setAskQuestion(event.target.value)} placeholder="What do we know about the current implementation?" value={askQuestion} /></Field><Button busy={isSubmitting} type="submit" variant="secondary"><Brain size={ICON_SIZE.md} /> Ask</Button></form>{askAnswer ? <div className="shared-ask-answer"><p className="shared-ai-generated-note">AI-generated from the cited passages · verify against the sources</p><div className="shared-markdown shared-ai-answer"><SafeMarkdown>{askAnswer.answer_markdown}</SafeMarkdown></div>{askAnswer.warnings.length ? <div className="shared-ai-overview-warning">{askAnswer.warnings.map((warning) => <p key={warning}>{warning}</p>)}</div> : null}{askAnswer.citations.length ? <EvidenceUsed accessToken={accessToken} citations={askAnswer.citations} numbered onOpenArtifact={onOpenArtifact} /> : null}<div className="shared-ai-actions"><Button disabled={isSubmitting} onClick={() => void exportAnswer()} type="button" variant="secondary">Export answer</Button>{canWrite && askAnswer.citations.length ? <Button disabled={isSubmitting} onClick={() => draftMemoryFromAnswer(askedQuestion || "Answer from workspace evidence", askAnswer.answer_markdown, "Ask your evidence", askAnswer.citations)} type="button" variant="secondary"><Shield size={ICON_SIZE.md} /> Save as memory</Button> : null}</div></div> : null}</section>
            </div></>
  );
}
