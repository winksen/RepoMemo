import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/desktop", () => ({
  isDesktopRuntime: true,
  desktopSettings: { get: vi.fn(async () => ({ name: null, supportUrl: null, suggestedServer: null, source: null, problem: null })) },
  desktopServer: { locked: vi.fn(async () => true) },
  appUpdates: { mode: vi.fn(), check: vi.fn(), install: vi.fn() },
}));
vi.mock("../components/ui/toast", () => ({ showToast: vi.fn(), Toast: () => null }));
vi.mock("../lib/sharedApi", () => ({ sharedApiUrl: "https://memo.example.com" }));

import { DesktopServerNote } from "../components/DesktopServerNote";
import { UpdateOffer } from "../components/UpdateOffer";
import { showToast } from "../components/ui/toast";
import { appUpdates } from "../lib/desktop";
import { readUpdateChoice, writeUpdateChoice } from "../lib/updateChoice";

beforeEach(() => window.localStorage.clear());
afterEach(() => { cleanup(); vi.clearAllMocks(); });

describe("the person's choice", () => {
  it("is off until they turn it on, and remembered", () => {
    expect(readUpdateChoice()).toBe(false);
    writeUpdateChoice(true);
    expect(readUpdateChoice()).toBe(true);
    writeUpdateChoice(false);
    expect(readUpdateChoice()).toBe(false);
  });

  it("is offered in the account area only when the project's feed is the one, and is saved when changed", async () => {
    vi.mocked(appUpdates.mode).mockResolvedValue("optional");
    render(<DesktopServerNote signedIn />);
    const box = (await screen.findByRole("checkbox", { name: "Check for updates" })) as HTMLInputElement;
    expect(box.checked).toBe(false);
    fireEvent.click(box);
    expect(readUpdateChoice()).toBe(true);
  });

  it("is not the person's when the administrator's feed is used, or there is none", async () => {
    for (const mode of ["managed", "off"] as const) {
      vi.mocked(appUpdates.mode).mockResolvedValue(mode);
      const view = render(<DesktopServerNote signedIn />);
      await screen.findByText("memo.example.com");
      expect(screen.queryByRole("checkbox", { name: "Check for updates" }), mode).toBeNull();
      view.unmount();
    }
  });
});

describe("an update that is found", () => {
  it("is offered with its version, and the shell is told the person's choice", async () => {
    writeUpdateChoice(true);
    vi.mocked(appUpdates.check).mockResolvedValue({ version: "0.4.99", notes: "Fixes the thing.\nMore." });
    render(<UpdateOffer />);
    expect(await screen.findByText("Version 0.4.99 is available")).toBeTruthy();
    expect(screen.getByText("Fixes the thing.")).toBeTruthy();
    expect(appUpdates.check).toHaveBeenCalledWith(true);
  });

  it("offers nothing when there is none, and nothing when the shell cannot look", async () => {
    vi.mocked(appUpdates.check).mockResolvedValue(null);
    const first = render(<UpdateOffer />);
    await waitFor(() => expect(appUpdates.check).toHaveBeenCalledWith(false));
    expect(screen.queryByText(/is available/)).toBeNull();
    first.unmount();
    vi.mocked(appUpdates.check).mockRejectedValue("the feed is down");
    render(<UpdateOffer />);
    await waitFor(() => expect(appUpdates.check).toHaveBeenCalledTimes(2));
    expect(screen.queryByText(/is available/)).toBeNull();
  });

  it("is installed only when the person says so, and a refusal is told", async () => {
    vi.mocked(appUpdates.check).mockResolvedValue({ version: "0.4.99", notes: null });
    vi.mocked(appUpdates.install).mockRejectedValue("the signature of the update does not match");
    render(<UpdateOffer />);
    await screen.findByText("Version 0.4.99 is available");
    expect(appUpdates.install).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Install and restart" }));
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", expect.stringContaining("signature")));
  });
});
