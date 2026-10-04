import type { MouseEvent as ReactMouseEvent, PointerEvent as ReactPointerEvent, ReactNode } from "react";
import { useEffect, useMemo, useRef, useState } from "react";
import {
  IconAlertTriangle as AlertTriangle,
  IconArrowRight as ArrowRight,
  IconBook2 as Book,
  IconChartDots3 as Graph,
  IconExternalLink as ExternalLink,
  IconFileText as FileText,
  IconHourglass as Hourglass,
  IconList as List,
  IconMinus as Minus,
  IconPlus as Plus,
  IconFocusCentered as Recenter,
  IconRefresh as Refresh,
  IconSearch as Search,
  IconStack2 as Layers,
} from "@tabler/icons-react";
import { getSharedKnowledgeMap } from "../lib/sharedApi";
import { layoutGraph } from "../lib/forceLayout";
import type { KnowledgeEdge, KnowledgeMap, KnowledgeNode } from "../types";
import { Button } from "./ui/button";
import { showToast } from "./ui/toast";

/** Files are coloured by one of three groups: past three hues, dots that can
 *  sit next to any other dot stop being distinguishable for colour-blind
 *  readers. The palette is validated in both themes (see the CSS tokens). */
type FileGroup = "documents" | "notes" | "code";

const GROUP_LABEL: Record<FileGroup, string> = {
  documents: "Documents & images",
  notes: "Notes & text",
  code: "Code & config",
};

const CANVAS_WIDTH = 900;
const CANVAS_HEIGHT = 520;
const POLL_INTERVAL_MS = 5000;
const POLL_ATTEMPTS = 120;
/** Largest files labelled on the canvas when nothing is selected. */
const LABELLED_FILES = 5;

function fileGroup(node: KnowledgeNode): FileGroup {
  const extension = node.path?.split(".").pop()?.toLowerCase() ?? "";
  if (node.artifact_type === "code_file" || node.artifact_type === "api_spec" || ["json", "toml", "yaml", "yml"].includes(extension)) return "code";
  if (node.artifact_type === "file" && !["txt", "log"].includes(extension)) return "documents";
  if (node.artifact_type === "image") return "documents";
  return "notes";
}

function percent(part: number, whole: number) {
  return whole > 0 ? Math.round((part / whole) * 100) : 0;
}

function shorten(text: string, length = 28) {
  return text.length > length ? `${text.slice(0, length - 1)}…` : text;
}

type Tooltip = { x: number; y: number; content: ReactNode };

/** The knowledge map: indexing and embedding progress, coverage per kind of
 *  file, and a graph of how files relate by meaning and through memory. */
export function KnowledgeMapPanel({
  accessToken,
  canConfigure,
  onOpenArtifact,
  onOpenEvidence,
  onOpenMemoryCard,
  onOpenSettings,
  workspaceId,
}: {
  accessToken: string;
  canConfigure: boolean;
  onOpenArtifact: (artifactId: string) => void;
  onOpenEvidence: () => void;
  onOpenMemoryCard: (cardId: string) => void;
  onOpenSettings: () => void;
  workspaceId: string;
}) {
  const [map, setMap] = useState<KnowledgeMap | null>(null);
  const [isRefreshing, setIsRefreshing] = useState(false);

  async function load() {
    setIsRefreshing(true);
    try {
      setMap(await getSharedKnowledgeMap(accessToken, workspaceId));
    } catch (error) {
      showToast("error", error instanceof Error ? error.message : "The knowledge map could not be loaded.");
    } finally {
      setIsRefreshing(false);
    }
  }

  useEffect(() => { setMap(null); void load(); }, [accessToken, workspaceId]);

  // Indexing and embedding run in the background; keep the picture current
  // while they do, holding the previous render instead of flashing.
  const inProgress = Boolean(map && (map.pipeline.pending_count > 0 || (map.pipeline.embedded_count !== null && map.pipeline.embedded_count < map.pipeline.passage_count)));
  useEffect(() => {
    if (!inProgress) return;
    let attempts = 0;
    const timer = window.setInterval(() => {
      attempts += 1;
      void getSharedKnowledgeMap(accessToken, workspaceId).then(setMap).catch(() => undefined);
      if (attempts >= POLL_ATTEMPTS) window.clearInterval(timer);
    }, POLL_INTERVAL_MS);
    return () => window.clearInterval(timer);
  }, [accessToken, inProgress, workspaceId]);

  if (!map) {
    return <section className="rm-map" aria-busy="true"><p className="shared-muted-copy">Loading the knowledge map…</p></section>;
  }

  return <section className="rm-map" aria-busy={isRefreshing}>
    <div className="rm-map-toolbar">
      <span>{inProgress ? "Updating while indexing and embedding run…" : "Up to date"}</span>
      <Button disabled={isRefreshing} onClick={() => void load()} type="button" variant="secondary"><Refresh size={15} /> Refresh</Button>
    </div>
    <Pipeline canConfigure={canConfigure} map={map} onOpenEvidence={onOpenEvidence} onOpenSettings={onOpenSettings} />
    <Coverage map={map} />
    <RelationGraph canConfigure={canConfigure} map={map} onOpenArtifact={onOpenArtifact} onOpenMemoryCard={onOpenMemoryCard} onOpenSettings={onOpenSettings} />
  </section>;
}

