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

export function rememberCairnWorkspace(workspaceId: string) {
  try { window.localStorage.setItem(CONTEXT_KEY, workspaceId); } catch { /* only a convenience */ }
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
