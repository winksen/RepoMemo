import type { AgentCapability } from "../types";

/** What the command palette asks the assistant to do. An empty message with a capability only selects it. */
export type AssistantCommand = { capability: AgentCapability | null; message: string };

type Listener = () => void;

let pending: AssistantCommand | null = null;
const listeners = new Set<Listener>();

/**
 * Hands a command to the assistant of the page. It is kept until the
 * assistant takes it, so a dock that is collapsed or not yet opened still
 * receives it once it mounts.
 */
export function sendToAssistant(command: AssistantCommand) {
  pending = command;
  listeners.forEach((listener) => listener());
}

export function takeAssistantCommand(): AssistantCommand | null {
  const command = pending;
  pending = null;
  return command;
}

export function onAssistantCommand(listener: Listener): () => void {
  listeners.add(listener);
  return () => { listeners.delete(listener); };
}

let pendingNote: string | null = null;
const noteListeners = new Set<Listener>();

/**
 * Asks the evidence page of a workspace to open its note editor (V0.3.13): the command palette's *New note*. The
 * request is kept until that page takes it, so a page that is still opening receives it once it mounts.
 */
export function requestNewNote(workspaceId: string) {
  pendingNote = workspaceId;
  noteListeners.forEach((listener) => listener());
}

/** True once, when a new note was asked for in this workspace. */
export function takeNewNoteRequest(workspaceId: string): boolean {
  if (pendingNote !== workspaceId) return false;
  pendingNote = null;
  return true;
}

export function onNewNoteRequest(listener: Listener): () => void {
  noteListeners.add(listener);
  return () => { noteListeners.delete(listener); };
}

const OPEN_PALETTE_EVENT = "repomemo:open-palette";

/** Opens the command palette from a button (the keyboard shortcut is Ctrl or Cmd with K). */
export function openCommandPalette() {
  window.dispatchEvent(new Event(OPEN_PALETTE_EVENT));
}

export function onOpenCommandPalette(listener: Listener): () => void {
  window.addEventListener(OPEN_PALETTE_EVENT, listener);
  return () => window.removeEventListener(OPEN_PALETTE_EVENT, listener);
}
