import { type FormEvent, type ReactNode, useEffect, useState } from "react";
import { SafeMarkdown } from "../components/SafeMarkdown";
import { IconArrowLeft as ArrowLeft, IconFileText as FileText, IconLoader2 as Loader, IconMarkdown as Markdown, IconPencil as Pencil, IconTrash as Trash } from "@tabler/icons-react";
import { deleteSharedMemoryCard, exportSharedMemoryCard, getSharedMemoryCard, updateSharedMemoryCard } from "../lib/sharedApi";
import { type MemoryCardDetail, type Organization, type SharedSession, type SharedWorkspace } from "../types";
import { Button } from "../components/ui/button";
import { Input } from "../components/ui/input";
import { Textarea } from "../components/ui/textarea";
import { Toast } from "../components/ui/toast";
import { SharedLayout } from "../components/SharedLayout";
import { type WorkspaceSection } from "./constants";
import { apiMessage, citationLines } from "./format";
import { navigate } from "./router";
import { OrganizationRail, WorkspaceTopbar } from "./Organization";

export function SharedMemoryCardDetail({
  accessToken,
  apiAvailable,
  cardId,
  onBack,
  onOpenArtifact,
  organization,
  organizations,
  session,
  signOut,
  workspace,
  workspaces,
}: {
  accessToken: string;
  apiAvailable: boolean | null;
  cardId: string;
  onBack: () => void;
  onOpenArtifact: (artifactId: string) => void;
  organization?: Organization;
  organizations: Organization[];
  session: SharedSession;
  signOut: () => void;
  workspace: SharedWorkspace;
  workspaces: SharedWorkspace[];
}) {
  const [card, setCard] = useState<MemoryCardDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isExporting, setIsExporting] = useState(false);
  const [isEditing, setIsEditing] = useState(false);
  const [isMutating, setIsMutating] = useState(false);
  const [title, setTitle] = useState("");
  const [bodyMarkdown, setBodyMarkdown] = useState("");
  const [source, setSource] = useState("");
  const canWrite = workspace.role !== "viewer";

  async function load() {
    setIsLoading(true); setError(null);
    try { setCard(await getSharedMemoryCard(accessToken, cardId)); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsLoading(false); }
  }

  useEffect(() => { void load(); }, [accessToken, cardId]);
  useEffect(() => {
    setTitle(card?.card.title ?? "");
    setBodyMarkdown(card?.card.body_markdown ?? "");
    setSource(card?.card.source ?? "");
  }, [card?.card.body_markdown, card?.card.source, card?.card.title]);

  async function exportCard() {
    setIsExporting(true); setError(null);
    try {
      const markdown = await exportSharedMemoryCard(accessToken, cardId);
      const file = new Blob([markdown], { type: "text/markdown;charset=utf-8" });
      const url = URL.createObjectURL(file);
      const link = document.createElement("a");
      link.href = url;
      link.download = `${(card?.card.title ?? "repomemo-memory").replace(/[^a-z0-9-_]+/gi, "-").toLowerCase()}.md`;
      link.click();
      URL.revokeObjectURL(url);
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsExporting(false); }
  }

  async function saveCard(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsMutating(true); setError(null);
    try {
      await updateSharedMemoryCard(accessToken, cardId, { title, bodyMarkdown, source });
      setIsEditing(false);
      await load();
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsMutating(false); }
  }

  async function removeCard() {
    if (!window.confirm(`Delete ${card?.card.title ?? "this memory card"}? This cannot be undone.`)) return;
    setIsMutating(true); setError(null);
    try { await deleteSharedMemoryCard(accessToken, cardId); onBack(); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsMutating(false); }
  }

  return (
    <SharedRecordLayout activeSection="memory" apiAvailable={apiAvailable} organization={organization} organizations={organizations} session={session} signOut={signOut} title={card?.card.title ?? "Memory card"} subtitle={card ? `Source: ${card.card.source}` : "Loading durable team memory…"} workspace={workspace} workspaces={workspaces}>
      <div className="shared-detail-actions">{canWrite ? <><Button disabled={isLoading || isMutating} onClick={() => setIsEditing((value) => !value)} type="button" variant="secondary"><Pencil size={16} /> {isEditing ? "Cancel edit" : "Edit"}</Button><Button className="shared-danger-action" disabled={isLoading || isMutating} onClick={() => void removeCard()} type="button" variant="secondary"><Trash size={16} /> Delete</Button></> : null}<Button disabled={isLoading || isExporting || isMutating} onClick={() => void exportCard()} type="button" variant="main">{isExporting ? <Loader className="spin" size={16} /> : <FileText size={16} />} Export Markdown</Button></div>
      <Toast kind="error" message={error} />
      <section className="shared-record-panel"><h2>Durable statement</h2>{isEditing ? <form className="shared-record-edit-form" onSubmit={saveCard}><label>Title<Input onChange={(event) => setTitle(event.target.value)} required value={title} /></label><label>Source<Input onChange={(event) => setSource(event.target.value)} required value={source} /></label><label>Statement<Textarea onChange={(event) => setBodyMarkdown(event.target.value)} required value={bodyMarkdown} /></label><Button disabled={isMutating} type="submit" variant="main">{isMutating ? <Loader className="spin" size={16} /> : <Pencil size={16} />} Save memory</Button></form> : <div className="shared-markdown shared-memory-body"><SafeMarkdown>{card?.card.body_markdown ?? ""}</SafeMarkdown></div>}</section>
      <section className="shared-record-panel"><h2>Linked evidence</h2>{card?.evidence.length ? <div className="shared-evidence-links">{card.evidence.map((evidence) => <article key={evidence.link_id}>{evidence.artifact_id && evidence.exists ? <button className="shared-assistant-citation shared-evidence-link-title" onClick={() => onOpenArtifact(evidence.artifact_id!)} type="button"><strong>{evidence.title ?? "Untitled evidence"}</strong></button> : <strong>{evidence.title ?? "Untitled evidence"}{evidence.exists ? "" : " (missing)"}</strong>}<span>{evidence.path ?? evidence.target_id}{citationLines(evidence)}</span></article>)}</div> : <p className="shared-muted-copy">This memory card currently has no linked evidence.</p>}</section>
    </SharedRecordLayout>
  );
}

