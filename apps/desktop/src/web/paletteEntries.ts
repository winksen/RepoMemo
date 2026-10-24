import { type AgentCapability, type ArtifactSummary, type MemoryCardSummary, type SharedWorkspace, type WorkspaceTemplateView } from "../types";
import { ADMIN_WORKSPACE_SECTIONS, SYSTEM_SECTIONS, WORKSPACE_SECTIONS, type WorkspaceSection } from "./constants";
import { routePath, workspaceHomePath } from "./router";
import { arrangeSections } from "./templateSections";
import { orderCapabilities } from "./templateAssistant";
import { type RecentItem } from "../lib/paletteRecents";

export type PaletteGroup = "Recent" | "Assistant" | "Files" | "Memory" | "This workspace" | "Workspaces" | "Pages" | "System";

export type PaletteAction =
  | { kind: "navigate"; to: string }
  /** Opens the note editor of a workspace (V0.3.13). */
  | { kind: "new-note"; workspaceId: string; to: string }
  | { kind: "assistant"; capability: AgentCapability | null; message: string };

export type PaletteEntry = {
  id: string;
  group: PaletteGroup;
  label: string;
  hint?: string;
  action: PaletteAction;
  /** The workspace the entry leads into: a place of it is remembered as recent (V0.3.13). */
  workspaceId?: string;
};

export const SECTION_LABELS: Record<WorkspaceSection, string> = {
  overview: "Overview",
  evidence: "Evidence",
  documents: "Documents",
  repositories: "Repositories",
  retrieval: "Retrieval",
  map: "Map",
  health: "Health",
  review: "Review queue",
  memory: "Memory",
  tasks: "Tasks",
  people: "People",
  settings: "Workspace settings",
};

const GROUP_ORDER: PaletteGroup[] = ["Recent", "Assistant", "Files", "Memory", "This workspace", "Workspaces", "Pages", "System"];
const MAX_FILES = 8;
const MAX_CARDS = 6;
const MAX_ENTRIES = 40;

export type PaletteInput = {
  query: string;
  workspaces: SharedWorkspace[];
  /** The workspace whose page has the assistant docked, if the open page is one. */
  assistantWorkspace: SharedWorkspace | undefined;
  /** The workspace the open page belongs to, whichever page it is. */
  currentWorkspace: SharedWorkspace | undefined;
  files: ArtifactSummary[];
  cards: MemoryCardSummary[];
  canAdministerSystem: boolean;
  /** How the current workspace's template arranges its sections: the palette lists them in that order (V0.2.172). */
  sections?: WorkspaceTemplateView["sections"] | null;
  /** The names the template gives to sections (V0.2.173): the palette lists and matches them under that name. */
  labels?: WorkspaceTemplateView["labels"]["sections"] | null;
  /** The assistant's jobs the template prefers first (V0.2.176): the palette lists its assistant commands in that order. */
  assistantOrder?: string[] | null;
  /** Places the person opened from the palette before, newest first (V0.3.13); listed while nothing is typed. */
  recents?: RecentItem[];
};

function terms(text: string): string[] {
  return text.toLowerCase().split(/\s+/).filter(Boolean);
}

function matchesAll(words: string[], ...fields: Array<string | undefined>): boolean {
  const haystack = fields.filter(Boolean).join(" ").toLowerCase();
  return words.every((word) => haystack.includes(word));
}

