import { type FormEvent, useEffect } from "react";
import { SafeMarkdown } from "../../components/SafeMarkdown";
import { IconBook2 as Book, IconBrain as Brain, IconCode as Code, IconLoader2 as Loader, IconSearch as Search, IconShieldLock as Shield, IconTrash as Trash } from "@tabler/icons-react";
import { askSharedWorkspace, createSharedSavedSearch, deleteSharedSavedSearch, searchSharedSymbols, searchSharedWorkspace } from "../../lib/sharedApi";
import { type ArtifactType, type AskAnswer, type SymbolSearchResult, type SavedSearch, type SearchResult } from "../../types";
import { Button } from "../../components/ui/button";
import { Pagination, paginate } from "../../components/ui/pagination";
import { Dropdown } from "../../components/ui/dropdown";
import { Input } from "../../components/ui/input";
import { Textarea } from "../../components/ui/textarea";
import { RETRIEVAL_STATES, type RetrievalState } from "../constants";
import { apiMessage, artifactTypeLabel, citationLines, lastChangeLabel } from "../format";
import { EvidenceUsed, renderHighlightedSnippet } from "../common";
import { useFolders, useRetrievalFacets, useSavedSearches } from "./data";
import { type WorkspacePage, sectionState } from "./page";

export function RetrievalSection({ page }: { page: WorkspacePage }) {
  const { accessToken, workspace, workspaceId, canWrite, isSubmitting, setIsSubmitting, setError, onOpenArtifact, draftMemoryFromAnswer } = page;
  const { section } = page;
  const useS = sectionState(page.store, "retrieval");
  const [resultsPage, setResultsPage] = useS("resultsPage", 0);
  const [results, setResults] = useS<SearchResult[]>("results", []);
  const [query, setQuery] = useS("query", "");
  const [retrievalArtifactType, setRetrievalArtifactType] = useS<ArtifactType | "all">("retrievalArtifactType", "all");
  const [retrievalLanguage, setRetrievalLanguage] = useS("retrievalLanguage", "all");
  const [retrievalSourceId, setRetrievalSourceId] = useS("retrievalSourceId", "all");
  const [retrievalState, setRetrievalState] = useS<RetrievalState>("retrievalState", "current");
  const [retrievalFolderId, setRetrievalFolderId] = useS("retrievalFolderId", "all");
  const [retrievalAfter, setRetrievalAfter] = useS("retrievalAfter", "");
  const [retrievalBefore, setRetrievalBefore] = useS("retrievalBefore", "");
  const [symbolQuery, setSymbolQuery] = useS("symbolQuery", "");
  const [symbolResults, setSymbolResults] = useS<SymbolSearchResult[] | null>("symbolResults", null);
  const [retrievalLimit, setRetrievalLimit] = useS("retrievalLimit", "20");
  const [savedSearchName, setSavedSearchName] = useS("savedSearchName", "");
  const [askQuestion, setAskQuestion] = useS("askQuestion", "");
  const [askAnswer, setAskAnswer] = useS<AskAnswer | null>("askAnswer", null);
  const [askedQuestion, setAskedQuestion] = useS("askedQuestion", "");
  const [retrievalFacets] = useRetrievalFacets(accessToken, workspaceId);
  const [folders] = useFolders(accessToken, workspaceId);
  const [savedSearches, setSavedSearches] = useSavedSearches(accessToken, workspaceId, true);
  // A new search, filter, folder or section starts again from the first page.
  useEffect(() => { setResultsPage(0); }, [results]);
  async function runSearch(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!query.trim()) return;
    setIsSubmitting(true); setError(null);
    try {
      setResults(await searchSharedWorkspace(accessToken, workspace.workspace.id, {
        query,
        artifactTypes: retrievalArtifactType === "all" ? [] : [retrievalArtifactType],
        languages: retrievalLanguage === "all" ? [] : [retrievalLanguage],
        sourceIds: retrievalSourceId === "all" ? [] : [retrievalSourceId],
        limit: Number(retrievalLimit),
        lifecycleStatuses: RETRIEVAL_STATES[retrievalState].statuses,
        folderId: retrievalFolderId === "all" ? null : retrievalFolderId,
        modifiedAfter: retrievalAfter || null,
        modifiedBefore: retrievalBefore || null,
      }));
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

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
      setSavedSearches((current) => [saved, ...current]); setSavedSearchName("");
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  function applySavedSearch(saved: SavedSearch) {
    setQuery(saved.query); setRetrievalArtifactType(saved.artifact_types[0] ?? "all"); setRetrievalLanguage(saved.languages[0] ?? "all"); setRetrievalSourceId(saved.source_ids[0] ?? "all"); setRetrievalLimit(String(saved.result_limit)); setResults([]);
  }

  async function removeSavedSearch(saved: SavedSearch) {
    setIsSubmitting(true); setError(null);
    try { await deleteSharedSavedSearch(accessToken, saved.id); setSavedSearches((current) => current.filter((entry) => entry.id !== saved.id)); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  async function askEvidence(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!askQuestion.trim()) return;
    setIsSubmitting(true); setError(null);
    const question = askQuestion.trim();
    try { setAskAnswer(await askSharedWorkspace(accessToken, workspace.workspace.id, askQuestion)); setAskedQuestion(question); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  return (
            <><div className="shared-panel-heading"><div><Search size={18} /><h2>Retrieve</h2></div></div>
            <form className="shared-retrieval-search" onSubmit={runSearch}>
              <Input onChange={(event) => setQuery(event.target.value)} placeholder="Search indexed evidence" value={query} />
              <div className="shared-retrieval-filter-row">
                <Dropdown aria-label="Limit retrieval to a file type" onValueChange={(value) => setRetrievalArtifactType(value as ArtifactType | "all")} options={[{ label: "All file types", value: "all" }, ...(retrievalFacets?.artifact_types ?? []).map((type) => ({ label: artifactTypeLabel(type), value: type }))]} value={retrievalArtifactType} />
                <Dropdown aria-label="Limit retrieval to a language" onValueChange={setRetrievalLanguage} options={[{ label: "All languages", value: "all" }, ...(retrievalFacets?.languages ?? []).map((language) => ({ label: language, value: language }))]} value={retrievalLanguage} />
                <Dropdown aria-label="Limit retrieval to a source" onValueChange={setRetrievalSourceId} options={[{ label: "All sources", value: "all" }, ...(retrievalFacets?.sources ?? []).map((source) => ({ label: source.name, value: source.id }))]} value={retrievalSourceId} />
                <Dropdown aria-label="Evidence state" onValueChange={(value) => setRetrievalState(value as RetrievalState)} options={Object.entries(RETRIEVAL_STATES).map(([value, entry]) => ({ label: entry.label, value }))} value={retrievalState} />
                <Dropdown aria-label="Limit retrieval to a folder" onValueChange={setRetrievalFolderId} options={[{ label: "All folders", value: "all" }, ...folders.map((folder) => ({ label: folder.name, value: folder.id }))]} value={retrievalFolderId} />
                <label className="shared-retrieval-date">Updated from<Input aria-label="Updated on or after" onChange={(event) => setRetrievalAfter(event.target.value)} type="date" value={retrievalAfter} /></label>
                <label className="shared-retrieval-date">to<Input aria-label="Updated on or before" onChange={(event) => setRetrievalBefore(event.target.value)} type="date" value={retrievalBefore} /></label>
                <Dropdown aria-label="Number of results" onValueChange={setRetrievalLimit} options={[{ label: "10 results", value: "10" }, { label: "20 results", value: "20" }, { label: "50 results", value: "50" }]} value={retrievalLimit} />
                <Button disabled={isSubmitting} type="submit" variant="main"><Search size={16} /> Search</Button>
                {results.length ? <Button disabled={isSubmitting} onClick={clearRetrievalFilters} type="button" variant="secondary">Clear</Button> : null}
              </div>
            </form>
            <section className="shared-symbol-search"><div className="shared-panel-heading"><div><Code size={17} /><h3>Code symbols</h3></div></div>
              <form className="shared-retrieval-search" onSubmit={runSymbolSearch}><Input aria-label="Function, class or method name" onChange={(event) => setSymbolQuery(event.target.value)} placeholder="Function, class or method name" value={symbolQuery} /><Button disabled={isSubmitting} type="submit" variant="secondary">Find definition</Button></form>
              {symbolResults ? (symbolResults.length ? <div className="shared-search-results">{symbolResults.map((hit) => <article key={hit.symbol.id}><Button className="shared-search-result" onClick={() => onOpenArtifact(hit.symbol.artifact_id)} type="button" variant="secondary"><span><strong>{hit.symbol.name}</strong> · {hit.symbol.kind}</span><span>{hit.path}{hit.symbol.start_line ? `:${hit.symbol.start_line}` : ""}</span></Button></article>)}</div> : <p className="shared-muted-copy">No indexed code defines a symbol named like that.</p>) : null}
            </section>
            <section className="shared-saved-searches"><div className="shared-panel-heading"><div><Book size={17} /><h3>Saved searches</h3></div><span>{savedSearches.length}</span></div>{savedSearches.length ? <div className="shared-saved-search-list">{savedSearches.map((saved) => <article key={saved.id}><Button onClick={() => applySavedSearch(saved)} type="button" variant="secondary"><span><strong>{saved.name}</strong><small>{saved.query}</small></span></Button>{canWrite ? <Button aria-label={`Delete ${saved.name}`} disabled={isSubmitting} onClick={() => void removeSavedSearch(saved)} type="button" variant="secondary"><Trash size={14} /></Button> : null}</article>)}</div> : <p className="shared-muted-copy">Save a query and its filters for recurring investigations.</p>}{canWrite ? <form className="shared-save-search-form" onSubmit={saveCurrentSearch}><Input onChange={(event) => setSavedSearchName(event.target.value)} placeholder="Search name" required value={savedSearchName} /><Button disabled={isSubmitting || !query.trim()} type="submit" variant="secondary">Save current search</Button></form> : null}</section>
            <div>{results.length ? <div className="shared-search-results"><p className="shared-search-result-count">{results.length} matching evidence {results.length === 1 ? "result" : "results"}</p>{paginate(results, resultsPage).map((result) => <article key={result.chunk_id}><Button className="shared-search-result" onClick={() => onOpenArtifact(result.artifact_id)} type="button" variant="secondary"><span className="shared-search-result-heading"><strong>{result.title}</strong><span>{artifactTypeLabel(result.artifact_type)} · {result.language ?? "Unspecified"} · {result.source_name}</span></span><p>{renderHighlightedSnippet(result.snippet)}</p><span className="shared-search-result-path">{result.path}{citationLines(result)}{result.last_author ? ` · last changed by ${lastChangeLabel(result.last_author, result.last_changed_at)}` : ""}</span></Button></article>)}<Pagination onPageChange={setResultsPage} page={resultsPage} total={results.length} /></div> : <p className="shared-muted-copy">Index one or more artifacts, then search the evidence base from here. Use filters to narrow large workspaces.</p>}
              <section className="shared-ask-evidence" aria-live="polite"><div><Brain size={18} /><h3>Ask your evidence</h3></div><p>Get a citation-backed answer from the indexed workspace context.</p><form onSubmit={askEvidence}><Textarea onChange={(event) => setAskQuestion(event.target.value)} placeholder="What do we know about the current implementation?" required value={askQuestion} /><Button disabled={isSubmitting} type="submit" variant="secondary">{isSubmitting ? <Loader className="spin" size={16} /> : <Brain size={16} />} Ask</Button></form>{askAnswer ? <div className="shared-ask-answer"><p className="shared-ai-generated-note">AI-generated from the cited passages · verify against the sources</p><div className="shared-markdown shared-ai-answer"><SafeMarkdown>{askAnswer.answer_markdown}</SafeMarkdown></div>{askAnswer.warnings.length ? <div className="shared-ai-overview-warning">{askAnswer.warnings.map((warning) => <p key={warning}>{warning}</p>)}</div> : null}{askAnswer.citations.length ? <EvidenceUsed accessToken={accessToken} citations={askAnswer.citations} numbered onOpenArtifact={onOpenArtifact} /> : null}{canWrite && askAnswer.citations.length ? <div className="shared-ai-actions"><Button disabled={isSubmitting} onClick={() => draftMemoryFromAnswer(askedQuestion || "Answer from workspace evidence", askAnswer.answer_markdown, "Ask your evidence", askAnswer.citations)} type="button" variant="secondary"><Shield size={16} /> Save as memory</Button></div> : null}</div> : null}</section>
            </div></>
  );
}
