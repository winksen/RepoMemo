import { type ReactNode, useEffect, useState } from "react";
import { type Citation, type SharedSession, type SharedWorkspace } from "../../types";
import { type WorkspaceSection } from "../constants";

/**
 * What the workspace page hands to each of its sections: who is looking, what
 * they may do, and the few things every section needs (the busy flag, error
 * toasts, a full reload, navigation). Server data is not passed down: each
 * section reads what it needs from the query cache (see `data.ts`).
 */
export type WorkspacePage = {
  accessToken: string;
  workspace: SharedWorkspace;
  workspaceId: string;
  session: SharedSession;
  section: WorkspaceSection;
  canWrite: boolean;
  canManageMembers: boolean;
  canAssignAdmin: boolean;
  canManageWorkspace: boolean;
  canInspectIndex: boolean;
  isWorkspaceAdmin: boolean;
  /** The live event stream is open (polling is only a safety net). */
  isLive: boolean;
  isLoading: boolean;
  isSubmitting: boolean;
  /** Indexing has been waiting longer than the page keeps polling. */
  indexingStalled: boolean;
  /** Progress of background work, shown above the evidence ledger. */
  liveStatus: ReactNode;
  setIsSubmitting: (value: boolean) => void;
  setError: (message: string | null) => void;
  /** Refetches everything the page shows (after a change that touches many things). */
  load: () => Promise<void>;
  onOpenArtifact: (artifactId: string) => void;
  onOpenMemoryCard: (cardId: string) => void;
  onNavigate: (section: WorkspaceSection) => void;
  onWorkspaceUpdated: (workspace: SharedWorkspace["workspace"]) => void;
  onWorkspaceDeleted: (workspaceId: string) => void;
  /** Opens the "save as memory" dialog with an AI answer and its sources. */
  draftMemoryFromAnswer: (title: string, body: string, source: string, citations: Citation[]) => void;
  /** What sections remember between visits to them, for this workspace. */
  store: Map<string, unknown>;
};

/**
 * `useState` whose value is kept in the page's store while the section is
 * away, so filters, drafts and results are there when a person comes back, as
 * they were when the whole page was one component.
 */
export function sectionState(store: Map<string, unknown>, scope: string) {
  return function useSectionValue<T>(key: string, initial: T) {
    const id = `${scope}.${key}`;
    const [value, setValue] = useState<T>(() => (store.has(id) ? (store.get(id) as T) : initial));
    useEffect(() => { store.set(id, value); }, [id, value]);
    return [value, setValue] as const;
  };
}
