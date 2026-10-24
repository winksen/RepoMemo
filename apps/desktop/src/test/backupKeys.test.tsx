import { QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({ getSystemBackups: vi.fn(), resealSystemBackup: vi.fn() }));
vi.mock("../components/ui/toast", () => ({ showToast: vi.fn() }));

import { getSystemBackups, resealSystemBackup } from "../lib/sharedApi";
import { showToast } from "../components/ui/toast";
import { queryClient } from "../lib/queryClient";
import { BackupsPanel } from "../web/system/Backups";
import type { BackupManifest, BackupsOverview } from "../types";

const encryption = { cipher: "aes-256-gcm", kdf: "pbkdf2-hmac-sha256", iterations: 600000, salt: "00", key_check: "11", chunk_bytes: 65536 };
const backup = (name: string, changes: Partial<BackupManifest>): BackupManifest => ({
  name, created_at: "2026-10-24T10:00:00Z", version: "0.3.24", trigger: "manual", database_bytes: 1024, blob_count: 2, blob_bytes: 2048,
  missing_blobs: 0, reused_blobs: 0, includes_key: true, encryption, ...changes,
} as BackupManifest);
const overview = (backups: BackupManifest[], changes: Partial<BackupsOverview> = {}): BackupsOverview => ({
  backups, status: { running: false, operation: null, started_at: null, finished_at: null, succeeded: null, message: null },
  directory: "/data/backups", keep: 7, interval_hours: 24, disk_free_bytes: 1 << 30, encryption_enabled: true, previous_key_set: true, ...changes,
});

beforeEach(() => {
  queryClient.clear();
  vi.mocked(getSystemBackups).mockReset();
  vi.mocked(resealSystemBackup).mockReset();
  vi.mocked(showToast).mockReset();
});
afterEach(() => cleanup());

function renderPanel() {
  return render(<QueryClientProvider client={queryClient}><BackupsPanel accessToken="t" canChange /></QueryClientProvider>);
}

describe("backups and a changed passphrase", () => {
  it("marks the backups the previous key made, or no key opens, and says the previous key is kept", async () => {
    vi.mocked(getSystemBackups).mockResolvedValue(overview([
      backup("backup-3", { sealed_with: "current" }),
      backup("backup-2", { sealed_with: "previous" }),
      backup("backup-1", { sealed_with: "neither" }),
    ]));
    renderPanel();
    const rows = (await screen.findAllByRole("row")).slice(1);
    expect(rows).toHaveLength(3);
    expect(within(rows[0]).queryByText("previous key")).toBeNull();
    expect(within(rows[1]).getByText("previous key")).toBeTruthy();
    expect(within(rows[2]).getByText("key not available")).toBeTruthy();
    expect(screen.getByText(/REPOMEMO_BACKUP_KEY_PREVIOUS opens the backups made before the key changed/)).toBeTruthy();
  });

  it("offers to seal again only a backup the previous key made, and starts it", async () => {
    vi.mocked(getSystemBackups).mockResolvedValue(overview([backup("backup-3", { sealed_with: "current" }), backup("backup-2", { sealed_with: "previous" })]));
    vi.mocked(resealSystemBackup).mockResolvedValue(overview([backup("backup-3", { sealed_with: "current" }), backup("backup-2", { sealed_with: "previous" })], {
      status: { running: true, operation: "reseal", started_at: "2026-10-24T11:00:00Z", finished_at: null, succeeded: null, message: null },
    }));
    renderPanel();
    await screen.findByText("backup-2");

    fireEvent.keyDown(screen.getByRole("button", { name: "Actions for backup-3" }), { key: "ArrowDown" });
    await screen.findByRole("menuitem", { name: /restore as a new environment/i });
    expect(screen.queryByRole("menuitem", { name: /seal again/i })).toBeNull();
    fireEvent.keyDown(document.activeElement ?? document.body, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("menuitem", { name: /restore as a new environment/i })).toBeNull());

    fireEvent.keyDown(screen.getByRole("button", { name: "Actions for backup-2" }), { key: "ArrowDown" });
    fireEvent.click(await screen.findByRole("menuitem", { name: /seal again with the current key/i }));
    await waitFor(() => expect(resealSystemBackup).toHaveBeenCalledWith("t", "backup-2"));
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("success", "Sealing the backup again with the current key was started."));
  });

  it("says why when sealing again could not start", async () => {
    vi.mocked(getSystemBackups).mockResolvedValue(overview([backup("backup-2", { sealed_with: "previous" })]));
    vi.mocked(resealSystemBackup).mockRejectedValue(new Error("A backup or restore is already running."));
    renderPanel();
    await screen.findByText("backup-2");
    fireEvent.keyDown(screen.getByRole("button", { name: "Actions for backup-2" }), { key: "ArrowDown" });
    fireEvent.click(await screen.findByRole("menuitem", { name: /seal again with the current key/i }));
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", "A backup or restore is already running."));
  });
});
