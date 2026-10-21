import { useState, useRef } from "react";
import { IconDownload as Download, IconExternalLink as ExternalLink, IconLoader2 as Loader, IconPaperclip as Paperclip } from "@tabler/icons-react";
import { createSharedFileLink, downloadSharedArtifactFile, downloadSharedRenderedPreview, getSharedDocumentPreview, getSharedRenderStatus, sharedApiUrl } from "../lib/sharedApi";
import { canOpenInApp, columnLetters, documentKindOf, OPEN_IN_APP } from "../lib/documents";
import { isDesktopRuntime } from "../lib/desktop";
import type { ArtifactSummary, DocumentPreview, SheetPreview } from "../types";
import type { RenderStatus } from "../lib/sharedApi";
import { Button } from "./ui/button";
import { SegmentedControl } from "./ui/segmented-control";
import { showToast } from "./ui/toast";
import { ICON_SIZE } from "./ui/icon";
import { apiMessage, formatBytes } from "../web/format";
import { workspaceKeys } from "../lib/queryClient";
import { useServerState } from "../lib/useServerState";
import { useErrorToast } from "../lib/useErrorToast";
import { useObjectUrl } from "../lib/useObjectUrl";

function saveBlob(blob: Blob, filename: string) {
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  link.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), 10_000);
}

