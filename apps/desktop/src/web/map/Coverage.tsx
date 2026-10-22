import type { MouseEvent as ReactMouseEvent, ReactNode } from "react";
import { useRef, useState } from "react";
import { IconAlertTriangle as AlertTriangle, IconFileText as FileText } from "@tabler/icons-react";
import type { KnowledgeMap } from "../../types";
import { Table } from "../../components/ui/table";
import { PanelHeading } from "../../components/ui/panel-heading";
import { ICON_SIZE } from "../../components/ui/icon";
import { percent, HoverTip } from "./shared";
import { orderCoverage } from "./emphasis";

export function Coverage({ first, map }: { first?: string[]; map: KnowledgeMap }) {
  const [tooltip, setTooltip] = useState<HoverTip | null>(null);
  const container = useRef<HTMLDivElement>(null);
  const embeddingOn = map.pipeline.embedded_count !== null;

  function hover(event: ReactMouseEvent, content: ReactNode) {
    const box = container.current?.getBoundingClientRect();
    if (box) setTooltip({ x: event.clientX - box.left, y: event.clientY - box.top, content });
  }

  return <section className="rm-map-section" aria-labelledby="rm-map-coverage-title">
    <PanelHeading icon={FileText} id="rm-map-coverage-title" title="Coverage by kind of file" />
    <div className="rm-map-legend" aria-label="Indexing states" role="group">
      <span><i className="rm-map-swatch indexed" /> Indexed</span>
      <span><i className="rm-map-swatch pending" /> Waiting</span>
      <span><i className="rm-map-swatch failed" /><AlertTriangle size={ICON_SIZE.sm} /> Failed</span>
    </div>
    {map.coverage.length ? <Table ref={container} onMouseLeave={() => setTooltip(null)} overlay={tooltip ? <div className="rm-map-tooltip" role="tooltip" style={{ left: tooltip.x, top: tooltip.y }}>{tooltip.content}</div> : null}>
      
        <thead><tr><th scope="col">Kind</th><th className="rm-table-number" scope="col">Files</th><th scope="col">Indexing</th><th className="rm-table-number" scope="col">Passages</th><th className="rm-table-number" scope="col">By meaning</th></tr></thead>
        <tbody>{orderCoverage(map.coverage, first).map((row) => {
          const segments = [
            { key: "indexed", count: row.indexed_count, label: "indexed" },
            { key: "pending", count: row.pending_count, label: "waiting" },
            { key: "failed", count: row.failed_count, label: "failed" },
          ].filter((segment) => segment.count > 0);
          return <tr key={row.label}>
            <th scope="row">{row.label}</th>
            <td className="rm-table-number">{row.file_count.toLocaleString()}</td>
            <td>
              <div className="rm-map-stack">{segments.map((segment) => <span
                className={`rm-map-segment ${segment.key}`}
                key={segment.key}
                onMouseMove={(event) => hover(event, <><strong>{row.label}</strong><span>{segment.count} of {row.file_count} files {segment.label}</span></>)}
                style={{ flexGrow: segment.count }}
              />)}</div>
              <span className="rm-map-stack-caption">{row.indexed_count} of {row.file_count} indexed{row.failed_count ? ` · ${row.failed_count} failed` : ""}</span>
            </td>
            <td className="rm-table-number">{row.passage_count.toLocaleString()}</td>
            <td className="rm-table-number">{embeddingOn ? `${percent(row.embedded_count, row.passage_count)}%` : <span className="rm-map-muted">Off</span>}</td>
          </tr>;
        })}</tbody>
      </Table> : <p className="shared-muted-copy">Add files in Evidence or Documents to see coverage.</p>}
  </section>;
}