function Pipeline({ canConfigure, map, onOpenEvidence, onOpenSettings }: { canConfigure: boolean; map: KnowledgeMap; onOpenEvidence: () => void; onOpenSettings: () => void }) {
  const { pipeline } = map;
  const indexedShare = percent(pipeline.indexed_count, pipeline.file_count);
  const embeddedShare = pipeline.embedded_count === null ? null : percent(pipeline.embedded_count, pipeline.passage_count);
  return <section className="rm-map-section" aria-labelledby="rm-map-pipeline-title">
    <div className="shared-panel-heading"><div><Layers size={18} /><h2 id="rm-map-pipeline-title">From files to searchable knowledge</h2></div></div>
    <p className="rm-map-intro">Every file goes through these steps. Search works after step 2; search by meaning needs step 4.</p>
    <ol className="rm-map-stages">
      <li>
        <span className="rm-map-stage-step">1 · Stored</span>
        <strong>{pipeline.file_count.toLocaleString()}</strong>
        <span>{pipeline.file_count === 1 ? "file" : "files"} in the workspace</span>
      </li>
      <li aria-hidden="true" className="rm-map-stage-arrow"><ArrowRight size={16} /></li>
      <li>
        <span className="rm-map-stage-step">2 · Indexed</span>
        <strong>{indexedShare}%</strong>
        <span>{pipeline.indexed_count.toLocaleString()} of {pipeline.file_count.toLocaleString()} files read and split</span>
        <Meter value={indexedShare} label="Files indexed" />
        {pipeline.pending_count || pipeline.failed_count ? <div className="rm-map-stage-issues">
          {pipeline.pending_count ? <span><Hourglass size={14} /> {pipeline.pending_count} waiting</span> : null}
          {pipeline.failed_count ? <button className="rm-map-failed" onClick={onOpenEvidence} type="button"><AlertTriangle size={14} /> {pipeline.failed_count} failed · see Evidence</button> : null}
        </div> : null}
      </li>
      <li aria-hidden="true" className="rm-map-stage-arrow"><ArrowRight size={16} /></li>
      <li>
        <span className="rm-map-stage-step">3 · Passages</span>
        <strong>{pipeline.passage_count.toLocaleString()}</strong>
        <span>searchable pieces of text (sections or ~100 lines of code)</span>
      </li>
      <li aria-hidden="true" className="rm-map-stage-arrow"><ArrowRight size={16} /></li>
      <li>
        <span className="rm-map-stage-step">4 · Searchable by meaning</span>
        {embeddedShare === null ? <>
          <strong>Off</strong>
          <span>No AI for search is set up, so search matches words only.</span>
          {canConfigure ? <Button onClick={onOpenSettings} type="button" variant="secondary"><Search size={15} /> Set up AI for search</Button> : <span className="rm-map-muted">An administrator can turn it on in Settings.</span>}
        </> : <>
          <strong>{embeddedShare}%</strong>
          <span>{pipeline.embedded_count!.toLocaleString()} of {pipeline.passage_count.toLocaleString()} passages embedded{pipeline.embedding_model ? ` with ${pipeline.embedding_model}` : ""}</span>
          <Meter value={embeddedShare} label="Passages embedded" />
        </>}
      </li>
    </ol>
  </section>;
}