/** Everything the palette can do for what was typed, in the order it is listed. */
export function paletteEntries(input: PaletteInput): PaletteEntry[] {
  const query = input.query.trim();
  const words = terms(query);
  const entries: PaletteEntry[] = [];
  const add = (entry: PaletteEntry) => entries.push(entry);
  const workspace = input.currentWorkspace;

  // Places opened before, while nothing is typed; only those of workspaces the person can still read.
  if (!words.length && input.recents?.length) {
    const readable = new Set(input.workspaces.map((entry) => entry.workspace.id));
    input.recents
      .filter((item) => readable.has(item.workspaceId))
      .forEach((item) => add({
        id: `recent:${item.id}`,
        group: "Recent",
        label: item.label,
        hint: item.hint,
        action: { kind: "navigate", to: item.to },
      }));
  }

  if (input.assistantWorkspace) {
    const name = input.assistantWorkspace.workspace.name;
    const assistant = (capability: AgentCapability, typedLabel: string, emptyLabel: string): PaletteEntry => ({
      id: `assistant:${capability}`,
      group: "Assistant",
      label: query ? `${typedLabel} “${query}”` : emptyLabel,
      hint: name,
      action: { kind: "assistant", capability, message: query },
    });
    // In the order the workspace's template prefers (V0.2.176); the usual order without one.
    const commands = [
      assistant("ask_question", "Ask the assistant:", "Ask the assistant a question"),
      assistant("find_files", "Find files named", "Find files with the assistant"),
      assistant("search_content", "Search content for", "Search content with the assistant"),
      assistant("find_symbol", "Find the symbol", "Find a code symbol with the assistant"),
    ];
    orderCapabilities(commands.map((entry) => ({ id: entry.id.replace("assistant:", ""), entry })), input.assistantOrder ? { order: input.assistantOrder } : null)
      .forEach(({ entry }) => add(entry));
    if (matchesAll(words, "summarize workspace overview")) {
      add({ id: "assistant:overview", group: "Assistant", label: "Give a workspace overview", hint: name, action: { kind: "assistant", capability: "workspace_overview", message: "" } });
    }
  }

  if (workspace && words.length) {
    input.files
      .filter((file) => matchesAll(words, file.title, file.path))
      .slice(0, MAX_FILES)
      .forEach((file) => add({
        id: `file:${file.id}`,
        group: "Files",
        label: file.title,
        hint: file.path !== file.title ? file.path : workspace.workspace.name,
        action: { kind: "navigate", to: routePath({ name: "artifact", workspaceId: workspace.workspace.id, artifactId: file.id }) },
        workspaceId: workspace.workspace.id,
      }));
    input.cards.slice(0, MAX_CARDS).forEach((card) => add({
      id: `card:${card.id}`,
      group: "Memory",
      label: card.title,
      hint: "Memory card",
      action: { kind: "navigate", to: routePath({ name: "memory-card", workspaceId: workspace.workspace.id, cardId: card.id }) },
      workspaceId: workspace.workspace.id,
    }));
  }

  // A new note in the open workspace, for anyone who can add evidence there.
  if (workspace && workspace.role !== "viewer" && matchesAll(words, "new note write create add shared note", workspace.workspace.name)) {
    add({
      id: "new-note",
      group: "This workspace",
      label: "New note",
      hint: workspace.workspace.name,
      action: { kind: "new-note", workspaceId: workspace.workspace.id, to: routePath({ name: "workspace", workspaceId: workspace.workspace.id, section: "evidence" }) },
    });
  }

  if (workspace) {
    const mayAdminister = workspace.role === "owner" || workspace.role === "admin";
    // The tabs in the template's order, then the sections it keeps under More.
    const arranged = arrangeSections(input.sections, "overview", mayAdminister);
    const nameOf = (section: WorkspaceSection) => input.labels?.[section] ?? SECTION_LABELS[section];
    [...arranged.tabs, ...arranged.more]
      .filter((section) => matchesAll(words, nameOf(section), workspace.workspace.name))
      .forEach((section) => add({
        id: `section:${section}`,
        group: "This workspace",
        label: nameOf(section),
        hint: workspace.workspace.name,
        action: { kind: "navigate", to: routePath({ name: "workspace", workspaceId: workspace.workspace.id, section }) },
        workspaceId: workspace.workspace.id,
      }));
  }

  input.workspaces
    .filter((entry) => matchesAll(words, entry.workspace.name, "workspace"))
    .forEach((entry) => add({
      id: `workspace:${entry.workspace.id}`,
      group: "Workspaces",
      label: entry.workspace.name,
      hint: `Open workspace · ${entry.role}`,
      action: { kind: "navigate", to: workspaceHomePath(entry.workspace.id) },
      workspaceId: entry.workspace.id,
    }));

  [
    { id: "dashboard", label: "Dashboard", hint: "Search all workspaces", to: routePath({ name: "dashboard" }) },
    { id: "workspaces", label: "All workspaces", hint: "Organizations and workspaces", to: routePath({ name: "workspaces" }) },
    { id: "notifications", label: "Notifications", hint: "Mentions and assignments", to: routePath({ name: "notifications" }) },
    { id: "profile", label: "Profile and security", hint: "Your account, password and tokens", to: routePath({ name: "profile" }) },
  ]
    .filter((page) => matchesAll(words, page.label, page.hint))
    .forEach((page) => add({ id: `page:${page.id}`, group: "Pages", label: page.label, hint: page.hint, action: { kind: "navigate", to: page.to } }));

  if (input.canAdministerSystem) {
    SYSTEM_SECTIONS
      .filter((section) => matchesAll(words, "system", section))
      .forEach((section) => add({
        id: `system:${section}`,
        group: "System",
        label: `System · ${section.charAt(0).toUpperCase()}${section.slice(1)}`,
        hint: "Server administration",
        action: { kind: "navigate", to: routePath({ name: "system", section }) },
      }));
  }

  return entries
    .map((entry, index) => ({ entry, index }))
    .sort((a, b) => GROUP_ORDER.indexOf(a.entry.group) - GROUP_ORDER.indexOf(b.entry.group) || a.index - b.index)
    .map(({ entry }) => entry)
    .slice(0, MAX_ENTRIES);
}
