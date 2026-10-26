import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/desktop", () => ({
  isDesktopRuntime: true,
  installExternalLinkHandler: () => () => undefined,
  followPageTitle: () => () => undefined,
  saveThroughShell: vi.fn(),
  desktopSettings: { get: vi.fn() },
  desktopServer: { current: vi.fn(), locked: vi.fn(), prepare: vi.fn(), remember: vi.fn(), forget: vi.fn() },
}));
vi.mock("../lib/sharedApi", () => ({ configureDesktopApi: vi.fn(), getSharedHealth: vi.fn(), sharedApiUrl: "https://memo.example.com" }));
vi.mock("../lib/desktopConnection", () => ({ pointAtServer: vi.fn(async () => undefined) }));
vi.mock("../components/ui/toast", () => ({ showToast: vi.fn(), Toast: () => null }));
vi.mock("../components/SharedLayout", () => ({ ThemeToggle: () => null }));
vi.mock("../SharedWebApp", () => ({ SharedWebApp: () => <p>the web client</p> }));

import { DesktopApp } from "../components/DesktopApp";
import { DesktopServerNote } from "../components/DesktopServerNote";
import { desktopServer, desktopSettings } from "../lib/desktop";
import { pointAtServer } from "../lib/desktopConnection";

beforeEach(() => { vi.mocked(desktopSettings.get).mockResolvedValue({ name: null, supportUrl: null, suggestedServer: null, source: null, problem: null }); });
afterEach(() => { cleanup(); vi.clearAllMocks(); });

describe("an app packaged for one server (V0.4.46)", () => {
  it("opens on that server, with no address field", async () => {
    vi.mocked(desktopServer.current).mockResolvedValue("https://memo.example.com");
    render(<DesktopApp />);
    expect(await screen.findByText("the web client")).toBeTruthy();
    expect(pointAtServer).toHaveBeenCalledWith("https://memo.example.com");
    expect(screen.queryByLabelText("Server address")).toBeNull();
    expect(desktopServer.prepare).not.toHaveBeenCalled();
  });

  it("shows the server and offers no way to change it", async () => {
    vi.mocked(desktopServer.locked).mockResolvedValue(true);
    render(<DesktopServerNote signedIn />);
    await waitFor(() => expect(desktopServer.locked).toHaveBeenCalled());
    expect(screen.getByText("memo.example.com")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Change server" })).toBeNull();
  });

  it("offers no change before the shell has answered either", () => {
    vi.mocked(desktopServer.locked).mockReturnValue(new Promise(() => undefined));
    render(<DesktopServerNote signedIn={false} />);
    expect(screen.queryByRole("button", { name: "Change server" })).toBeNull();
  });
});

describe("an app packaged without one", () => {
  it("offers to change the server, as before", async () => {
    vi.mocked(desktopServer.locked).mockResolvedValue(false);
    render(<DesktopServerNote signedIn={false} />);
    expect(await screen.findByRole("button", { name: "Change server" })).toBeTruthy();
  });

  it("offers it too when the shell cannot say, since the shell refuses a change anyway", async () => {
    vi.mocked(desktopServer.locked).mockRejectedValue("no such command");
    render(<DesktopServerNote signedIn={false} />);
    expect(await screen.findByRole("button", { name: "Change server" })).toBeTruthy();
  });
});
