import { useEffect, useState } from "react";
import {
  IconDownload as Download,
  IconExternalLink as ExternalLink,
  IconLoader2 as Loader,
  IconPaperclip as Paperclip,
} from "@tabler/icons-react";
import {
  createSharedFileLink,
  downloadSharedArtifactFile,
  downloadSharedRenderedPreview,
  getSharedDocumentPreview,
  getSharedRenderStatus,
  sharedApiUrl,
} from "../lib/sharedApi";
import { columnLetters, documentKindOf, OPEN_IN_APP } from "../lib/documents";
import type { ArtifactSummary, DocumentPreview, SheetPreview } from "../types";
import type { RenderStatus } from "../lib/sharedApi";
import { Button } from "./ui/button";
import { showToast } from "./ui/toast";

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : "Something went wrong.";
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

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
  const [preview, setPreview] = useState<DocumentPreview | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isBusy, setIsBusy] = useState(false);
  const [pdfUrl, setPdfUrl] = useState<string | null>(null);
  const [renderState, setRenderState] = useState<RenderStatus | null>(null);
  const [renderedUrl, setRenderedUrl] = useState<string | null>(null);
  const [showExtracted, setShowExtracted] = useState(false);
  const rendersLayout = kind === "word" || kind === "excel" || kind === "powerpoint";

  useEffect(() => {
    let cancelled = false;
    setPreview(null);
    setIsLoading(true);
    getSharedDocumentPreview(accessToken, artifact.id)
      .then((next) => { if (!cancelled) setPreview(next); })
      .catch((error) => { if (!cancelled) setPreview({ kind: "unavailable", reason: errorText(error) }); })
      .finally(() => { if (!cancelled) setIsLoading(false); });
    return () => { cancelled = true; };
  }, [accessToken, artifact.id]);

  // Office files: LibreOffice renders the real layout to a PDF on the server.
  // The status call starts that conversion, so poll until it is ready.
  useEffect(() => {
    if (!rendersLayout) return;
    let cancelled = false;
    let timer: number | undefined;
    let url: string | null = null;
    let attempts = 0;
    setRenderState(null);
    setRenderedUrl(null);
    setShowExtracted(false);
    const check = async () => {
      try {
        const status = await getSharedRenderStatus(accessToken, artifact.id);
        if (cancelled) return;
        setRenderState(status);
        if (status.state === "converting" && attempts++ < 80) {
          timer = window.setTimeout(() => void check(), 1500);
        } else if (status.state === "ready") {
          const blob = await downloadSharedRenderedPreview(accessToken, artifact.id);
          if (cancelled) return;
          url = URL.createObjectURL(new Blob([blob], { type: "application/pdf" }));
          setRenderedUrl(url);
        }
      } catch (error) {
        if (!cancelled) setRenderState({ state: "failed", message: errorText(error) });
      }
    };
    void check();
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
      if (url) URL.revokeObjectURL(url);
    };
  }, [accessToken, artifact.id, rendersLayout]);

  // The browser's own PDF viewer shows the original file.
  useEffect(() => {
    if (preview?.kind !== "pdf") { setPdfUrl(null); return; }
    let cancelled = false;
    let url: string | null = null;
    downloadSharedArtifactFile(accessToken, artifact.id)
      .then((blob) => {
        if (cancelled) return;
        url = URL.createObjectURL(new Blob([blob], { type: "application/pdf" }));
        setPdfUrl(url);
      })
      .catch((error) => { if (!cancelled) showToast("error", errorText(error)); });
    return () => { cancelled = true; if (url) URL.revokeObjectURL(url); };
  }, [accessToken, artifact.id, preview?.kind]);

  async function download() {
    setIsBusy(true);
    try { saveBlob(await downloadSharedArtifactFile(accessToken, artifact.id), filename); }
    catch (error) { showToast("error", errorText(error)); }
    finally { setIsBusy(false); }
  }

  async function openInApp() {
    if (!kind) return;
    const target = OPEN_IN_APP[kind];
    setIsBusy(true);
    try {
      if (kind === "pdf") {
        // Open the tab first so the browser treats it as a user action.
        const tab = window.open("", "_blank");
        const blob = await downloadSharedArtifactFile(accessToken, artifact.id);
        const url = URL.createObjectURL(new Blob([blob], { type: "application/pdf" }));
        if (tab) tab.location.href = url; else window.location.href = url;
        window.setTimeout(() => URL.revokeObjectURL(url), 60_000);
      } else if (target.scheme) {
        const link = await createSharedFileLink(accessToken, artifact.id);
        const fileUrl = `${sharedApiUrl}/v1/shared-files/${link.token}/${encodeURIComponent(link.filename)}`;
        window.location.href = `${target.scheme}${fileUrl}`;
        showToast("info", `Opening in ${target.label}. If nothing happens, use Download and open the file from your downloads.`);
      } else {
        saveBlob(await downloadSharedArtifactFile(accessToken, artifact.id), filename);
        showToast("info", `Saved ${filename}. Open it to read it in ${target.label}.`);
      }
    } catch (error) { showToast("error", errorText(error)); }
    finally { setIsBusy(false); }
  }

  const target = kind ? OPEN_IN_APP[kind] : null;
  return (
    <div className="shared-doc-viewer">
      <div className="shared-doc-toolbar">
        <div className="shared-doc-title"><strong>{artifact.title}</strong></div>
        <div className="shared-doc-actions">
          {target ? <Button disabled={isBusy} onClick={() => void openInApp()} type="button" variant="main"><ExternalLink size={16} /> Open in {target.label}</Button> : null}
          {renderedUrl ? <Button onClick={() => setShowExtracted((value) => !value)} type="button" variant="secondary">{showExtracted ? "Show original layout" : "Show extracted view"}</Button> : null}
          <Button disabled={isBusy} onClick={() => void download()} type="button" variant="secondary"><Download size={16} /> Download</Button>
        </div>
      </div>
      {rendersLayout && renderState?.state === "converting" ? <p className="shared-doc-note"><Loader className="spin" size={14} /> Rendering the original layout. The extracted view is shown meanwhile.</p> : null}
      {rendersLayout && renderState?.state === "failed" ? <p className="shared-doc-note">The original layout could not be rendered{renderState.message ? `: ${renderState.message}` : ""}. Showing the extracted view.</p> : null}
      {rendersLayout && renderState?.state === "disabled" ? <p className="shared-muted-copy">Install LibreOffice on the server to preview the original layout of Office files.</p> : null}
      {renderedUrl && !showExtracted ? <iframe className="shared-pdf-frame shared-rendered-frame" src={renderedUrl} title="Document preview" /> : <>
        {isLoading ? <p className="shared-muted-copy"><Loader className="spin" size={14} /> Loading preview…</p> : null}
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
        {preview.attachments.length ? <div className="shared-email-attachments">{preview.attachments.map((attachment, index) => <span key={`${attachment.name}-${index}`}><Paperclip size={13} /> {attachment.name} · {formatBytes(attachment.size_bytes)}</span>)}</div> : null}
        {preview.body ? <pre className="shared-content-preview shared-email-body">{preview.body}</pre> : <p className="shared-muted-copy">This message has no text body.</p>}
        {preview.truncated ? <p className="shared-muted-copy">Preview truncated to keep the page responsive.</p> : null}
      </div>;
    case "pdf":
      return <div className="shared-pdf">
        {preview.page_count ? <p className="shared-muted-copy">{preview.page_count} {preview.page_count === 1 ? "page" : "pages"}</p> : null}
        {pdfUrl ? <iframe className="shared-pdf-frame" src={pdfUrl} title="PDF preview" /> : <p className="shared-muted-copy"><Loader className="spin" size={14} /> Loading the PDF…</p>}
      </div>;
    default:
      return <p className="shared-doc-note">A preview is not available: {preview.reason}. You can still open or download the original file.</p>;
  }
}