/** A preview of a business document with "Open in ..." and Download actions. */
export function DocumentViewer({ accessToken, artifact }: { accessToken: string; artifact: ArtifactSummary }) {
  const kind = documentKindOf(artifact);
  const filename = artifact.path.split(/[\\/]/).pop() ?? artifact.title;
  const [isBusy, setIsBusy] = useState(false);
  // Which file the person asked to see as extracted text instead of the original layout.
  const [extractedFor, setExtractedFor] = useState<string | null>(null);
  const showExtracted = extractedFor === artifact.id;
  const rendersLayout = kind === "word" || kind === "powerpoint";
  const workspaceId = artifact.workspace_id;

  const [loadedPreview, , previewQuery] = useServerState<DocumentPreview | null>({
    key: workspaceKeys.artifactPreview(workspaceId, artifact.id),
    fetch: () => getSharedDocumentPreview(accessToken, artifact.id),
    initial: null,
    staleTime: 0,
  });
  const preview: DocumentPreview | null = loadedPreview ?? (previewQuery.isError ? { kind: "unavailable", reason: apiMessage(previewQuery.error, "Something went wrong.") } : null);
  const isLoading = preview === null;

  // Office files: LibreOffice renders the real layout to a PDF on the server.
  // The status call starts that conversion, so it is asked again until it is ready.
  const checks = useRef(0);
  const [loadedStatus, , statusQuery] = useServerState<RenderStatus | null>({
    key: workspaceKeys.artifactRenderStatus(workspaceId, artifact.id),
    fetch: () => getSharedRenderStatus(accessToken, artifact.id),
    initial: null,
    enabled: rendersLayout,
    staleTime: 0,
    refetchInterval: (status) => {
      if (status?.state !== "converting") { checks.current = 0; return false; }
      return checks.current++ < 80 ? 1500 : false;
    },
  });
  const [renderedFile, , renderedQuery] = useServerState<Blob | null>({
    key: workspaceKeys.artifactRendered(workspaceId, artifact.id),
    fetch: () => downloadSharedRenderedPreview(accessToken, artifact.id),
    initial: null,
    enabled: rendersLayout && loadedStatus?.state === "ready",
    staleTime: 0,
  });
  const renderedUrl = useObjectUrl(renderedFile);
  const failure = statusQuery.error ?? renderedQuery.error;
  const renderState: RenderStatus | null = failure ? { state: "failed", message: apiMessage(failure, "Something went wrong.") } : rendersLayout ? loadedStatus : null;

  // The browser's own PDF viewer shows the original file.
  const [pdfFile, , pdfQuery] = useServerState<Blob | null>({
    key: workspaceKeys.artifactFile(workspaceId, artifact.id),
    fetch: () => downloadSharedArtifactFile(accessToken, artifact.id),
    initial: null,
    enabled: preview?.kind === "pdf",
    staleTime: 0,
  });
  const pdfUrl = useObjectUrl(preview?.kind === "pdf" ? pdfFile : null);
  useErrorToast(pdfQuery.error, "Something went wrong.");

  async function download() {
    setIsBusy(true);
    try { saveBlob(await downloadSharedArtifactFile(accessToken, artifact.id), filename); }
    catch (error) { showToast("error", apiMessage(error, "Something went wrong.")); }
    finally { setIsBusy(false); }
  }

  async function openInApp() {
    if (!kind) return;
    const target = OPEN_IN_APP[kind];
    setIsBusy(true);
    try {
      // The desktop window has no tabs and cannot hand a link to Word: the file is saved to open from there.
      if (kind === "pdf" && !isDesktopRuntime) {
        // Open the tab first so the browser treats it as a user action.
        const tab = window.open("", "_blank");
        const blob = await downloadSharedArtifactFile(accessToken, artifact.id);
        const url = URL.createObjectURL(new Blob([blob], { type: "application/pdf" }));
        if (tab) tab.location.href = url; else window.location.href = url;
        window.setTimeout(() => URL.revokeObjectURL(url), 60_000);
      } else if (target.scheme && !isDesktopRuntime) {
        const link = await createSharedFileLink(accessToken, artifact.id);
        const fileUrl = `${sharedApiUrl}/v1/shared-files/${link.token}/${encodeURIComponent(link.filename)}`;
        window.location.href = `${target.scheme}${fileUrl}`;
        showToast("info", `Opening in ${target.label}. If nothing happens, use Download and open the file from your downloads.`);
      } else {
        saveBlob(await downloadSharedArtifactFile(accessToken, artifact.id), filename);
        showToast("info", `Saved ${filename}. Open it to read it in ${target.label}.`);
      }
    } catch (error) { showToast("error", apiMessage(error, "Something went wrong.")); }
    finally { setIsBusy(false); }
  }

  const target = kind && canOpenInApp(artifact) ? OPEN_IN_APP[kind] : null;
  return (
    <div className="shared-doc-viewer">
      <div className="shared-doc-toolbar">
        <div className="shared-doc-title"><strong>{artifact.title}</strong></div>
        <div className="shared-doc-actions">
          {target ? <Button disabled={isBusy} onClick={() => void openInApp()} type="button" variant="main"><ExternalLink size={ICON_SIZE.md} /> Open in {target.label}</Button> : null}
          {renderedUrl ? <Button onClick={() => setExtractedFor(showExtracted ? null : artifact.id)} type="button" variant="secondary">{showExtracted ? "Show original layout" : "Show extracted view"}</Button> : null}
          <Button disabled={isBusy} onClick={() => void download()} type="button" variant="secondary"><Download size={ICON_SIZE.md} /> Download</Button>
        </div>
      </div>
      {rendersLayout && renderState?.state === "converting" ? <p className="shared-doc-note"><Loader className="spin" size={ICON_SIZE.sm} /> Rendering the original layout. The extracted view is shown meanwhile.</p> : null}
      {rendersLayout && renderState?.state === "failed" ? <p className="shared-doc-note">The original layout could not be rendered{renderState.message ? `: ${renderState.message}` : ""}. Showing the extracted view.</p> : null}
      {rendersLayout && renderState?.state === "disabled" ? <p className="shared-muted-copy">The original layout is not available: an administrator turns it on in the server settings (it needs LibreOffice).</p> : null}
      {renderedUrl && !showExtracted ? <iframe className="shared-pdf-frame shared-rendered-frame" src={renderedUrl} title="Document preview" /> : <>
        {isLoading ? <p className="shared-muted-copy"><Loader className="spin" size={ICON_SIZE.sm} /> Loading preview…</p> : null}
        {preview ? <PreviewBody pdfUrl={pdfUrl} preview={preview} /> : null}
      </>}
    </div>
  );
}

