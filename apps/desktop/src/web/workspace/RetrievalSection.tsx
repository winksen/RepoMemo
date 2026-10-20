import { type FormEvent, useEffect } from "react";
import { SafeMarkdown } from "../../components/SafeMarkdown";
import { IconBook2 as Book, IconBrain as Brain, IconCode as Code, IconSearch as Search, IconShieldLock as Shield, IconTrash as Trash } from "@tabler/icons-react";
import { askSharedWorkspace, createSharedSavedSearch, deleteSharedSavedSearch, searchSharedSymbols, searchSharedWorkspace } from "../../lib/sharedApi";
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
import { useFolders, useRetrievalFacets, useSavedSearches } from "./data";
import { type WorkspacePage, sectionState } from "./page";
import { ICON_SIZE } from "../../components/ui/icon";

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
            <><PanelHeading icon={Search} title="Retrieve" />
            <form className="shared-retrieval-search" onSubmit={runSearch}>
              <SearchField aria-label="Search indexed evidence" onValueChange={setQuery} placeholder="Search indexed evidence" value={query} />
              <div className="shared-retrieval-filter-row">
                <Field label="File type">{(control) => <Dropdown id={control.id} onValueChange={(value) => setRetrievalArtifactType(value as ArtifactType | "all")} options={[{ label: "All file types", value: "all" }, ...(retrievalFacets?.artifact_types ?? []).map((type) => ({ label: artifactTypeLabel(type), value: type }))]} value={retrievalArtifactType} />}</Field>
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
              {symbolResults ? (symbolResults.length ? <div className="shared-search-results">{symbolResults.map((hit) => <article key={hit.symbol.id}><Button className="shared-search-result" onClick={() => onOpenArtifact(hit.symbol.artifact_id)} type="button" variant="secondary"><span><strong>{hit.symbol.name}</strong> · {hit.symbol.kind}</span><span>{hit.path}{hit.symbol.start_line ? `:${hit.symbol.start_line}` : ""}</span></Button></article>)}</div> : <EmptyState description="No indexed code defines a symbol named like that." title="No symbol found" />) : null}
            </section>
            <section className="shared-saved-searches"><PanelHeading description={savedSearches.length} icon={Book} level={3} title="Saved searches" />{savedSearches.length ? <div className="shared-saved-search-list">{savedSearches.map((saved) => <article key={saved.id}><Button onClick={() => applySavedSearch(saved)} type="button" variant="secondary"><span><strong>{saved.name}</strong><small>{saved.query}</small></span></Button>{canWrite ? <IconButton disabled={isSubmitting} label={`Delete ${saved.name}`} onClick={() => void removeSavedSearch(saved)}><Trash size={ICON_SIZE.sm} /></IconButton> : null}</article>)}</div> : <EmptyState description="Save a query and its filters for recurring investigations." title="No saved searches" />}{canWrite ? <form className="shared-save-search-form" onSubmit={saveCurrentSearch}><Field label="Name for this search" required><Input onChange={(event) => setSavedSearchName(event.target.value)} placeholder="Search name" value={savedSearchName} /></Field><Button busy={isSubmitting} disabled={!query.trim()} type="submit" variant="secondary">Save current search</Button></form> : null}</section>
            <div>{results.length ? <div className="shared-search-results"><p className="shared-search-result-count">{results.length} matching evidence {results.length === 1 ? "result" : "results"}</p>{paginate(results, resultsPage).map((result) => <article key={result.chunk_id}><Button className="shared-search-result" onClick={() => onOpenArtifact(result.artifact_id)} type="button" variant="secondary"><span className="shared-search-result-heading"><strong>{result.title}</strong><span>{artifactTypeLabel(result.artifact_type)} · {result.language ?? "Unspecified"} · {result.source_name}</span></span><p>{renderHighlightedSnippet(result.snippet)}</p><span className="shared-search-result-path">{result.path}{citationLines(result)}{result.last_author ? ` · last changed by ${lastChangeLabel(result.last_author, result.last_changed_at)}` : ""}</span></Button></article>)}<Pagination onPageChange={setResultsPage} page={resultsPage} total={results.length} /></div> : <EmptyState description="Index one or more artifacts, then search the evidence base from here. Use filters to narrow large workspaces." title="Search the evidence base" />}
              <section className="shared-ask-evidence" aria-live="polite"><div><Brain size={ICON_SIZE.lg} /><h3>Ask your evidence</h3></div><p>Get a citation-backed answer from the indexed workspace context.</p><form onSubmit={askEvidence}><Field label="Question about the evidence" required><Textarea onChange={(event) => setAskQuestion(event.target.value)} placeholder="What do we know about the current implementation?" value={askQuestion} /></Field><Button busy={isSubmitting} type="submit" variant="secondary"><Brain size={ICON_SIZE.md} /> Ask</Button></form>{askAnswer ? <div className="shared-ask-answer"><p className="shared-ai-generated-note">AI-generated from the cited passages · verify against the sources</p><div className="shared-markdown shared-ai-answer"><SafeMarkdown>{askAnswer.answer_markdown}</SafeMarkdown></div>{askAnswer.warnings.length ? <div className="shared-ai-overview-warning">{askAnswer.warnings.map((warning) => <p key={warning}>{warning}</p>)}</div> : null}{askAnswer.citations.length ? <EvidenceUsed accessToken={accessToken} citations={askAnswer.citations} numbered onOpenArtifact={onOpenArtifact} /> : null}{canWrite && askAnswer.citations.length ? <div className="shared-ai-actions"><Button disabled={isSubmitting} onClick={() => draftMemoryFromAnswer(askedQuestion || "Answer from workspace evidence", askAnswer.answer_markdown, "Ask your evidence", askAnswer.citations)} type="button" variant="secondary"><Shield size={ICON_SIZE.md} /> Save as memory</Button></div> : null}</div> : null}</section>
            </div></>
  );
}
