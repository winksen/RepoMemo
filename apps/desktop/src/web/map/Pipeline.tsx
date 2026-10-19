import { IconAlertTriangle as AlertTriangle, IconArrowRight as ArrowRight, IconHourglass as Hourglass, IconSearch as Search, IconStack2 as Layers } from "@tabler/icons-react";
import type { KnowledgeMap } from "../../types";
import { Button } from "../../components/ui/button";
import { PanelHeading } from "../../components/ui/panel-heading";
import { ICON_SIZE } from "../../components/ui/icon";
import { percent } from "./shared";

export function Pipeline({ canConfigure, map, onOpenEvidence, onOpenSettings }: { canConfigure: boolean; map: KnowledgeMap; onOpenEvidence: () => void; onOpenSettings: () => void }) {
  const { pipeline } = map;
  const indexedShare = percent(pipeline.indexed_count, pipeline.file_count);
  const embeddedShare = pipeline.embedded_count === null ? null : percent(pipeline.embedded_count, pipeline.passage_count);
  return <section className="rm-map-section" aria-labelledby="rm-map-pipeline-title">
    <PanelHeading icon={Layers} id="rm-map-pipeline-title" title="From files to searchable knowledge" />
    <p className="rm-map-intro">Every file goes through these steps. Search works after step 2; search by meaning needs step 4.</p>
    <ol className="rm-map-stages">
      <li>
        <span className="rm-map-stage-step">1 · Stored</span>
        <strong>{pipeline.file_count.toLocaleString()}</strong>
        <span>{pipeline.file_count === 1 ? "file" : "files"} in the workspace</span>
      </li>
      <li aria-hidden="true" className="rm-map-stage-arrow"><ArrowRight size={ICON_SIZE.md} /></li>
      <li>
        <span className="rm-map-stage-step">2 · Indexed</span>
        <strong>{indexedShare}%</strong>
        <span>{pipeline.indexed_count.toLocaleString()} of {pipeline.file_count.toLocaleString()} files read and split</span>
        <Meter value={indexedShare} label="Files indexed" />
        {pipeline.pending_count || pipeline.failed_count ? <div className="rm-map-stage-issues">
          {pipeline.pending_count ? <span><Hourglass size={ICON_SIZE.sm} /> {pipeline.pending_count} waiting</span> : null}
          {pipeline.failed_count ? <button className="rm-map-failed" onClick={onOpenEvidence} type="button"><AlertTriangle size={ICON_SIZE.sm} /> {pipeline.failed_count} failed · see Evidence</button> : null}
        </div> : null}
      </li>
      <li aria-hidden="true" className="rm-map-stage-arrow"><ArrowRight size={ICON_SIZE.md} /></li>
      <li>
        <span className="rm-map-stage-step">3 · Passages</span>
        <strong>{pipeline.passage_count.toLocaleString()}</strong>
        <span>searchable pieces of text (sections or ~100 lines of code)</span>
      </li>
      <li aria-hidden="true" className="rm-map-stage-arrow"><ArrowRight size={ICON_SIZE.md} /></li>
      <li>
        <span className="rm-map-stage-step">4 · Searchable by meaning</span>
        {embeddedShare === null ? <>
          <strong>Off</strong>
          <span>No AI for search is set up, so search matches words only.</span>
          {canConfigure ? <Button onClick={onOpenSettings} type="button" variant="secondary"><Search size={ICON_SIZE.sm} /> Set up AI for search</Button> : <span className="rm-map-muted">An administrator can turn it on in Settings.</span>}
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