function SheetsPreview({ sheets, totalSheets }: { sheets: SheetPreview[]; totalSheets: number }) {
  const [active, setActive] = useState(0);
  const sheet = sheets[Math.min(active, sheets.length - 1)];
  if (!sheet) return <p className="shared-muted-copy">This workbook has no sheets.</p>;
  const columns = Math.min(sheet.total_columns, sheet.rows[0]?.length ?? sheet.total_columns);
  return (
    <div className="shared-sheet">
      <div className="shared-sheet-tabs" role="tablist">
        {sheets.map((entry, index) => <button aria-selected={index === active} className={index === active ? "active" : ""} key={`${entry.name}-${index}`} onClick={() => setActive(index)} role="tab" type="button">{entry.name}</button>)}
        {totalSheets > sheets.length ? <span className="shared-muted-copy">+{totalSheets - sheets.length} more sheets</span> : null}
      </div>
      {sheet.rows.length ? (
        <div className="shared-sheet-scroll">
          <table>
            <thead><tr><th />{Array.from({ length: columns }, (_, index) => <th key={index}>{columnLetters(index)}</th>)}</tr></thead>
            <tbody>{sheet.rows.map((row, rowIndex) => <tr key={rowIndex}><th>{rowIndex + 1}</th>{Array.from({ length: columns }, (_, columnIndex) => <td key={columnIndex}>{row[columnIndex] ?? ""}</td>)}</tr>)}</tbody>
          </table>
        </div>
      ) : <p className="shared-muted-copy">This sheet is empty.</p>}
      {sheet.total_rows > sheet.rows.length || sheet.total_columns > columns ? <p className="shared-muted-copy">Showing {sheet.rows.length} of {sheet.total_rows} rows and {columns} of {sheet.total_columns} columns. Open the file to see everything.</p> : null}
    </div>
  );
}
