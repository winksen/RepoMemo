import { IconBook2 as Book, IconExternalLink as ExternalLink } from "@tabler/icons-react";
import type { KnowledgeEdge, KnowledgeNode } from "../../types";
import { Button } from "../../components/ui/button";
import { ICON_SIZE } from "../../components/ui/icon";
import { GROUP_LABEL, fileGroup, percent } from "./shared";

export function SelectionPanel({ edges, nodeById, onOpenArtifact, onOpenMemoryCard, onSelect, selected, similarityAvailable }: { edges: KnowledgeEdge[]; nodeById: Map<string, KnowledgeNode>; onOpenArtifact: (artifactId: string) => void; onOpenMemoryCard: (cardId: string) => void; onSelect: (id: string) => void; selected: KnowledgeNode | null; similarityAvailable: boolean }) {
  if (!selected) {
    return <aside className="rm-map-selection"><p className="rm-map-muted">Select a dot to see what it is connected to. Use Tab and Enter to move through dots with the keyboard.</p></aside>;
  }
  const linked = (kind: KnowledgeEdge["kind"]) => edges
    .filter((edge) => edge.kind === kind && (edge.source === selected.id || edge.target === selected.id))
    .map((edge) => ({ node: nodeById.get(edge.source === selected.id ? edge.target : edge.source), weight: edge.weight }))
    .filter((entry): entry is { node: KnowledgeNode; weight: number } => Boolean(entry.node))
    .sort((a, b) => b.weight - a.weight);
  const similar = linked("similar");
  const cites = linked("cites");

  if (selected.kind === "memory") {
    return <aside className="rm-map-selection">
      <span className="rm-map-stage-step">Memory card</span>
      <strong className="rm-map-selection-title">{selected.title}</strong>
      <Button onClick={() => onOpenMemoryCard(selected.id)} type="button" variant="secondary"><Book size={ICON_SIZE.sm} /> Open memory card</Button>
      <h3>Cites</h3>
      <ul>{cites.map(({ node }) => <li key={node.id}><button onClick={() => onSelect(node.id)} type="button">{node.title}</button></li>)}</ul>
    </aside>;
  }

  return <aside className="rm-map-selection">
    <span className="rm-map-stage-step">{GROUP_LABEL[fileGroup(selected)]}</span>
    <strong className="rm-map-selection-title">{selected.title}</strong>
    <span className="rm-map-selection-path">{selected.path}</span>
    <dl>
      <dt>Status</dt><dd>{selected.state === "indexed" ? "Indexed" : selected.state === "failed" ? "Indexing failed" : "Waiting to be indexed"}</dd>
      <dt>Passages</dt><dd>{selected.passage_count}</dd>
      {similarityAvailable ? <><dt>By meaning</dt><dd>{percent(selected.embedded_count, selected.passage_count)}% embedded</dd></> : null}
    </dl>
    <Button onClick={() => onOpenArtifact(selected.id)} type="button" variant="secondary"><ExternalLink size={ICON_SIZE.sm} /> Open file</Button>
    {similarityAvailable ? <>
      <h3>Closest by meaning</h3>
      {similar.length ? <ul>{similar.map(({ node, weight }) => <li key={node.id}><button onClick={() => onSelect(node.id)} type="button">{node.title}</button><span>{Math.round(weight * 100)}% similar</span></li>)}</ul> : <p className="rm-map-muted">No file is notably close to this one.</p>}
    </> : null}
    {cites.length ? <>
      <h3>Cited by memory</h3>
      <ul>{cites.map(({ node }) => <li key={node.id}><button onClick={() => onOpenMemoryCard(node.id)} type="button">{node.title}</button></li>)}</ul>
    </> : null}
  </aside>;
}
