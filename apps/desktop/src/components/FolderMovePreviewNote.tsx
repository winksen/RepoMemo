import { useEffect, useState } from "react";
import { previewSharedFolderMove } from "../lib/sharedApi";
import type { FolderMovePreview } from "../types";
import { apiMessage } from "../web/format";

const plural = (count: number, one: string, many: string) => `${count} ${count === 1 ? one : many}`;

/** The sentence that says what moving a folder does, from its preview (V0.6.24). */
export function folderMoveSummary(preview: FolderMovePreview): string {
  if (!preview.allowed) return preview.refusal ?? "This move is not allowed.";
  if (preview.unchanged) return `${preview.folder_name} is already there.`;
  const inside = [plural(preview.files, "file", "files"), preview.subfolders ? plural(preview.subfolders, "folder inside it", "folders inside it") : ""].filter(Boolean).join(" and ");
  const attached = preview.citations || preview.comments
    ? ` ${plural(preview.citing_cards, "memory card cites", "memory cards cite")} these files (${plural(preview.citations, "citation", "citations")}) and ${plural(preview.comments, "comment is", "comments are")} on them: they all stay attached, because files keep their identity.`
    : " Nothing cites these files and nothing is commented on them.";
  return `${preview.from_path} becomes ${preview.to_path}, with ${inside}.${attached}`;
}

/**
 * What a folder move will do, shown in the move dialog before the person confirms: where the folder ends up, what is inside it,
 * what stays attached, or why the move is refused. The preview changes nothing.
 */
export function FolderMovePreviewNote({ accessToken, folderId, parentId, workspaceId }: { accessToken: string; folderId: string; parentId: string | null; workspaceId: string }) {
  const [preview, setPreview] = useState<FolderMovePreview | null>(null);
  const [problem, setProblem] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setPreview(null);
    setProblem(null);
    previewSharedFolderMove(accessToken, workspaceId, folderId, parentId)
      .then((next) => { if (!cancelled) setPreview(next); })
      .catch((error) => { if (!cancelled) setProblem(apiMessage(error, "The preview could not be read.")); });
    return () => { cancelled = true; };
  }, [accessToken, workspaceId, folderId, parentId]);

  if (problem) return <p className="shared-muted-copy" role="status">{problem}</p>;
  if (!preview) return <p aria-busy="true" className="shared-muted-copy" role="status">Checking what the move would do…</p>;
  return <p className="shared-muted-copy" role="status">{folderMoveSummary(preview)}</p>;
}
