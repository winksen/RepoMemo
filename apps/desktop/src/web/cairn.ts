import { useQueries } from "@tanstack/react-query";
import { listSharedAgentConversations } from "../lib/sharedApi";
import { workspaceKeys } from "../lib/queryClient";
import type { AgentConversation, SharedWorkspace } from "../types";

/** A chat with the workspace it belongs to. */
export type CairnChat = { conversation: AgentConversation; workspace: SharedWorkspace };

const CONTEXT_KEY = "repomemo.cairn.workspace";

/** The workspace a new chat starts in: the one last worked in, when the person can still open it, else the first. */
export function cairnContextWorkspace(workspaces: SharedWorkspace[], remembered: string | null): SharedWorkspace | undefined {
  return workspaces.find((entry) => entry.workspace.id === remembered) ?? workspaces[0];
}

export function rememberedCairnWorkspace(): string | null {
  try { return window.localStorage.getItem(CONTEXT_KEY); } catch { return null; }
}

const CONTEXT_EVENT = "repomemo:cairn-context";

/** Sets the workspace a new chat starts in, and tells whoever shows it (the page's chooser). */
export function rememberCairnWorkspace(workspaceId: string) {
  try { window.localStorage.setItem(CONTEXT_KEY, workspaceId); } catch { /* only a convenience */ }
  window.dispatchEvent(new Event(CONTEXT_EVENT));
}

/** Calls `listener` when `rememberCairnWorkspace` is used; returns the function that stops listening. */
export function onCairnWorkspaceChange(listener: () => void): () => void {
  window.addEventListener(CONTEXT_EVENT, listener);
  return () => window.removeEventListener(CONTEXT_EVENT, listener);
}

/** The chats of each workspace, the workspace with the latest chat first; a workspace with no chat has no group. */
export function groupCairnChats(chats: CairnChat[]): { workspace: SharedWorkspace; chats: CairnChat[] }[] {
  const groups = new Map<string, { workspace: SharedWorkspace; chats: CairnChat[] }>();
  for (const chat of chats) {
    const group = groups.get(chat.workspace.workspace.id) ?? { workspace: chat.workspace, chats: [] };
    group.chats.push(chat);
    groups.set(chat.workspace.workspace.id, group);
  }
  return [...groups.values()];
}

/** Every chat of the person across their workspaces, the most recent first. Shares the cache the chat panel writes to. */
export function useCairnChats(accessToken: string, workspaces: SharedWorkspace[]): { chats: CairnChat[]; isLoading: boolean; loaded: boolean } {
  const results = useQueries({
    queries: workspaces.map((entry) => ({
      queryKey: workspaceKeys.assistantChats(entry.workspace.id),
      queryFn: () => listSharedAgentConversations(accessToken, entry.workspace.id),
      staleTime: 0,
    })),
  });
  const chats: CairnChat[] = [];
  results.forEach((result, index) => {
    for (const conversation of result.data ?? []) chats.push({ conversation, workspace: workspaces[index] });
  });
  chats.sort((left, right) => right.conversation.updated_at.localeCompare(left.conversation.updated_at));
  return { chats, isLoading: results.some((result) => result.isLoading), loaded: results.every((result) => !result.isLoading) };
}
