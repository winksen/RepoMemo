import { useState } from "react";
import { IconDownload as Download, IconPackageExport as PackageExport } from "@tabler/icons-react";
import { downloadSharedWorkspaceArchive } from "../lib/sharedApi";
import { Button } from "./ui/button";
import { PanelHeading } from "./ui/panel-heading";
import { showToast } from "./ui/toast";
import { ICON_SIZE } from "./ui/icon";
import { apiMessage } from "../web/format";

/** Downloads the workspace as one archive file. Shown to owners and administrators. */
export function WorkspaceExportSettings({ accessToken, workspaceId }: { accessToken: string; workspaceId: string }) {
  const [isExporting, setIsExporting] = useState(false);

  async function exportWorkspace() {
    setIsExporting(true);
    try {
      const { blob, filename } = await downloadSharedWorkspaceArchive(accessToken, workspaceId);
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = filename;
      document.body.appendChild(link);
      link.click();
      link.remove();
      window.setTimeout(() => URL.revokeObjectURL(url), 10_000);
      showToast("success", "The workspace archive was downloaded.");
    } catch (error) {
      showToast("error", apiMessage(error, "The workspace could not be exported."));
    } finally { setIsExporting(false); }
  }

  return (
    <section className="shared-settings-group">
      <PanelHeading description="owners and administrators" icon={PackageExport} title="Export this workspace" />
      <p className="shared-muted-copy">One file with the evidence and its original files, folders, memory cards with their citations, tasks, comments and each file's lifecycle history, plus a manifest. People are carried by name and address, never by account. Settings that hold secrets (AI provider keys, repository and tracker tokens) are not included.</p>
      <div><Button busy={isExporting} onClick={() => void exportWorkspace()} variant="secondary"><Download size={ICON_SIZE.md} /> {isExporting ? "Preparing the archive…" : "Download archive"}</Button></div>
    </section>
  );
}
