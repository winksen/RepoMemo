import { type FormEvent, useState } from "react";
import { IconLoader2 as Loader, IconNotebook as Notebook, IconSearch as Search } from "@tabler/icons-react";
import { searchAllWorkspaces } from "../lib/sharedApi";
import { type ArtifactType, type CrossSearchResponse } from "../types";
import { Button } from "../components/ui/button";
import { Checkbox } from "../components/ui/checkbox";
import { Dropdown } from "../components/ui/dropdown";
import { Field } from "../components/ui/field";
import { PanelHeading } from "../components/ui/panel-heading";
import { Input } from "../components/ui/input";
import { showToast } from "../components/ui/toast";
import { SearchHitCard } from "./SearchHitCard";
import { apiMessage, artifactTypeLabel } from "./format";
import { navigate, routePath } from "./router";
import { ICON_SIZE } from "../components/ui/icon";

/** The kinds of file a search across workspaces can be kept to. */
export const CROSS_SEARCH_TYPES: ArtifactType[] = ["note", "markdown_doc", "code_file", "file", "image", "issue", "pr", "decision", "incident", "runbook", "api_spec", "repository"];

const ANY = "__any__";

/** One search over every workspace the account can read; each result names its workspace. */
export function CrossWorkspaceSearch({ accessToken }: { accessToken: string }) {
  const [query, setQuery] = useState("");
  const [type, setType] = useState(ANY);
  const [language, setLanguage] = useState("");
  const [after, setAfter] = useState("");
  const [before, setBefore] = useState("");
  const [withCards, setWithCards] = useState(true);
  const [searchedFor, setSearchedFor] = useState("");
  const [found, setFound] = useState<CrossSearchResponse | null>(null);
  const [isSearching, setIsSearching] = useState(false);

  // Memory cards are not files: a kind of file or a language leaves them out.
  const filesOnly = type !== ANY || language.trim() !== "";

  async function run(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const text = query.trim();
    if (!text) return;
    setIsSearching(true);
    try {
      setFound(await searchAllWorkspaces(accessToken, {
        query: text,
        artifactTypes: type === ANY ? [] : [type],
        languages: language.trim() ? [language.trim()] : [],
        modifiedAfter: after,
        modifiedBefore: before,
        memoryCards: withCards,
      }));
      setSearchedFor(text);
    } catch (error) { showToast("error", apiMessage(error)); } finally { setIsSearching(false); }
  }

  const notes = found?.workspaces.filter((workspace) => workspace.note) ?? [];
  const cards = found?.memory_cards ?? [];
  const total = (found?.results.length ?? 0) + cards.length;
  const places = found ? `${found.workspaces.length} ${found.workspaces.length === 1 ? "workspace" : "workspaces"}` : "";
  return <section aria-label="Search all workspaces" className="shared-cross-search">
    <PanelHeading icon={Search} title="Search all workspaces" />
    <form className="shared-retrieval-search" onSubmit={(event) => void run(event)}>
      <div className="shared-retrieval-filter-row">
        <Input aria-label="Search every workspace you can read" onChange={(event) => setQuery(event.target.value)} placeholder="Search the evidence and memory of every workspace you can read" value={query} />
        <Button disabled={isSearching || !query.trim()} type="submit" variant="main">{isSearching ? <Loader className="spin" size={ICON_SIZE.md} /> : <Search size={ICON_SIZE.md} />} Search</Button>
      </div>
      <div className="shared-retrieval-filter-row">
        <Field label="Kind of file"><Dropdown onValueChange={setType} options={[{ label: "Any kind", value: ANY }, ...CROSS_SEARCH_TYPES.map((value) => ({ label: artifactTypeLabel(value), value }))]} value={type} /></Field>
        <Field label="Language"><Input onChange={(event) => setLanguage(event.target.value)} placeholder="Any, such as Rust" value={language} /></Field>
        <Field label="Updated from"><Input max={before || undefined} onChange={(event) => setAfter(event.target.value)} type="date" value={after} /></Field>
        <Field label="Updated to"><Input min={after || undefined} onChange={(event) => setBefore(event.target.value)} type="date" value={before} /></Field>
      </div>
      <Checkbox checked={withCards && !filesOnly} description={filesOnly ? "Cards are not files: they are left out while a kind of file or a language is chosen." : undefined} disabled={filesOnly} label="Include memory cards" onCheckedChange={setWithCards} />
    </form>
    {isSearching ? <p className="shared-search-result-count" role="status">Searching all workspaces… {found ? `${total} previous results shown` : ""}</p> : null}
    {found ? <div className="shared-search-results" aria-live="polite">
      <p className="shared-search-result-count">{total ? `${found.results.length} ${found.results.length === 1 ? "result" : "results"}${cards.length ? ` and ${cards.length} memory ${cards.length === 1 ? "card" : "cards"}` : ""} for “${searchedFor}” in ${places}` : `Nothing matches “${searchedFor}” in ${places}.`}{found.not_searched ? ` ${found.not_searched} more were not searched; open one to search it alone.` : ""}</p>
      {found.results.map(({ workspace_id, workspace_name, result }) => <article key={`${workspace_id}-${result.chunk_id}`}>
        <SearchHitCard onOpen={() => navigate(routePath({ name: "artifact", workspaceId: workspace_id, artifactId: result.artifact_id }))} query={searchedFor} result={result} workspaceName={workspace_name} />
      </article>)}
      {cards.length ? <ul aria-label="Memory cards found" className="shared-cross-cards">
        {cards.map(({ workspace_id, workspace_name, card }) => <li key={card.id}>
          <Button className="shared-search-result" onClick={() => navigate(routePath({ name: "memory-card", workspaceId: workspace_id, cardId: card.id }))} type="button" variant="secondary">
            <span className="shared-search-result-heading"><strong><Notebook size={ICON_SIZE.sm} /> {card.title}</strong><span>{workspace_name} · Memory card{card.needs_review ? " · Needs review" : ""}</span></span>
            <p>{card.body_excerpt}</p>
          </Button>
        </li>)}
      </ul> : null}
      {notes.map((workspace) => <p className="shared-muted-copy" key={workspace.workspace_id}><strong>{workspace.name}:</strong> {workspace.note}</p>)}
    </div> : null}
  </section>;
}
