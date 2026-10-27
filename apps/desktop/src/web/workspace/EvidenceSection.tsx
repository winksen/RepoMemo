import { type FormEvent, useEffect, useMemo, useRef, useState } from "react";
import { IconAlertCircle as AlertCircle, IconBook2 as Book, IconChevronDown as ChevronDown, IconChevronRight as ChevronRight, IconFileText as FileText, IconFilter as Filter, IconFolder as FolderIcon, IconFolderPlus as FolderPlus, IconFolderSymlink as FolderMove, IconLayoutGrid as Grid, IconList as List, IconMarkdown as Markdown, IconNote as Note, IconPencil as Pencil, IconPlus as Plus, IconSearch as Search, IconShieldLock as Shield, IconTrash as Trash, IconUpload as Upload } from "@tabler/icons-react";
import { bulkSharedArtifacts, type BulkArtifactsResult, moveSharedFolder, createSharedFolder, createSharedTextArtifact, deleteSharedArtifact, deleteSharedFolder, moveSharedArtifact, renameSharedFolder, uploadSharedArtifact } from "../../lib/sharedApi";
import { type ArtifactSummary, type Folder } from "../../types";
import { Button } from "../../components/ui/button";
import { PanelHeading } from "../../components/ui/panel-heading";
import { Checkbox } from "../../components/ui/checkbox";
import { EmptyState } from "../../components/ui/empty-state";
import { FeedbackNotice } from "../../components/ui/feedback-notice";
import { Field } from "../../components/ui/field";
import { Pagination } from "../../components/ui/pagination";
import { ActionMenu } from "../../components/ui/action-menu";
import { Dialog, DialogCancel } from "../../components/ui/dialog";
import { SearchField } from "../../components/ui/search-field";
import { SegmentedControl } from "../../components/ui/segmented-control";
import { Input } from "../../components/ui/input";
import { Tooltip } from "../../components/ui/tooltip";
import { DOCUMENT_ACCEPT, isDocumentArtifact } from "../../lib/documents";
import { DocumentViewer, RichNoteEditor } from "../lazy";
import { showToast } from "../../components/ui/toast";
import { type ArtifactViewMode, FILE_CATEGORIES, type FileCategory, type ItemDialog, MAX_FOLDER_DEPTH, UPLOAD_CONCURRENCY, UPLOAD_EXTENSIONS, type UploadProgress } from "../constants";
import { apiMessage, artifactKindLabel, documentKindLabel, fileCategory, formatBytes } from "../format";
import { selectUploads } from "../../lib/uploadFilter";
import { useUnsavedChanges } from "../../lib/unsavedChanges";
import { submitHint } from "../../lib/submitShortcut";
import { parseRoute } from "../router";
import { fileCategoryIcon } from "../common";
import { useEvidencePage, useFolders, useIndexFailures, useTemplate } from "./data";
import { defaultLedgerView, orderedFileCategories } from "../templateEvidence";
import { type WorkspacePage, sectionState } from "./page";
import { ICON_SIZE } from "../../components/ui/icon";
import { onNewNoteRequest, takeNewNoteRequest } from "../../lib/assistantBridge";
import { useToolbarKeys } from "../../lib/useToolbarKeys";

