import type { KnowledgeEdge, KnowledgeNode } from "../../types";
import { Table } from "../../components/ui/table";
import { orderLinks } from "./emphasis";

export function EdgeTable({ edges, first, nodeById, onSelect }: { edges: KnowledgeEdge[]; first?: string[]; nodeById: Map<string, KnowledgeNode>; onSelect: (id: string) => void }) {
  // Strongest first inside a kind; the kinds in the template's order (similar files before memory citations otherwise).
  const rows = orderLinks([...edges].sort((a, b) => b.weight - a.weight), first);
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
