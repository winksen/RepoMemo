import { type FormEvent, useState } from "react";
import { IconLoader2 as Loader, IconSearch as Search } from "@tabler/icons-react";
import { searchAllWorkspaces } from "../lib/sharedApi";
import { type CrossSearchResponse } from "../types";
import { Button } from "../components/ui/button";
import { Input } from "../components/ui/input";
import { showToast } from "../components/ui/toast";
import { renderHighlightedSnippet } from "./common";
import { apiMessage, artifactTypeLabel } from "./format";
import { navigate, routePath } from "./router";
import { ICON_SIZE } from "../components/ui/icon";

/** One search over every workspace the account can read; each result names its workspace. */
export function CrossWorkspaceSearch({ accessToken }: { accessToken: string }) {
  const [query, setQuery] = useState("");
  const [searchedFor, setSearchedFor] = useState("");
  const [found, setFound] = useState<CrossSearchResponse | null>(null);
  const [isSearching, setIsSearching] = useState(false);

  async function run(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const text = query.trim();
    if (!text) return;
    setIsSearching(true);
    try {
      setFound(await searchAllWorkspaces(accessToken, { query: text }));
      setSearchedFor(text);
    } catch (error) { showToast("error", apiMessage(error)); } finally { setIsSearching(false); }
  }

  const notes = found?.workspaces.filter((workspace) => workspace.note) ?? [];
  return <section aria-label="Search all workspaces" className="shared-cross-search">
    <div className="shared-panel-heading"><div><Search size={ICON_SIZE.lg} /><h2>Search all workspaces</h2></div></div>
    <form className="shared-retrieval-search" onSubmit={(event) => void run(event)}>
      <div className="shared-retrieval-filter-row">
        <Input aria-label="Search every workspace you can read" onChange={(event) => setQuery(event.target.value)} placeholder="Search the evidence of every workspace you can read" value={query} />
        <Button disabled={isSearching || !query.trim()} type="submit" variant="main">{isSearching ? <Loader className="spin" size={ICON_SIZE.md} /> : <Search size={ICON_SIZE.md} />} Search</Button>
      </div>
    </form>
    {found ? <div className="shared-search-results" aria-live="polite">
      <p className="shared-search-result-count">{found.results.length ? `${found.results.length} ${found.results.length === 1 ? "result" : "results"} for “${searchedFor}” in ${found.workspaces.length} ${found.workspaces.length === 1 ? "workspace" : "workspaces"}` : `Nothing matches “${searchedFor}” in ${found.workspaces.length} ${found.workspaces.length === 1 ? "workspace" : "workspaces"}.`}{found.not_searched ? ` ${found.not_searched} more were not searched; open one to search it alone.` : ""}</p>
      {found.results.map(({ workspace_id, workspace_name, result }) => <article key={`${workspace_id}-${result.chunk_id}`}>
        <Button className="shared-search-result" onClick={() => navigate(routePath({ name: "artifact", workspaceId: workspace_id, artifactId: result.artifact_id }))} type="button" variant="secondary">
          <span className="shared-search-result-heading"><strong>{result.title}</strong><span>{workspace_name} · {artifactTypeLabel(result.artifact_type)} · {result.source_name}</span></span>
          <p>{renderHighlightedSnippet(result.snippet)}</p>
          <span className="shared-search-result-path">{result.path}{result.start_line ? ` · line ${result.start_line}` : ""}</span>
        </Button>
      </article>)}
      {notes.map((workspace) => <p className="shared-muted-copy" key={workspace.workspace_id}><strong>{workspace.name}:</strong> {workspace.note}</p>)}
    </div> : null}
  </section>;
}