function Meter({ label, value }: { label: string; value: number }) {
  return <div aria-label={`${label}: ${value}%`} aria-valuemax={100} aria-valuemin={0} aria-valuenow={value} className="rm-map-meter" role="meter"><span style={{ width: `${value}%` }} /></div>;
}

function Coverage({ map }: { map: KnowledgeMap }) {
  const [tooltip, setTooltip] = useState<Tooltip | null>(null);
  const container = useRef<HTMLDivElement>(null);
  const embeddingOn = map.pipeline.embedded_count !== null;

  function hover(event: ReactMouseEvent, content: ReactNode) {
    const box = container.current?.getBoundingClientRect();
    if (box) setTooltip({ x: event.clientX - box.left, y: event.clientY - box.top, content });
  }

  return <section className="rm-map-section" aria-labelledby="rm-map-coverage-title">
    <div className="shared-panel-heading"><div><FileText size={18} /><h2 id="rm-map-coverage-title">Coverage by kind of file</h2></div></div>
    <div className="rm-map-legend" aria-label="Indexing states">
      <span><i className="rm-map-swatch indexed" /> Indexed</span>
      <span><i className="rm-map-swatch pending" /> Waiting</span>
      <span><i className="rm-map-swatch failed" /><AlertTriangle size={13} /> Failed</span>
    </div>
    {map.coverage.length ? <div className="rm-map-coverage" ref={container} onMouseLeave={() => setTooltip(null)}>
      <table>
        <thead><tr><th scope="col">Kind</th><th scope="col">Files</th><th scope="col">Indexing</th><th scope="col">Passages</th><th scope="col">By meaning</th></tr></thead>
        <tbody>{map.coverage.map((row) => {
          const segments = [
            { key: "indexed", count: row.indexed_count, label: "indexed" },
            { key: "pending", count: row.pending_count, label: "waiting" },
            { key: "failed", count: row.failed_count, label: "failed" },
          ].filter((segment) => segment.count > 0);
          return <tr key={row.label}>
            <th scope="row">{row.label}</th>
            <td>{row.file_count.toLocaleString()}</td>
            <td>
              <div className="rm-map-stack">{segments.map((segment) => <span
                className={`rm-map-segment ${segment.key}`}
                key={segment.key}
                onMouseMove={(event) => hover(event, <><strong>{row.label}</strong><span>{segment.count} of {row.file_count} files {segment.label}</span></>)}
                style={{ flexGrow: segment.count }}
              />)}</div>
              <span className="rm-map-stack-caption">{row.indexed_count} of {row.file_count} indexed{row.failed_count ? ` · ${row.failed_count} failed` : ""}</span>
            </td>
            <td>{row.passage_count.toLocaleString()}</td>
            <td>{embeddingOn ? `${percent(row.embedded_count, row.passage_count)}%` : <span className="rm-map-muted">Off</span>}</td>
          </tr>;
        })}</tbody>
      </table>
      {tooltip ? <div className="rm-map-tooltip" role="tooltip" style={{ left: tooltip.x, top: tooltip.y }}>{tooltip.content}</div> : null}
    </div> : <p className="shared-muted-copy">Add files in Evidence or Documents to see coverage.</p>}
  </section>;
}

