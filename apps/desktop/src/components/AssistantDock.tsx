import type { KeyboardEvent, PointerEvent } from "react";
import { useEffect, useRef, useState } from "react";
import {
  IconLayoutSidebarRightCollapse as Collapse,
  IconLayoutSidebarRightExpand as Expand,
  IconSparkles as Sparkles,
  IconX as Close,
} from "@tabler/icons-react";
import { onAssistantCommand } from "../lib/assistantBridge";
import { NARROW_QUERY, useMediaQuery } from "../lib/useMediaQuery";
import { AssistantPanel } from "./AssistantPanel";
import { Button } from "./ui/button";
import { ICON_SIZE } from "./ui/icon";

const DOCK_STORAGE_KEY = "repomemo.assistant.dock";
const DEFAULT_WIDTH = 400;
const MIN_WIDTH = 320;
const MAX_WIDTH = 720;
const KEY_STEP = 24;

type DockState = { open: boolean; width: number };

function clampWidth(width: number) {
  return Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, Math.round(width)));
}

function readDockState(): DockState {
  try {
    const parsed = JSON.parse(window.localStorage.getItem(DOCK_STORAGE_KEY) ?? "null") as Partial<DockState> | null;
    return {
      open: parsed?.open ?? true,
      width: typeof parsed?.width === "number" ? clampWidth(parsed.width) : DEFAULT_WIDTH,
    };
  } catch {
    return { open: true, width: DEFAULT_WIDTH };
  }
}

/**
 * The workspace assistant as a section on the right of the page that can be expanded, resized or shrunk to a slim rail.
 * On a narrow screen (V0.2.48) it is a sheet instead: a floating button opens it over the bottom of the page, and *Esc*,
 * the close button or a tap outside closes it. It starts closed there whatever the wide-screen state is, and that state is kept.
 */
export function AssistantDock({ accessToken, onOpenArtifact, workspaceId }: { accessToken: string; onOpenArtifact: (artifactId: string) => void; workspaceId: string }) {
  const [initial] = useState(readDockState);
  const [open, setOpen] = useState(initial.open);
  const [width, setWidth] = useState(initial.width);
  // Mounted on first expansion and then kept, so a reply in flight survives shrinking the dock.
  const [hasOpened, setHasOpened] = useState(initial.open);
  const narrow = useMediaQuery(NARROW_QUERY);
  const [sheetOpen, setSheetOpen] = useState(false);
  const isOpen = narrow ? sheetOpen : open;
  const dock = useRef<HTMLElement>(null);
  const opener = useRef<HTMLButtonElement>(null);
  const returnFocus = useRef(false);

  useEffect(() => {
    try {
      window.localStorage.setItem(DOCK_STORAGE_KEY, JSON.stringify({ open, width }));
    } catch { /* only a convenience */ }
  }, [open, width]);

  function toggle(next: boolean) {
    if (narrow) {
      setSheetOpen(next);
      returnFocus.current = !next;
    } else setOpen(next);
    if (next) setHasOpened(true);
  }

  // A command from the palette opens the dock so the assistant can take it.
  useEffect(() => onAssistantCommand(() => { setOpen(true); setSheetOpen(true); setHasOpened(true); }), []);

  // A sheet takes keyboard focus when it opens and gives it back to its button when it is closed.
  useEffect(() => {
    if (!narrow) return;
    if (sheetOpen) dock.current?.focus();
    else if (returnFocus.current) { returnFocus.current = false; opener.current?.focus(); }
  }, [narrow, sheetOpen]);

  function closeSheetWithEscape(event: KeyboardEvent<HTMLElement>) {
    // Keys pressed in a menu or dialog the assistant opened (rendered elsewhere in the page) are theirs.
    if (event.key === "Escape" && narrow && sheetOpen && event.currentTarget.contains(event.target as Node)) {
      event.preventDefault();
      toggle(false);
    }
  }

  function startResize(event: PointerEvent<HTMLDivElement>) {
    event.preventDefault();
    const startX = event.clientX;
    const startWidth = width;
    const move = (moveEvent: globalThis.PointerEvent) => setWidth(clampWidth(startWidth + startX - moveEvent.clientX));
    const stop = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", stop);
      window.removeEventListener("pointercancel", stop);
      document.body.classList.remove("shared-dock-resizing");
    };
    document.body.classList.add("shared-dock-resizing");
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", stop);
    window.addEventListener("pointercancel", stop);
  }

  function resizeWithKeys(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === "ArrowLeft") setWidth((current) => clampWidth(current + KEY_STEP));
    else if (event.key === "ArrowRight") setWidth((current) => clampWidth(current - KEY_STEP));
    else return;
    event.preventDefault();
  }

  return <>
    {narrow && sheetOpen ? <button aria-label="Close assistant" className="shared-assistant-backdrop" onClick={() => toggle(false)} tabIndex={-1} type="button" /> : null}
    <aside aria-label="Workspace assistant" className={`shared-assistant-dock${isOpen ? "" : " collapsed"}${narrow ? " sheet" : ""}`} onKeyDown={closeSheetWithEscape} ref={dock} style={isOpen && !narrow ? { width } : undefined} tabIndex={narrow && sheetOpen ? -1 : undefined}>
    {isOpen ? null : <Button aria-expanded={false} aria-label="Open assistant" onClick={() => toggle(true)} ref={opener} title="Open assistant" type="button" variant="secondary"><Expand size={ICON_SIZE.lg} /><Sparkles size={ICON_SIZE.md} /><span className="shared-assistant-fab-label">Assistant</span></Button>}
    <div className="shared-assistant-dock-body" hidden={!isOpen}>
      <div aria-label="Resize assistant" aria-orientation="vertical" aria-valuemax={MAX_WIDTH} aria-valuemin={MIN_WIDTH} aria-valuenow={width} className="shared-assistant-resize" onKeyDown={resizeWithKeys} onPointerDown={startResize} role="separator" tabIndex={0} />
      <header className="shared-assistant-dock-header">
        <span><Sparkles size={ICON_SIZE.md} /> Assistant</span>
        <Button aria-expanded onClick={() => toggle(false)} aria-label={narrow ? "Close assistant" : "Shrink assistant"} title={narrow ? "Close assistant" : "Shrink assistant"} type="button" variant="secondary">{narrow ? <Close size={ICON_SIZE.md} /> : <Collapse size={ICON_SIZE.md} />}</Button>
      </header>
      {hasOpened ? <AssistantPanel accessToken={accessToken} key={workspaceId} onOpenArtifact={onOpenArtifact} workspaceId={workspaceId} /> : null}
    </div>
    </aside>
  </>;
}
