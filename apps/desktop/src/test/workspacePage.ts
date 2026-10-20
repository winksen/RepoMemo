import { vi } from "vitest";
import type { WorkspacePage } from "../web/workspace/page";

/** A workspace page for section tests: an administrator who may write, with spies for the callbacks. */
export function makePage(overrides: Partial<WorkspacePage> = {}): WorkspacePage {
  return {
    accessToken: "t",
    canAssignAdmin: true,
    canInspectIndex: true,
    canManageMembers: true,
    canManageWorkspace: true,
    canWrite: true,
    draftMemoryFromAnswer: vi.fn(),
    indexingStalled: false,
    isLive: false,
    isLoading: false,
    isSubmitting: false,
    isWorkspaceAdmin: true,
    liveStatus: null,
    load: vi.fn(),
    onNavigate: vi.fn(),
    onOpenArtifact: vi.fn(),
    onOpenMemoryCard: vi.fn(),
    onWorkspaceDeleted: vi.fn(),
    onWorkspaceUpdated: vi.fn(),
    section: "evidence",
    session: { user: { id: "u1", display_name: "Ada", email: "ada@example.com" } },
    setError: vi.fn(),
    setIsSubmitting: vi.fn(),
    store: new Map<string, unknown>(),
    workspace: { role: "owner", workspace: { id: "w1", name: "Atlas" } },
    workspaceId: "w1",
    ...overrides,
  } as unknown as WorkspacePage;
}