function RelationGraph({ canConfigure, map, onOpenArtifact, onOpenMemoryCard, onOpenSettings }: { canConfigure: boolean; map: KnowledgeMap; onOpenArtifact: (artifactId: string) => void; onOpenMemoryCard: (cardId: string) => void; onOpenSettings: () => void }) {
  const [showSimilar, setShowSimilar] = useState(true);
  const [showCites, setShowCites] = useState(true);
  const [view, setView] = useState<"graph" | "table">("graph");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [tooltip, setTooltip] = useState<Tooltip | null>(null);
  const canvas = useRef<HTMLDivElement>(null);
  const svgRef = useRef<SVGSVGElement>(null);
  const drag = useRef<{ x: number; y: number; moved: boolean; active: boolean } | null>(null);
  const [vp, setViewport] = useState({ k: 1, x: 0, y: 0 });
  const [hoveredId, setHoveredId] = useState<string | null>(null);

  const nodeById = useMemo(() => new Map(map.nodes.map((node) => [node.id, node])), [map.nodes]);
  const maxPassages = useMemo(() => Math.max(1, ...map.nodes.map((node) => node.passage_count)), [map.nodes]);
  const radius = (node: KnowledgeNode) => node.kind === "memory" ? 7 : 5 + 9 * Math.sqrt(node.passage_count / maxPassages);
  // Layout uses every link, so toggling a link type hides lines without
  // moving the dots the reader has already learned.
  const positions = useMemo(() => layoutGraph(
    map.nodes.map((node) => ({ id: node.id, radius: radius(node) })),
    map.edges.map((edge) => ({ source: edge.source, target: edge.target, strength: edge.kind === "similar" ? Math.max(edge.weight, 0.2) : 0.6 })),
    CANVAS_WIDTH,
    CANVAS_HEIGHT,
  ), [map.nodes, map.edges]);
  const edges = map.edges.filter((edge) => (edge.kind === "similar" ? showSimilar : showCites) && positions.has(edge.source) && positions.has(edge.target));
  const neighbours = useMemo(() => {
    if (!selectedId) return null;
    const ids = new Set([selectedId]);
    for (const edge of edges) {
      if (edge.source === selectedId) ids.add(edge.target);
      if (edge.target === selectedId) ids.add(edge.source);
    }
    return ids;
  }, [edges, selectedId]);
  const labelled = useMemo(() => new Set(map.nodes.filter((node) => node.kind === "file").sort((a, b) => b.passage_count - a.passage_count).slice(0, LABELLED_FILES).map((node) => node.id)), [map.nodes]);
  const similarWeights = map.edges.filter((edge) => edge.kind === "similar").map((edge) => edge.weight);
  const [minWeight, maxWeight] = similarWeights.length ? [Math.min(...similarWeights), Math.max(...similarWeights)] : [0, 1];
  const selected = selectedId ? nodeById.get(selectedId) ?? null : null;
  const fileCount = map.nodes.filter((node) => node.kind === "file").length;

  useEffect(() => { if (selectedId && !nodeById.has(selectedId)) setSelectedId(null); }, [nodeById, selectedId]);

  const vpRef = useRef(vp);
  vpRef.current = vp;
  const frame = useRef<number | null>(null);
  const stopAnimation = () => { if (frame.current !== null) { cancelAnimationFrame(frame.current); frame.current = null; } };
  useEffect(() => stopAnimation, []);

  /** Glide the viewport to a target: ease-out, interpolating the centre and
   *  the zoom on a log scale so zooming feels even. Instant for reduced motion. */
  function animateTo(target: { k: number; x: number; y: number }, instant = false) {
    stopAnimation();
    if (instant || window.matchMedia("(prefers-reduced-motion: reduce)").matches) { setViewport(target); return; }
    const from = vpRef.current;
    const fromCx = from.x + CANVAS_WIDTH / from.k / 2, fromCy = from.y + CANVAS_HEIGHT / from.k / 2;
    const toCx = target.x + CANVAS_WIDTH / target.k / 2, toCy = target.y + CANVAS_HEIGHT / target.k / 2;
    const start = performance.now();
    const duration = 550;
    const step = (now: number) => {
      const t = Math.min(1, (now - start) / duration);
      const e = 1 - Math.pow(1 - t, 3);
      const k = from.k * Math.pow(target.k / from.k, e);
      const cx = fromCx + (toCx - fromCx) * e, cy = fromCy + (toCy - fromCy) * e;
      setViewport({ k, x: cx - CANVAS_WIDTH / k / 2, y: cy - CANVAS_HEIGHT / k / 2 });
      frame.current = t < 1 ? requestAnimationFrame(step) : null;
    };
    frame.current = requestAnimationFrame(step);
  }

  /** Frame every dot with a little breathing room. */
  function fitView(instant = false) {
    const points = [...positions.values()];
    if (!points.length) { animateTo({ k: 1, x: 0, y: 0 }, instant); return; }
    const pad = 60;
    const minX = Math.min(...points.map((p) => p.x)) - pad, maxX = Math.max(...points.map((p) => p.x)) + pad;
    const minY = Math.min(...points.map((p) => p.y)) - pad, maxY = Math.max(...points.map((p) => p.y)) + pad;
    const k = Math.min(CANVAS_WIDTH / (maxX - minX), CANVAS_HEIGHT / (maxY - minY), 3);
    const w = CANVAS_WIDTH / k, h = CANVAS_HEIGHT / k;
    animateTo({ k, x: (minX + maxX) / 2 - w / 2, y: (minY + maxY) / 2 - h / 2 }, instant);
  }
  useEffect(() => { fitView(true); }, [positions]);

  // Selecting a dot, from the canvas, the side panel or the table, glides to it.
  useEffect(() => {
    const point = selectedId ? positions.get(selectedId) : null;
    if (!point) return;
    const k = Math.min(Math.max(vpRef.current.k, 2.2), 4);
    animateTo({ k, x: point.x - CANVAS_WIDTH / k / 2, y: point.y - CANVAS_HEIGHT / k / 2 });
  }, [selectedId]);

  /** Zoom by a factor, keeping the point under (fx, fy) (0–1 of the canvas) still. */
  function zoomBy(factor: number, fx = 0.5, fy = 0.5) {
    stopAnimation();
    setViewport((current) => {
      const k = Math.min(Math.max(current.k * factor, 0.4), 8);
      const px = current.x + (CANVAS_WIDTH / current.k) * fx;
      const py = current.y + (CANVAS_HEIGHT / current.k) * fy;
      return { k, x: px - (CANVAS_WIDTH / k) * fx, y: py - (CANVAS_HEIGHT / k) * fy };
    });
  }

  // React attaches wheel listeners as passive; zooming needs preventDefault so
  // the page does not scroll underneath the map.
  useEffect(() => {
    const svg = svgRef.current;
    if (!svg) return;
    const onWheel = (event: WheelEvent) => {
      event.preventDefault();
      const box = svg.getBoundingClientRect();
      zoomBy(Math.exp(-event.deltaY * 0.0015), (event.clientX - box.left) / box.width, (event.clientY - box.top) / box.height);
    };
    svg.addEventListener("wheel", onWheel, { passive: false });
    return () => svg.removeEventListener("wheel", onWheel);
  }, [view, map.nodes.length]);

  function onPointerDown(event: ReactPointerEvent<SVGSVGElement>) {
    drag.current = { x: event.clientX, y: event.clientY, moved: false, active: true };
  }
  function onPointerMove(event: ReactPointerEvent<SVGSVGElement>) {
    const state = drag.current;
    if (!state?.active) return;
    const box = event.currentTarget.getBoundingClientRect();
    const dx = event.clientX - state.x, dy = event.clientY - state.y;
    if (!state.moved && Math.hypot(dx, dy) < 4) return;
    if (!state.moved) { state.moved = true; stopAnimation(); setTooltip(null); event.currentTarget.setPointerCapture(event.pointerId); }
    state.x = event.clientX; state.y = event.clientY;
    setViewport((current) => ({ ...current, x: current.x - (dx / box.width) * (CANVAS_WIDTH / current.k), y: current.y - (dy / box.height) * (CANVAS_HEIGHT / current.k) }));
  }
  function onPointerUp() {
    if (drag.current) drag.current.active = false;
  }
  const wasDragged = () => Boolean(drag.current?.moved);

  function hover(event: ReactMouseEvent, node: KnowledgeNode) {
    const box = canvas.current?.getBoundingClientRect();
    if (!box) return;
    setTooltip({
      x: event.clientX - box.left,
      y: event.clientY - box.top,
      content: node.kind === "memory"
        ? <><strong>{node.title}</strong><span>Memory card</span></>
        : <><strong>{node.title}</strong><span>{GROUP_LABEL[fileGroup(node)]} · {node.passage_count} passages</span>{map.pipeline.embedded_count !== null ? <span>{percent(node.embedded_count, node.passage_count)}% searchable by meaning</span> : null}{node.state !== "indexed" ? <span>{node.state === "failed" ? "Indexing failed" : "Waiting to be indexed"}</span> : null}</>,
    });
  }

  function similarityOpacity(edge: KnowledgeEdge) {
    if (edge.kind === "cites") return 0.7;
    const span = maxWeight - minWeight;
    return 0.25 + 0.55 * (span > 0 ? (edge.weight - minWeight) / span : 1);
  }

  return <section className="rm-map-section" aria-labelledby="rm-map-graph-title">
    <div className="shared-panel-heading"><div><Graph size={18} /><h2 id="rm-map-graph-title">How your content connects</h2></div><span>{fileCount} files · {edges.length} links</span></div>
    <p className="rm-map-intro">Each dot is a file; bigger dots hold more passages. A line joins files whose content means similar things, or a memory card (◆) to the files it cites.</p>
    <div className="rm-map-filters">
      <label title={map.similarity_available ? undefined : "Needs AI for search"}><input checked={showSimilar && map.similarity_available} disabled={!map.similarity_available} onChange={(event) => setShowSimilar(event.target.checked)} type="checkbox" /> Similar content</label>
      <label><input checked={showCites} onChange={(event) => setShowCites(event.target.checked)} type="checkbox" /> Memory links</label>
      <div className="shared-artifact-view-switch" role="group" aria-label="Graph view">
        <Button aria-label="Graph view" aria-pressed={view === "graph"} className={view === "graph" ? "active" : ""} onClick={() => setView("graph")} type="button" variant="secondary"><Graph size={16} /></Button>
        <Button aria-label="Table view" aria-pressed={view === "table"} className={view === "table" ? "active" : ""} onClick={() => setView("table")} type="button" variant="secondary"><List size={16} /></Button>
      </div>
    </div>
    {!map.similarity_available ? <p className="rm-map-notice">Lines between similar files appear once AI for search is set up and passages are embedded.{canConfigure ? <> <button onClick={onOpenSettings} type="button">Open Settings</button></> : null}</p> : null}
    {map.hidden_file_count ? <p className="rm-map-notice">Showing the {fileCount} largest files; {map.hidden_file_count} smaller ones are left out to keep the map readable.</p> : null}
    <div className="rm-map-legend" aria-label="Graph legend">
      {(Object.keys(GROUP_LABEL) as FileGroup[]).map((group) => <span key={group}><i className={`rm-map-dot ${group}`} /> {GROUP_LABEL[group]}</span>)}
      <span><i className="rm-map-diamond" /> Memory card</span>
      <span><i className="rm-map-dot hollow" /> Not indexed yet</span>
    </div>
    {!map.nodes.length ? <div className="shared-empty-state"><Graph size={25} /><strong>Nothing to map yet</strong><span>Add files in Evidence or Documents; they appear here once stored.</span></div>
      : view === "table" ? <EdgeTable edges={edges} nodeById={nodeById} onSelect={(id) => { setSelectedId(id); setView("graph"); }} />
      : <div className="rm-map-graph">
        <div className="rm-map-canvas" ref={canvas} onMouseLeave={() => setTooltip(null)}>
          <div className="rm-map-zoom" role="group" aria-label="Zoom">
            <Button aria-label="Zoom in" onClick={() => zoomBy(1.4)} type="button" variant="secondary"><Plus size={15} /></Button>
            <Button aria-label="Zoom out" onClick={() => zoomBy(1 / 1.4)} type="button" variant="secondary"><Minus size={15} /></Button>
            <Button aria-label="Fit all dots" onClick={() => fitView()} type="button" variant="secondary"><Recenter size={15} /></Button>
          </div>
          <svg
            aria-label={`Graph of ${fileCount} files and ${edges.length} links. Scroll to zoom, drag to move. Use the table view for a text version.`}
            className={drag.current?.moved && drag.current.active ? "panning" : ""}
            onClick={() => { if (!wasDragged()) setSelectedId(null); }}
            onPointerCancel={onPointerUp}
            onPointerDown={onPointerDown}
            onPointerMove={onPointerMove}
            onPointerUp={onPointerUp}
            ref={svgRef}
            role="img"
            viewBox={`${vp.x} ${vp.y} ${CANVAS_WIDTH / vp.k} ${CANVAS_HEIGHT / vp.k}`}
          >
            <g>{edges.map((edge) => {
              const from = positions.get(edge.source)!;
              const to = positions.get(edge.target)!;
              const active = !selectedId || edge.source === selectedId || edge.target === selectedId;
              return <line className={`rm-map-edge ${edge.kind}`} key={`${edge.kind}-${edge.source}-${edge.target}`} opacity={active ? similarityOpacity(edge) : 0.06} x1={from.x} x2={to.x} y1={from.y} y2={to.y} />;
            })}</g>
            <g>{map.nodes.map((node) => {
              const point = positions.get(node.id);
              if (!point) return null;
              const size = radius(node);
              const dimmed = neighbours !== null && !neighbours.has(node.id);
              const showLabel = node.id === selectedId || node.id === hoveredId || (!selectedId && labelled.has(node.id)) || (neighbours?.has(node.id) ?? false) || (vp.k >= 2.2 && !dimmed);
              const select = () => { if (!wasDragged()) setSelectedId(node.id === selectedId ? null : node.id); };
              return <g
                aria-label={`${node.kind === "memory" ? "Memory card" : "File"} ${node.title}`}
                aria-pressed={node.id === selectedId}
                className={`rm-map-node${dimmed ? " dimmed" : ""}${node.id === selectedId ? " selected" : ""}`}
                key={node.id}
                onClick={(event) => { event.stopPropagation(); select(); }}
                onKeyDown={(event) => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); select(); } }}
                onMouseLeave={() => setHoveredId(null)}
                onMouseMove={(event) => { setHoveredId(node.id); hover(event, node); }}
                role="button"
                tabIndex={0}
                transform={`translate(${point.x} ${point.y})`}
              >
                {/* Hit area larger than the mark, so small dots are easy to reach. */}
                <circle className="rm-map-hit" r={Math.max(size + 4, 12)} />
                {node.kind === "memory"
                  ? <rect className="rm-map-memory" height={size * 1.6} transform="rotate(45)" width={size * 1.6} x={-size * 0.8} y={-size * 0.8} />
                  : <>
                    <circle className={`rm-map-halo ${fileGroup(node)}`} r={size + 5} />
                    <circle className={`rm-map-file ${fileGroup(node)}${node.state !== "indexed" ? " hollow" : ""}`} r={size} />
                  </>}
                {showLabel ? <text className="rm-map-label" fontSize={11 / vp.k} strokeWidth={3 / vp.k} x={size + 5 / vp.k} y={4 / vp.k}>{shorten(node.title)}</text> : null}
              </g>;
            })}</g>
          </svg>
          {tooltip ? <div className="rm-map-tooltip" role="tooltip" style={{ left: tooltip.x, top: tooltip.y }}>{tooltip.content}</div> : null}
        </div>
        <SelectionPanel edges={map.edges} nodeById={nodeById} onOpenArtifact={onOpenArtifact} onOpenMemoryCard={onOpenMemoryCard} onSelect={setSelectedId} selected={selected} similarityAvailable={map.similarity_available} />
      </div>}
  </section>;
}