function PreviewBody({ pdfUrl, preview }: { pdfUrl: string | null; preview: DocumentPreview }) {
  switch (preview.kind) {
    case "text":
      return <>
        {preview.approximate ? <p className="shared-doc-note">This text was recovered from the file and may be incomplete. Open the original for the full content.</p> : null}
        {preview.text.trim() ? <pre className="shared-content-preview shared-word-preview-body">{preview.text}</pre> : <p className="shared-muted-copy">No readable text was found in this file.</p>}
        {preview.truncated ? <p className="shared-muted-copy">Preview truncated to keep the page responsive.</p> : null}
      </>;
    case "sheets":
      return <SheetsPreview sheets={preview.sheets} totalSheets={preview.total_sheets} />;
    case "slides":
      return <div className="shared-slide-list">
        {preview.slides.map((slide) => (
          <article className="shared-slide" key={slide.number}>
            <span className="shared-slide-number">{slide.number}</span>
            <div>
              <strong>{slide.title ?? `Slide ${slide.number}`}</strong>
              {slide.text.map((line, index) => <p key={index}>{line}</p>)}
              {slide.notes ? <details><summary>Speaker notes</summary><p>{slide.notes}</p></details> : null}
            </div>
          </article>
        ))}
        {preview.total_slides > preview.slides.length ? <p className="shared-muted-copy">Showing the first {preview.slides.length} of {preview.total_slides} slides.</p> : null}
      </div>;
    case "email":
      return <div className="shared-email">
        <dl className="shared-email-headers">
          {preview.subject ? <><dt>Subject</dt><dd><strong>{preview.subject}</strong></dd></> : null}
          {preview.from ? <><dt>From</dt><dd>{preview.from}</dd></> : null}
          {preview.to.length ? <><dt>To</dt><dd>{preview.to.join(", ")}</dd></> : null}
          {preview.cc.length ? <><dt>Cc</dt><dd>{preview.cc.join(", ")}</dd></> : null}
          {preview.date ? <><dt>Date</dt><dd>{preview.date}</dd></> : null}
        </dl>
        {preview.attachments.length ? <div className="shared-email-attachments">{preview.attachments.map((attachment, index) => <span key={`${attachment.name}-${index}`}><Paperclip size={ICON_SIZE.sm} /> {attachment.name} · {formatBytes(attachment.size_bytes)}</span>)}</div> : null}
        {preview.body ? <pre className="shared-content-preview shared-email-body">{preview.body}</pre> : <p className="shared-muted-copy">This message has no text body.</p>}
        {preview.truncated ? <p className="shared-muted-copy">Preview truncated to keep the page responsive.</p> : null}
      </div>;
    case "pdf":
      return <div className="shared-pdf">
        {preview.page_count ? <p className="shared-muted-copy">{preview.page_count} {preview.page_count === 1 ? "page" : "pages"}</p> : null}
        {pdfUrl ? <iframe className="shared-pdf-frame" src={pdfUrl} title="PDF preview" /> : <p className="shared-muted-copy"><Loader className="spin" size={ICON_SIZE.sm} /> Loading the PDF…</p>}
      </div>;
    default:
      return <p className="shared-doc-note">A preview is not available: {preview.reason}. You can still open or download the original file.</p>;
  }
}

export function SheetsPreview({ sheets, totalSheets }: { sheets: SheetPreview[]; totalSheets: number }) {
  const [active, setActive] = useState(0);
  const sheet = sheets[Math.min(active, sheets.length - 1)];
  if (!sheet) return <p className="shared-muted-copy">This workbook has no sheets.</p>;
  const columns = Math.min(sheet.total_columns, sheet.rows[0]?.length ?? sheet.total_columns);
  return (
    <div className="shared-sheet">
      <div className="shared-sheet-tabs">
        <SegmentedControl aria-label="Sheets" followFocus onValueChange={(next) => setActive(Number(next))} options={sheets.map((entry, index) => ({ label: entry.name, value: String(index) }))} value={String(Math.min(active, sheets.length - 1))} />
        {totalSheets > sheets.length ? <span className="shared-muted-copy">+{totalSheets - sheets.length} more sheets</span> : null}
      </div>
      {sheet.rows.length ? (
        <div aria-label={`Sheet ${sheet.name}`} className="shared-sheet-scroll" role="region" tabIndex={0}>
          <table>
            <thead><tr><th scope="col"><span className="sr-only">Row</span></th>{Array.from({ length: columns }, (_, index) => <th key={index} scope="col">{columnLetters(index)}</th>)}</tr></thead>
            <tbody>{sheet.rows.map((row, rowIndex) => <tr key={rowIndex}><th scope="row">{rowIndex + 1}</th>{Array.from({ length: columns }, (_, columnIndex) => <td key={columnIndex}>{row[columnIndex] ?? ""}</td>)}</tr>)}</tbody>
          </table>
        </div>
      ) : <p className="shared-muted-copy">This sheet is empty.</p>}
      {sheet.total_rows > sheet.rows.length || sheet.total_columns > columns ? <p className="shared-muted-copy">Showing {sheet.rows.length} of {sheet.total_rows} rows and {columns} of {sheet.total_columns} columns. Open the file to see everything.</p> : null}
    </div>
  );
}