export function EvidenceSection({ page }: { page: WorkspacePage }) {
  const { accessToken, workspace, workspaceId, canWrite, isSubmitting, setIsSubmitting, setError, load, onOpenArtifact, indexingStalled, liveStatus } = page;
  const { section } = page;
  const noteForm = useRef<HTMLFormElement>(null);
  const bulkToolbar = useToolbarKeys<HTMLDivElement>();
  // "New note" from the command palette: bring the note form into view and put the cursor in its title (V0.3.13).
  useEffect(() => {
    if (section !== "evidence" || !canWrite) return;
    const openEditor = () => {
      if (!takeNewNoteRequest(workspaceId)) return;
      noteForm.current?.scrollIntoView?.({ block: "start" });
      noteForm.current?.querySelector<HTMLInputElement>("input")?.focus();
    };
    openEditor();
    return onNewNoteRequest(openEditor);
  }, [section, canWrite, workspaceId]);
  const isEvidenceView = page.section === "evidence";
  const isDocumentsView = page.section === "documents";
  const useS = sectionState(page.store, "evidence");
  const [artifactQuery, setArtifactQuery] = useS("artifactQuery", "");
  const [artifactTypes, setArtifactTypes] = useS<FileCategory[]>("artifactTypes", []);
  const [artifactPage, setArtifactPage] = useS("artifactPage", 0);
  const [artifactViewMode, setArtifactViewMode] = useS<ArtifactViewMode>("artifactViewMode", "grid");
  const [noteTitle, setNoteTitle] = useS("noteTitle", "");
  const [noteContent, setNoteContent] = useS("noteContent", "");
  // The note draft stays while the person moves between sections of this workspace.
  useUnsavedChanges(Boolean(noteTitle.trim() || noteContent.trim()), {
    keepsOn: (to) => { const route = parseRoute(to.split("?")[0]); return route.name === "workspace" && route.workspaceId === page.workspaceId; },
    subject: "the note you are writing",
  });
  const [uploadFiles, setUploadFiles] = useS<File[]>("uploadFiles", []);
  const [uploadProgress, setUploadProgress] = useS<UploadProgress | null>("uploadProgress", null);
  const [selectedDocumentId, setSelectedDocumentId] = useS<string | null>("selectedDocumentId", null);
  const [currentFolderId, setCurrentFolderId] = useS<string | null>("currentFolderId", null);
  const [newFolderOpen, setNewFolderOpen] = useS("newFolderOpen", false);
  const [newFolderName, setNewFolderName] = useS("newFolderName", "");
  const [itemDialog, setItemDialog] = useS<ItemDialog | null>("itemDialog", null);
  const [renameValue, setRenameValue] = useS("renameValue", "");
  const [moveTarget, setMoveTarget] = useS<string | null>("moveTarget", null);
  // Several files at once: pick them, then move, change their state or delete them together.
  const [selecting, setSelecting] = useS("selecting", false);
  const [selectedIds, setSelectedIds] = useS<Set<string>>("selectedIds", new Set());
  const [reviewIntent, setReviewIntent] = useState<{ status: "verified" | "needs_review" | "outdated"; artifacts: ArtifactSummary[] } | null>(null);
  const [reviewResult, setReviewResult] = useState<{ result: BulkArtifactsResult; artifacts: ArtifactSummary[] } | null>(null);
  // The server cuts the page on screen: the files of the folder open (or of the whole ledger while a name or a
  // file type is chosen), the name typed (after a short pause), the types and the page.
  const hasArtifactFilters = Boolean(artifactQuery.trim() || artifactTypes.length);
  const [searchedText, setSearchedText] = useState("");
  useEffect(() => {
    const timer = window.setTimeout(() => setSearchedText(artifactQuery.trim()), 250);
    return () => window.clearTimeout(timer);
  }, [artifactQuery]);
  const [evidencePage, , evidenceQuery] = useEvidencePage(accessToken, workspaceId, {
    query: searchedText,
    categories: artifactTypes,
    folderId: isEvidenceView && !(searchedText || artifactTypes.length) ? currentFolderId : undefined,
    documentsOnly: isDocumentsView,
    page: artifactPage,
  });
  const pagedArtifacts = evidencePage.items;
  const artifactTotal = evidencePage.total;
  // Files met on any page, so a selection that spans pages can still be acted on.
  const seenArtifacts = useRef(new Map<string, ArtifactSummary>());
  for (const artifact of pagedArtifacts) seenArtifacts.current.set(artifact.id, artifact);
  const selectedArtifacts = () => [...selectedIds].map((id) => seenArtifacts.current.get(id)).filter((artifact): artifact is ArtifactSummary => Boolean(artifact));
  const [indexFailures] = useIndexFailures(accessToken, workspaceId);
  const [template] = useTemplate(accessToken, workspaceId);
  const typeLabels = template?.labels.artifact_types;
  // The ledger opens in the template's view until the person picks one themselves; their choice then stays.
  const [viewChosen, setViewChosen] = useS("artifactViewChosen", false);
  const templateView = defaultLedgerView(template?.evidence);
  useEffect(() => { if (!viewChosen) setArtifactViewMode(templateView); }, [setArtifactViewMode, templateView, viewChosen]);
  const chooseViewMode = (mode: ArtifactViewMode) => { setViewChosen(true); setArtifactViewMode(mode); };
  const fileCategories = orderedFileCategories(template?.evidence);
  const [folders, setFolders] = useFolders(accessToken, workspaceId);
  useEffect(() => { setArtifactPage(0); }, [artifactQuery, artifactTypes, currentFolderId, section, workspace.workspace.id]);
  const hasFolderFilters = Boolean(artifactQuery.trim() || artifactTypes.length);
  const folderById = useMemo(() => new Map(folders.map((folder) => [folder.id, folder])), [folders]);
  const folderPath = useMemo(() => {
    const path: Folder[] = [];
    let cursor = currentFolderId ? folderById.get(currentFolderId) : undefined;
    while (cursor) {
      path.unshift(cursor);
      cursor = cursor.parent_id ? folderById.get(cursor.parent_id) : undefined;
    }
    return path;
  }, [currentFolderId, folderById]);
  const visibleFolders = useMemo(
    () => isEvidenceView && !hasFolderFilters ? folders.filter((folder) => (folder.parent_id ?? null) === currentFolderId) : [],
    [currentFolderId, folders, hasFolderFilters, isEvidenceView],
  );
  const folderItemCounts = useMemo(() => {
    const counts = new Map<string, number>();
    // A folder's items: the files directly in it (counted by the server) and its subfolders.
    for (const folder of folders) {
      counts.set(folder.id, (counts.get(folder.id) ?? 0) + folder.file_count);
      if (folder.parent_id) counts.set(folder.parent_id, (counts.get(folder.parent_id) ?? 0) + 1);
    }
    return counts;
  }, [folders]);
  const activeFolderId = isEvidenceView ? currentFolderId : null;
  // Repository files are browsed in the Repositories section, not the ledger.
  const documentArtifacts = useMemo(() => isDocumentsView ? pagedArtifacts : [], [isDocumentsView, pagedArtifacts]);
  const selectedDocument = documentArtifacts.find((artifact) => artifact.id === selectedDocumentId) ?? null;
  useEffect(() => {
    if (!isDocumentsView) {
      return;
    }

    setSelectedDocumentId((current) =>
      current && documentArtifacts.some((artifact) => artifact.id === current)
        ? current
        : documentArtifacts[0]?.id ?? null,
    );
  }, [documentArtifacts, isDocumentsView]);

  async function addNote(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!noteContent.trim()) { setError("Write something in the note body before storing it."); return; }
    setIsSubmitting(true); setError(null);
    try {
      await createSharedTextArtifact(accessToken, workspace.workspace.id, {
        title: noteTitle,
        content: noteContent,
        language: "Note",
        folder_id: activeFolderId,
      });
      setNoteTitle(""); setNoteContent("");
      await load();
      showToast("success", "Note stored. It is indexed in the background.");
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  /** Keeps the files the upload accepts and says why the others are left out.
   *  Files from a chosen folder keep their folder path (`webkitRelativePath`). */
  function chooseUploadFiles(list: FileList | null) {
    const { kept, unsupported, tooLarge, ignored } = selectUploads(Array.from(list ?? []), isDocumentsView);
    const notes = [
      unsupported ? `${unsupported} of an unsupported type` : "",
      tooLarge ? `${tooLarge} over 10 MiB` : "",
      ignored ? `${ignored} in hidden, dependency or build folders` : "",
    ].filter(Boolean);
    if (notes.length) showToast("warning", `${notes.join(", ")} left out.`);
    setUploadFiles(kept);
  }

  /** Finds or creates the folder chain for a file chosen inside a folder,
   *  below the folder being viewed. Deeper levels than allowed go into the
   *  deepest folder that fits. */
  async function uploadFolderFor(file: File, created: Map<string, string>, known: Folder[]): Promise<string | null> {
    const segments = (file.webkitRelativePath || "").split("/").slice(0, -1);
    let parentId = activeFolderId;
    const room = MAX_FOLDER_DEPTH - (activeFolderId ? folderPath.length : 0);
    for (let index = 0; index < Math.min(segments.length, Math.max(0, room)); index += 1) {
      const key = segments.slice(0, index + 1).join("/");
      const cached = created.get(key);
      if (cached) { parentId = cached; continue; }
      const name = segments[index].slice(0, 80);
      const existing = known.find((folder) => (folder.parent_id ?? null) === parentId && folder.name === name);
      const folder = existing ?? await createSharedFolder(accessToken, workspace.workspace.id, { name, parentId });
      if (!existing) known.push(folder);
      created.set(key, folder.id);
      parentId = folder.id;
    }
    return parentId;
  }

  async function submitUpload(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = event.currentTarget;
    const files = uploadFiles;
    if (!files.length) return;
    setIsSubmitting(true); setError(null);
    setUploadProgress({ done: 0, total: files.length, failed: 0 });
    const failures: string[] = [];
    let lastDocument: ArtifactSummary | null = null;
    let uploaded = 0;
    try {
      // Folders are created one at a time first, so parallel uploads never
      // race to create the same one.
      const created = new Map<string, string>();
      const known = [...folders];
      const targets: Array<{ file: File; folderId: string | null }> = [];
      for (const file of files) targets.push({ file, folderId: await uploadFolderFor(file, created, known) });
      if (created.size) setFolders(known);
      let next = 0;
      const worker = async () => {
        while (next < targets.length) {
          const { file, folderId } = targets[next++];
          try {
            const artifact = await uploadSharedArtifact(accessToken, workspace.workspace.id, file, folderId);
            uploaded += 1;
            if (isDocumentArtifact(artifact)) lastDocument = artifact;
          } catch (requestError) {
            failures.push(`${file.name}: ${apiMessage(requestError)}`);
          }
          setUploadProgress({ done: uploaded + failures.length, total: targets.length, failed: failures.length });
        }
      };
      await Promise.all(Array.from({ length: Math.min(UPLOAD_CONCURRENCY, targets.length) }, worker));
      const uploadedDocument = lastDocument as ArtifactSummary | null;
      if (isDocumentsView && uploadedDocument) setSelectedDocumentId(uploadedDocument.id);
      setUploadFiles([]);
      form.reset();
      await load();
      if (uploaded) showToast("success", uploaded === 1 ? "File uploaded. It is indexed in the background." : `${uploaded} files uploaded. They are indexed in the background.`);
      if (failures.length) setError(failures.length === 1 ? failures[0] : `${failures.length} files were not uploaded. ${failures.slice(0, 3).join(" · ")}${failures.length > 3 ? " …" : ""}`);
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); setUploadProgress(null); }
  }

  function toggleArtifactType(type: FileCategory) {
    setArtifactTypes((current) => current.includes(type)
      ? current.filter((currentType) => currentType !== type)
      : [...current, type]);
  }

  async function addFolder(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSubmitting(true); setError(null);
    try {
      const folder = await createSharedFolder(accessToken, workspace.workspace.id, { name: newFolderName, parentId: currentFolderId });
      setFolders((current) => [...current, folder]);
      setNewFolderName(""); setNewFolderOpen(false);
      showToast("success", `Folder "${folder.name}" created.`);
    } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  function openItemDialog(next: ItemDialog) {
    setError(null);
    if (next.kind === "rename-folder") setRenameValue(next.folder.name);
    if (next.kind === "move-file") setMoveTarget(next.artifact.folder_id ?? null);
    if (next.kind === "move-folder") setMoveTarget(next.folder.parent_id ?? null);
    if (next.kind === "move-files") setMoveTarget(null);
    setItemDialog(next);
  }

  /** The folder and everything below it, with the number of files inside. */
  function folderSubtree(rootId: string) {
    const ids = new Set([rootId]);
    let grew = true;
    while (grew) {
      grew = false;
      for (const folder of folders) {
        if (folder.parent_id && ids.has(folder.parent_id) && !ids.has(folder.id)) { ids.add(folder.id); grew = true; }
      }
    }
    const files = folders.filter((folder) => ids.has(folder.id)).reduce((sum, folder) => sum + folder.file_count, 0);
    return { ids, files, subfolders: ids.size - 1 };
  }

  async function runItemAction(action: () => Promise<void>) {
    setIsSubmitting(true); setError(null);
    try { await action(); setItemDialog(null); } catch (requestError) { setError(apiMessage(requestError)); } finally { setIsSubmitting(false); }
  }

  const renameFolderAction = (folder: Folder) => runItemAction(async () => {
    const renamed = await renameSharedFolder(accessToken, workspace.workspace.id, folder.id, renameValue);
    setFolders((current) => current.map((entry) => entry.id === renamed.id ? renamed : entry));
    showToast("success", "Folder renamed.");
  });

  const deleteFolderAction = (folder: Folder) => runItemAction(async () => {
    const { ids } = folderSubtree(folder.id);
    await deleteSharedFolder(accessToken, workspace.workspace.id, folder.id);
    if (currentFolderId && ids.has(currentFolderId)) setCurrentFolderId(folder.parent_id);
    await load();
    showToast("success", `Folder "${folder.name}" deleted.`);
  });

  const deleteFileAction = (artifact: ArtifactSummary) => runItemAction(async () => {
    await deleteSharedArtifact(accessToken, artifact.id);
    if (selectedDocumentId === artifact.id) setSelectedDocumentId(null);
    await load();
    showToast("success", `"${artifact.title}" deleted.`);
  });

  const moveFileAction = (artifact: ArtifactSummary) => runItemAction(async () => {
    await moveSharedArtifact(accessToken, artifact.id, moveTarget);
    await load();
    showToast("success", moveTarget ? `Moved to ${folderById.get(moveTarget)?.name ?? "folder"}.` : "Moved to the top level.");
  });

  const moveFolderAction = (folder: Folder) => runItemAction(async () => {
    const moved = await moveSharedFolder(accessToken, workspace.workspace.id, folder.id, moveTarget);
    setFolders((current) => current.map((entry) => entry.id === moved.id ? moved : entry));
    showToast("success", moveTarget ? `Moved to ${folderById.get(moveTarget)?.name ?? "folder"}.` : "Moved to the top level.");
  });

  /** One action over the selected files; those that could not be handled are reported, the rest done. */
  const bulkAction = (input: { action: "move" | "delete" | "lifecycle"; folderId?: string | null; status?: "active" | "needs_review" | "verified" | "outdated" }) => runItemAction(async () => {
    const result = await bulkSharedArtifacts(accessToken, workspace.workspace.id, { ...input, artifactIds: [...selectedIds] });
    if (result.failed.length) {
      showToast("warning", `${result.done.length} done, ${result.failed.length} left alone: ${result.failed[0].message}`);
    } else {
      showToast("success", `${result.done.length} ${result.done.length === 1 ? "file" : "files"} updated.`);
    }
    setSelectedIds(new Set());
    setSelecting(false);
    await load();
  });

  const reviewStatusLabel = (status: "verified" | "needs_review" | "outdated") => status.replace(/_/g, " ");
  const previewReview = (status: "verified" | "needs_review" | "outdated") => {
    setReviewResult(null);
    setReviewIntent({ status, artifacts: selectedArtifacts() });
  };
  const applyReview = () => {
    if (!reviewIntent) return;
    const { status, artifacts } = reviewIntent;
    void runItemAction(async () => {
      const result = await bulkSharedArtifacts(accessToken, workspaceId, { action: "lifecycle", status, artifactIds: [...new Set(artifacts.map((artifact) => artifact.id))] });
      setReviewResult({ result, artifacts });
      setSelectedIds(new Set(result.failed.map((failure) => failure.artifact_id)));
      setSelecting(result.failed.length > 0);
      setReviewIntent(null);
      await load();
      showToast(result.failed.length ? "warning" : "success", `${result.done.length} reviewed; ${result.failed.length} failed.`);
    });
  };

  function toggleSelected(artifactId: string) {
    setSelectedIds((current) => {
      const next = new Set(current);
      if (!next.delete(artifactId)) next.add(artifactId);
      return next;
    });
  }

  const folderPickerRows = (() => {
    const hidden = itemDialog?.kind === "move-folder" ? folderSubtree(itemDialog.folder.id).ids : new Set<string>();
    const rows: Array<{ folder: Folder; depth: number }> = [];
    const walk = (parentId: string | null, depth: number) => {
      for (const folder of folders.filter((entry) => (entry.parent_id ?? null) === parentId && !hidden.has(entry.id))) {
        rows.push({ folder, depth });
        walk(folder.id, depth + 1);
      }
    };
    walk(null, 0);
    return rows;
  })();

  const itemDialogs = (() => {
    if (!itemDialog) return null;
    const busy = isSubmitting;
    if (itemDialog.kind === "rename-folder") {
      const { folder } = itemDialog;
      return <Dialog footer={<><DialogCancel onClick={() => setItemDialog(null)} /><Button busy={busy} disabled={!renameValue.trim()} type="submit" variant="main">Rename</Button></>} onClose={() => setItemDialog(null)} onSubmit={() => { if (renameValue.trim()) void renameFolderAction(folder); }} open title="Rename folder">
        <Field label="Folder name"><Input maxLength={80} onChange={(event) => setRenameValue(event.target.value)} value={renameValue} /></Field>
      </Dialog>;
    }
    if (itemDialog.kind === "delete-folder") {
      const { folder } = itemDialog;
      const { files, subfolders } = folderSubtree(folder.id);
      const contents = files || subfolders
        ? `This also permanently deletes ${files} ${files === 1 ? "file" : "files"} and ${subfolders} ${subfolders === 1 ? "subfolder" : "subfolders"} inside it.`
        : "The folder is empty.";
      return <Dialog description={<>Delete the folder <strong>{folder.name}</strong>? {contents} This cannot be undone.</>} footer={<><DialogCancel onClick={() => setItemDialog(null)} /><Button busy={busy} onClick={() => void deleteFolderAction(folder)} variant="danger">Delete folder</Button></>} onClose={() => setItemDialog(null)} open title="Delete folder" />;
    }
    if (itemDialog.kind === "delete-file") {
      const { artifact } = itemDialog;
      return <Dialog description={<>Permanently delete <strong>{artifact.title}</strong> and its index? Links to it from memory cards are removed too. This cannot be undone.</>} footer={<><DialogCancel onClick={() => setItemDialog(null)} /><Button busy={busy} onClick={() => void deleteFileAction(artifact)} variant="danger">Delete file</Button></>} onClose={() => setItemDialog(null)} open title="Delete file" />;
    }
    if (itemDialog.kind === "delete-files") {
      const { artifacts: chosen } = itemDialog;
      return <Dialog description={<>Permanently delete <strong>{chosen.length} {chosen.length === 1 ? "file" : "files"}</strong> and their indexes? Links to them from memory cards are removed too. This cannot be undone.</>} footer={<><DialogCancel onClick={() => setItemDialog(null)} /><Button busy={busy} onClick={() => void bulkAction({ action: "delete" })} variant="danger">Delete {chosen.length} {chosen.length === 1 ? "file" : "files"}</Button></>} onClose={() => setItemDialog(null)} open title="Delete files" />;
    }
    const subject = itemDialog.kind === "move-folder" ? itemDialog.folder : itemDialog.kind === "move-files" ? null : itemDialog.artifact;
    const unchanged = itemDialog.kind === "move-folder"
      ? moveTarget === (itemDialog.folder.parent_id ?? null)
      : itemDialog.kind === "move-files" ? false : moveTarget === (itemDialog.artifact.folder_id ?? null);
    const run = () => itemDialog.kind === "move-folder"
      ? moveFolderAction(itemDialog.folder)
      : itemDialog.kind === "move-files" ? bulkAction({ action: "move", folderId: moveTarget }) : moveFileAction(itemDialog.artifact);
    const name = subject ? ("title" in subject ? subject.title : subject.name) : `${selectedIds.size} selected files`;
    return <Dialog description={<>Choose where to put <strong>{name}</strong>.</>} footer={<><DialogCancel onClick={() => setItemDialog(null)} /><Button busy={busy} disabled={unchanged} onClick={() => void run()} variant="main">Move here</Button></>} onClose={() => setItemDialog(null)} open title="Move to folder">
      <div className="rm-option-list rm-option-list-box" role="group" aria-label="Destination folder">
        <button aria-pressed={moveTarget === null} className="rm-option" onClick={() => setMoveTarget(null)} type="button"><FolderIcon size={ICON_SIZE.md} /> All evidence (top level)</button>
        {folderPickerRows.map(({ folder, depth }) => <button aria-pressed={moveTarget === folder.id} className="rm-option" key={folder.id} onClick={() => setMoveTarget(folder.id)} style={{ paddingLeft: 10 + (depth + 1) * 16 }} type="button"><FolderIcon size={ICON_SIZE.md} /> {folder.name}</button>)}
      </div>
    </Dialog>;
  })();

  return (
          <section className="shared-detail-panel">
            <div className="shared-artifact-browser">
              {itemDialogs}
              {reviewIntent ? <Dialog description={<>Review the selection before marking it <strong>{reviewStatusLabel(reviewIntent.status)}</strong>. Each item keeps its review history.</>} footer={<><DialogCancel onClick={() => setReviewIntent(null)} /><Button busy={isSubmitting} onClick={applyReview} variant="main">Mark {reviewIntent.artifacts.length} {reviewIntent.artifacts.length === 1 ? "item" : "items"} {reviewStatusLabel(reviewIntent.status)}</Button></>} onClose={() => setReviewIntent(null)} open title="Review selected evidence"><ul className="shared-bulk-review-list">{reviewIntent.artifacts.map((artifact) => <li key={artifact.id}><strong>{artifact.title}</strong><span>{artifact.path}</span></li>)}</ul></Dialog> : null}
              <div className="shared-artifact-filters">
                <SearchField aria-label="Search evidence files" onValueChange={setArtifactQuery} placeholder="Search file name or path" value={artifactQuery} />
                <details className="shared-artifact-type-menu">
                  <summary><Filter size={ICON_SIZE.md} /><span>{artifactTypes.length ? `${artifactTypes.length} types` : "File types"}</span><ChevronDown size={ICON_SIZE.md} /></summary>
                  <div className="shared-artifact-type-menu-panel">
                    <Checkbox checked={artifactTypes.length === 0} label="All types" onCheckedChange={() => setArtifactTypes([])} />
                    {fileCategories.map((option) => {
                      const isSelected = artifactTypes.includes(option.value);
                      return <Checkbox checked={isSelected} key={option.value} label={<>{fileCategoryIcon(option.value, ICON_SIZE.sm)}<span>{option.label}</span></>} onCheckedChange={() => toggleArtifactType(option.value)} />;
                    })}
                  </div>
                </details>
                {isEvidenceView && canWrite ? <Tooltip content={folderPath.length >= MAX_FOLDER_DEPTH ? `Folders can be nested up to ${MAX_FOLDER_DEPTH} levels deep` : "Create a folder here"}><span className="rm-tooltip-anchor"><Button disabled={folderPath.length >= MAX_FOLDER_DEPTH} onClick={() => setNewFolderOpen((open) => !open)} type="button" variant="secondary"><FolderPlus size={ICON_SIZE.md} /> New folder</Button></span></Tooltip> : null}
              </div>
              {isEvidenceView && newFolderOpen ? <form className="shared-folder-form" onSubmit={addFolder}><Field label="Folder name" required><Input autoFocus maxLength={80} onChange={(event) => setNewFolderName(event.target.value)} placeholder={folderPath.length ? `New folder in ${folderPath[folderPath.length - 1].name}` : "New folder name"} value={newFolderName} /></Field><Button busy={isSubmitting} type="submit" variant="main">Create</Button><Button onClick={() => { setNewFolderOpen(false); setNewFolderName(""); }} variant="secondary">Cancel</Button></form> : null}
              {evidenceQuery?.isLoading ? <FeedbackNotice kind="loading" title={isDocumentsView ? "Loading documents" : "Loading evidence"} /> : null}{evidenceQuery?.error ? <FeedbackNotice action="Try again" detail={apiMessage(evidenceQuery?.error)} kind="error" onAction={() => void evidenceQuery?.refetch()} title={isDocumentsView ? "Documents could not be loaded" : "Evidence could not be loaded"} /> : null}{liveStatus}<div className="shared-artifact-browser-meta"><div className="shared-artifact-browser-summary">{isEvidenceView && !hasFolderFilters && (folders.length || currentFolderId) ? <nav aria-label="Folder path" className="shared-folder-path"><button className={currentFolderId ? "" : "current"} onClick={() => setCurrentFolderId(null)} type="button">All evidence</button>{folderPath.map((folder, index) => <span key={folder.id}><ChevronRight size={ICON_SIZE.sm} /><button className={index === folderPath.length - 1 ? "current" : ""} onClick={() => setCurrentFolderId(folder.id)} type="button">{folder.name}</button></span>)}</nav> : null}<span>{isEvidenceView && !hasFolderFilters ? `${visibleFolders.length} ${visibleFolders.length === 1 ? "folder" : "folders"} · ${artifactTotal} ${artifactTotal === 1 ? "file" : "files"}` : `${artifactTotal} ${artifactTotal === 1 ? "file" : "files"}${hasArtifactFilters ? " matching filters" : ""}`}</span><span className="shared-artifact-sort-note">Newest first</span></div><SegmentedControl aria-label="Evidence view" onValueChange={chooseViewMode} options={[{ icon: <Grid size={ICON_SIZE.md} />, name: "Grid view", value: "grid" }, { icon: <List size={ICON_SIZE.md} />, name: "List view", value: "list" }]} value={artifactViewMode} /></div>
              {canWrite && isEvidenceView && pagedArtifacts.length ? <div aria-label="Select several files" aria-orientation="horizontal" className="shared-bulk-bar" onFocus={bulkToolbar.onFocus} onKeyDown={bulkToolbar.onKeyDown} ref={bulkToolbar.ref} role="toolbar">{selecting ? <><span>{selectedIds.size} selected</span><Button disabled={isSubmitting || !selectedIds.size} onClick={() => openItemDialog({ kind: "move-files", artifacts: selectedArtifacts() })} type="button" variant="secondary"><FolderMove size={ICON_SIZE.sm} /> Move…</Button><Button disabled={isSubmitting || !selectedIds.size} onClick={() => previewReview("verified")} type="button" variant="secondary">Mark verified</Button><Button disabled={isSubmitting || !selectedIds.size} onClick={() => previewReview("needs_review")} type="button" variant="secondary">Needs review</Button><Button disabled={isSubmitting || !selectedIds.size} onClick={() => previewReview("outdated")} type="button" variant="secondary">Mark outdated</Button><Button disabled={isSubmitting || !selectedIds.size} onClick={() => openItemDialog({ kind: "delete-files", artifacts: selectedArtifacts() })} type="button" variant="secondary"><Trash size={ICON_SIZE.sm} /> Delete</Button><Button onClick={() => { setSelecting(false); setSelectedIds(new Set()); }} type="button" variant="secondary">Done</Button></> : <Button onClick={() => setSelecting(true)} type="button" variant="secondary">Select files</Button>}</div> : null}
              {reviewResult ? <section aria-label="Bulk review results" aria-live="polite" className="shared-bulk-review-results"><div><strong>Review results</strong><Button onClick={() => setReviewResult(null)} type="button" variant="secondary">Dismiss</Button></div><ul>{reviewResult.artifacts.map((artifact) => { const failure = reviewResult.result.failed.find((entry) => entry.artifact_id === artifact.id); return <li key={artifact.id}><span>{artifact.title}</span><span>{failure ? `Failed: ${failure.message}` : "Updated"}</span></li>; })}</ul></section> : null}
              {pagedArtifacts.length || visibleFolders.length ? <div className={`shared-artifact-manager ${artifactViewMode}`}>{visibleFolders.map((folder) => <article key={folder.id}><Button className="shared-artifact-entry shared-folder-entry" onClick={() => setCurrentFolderId(folder.id)} type="button" variant="secondary"><span className="shared-artifact-file-icon category-folder"><FolderIcon size={20} /></span><span className="shared-artifact-entry-copy"><strong>{folder.name}</strong></span><span className="shared-artifact-entry-meta"><span>{folderItemCounts.get(folder.id) ?? 0} {(folderItemCounts.get(folder.id) ?? 0) === 1 ? "item" : "items"}</span></span></Button>{canWrite ? <div className="shared-item-actions"><ActionMenu items={[{ label: "Rename", icon: <Pencil size={ICON_SIZE.sm} />, onSelect: () => openItemDialog({ kind: "rename-folder", folder }) }, { label: "Move to folder…", icon: <FolderMove size={ICON_SIZE.sm} />, onSelect: () => openItemDialog({ kind: "move-folder", folder }) }, { label: "Delete folder", icon: <Trash size={ICON_SIZE.sm} />, destructive: true, onSelect: () => openItemDialog({ kind: "delete-folder", folder }) }]} label={`Actions for folder ${folder.name}`} /></div> : null}</article>)}{pagedArtifacts.map((artifact) => <article key={artifact.id}>{selecting && canWrite ? <Checkbox checked={selectedIds.has(artifact.id)} className="shared-select-box" disabled={Boolean(artifact.repository_id)} label={<span className="sr-only">Select {artifact.title}</span>} onCheckedChange={() => toggleSelected(artifact.id)} /> : null}<Button className={isDocumentsView && artifact.id === selectedDocument?.id ? "shared-artifact-entry selected" : "shared-artifact-entry"} onClick={() => selecting && canWrite ? (artifact.repository_id ? undefined : toggleSelected(artifact.id)) : isDocumentsView ? setSelectedDocumentId(artifact.id) : onOpenArtifact(artifact.id)} type="button" variant="secondary"><span className={`shared-artifact-file-icon category-${fileCategory(artifact)}`}>{fileCategoryIcon(fileCategory(artifact), 20)}</span><span className="shared-artifact-entry-copy"><strong>{artifact.title}</strong><span>{artifact.path}</span></span><span className="shared-artifact-entry-meta"><span>{artifactKindLabel(artifact, typeLabels)}</span><span>{formatBytes(artifact.size_bytes)}</span></span>{artifact.indexed_at ? null : indexFailures[artifact.id] || indexingStalled ? <span aria-label="Not indexed" className="shared-artifact-status failed" role="img" title={indexFailures[artifact.id] ? `Indexing failed: ${indexFailures[artifact.id].message}` : "Indexing problem: this file is not indexed"}><AlertCircle size={ICON_SIZE.md} /></span> : <span aria-label="Indexing" className="shared-artifact-status" role="img" title="Indexing…"><span aria-hidden="true" className="rm-indexing"><i /><i /><i /></span></span>}</Button>{canWrite ? <div className="shared-item-actions"><ActionMenu items={[{ label: "Move to folder…", icon: <FolderMove size={ICON_SIZE.sm} />, onSelect: () => openItemDialog({ kind: "move-file", artifact }) }, ...(artifact.artifact_type === "repository" ? [] : [{ label: "Delete file", icon: <Trash size={ICON_SIZE.sm} />, destructive: true, onSelect: () => openItemDialog({ kind: "delete-file", artifact }) }])]} label={`Actions for ${artifact.title}`} /></div> : null}</article>)}</div> : <EmptyState description={isEvidenceView && currentFolderId && !hasFolderFilters ? "Add a note or upload a file below, or create a folder inside it." : hasArtifactFilters ? "Adjust the search or selected types to see other workspace files." : isDocumentsView ? "Upload a Word, Excel, PowerPoint, PDF, OneNote or Outlook file to preview it here." : "Add a pasted note below, then index it when you are ready to search."} icon={<FileText size={25} />} action={hasArtifactFilters ? <Button onClick={() => { setArtifactQuery(""); setArtifactTypes([]); }} type="button" variant="secondary">Clear filters</Button> : null} title={isEvidenceView && currentFolderId && !hasFolderFilters ? "This folder is empty" : hasArtifactFilters ? "No files match these filters" : isDocumentsView ? "No documents yet" : "No shared evidence yet"} />}
              <Pagination onPageChange={setArtifactPage} page={artifactPage} total={artifactTotal} />
            </div>
            {isDocumentsView ? <section className="shared-document-preview" aria-live="polite"><PanelHeading description={selectedDocument ? documentKindLabel(selectedDocument) : `${artifactTotal} documents`} icon={Book} title="Preview" />{selectedDocument ? <DocumentViewer accessToken={accessToken} artifact={selectedDocument} key={selectedDocument.id} /> : <EmptyState description="Choose a document above to preview it." icon={<FileText size={25} />} title="No document selected" />}</section> : null}
            {canWrite ? <div className="shared-evidence-additions">{isEvidenceView ? <form className="shared-note-form" onSubmit={addNote} ref={noteForm}><h3>Add shared note{activeFolderId ? <small className="shared-folder-hint"> in {folderPath.map((folder) => folder.name).join(" / ")}</small> : null}</h3><Field label="Note title" required><Input onChange={(event) => setNoteTitle(event.target.value)} placeholder="Decision or implementation note" value={noteTitle} /></Field><RichNoteEditor onChange={setNoteContent} placeholder="Write a note. Use the toolbar or Markdown for formatting…" value={noteContent} /><small className="rm-field-hint">{submitHint("stores the note")}</small><Button busy={isSubmitting} type="submit" variant="main"><Plus size={ICON_SIZE.md} /> Store evidence</Button></form> : null}<form className="shared-upload-form" onSubmit={submitUpload}><div><strong>{isDocumentsView ? "Upload documents" : "Upload files"}{activeFolderId ? <small className="shared-folder-hint"> to {folderPath.map((folder) => folder.name).join(" / ")}</small> : null}</strong><span>{isDocumentsView ? "Word, Excel, PowerPoint, PDF, OneNote, Outlook · up to 10 MiB each · text is extracted locally for search" : "Markdown, text, code, image, or business document · up to 10 MiB each · choose several files or a whole folder"}</span></div><label className="shared-upload-picker"><input accept={isDocumentsView ? DOCUMENT_ACCEPT : `${UPLOAD_EXTENSIONS.map((extension) => `.${extension}`).join(",")},${DOCUMENT_ACCEPT}`} aria-label={isDocumentsView ? "Upload documents" : "Upload shared files"} className="shared-upload-native-input" disabled={isSubmitting} multiple onChange={(event) => chooseUploadFiles(event.target.files)} type="file" /><span className="shared-upload-picker-icon"><Upload size={ICON_SIZE.lg} /></span><span className="shared-upload-picker-copy"><strong>{uploadFiles.length === 1 ? uploadFiles[0].name : uploadFiles.length ? `${uploadFiles.length} files selected` : isDocumentsView ? "Choose documents" : "Choose shared files"}</strong><span>{uploadProgress ? `Uploading ${uploadProgress.done} of ${uploadProgress.total}${uploadProgress.failed ? ` · ${uploadProgress.failed} failed` : ""}…` : uploadFiles.length ? `${formatBytes(uploadFiles.reduce((total, file) => total + file.size, 0))} · ready to upload` : isDocumentsView ? "Word, Excel, PowerPoint, PDF, OneNote or Outlook" : "Markdown, text, code, image, or business document"}</span></span><span className="shared-upload-picker-action">Browse</span></label>{uploadProgress ? <progress aria-label="Upload progress" className="shared-upload-progress" max={uploadProgress.total} value={uploadProgress.done} /> : null}<div className="shared-upload-actions">{isEvidenceView ? <label className="rm-button rm-button-secondary shared-upload-folder"><FolderIcon size={ICON_SIZE.md} /> Choose a folder<input {...({ webkitdirectory: "", directory: "" } as Record<string, string>)} aria-label="Upload a folder" className="shared-upload-native-input" disabled={isSubmitting} onChange={(event) => chooseUploadFiles(event.target.files)} type="file" /></label> : null}<Button busy={Boolean(uploadProgress)} disabled={isSubmitting || !uploadFiles.length} type="submit" variant="secondary"><Upload size={ICON_SIZE.md} /> {uploadFiles.length > 1 ? `Upload ${uploadFiles.length} files` : "Upload"}</Button></div></form></div> : <p className="shared-readonly-note"><Shield size={ICON_SIZE.sm} /> Your viewer membership can inspect shared evidence but cannot change it.</p>}
          </section>
  );
}