function SelectionPanel({ edges, nodeById, onOpenArtifact, onOpenMemoryCard, onSelect, selected, similarityAvailable }: { edges: KnowledgeEdge[]; nodeById: Map<string, KnowledgeNode>; onOpenArtifact: (artifactId: string) => void; onOpenMemoryCard: (cardId: string) => void; onSelect: (id: string) => void; selected: KnowledgeNode | null; similarityAvailable: boolean }) {
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
      <Button onClick={() => onOpenMemoryCard(selected.id)} type="button" variant="secondary"><Book size={15} /> Open memory card</Button>
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
    <Button onClick={() => onOpenArtifact(selected.id)} type="button" variant="secondary"><ExternalLink size={15} /> Open file</Button>
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

function EdgeTable({ edges, nodeById, onSelect }: { edges: KnowledgeEdge[]; nodeById: Map<string, KnowledgeNode>; onSelect: (id: string) => void }) {
  const rows = [...edges].sort((a, b) => a.kind === b.kind ? b.weight - a.weight : a.kind === "similar" ? -1 : 1);
  if (!rows.length) return <p className="shared-muted-copy">No links to list with the current filters.</p>;
  return <div className="rm-map-coverage">
    <table>
      <caption className="sr-only">Links between files and memory cards</caption>
      <thead><tr><th scope="col">From</th><th scope="col">To</th><th scope="col">Link</th><th scope="col">Strength</th></tr></thead>
      <tbody>{rows.map((edge) => <tr key={`${edge.kind}-${edge.source}-${edge.target}`}>
        <td><button className="rm-map-link" onClick={() => onSelect(edge.source)} type="button">{nodeById.get(edge.source)?.title ?? "Unknown"}</button></td>
        <td><button className="rm-map-link" onClick={() => onSelect(edge.target)} type="button">{nodeById.get(edge.target)?.title ?? "Unknown"}</button></td>
        <td>{edge.kind === "similar" ? "Similar content" : "Memory cites file"}</td>
        <td>{edge.kind === "similar" ? `${Math.round(edge.weight * 100)}%` : "—"}</td>
      </tr>)}</tbody>
    </table>
  </div>;
}
