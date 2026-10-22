import { type KeyboardEvent, useEffect, useMemo, useRef, useState } from "react";
import { querySharedArtifacts, searchSharedMemoryCards } from "../lib/sharedApi";
import { onOpenCommandPalette, sendToAssistant } from "../lib/assistantBridge";
import { type ArtifactSummary, type MemoryCardSummary, type SharedWorkspace } from "../types";
import { Dialog, DialogCancel } from "../components/ui/dialog";
import { EmptyState } from "../components/ui/empty-state";
import { SearchField } from "../components/ui/search-field";
import { navigate } from "./router";
import { paletteEntries, type PaletteEntry } from "./paletteEntries";
import { useTemplate } from "./workspace/data";

const CARD_SEARCH_DELAY_MS = 200;
const FILE_SEARCH_LIMIT = 50;

/**
 * A keyboard command palette (Ctrl or Cmd with K): jump to a workspace, one of
 * its sections, a file, a memory card or a system page, or start an assistant
 * action, without the mouse. Files and memory cards come from the open
 * workspace's searches as the person types, both read with the person's own access.
 */
export function CommandPalette({ accessToken, assistantWorkspaceId, canAdministerSystem, currentWorkspaceId, workspaces }: {
  accessToken: string;
  /** The workspace whose page has the assistant docked, if the open page is one. */
  assistantWorkspaceId: string | null;
  canAdministerSystem: boolean;
  /** The workspace the open page belongs to, if any. */
  currentWorkspaceId: string | null;
  workspaces: SharedWorkspace[];
}) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const [files, setFiles] = useState<ArtifactSummary[]>([]);
  const [cards, setCards] = useState<MemoryCardSummary[]>([]);
  const list = useRef<HTMLUListElement>(null);
  const currentWorkspace = workspaces.find((entry) => entry.workspace.id === currentWorkspaceId);
  const assistantWorkspace = workspaces.find((entry) => entry.workspace.id === assistantWorkspaceId);

  useEffect(() => {
    function onKey(event: globalThis.KeyboardEvent) {
      if ((event.ctrlKey || event.metaKey) && !event.altKey && !event.shiftKey && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setOpen((current) => !current);
      }
    }
    window.addEventListener("keydown", onKey);
    const stopListening = onOpenCommandPalette(() => setOpen(true));
    return () => { window.removeEventListener("keydown", onKey); stopListening(); };
  }, []);

  useEffect(() => {
    if (!open) return;
    setQuery("");
    setActive(0);
    setCards([]);
  }, [open]);

  // Files are searched by name or path as the person types, whatever the workspace's size. The server
  // takes the longest word; the palette then keeps the files that hold every word.
  useEffect(() => {
    const word = query.trim().split(/\s+/).reduce((longest, next) => next.length > longest.length ? next : longest, "");
    if (!open || !currentWorkspaceId || !word) { setFiles([]); return; }
    let cancelled = false;
    const timer = window.setTimeout(() => {
      querySharedArtifacts(accessToken, currentWorkspaceId, { query: word, limit: FILE_SEARCH_LIMIT })
        .then((next) => { if (!cancelled) setFiles(next); })
        .catch(() => { if (!cancelled) setFiles([]); });
    }, CARD_SEARCH_DELAY_MS);
    return () => { cancelled = true; window.clearTimeout(timer); };
  }, [accessToken, currentWorkspaceId, open, query]);

  useEffect(() => {
    const text = query.trim();
    if (!open || !currentWorkspaceId || text.length < 2) { setCards([]); return; }
    let cancelled = false;
    const timer = window.setTimeout(() => {
      searchSharedMemoryCards(accessToken, currentWorkspaceId, text)
        .then((next) => { if (!cancelled) setCards(next); })
        .catch(() => { if (!cancelled) setCards([]); });
    }, CARD_SEARCH_DELAY_MS);
    return () => { cancelled = true; window.clearTimeout(timer); };
  }, [accessToken, currentWorkspaceId, open, query]);

  const [template] = useTemplate(accessToken, currentWorkspaceId ?? "", open && Boolean(currentWorkspaceId));
  const sections = template?.sections;
  const labels = template?.labels.sections;
  const entries = useMemo(
    () => paletteEntries({ query, workspaces, assistantWorkspace, currentWorkspace, files, cards, canAdministerSystem, sections, labels }),
    [query, workspaces, assistantWorkspace, currentWorkspace, files, cards, canAdministerSystem, sections, labels],
  );
  const activeIndex = Math.min(active, Math.max(entries.length - 1, 0));

  useEffect(() => {
    list.current?.querySelector<HTMLElement>('[aria-selected="true"]')?.scrollIntoView?.({ block: "nearest" });
  }, [activeIndex, entries]);

  function run(entry: PaletteEntry | undefined) {
    if (!entry) return;
    setOpen(false);
    if (entry.action.kind === "navigate") navigate(entry.action.to);
    else sendToAssistant({ capability: entry.action.capability, message: entry.action.message });
  }

  function onKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    const count = Math.max(entries.length, 1);
    if (event.key === "ArrowDown") { event.preventDefault(); setActive((activeIndex + 1) % count); }
    else if (event.key === "ArrowUp") { event.preventDefault(); setActive((activeIndex - 1 + count) % count); }
    else if (event.key === "Home") { event.preventDefault(); setActive(0); }
    else if (event.key === "End") { event.preventDefault(); setActive(count - 1); }
    else if (event.key === "Enter" && !event.nativeEvent.isComposing) { event.preventDefault(); run(entries[activeIndex]); }
  }

  if (!open) return null;
  let lastGroup = "";
  return <Dialog className="shared-palette" size="lg" footer={<><span className="shared-palette-hint">Ctrl K opens this anywhere · ↑ ↓ move · Enter runs · Esc closes</span><DialogCancel label="Close" onClick={() => setOpen(false)} /></>} onClose={() => setOpen(false)} open title="Command palette">
    <SearchField
      aria-activedescendant={entries[activeIndex] ? `palette-${entries[activeIndex].id}` : undefined}
      aria-autocomplete="list"
      aria-controls="palette-results"
      aria-expanded
      aria-label="Type a command, a workspace, a file or a memory card"
      autoFocus
      clearOnEscape={false}
      onKeyDown={onKeyDown}
      onValueChange={(next) => { setQuery(next); setActive(0); }}
      placeholder={assistantWorkspaceId ? "Go to, search, or ask the assistant…" : "Go to a workspace, file or page…"}
      role="combobox"
      value={query}
    />
    {entries.length ? <ul aria-label="Results" className="rm-option-list shared-palette-list" id="palette-results" ref={list} role="listbox">
      {entries.map((entry, index) => {
        const heading = entry.group !== lastGroup ? entry.group : null;
        lastGroup = entry.group;
        return <li key={entry.id} role="presentation">
          {heading ? <span className="rm-option-group">{heading}</span> : null}
          <button aria-selected={index === activeIndex} className="rm-option" id={`palette-${entry.id}`} onClick={() => run(entry)} onMouseMove={() => setActive(index)} role="option" tabIndex={-1} type="button">
            <strong>{entry.label}</strong>{entry.hint ? <small>{entry.hint}</small> : null}
          </button>
        </li>;
      })}
    </ul> : <EmptyState className="shared-palette-empty" title={`Nothing matches “${query.trim()}”`} />}
  </Dialog>;
}