export function SharedRecordLayout({
  activeSection,
  apiAvailable,
  children,
  organization,
  organizations,
  session,
  signOut,
  subtitle,
  title,
  workspace,
  workspaces,
}: {
  activeSection: WorkspaceSection;
  apiAvailable: boolean | null;
  children: ReactNode;
  organization?: Organization;
  organizations: Organization[];
  session: SharedSession;
  signOut: () => void;
  subtitle: string;
  title: string;
  workspace: SharedWorkspace;
  workspaces: SharedWorkspace[];
}) {
  return (
    <SharedLayout apiAvailable={apiAvailable} onNavigate={navigate} session={session} signOut={signOut} sidebar={<OrganizationRail organizationId={organization?.id} organizations={organizations} workspaceId={workspace.workspace.id} workspaces={workspaces} />} workspaceNavigation={<WorkspaceTopbar activeSection={activeSection} onNavigate={(section) => navigate(`/workspaces/${encodeURIComponent(workspace.workspace.id)}/${section}`)} workspace={workspace} />}>
      <section className="shared-detail-shell">
        <div className="shared-detail-heading"><div><h1>{title}</h1><p className="shared-record-subtitle">{subtitle}</p></div></div>
        <div className="shared-record-content">{children}</div>
      </section>
    </SharedLayout>
  );
}

export function SharedRouteNotFound({
  apiAvailable,
  onBack,
  organization,
  organizations,
  session,
  signOut,
  workspace,
  workspaces,
}: {
  apiAvailable: boolean | null;
  onBack: () => void;
  organization?: Organization;
  organizations: Organization[];
  session: SharedSession;
  signOut: () => void;
  workspace?: SharedWorkspace;
  workspaces?: SharedWorkspace[];
}) {
  return (
    <SharedLayout apiAvailable={apiAvailable} onNavigate={navigate} session={session} signOut={signOut} sidebar={<OrganizationRail organizationId={organization?.id} organizations={organizations} workspaceId={workspace?.workspace.id} workspaces={workspaces} />} workspaceNavigation={workspace ? <WorkspaceTopbar workspace={workspace} /> : undefined}>
      <section className="shared-page-content shared-route-page" aria-label="Unknown shared route">
        <p className="shared-eyebrow">Shared workspace</p>
        <h1>This address has no shared view.</h1>
        <p className="shared-intro-copy">The requested route is not available, or the workspace is not in your current signed-in membership.</p>
        <Button className="shared-route-action" onClick={onBack} type="button" variant="main"><ArrowLeft size={17} /> Go to workspaces</Button>
      </section>
    </SharedLayout>
  );
}
