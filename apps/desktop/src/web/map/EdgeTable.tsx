import type { KnowledgeEdge, KnowledgeNode } from "../../types";
import { Table } from "../../components/ui/table";

export function EdgeTable({ edges, nodeById, onSelect }: { edges: KnowledgeEdge[]; nodeById: Map<string, KnowledgeNode>; onSelect: (id: string) => void }) {
  const rows = [...edges].sort((a, b) => a.kind === b.kind ? b.weight - a.weight : a.kind === "similar" ? -1 : 1);
  if (!rows.length) return <p className="shared-muted-copy">No links to list with the current filters.</p>;
  return <Table>
    
      <caption className="sr-only">Links between files and memory cards</caption>
      <thead><tr><th scope="col">From</th><th scope="col">To</th><th scope="col">Link</th><th scope="col">Strength</th></tr></thead>
      <tbody>{rows.map((edge) => <tr key={`${edge.kind}-${edge.source}-${edge.target}`}>
        <td><button className="rm-map-link" onClick={() => onSelect(edge.source)} type="button">{nodeById.get(edge.source)?.title ?? "Unknown"}</button></td>
        <td><button className="rm-map-link" onClick={() => onSelect(edge.target)} type="button">{nodeById.get(edge.target)?.title ?? "Unknown"}</button></td>
        <td>{edge.kind === "similar" ? "Similar content" : "Memory cites file"}</td>
        <td>{edge.kind === "similar" ? `${Math.round(edge.weight * 100)}%` : "—"}</td>
      </tr>)}</tbody>
    </Table>;
}
