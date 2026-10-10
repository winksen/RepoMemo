import type { KeyboardEvent, PointerEvent } from "react";
import { useEffect, useState } from "react";
import {
  IconLayoutSidebarRightCollapse as Collapse,
  IconLayoutSidebarRightExpand as Expand,
  IconSparkles as Sparkles,
} from "@tabler/icons-react";
import { AssistantPanel } from "./AssistantPanel";
import { Button } from "./ui/button";

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

/** The workspace assistant as a section on the right of the page that can be expanded, resized or shrunk to a slim rail. */
export function AssistantDock({ accessToken, onOpenArtifact, workspaceId }: { accessToken: string; onOpenArtifact: (artifactId: string) => void; workspaceId: string }) {
  const [initial] = useState(readDockState);
  const [open, setOpen] = useState(initial.open);
  const [width, setWidth] = useState(initial.width);
  // Mounted on first expansion and then kept, so a reply in flight survives shrinking the dock.
  const [hasOpened, setHasOpened] = useState(initial.open);

  useEffect(() => {
    try {
      window.localStorage.setItem(DOCK_STORAGE_KEY, JSON.stringify({ open, width }));
    } catch { /* only a convenience */ }
  }, [open, width]);

  function toggle(next: boolean) {
    setOpen(next);
    if (next) setHasOpened(true);
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

  return <aside aria-label="Workspace assistant" className={`shared-assistant-dock${open ? "" : " collapsed"}`} style={open ? { width } : undefined}>
    {open ? null : <Button aria-expanded={false} aria-label="Open assistant" onClick={() => toggle(true)} title="Open assistant" type="button" variant="secondary"><Expand size={18} /><Sparkles size={16} /></Button>}
    <div className="shared-assistant-dock-body" hidden={!open}>
      <div aria-label="Resize assistant" aria-orientation="vertical" aria-valuemax={MAX_WIDTH} aria-valuemin={MIN_WIDTH} aria-valuenow={width} className="shared-assistant-resize" onKeyDown={resizeWithKeys} onPointerDown={startResize} role="separator" tabIndex={0} />
      <header className="shared-assistant-dock-header">
        <span><Sparkles size={16} /> Assistant</span>
        <Button aria-expanded onClick={() => toggle(false)} aria-label="Shrink assistant" title="Shrink assistant" type="button" variant="secondary"><Collapse size={16} /></Button>
      </header>
      {hasOpened ? <AssistantPanel accessToken={accessToken} key={workspaceId} onOpenArtifact={onOpenArtifact} workspaceId={workspaceId} /> : null}
    </div>
  </aside>;
}
