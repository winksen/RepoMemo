import type { MouseEvent as ReactMouseEvent, PointerEvent as ReactPointerEvent } from "react";
import { useEffect, useMemo, useRef, useState } from "react";
import { IconChartDots3 as Graph, IconList as List, IconMinus as Minus, IconPlus as Plus, IconFocusCentered as Recenter } from "@tabler/icons-react";
import { layoutGraph } from "../../lib/forceLayout";
import type { KnowledgeEdge, KnowledgeMap, KnowledgeNode } from "../../types";
import { Button } from "../../components/ui/button";
import { Table } from "../../components/ui/table";
import { PanelHeading } from "../../components/ui/panel-heading";
import { SegmentedControl } from "../../components/ui/segmented-control";
import { Tooltip } from "../../components/ui/tooltip";
import { ICON_SIZE } from "../../components/ui/icon";
import {
  FileGroup,
  GROUP_LABEL,
  CANVAS_WIDTH,
  CANVAS_HEIGHT,
  LABELLED_FILES,
  fileGroup,
  percent,
  shorten,
  HoverTip,
} from "./shared";
import { SelectionPanel } from "./SelectionPanel";
import { EdgeTable } from "./EdgeTable";
import { orderLinks } from "./emphasis";

export function RelationGraph({ canConfigure, first, map, onOpenArtifact, onOpenMemoryCard, onOpenSettings }: { canConfigure: boolean; first?: string[]; map: KnowledgeMap; onOpenArtifact: (artifactId: string) => void; onOpenMemoryCard: (cardId: string) => void; onOpenSettings: () => void }) {
  const [showSimilar, setShowSimilar] = useState(true);
  const [showCites, setShowCites] = useState(true);
  const [view, setView] = useState<"graph" | "table">("graph");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [tooltip, setTooltip] = useState<HoverTip | null>(null);
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
  // Repositories are always named; then the largest files.
  const labelled = useMemo(() => new Set([
    ...map.nodes.filter((node) => node.artifact_type === "repository").map((node) => node.id),
    ...map.nodes.filter((node) => node.kind === "file" && node.artifact_type !== "repository").sort((a, b) => b.passage_count - a.passage_count).slice(0, LABELLED_FILES).map((node) => node.id),
  ]), [map.nodes]);
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
        : <><strong>{node.title}</strong><span>{node.artifact_type === "repository" ? "Git repository, all files" : GROUP_LABEL[fileGroup(node)]} · {node.passage_count} passages</span>{map.pipeline.embedded_count !== null ? <span>{percent(node.embedded_count, node.passage_count)}% searchable by meaning</span> : null}{node.state !== "indexed" ? <span>{node.state === "failed" ? "Indexing failed" : "Waiting to be indexed"}</span> : null}</>,
    });
  }

  function similarityOpacity(edge: KnowledgeEdge) {
    if (edge.kind === "cites") return 0.7;
    const span = maxWeight - minWeight;
    return 0.25 + 0.55 * (span > 0 ? (edge.weight - minWeight) / span : 1);
  }

  return <section className="rm-map-section" aria-labelledby="rm-map-graph-title">
    <PanelHeading description={<>{fileCount} files · {edges.length} links</>} icon={Graph} id="rm-map-graph-title" title="How your content connects" />
    <p className="rm-map-intro">Each dot is a file, or a whole git repository (ringed dot) with all of its files; bigger dots hold more passages. A line joins items whose content means similar things, or a memory card (◆) to what it cites.</p>
    <div className="rm-map-filters">
      <Tooltip content={map.similarity_available ? "Lines between similar files" : "Needs AI for search"}><label><input checked={showSimilar && map.similarity_available} disabled={!map.similarity_available} onChange={(event) => setShowSimilar(event.target.checked)} type="checkbox" /> Similar content</label></Tooltip>
      <label><input checked={showCites} onChange={(event) => setShowCites(event.target.checked)} type="checkbox" /> Memory links</label>
      <SegmentedControl aria-label="Map view" onValueChange={setView} options={[{ icon: <Graph size={ICON_SIZE.md} />, name: "Graph view", value: "graph" }, { icon: <List size={ICON_SIZE.md} />, name: "Table view", value: "table" }]} value={view} />
    </div>
    {!map.similarity_available ? <p className="rm-map-notice">Lines between similar files appear once AI for search is set up and passages are embedded.{canConfigure ? <> <button onClick={onOpenSettings} type="button">Open Settings</button></> : null}</p> : null}
    {map.hidden_file_count ? <p className="rm-map-notice">Showing the {fileCount} largest files; {map.hidden_file_count} smaller ones are left out to keep the map readable.</p> : null}
    <div className="rm-map-legend" aria-label="Graph legend" role="group">
      {(Object.keys(GROUP_LABEL) as FileGroup[]).map((group) => <span key={group}><i className={`rm-map-dot ${group}`} /> {GROUP_LABEL[group]}</span>)}
      <span><i className="rm-map-dot code repository" /> Repository</span>
      <span><i className="rm-map-diamond" /> Memory card</span>
      <span><i className="rm-map-dot hollow" /> Not indexed yet</span>
    </div>
    {!map.nodes.length ? <div className="shared-empty-state"><Graph size={25} /><strong>Nothing to map yet</strong><span>Add files in Evidence or Documents; they appear here once stored.</span></div>
      : view === "table" ? <EdgeTable edges={edges} first={first} nodeById={nodeById} onSelect={(id) => { setSelectedId(id); setView("graph"); }} />
      : <div className="rm-map-graph">
        <div className="rm-map-canvas" ref={canvas} onMouseLeave={() => setTooltip(null)}>
          <div className="rm-map-zoom" role="group" aria-label="Zoom">
            <Button aria-label="Zoom in" onClick={() => zoomBy(1.4)} type="button" variant="secondary"><Plus size={ICON_SIZE.sm} /></Button>
            <Button aria-label="Zoom out" onClick={() => zoomBy(1 / 1.4)} type="button" variant="secondary"><Minus size={ICON_SIZE.sm} /></Button>
            <Button aria-label="Fit all dots" onClick={() => fitView()} type="button" variant="secondary"><Recenter size={ICON_SIZE.sm} /></Button>
          </div>
          {/* a11y-allow: click a click on the empty canvas only clears the selection; the dots and the table view are the keyboard paths */}
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
            <g>{orderLinks(edges, first, true).map((edge) => {
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
                aria-label={`${node.kind === "memory" ? "Memory card" : node.artifact_type === "repository" ? "Repository" : "File"} ${node.title}`}
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
                    {node.artifact_type === "repository" ? <circle className={`rm-map-repo-ring ${fileGroup(node)}`} r={size + 3} /> : null}
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
