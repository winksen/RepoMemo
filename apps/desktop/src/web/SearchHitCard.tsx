import type { SearchResult } from "../types";
import { Button } from "../components/ui/button";
import { renderHighlightedSnippet } from "./common";
import { artifactTypeLabel, citationLines, lastChangeLabel } from "./format";

const STATUS_LABELS: Record<string, string> = {
  active: "Current",
  verified: "Verified",
  needs_review: "Needs review",
  outdated: "Outdated",
  superseded: "Superseded",
};

export function SearchHitCard({ result, query, onOpen, workspaceName, typeLabels }: {
  result: SearchResult;
  query: string;
  onOpen: () => void;
  workspaceName?: string;
  typeLabels?: Record<string, string>;
}) {
  const match = result.match_kind === "keyword_and_semantic" ? "Keyword and semantic match"
    : result.match_kind === "semantic" ? "Semantic match"
    : result.match_kind === "keyword" ? `Keyword match: “${query.trim()}”` : "Match type unavailable";
  return <Button className="shared-search-result" onClick={onOpen} type="button" variant="secondary">
    <span className="shared-search-result-heading"><strong>{result.title}</strong>{result.status ? <span className={`shared-search-status shared-search-status-${result.status}`}>{STATUS_LABELS[result.status] ?? result.status}</span> : null}</span>
    <span className="shared-search-result-meta">{workspaceName ? `${workspaceName} · ` : ""}{artifactTypeLabel(result.artifact_type, typeLabels)} · {result.source_name}{result.last_author ? ` · last changed by ${lastChangeLabel(result.last_author, result.last_changed_at)}` : ""}</span>
    <span className="shared-search-result-path">{result.path}{citationLines(result)}</span>
    <span className="shared-search-match">{match}</span>
    <p className="shared-search-citation-preview">{renderHighlightedSnippet(result.snippet)}</p>
  </Button>;
}
