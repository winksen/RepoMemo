import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/desktop", () => ({
  isDesktopRuntime: true,
  installExternalLinkHandler: () => () => undefined,
  followPageTitle: () => () => undefined,
  saveThroughShell: vi.fn(),
  shellNotifier: {},
  appLinks: { takePending: vi.fn(async () => null), listen: vi.fn(async () => () => undefined) },
  desktopSettings: { get: vi.fn() },
  desktopServer: { current: vi.fn(), locked: vi.fn(), route: vi.fn(async () => null), prepare: vi.fn(), remember: vi.fn(), forget: vi.fn() },
}));
vi.mock("../lib/sharedApi", () => ({ configureDesktopApi: vi.fn(), getSharedHealth: vi.fn(), sharedApiUrl: "https://memo.example.com" }));
vi.mock("../lib/desktopConnection", () => ({ pointAtServer: vi.fn(async () => undefined) }));
vi.mock("../components/ui/toast", () => ({ showToast: vi.fn(), Toast: () => null }));
vi.mock("../components/SharedLayout", () => ({ ThemeToggle: () => null }));
vi.mock("../SharedWebApp", () => ({ SharedWebApp: () => <p>the web client</p> }));

import { DesktopApp } from "../components/DesktopApp";
import { DesktopServerNote } from "../components/DesktopServerNote";
import { desktopServer, desktopSettings, type DesktopSettings } from "../lib/desktop";
import { pointAtServer } from "../lib/desktopConnection";

const settings = (over: Partial<DesktopSettings>): DesktopSettings => ({ name: null, supportUrl: null, suggestedServer: null, source: null, problem: null, ...over });

beforeEach(() => {
  document.title = "RepoMemo";
  vi.mocked(desktopServer.current).mockResolvedValue(null);
  vi.mocked(desktopServer.locked).mockResolvedValue(false);
});
afterEach(() => { cleanup(); vi.clearAllMocks(); });

describe("a configuration file that cannot be used (V0.4.47)", () => {
  it("is named and not obeyed: no connection, no address field, the reason on the page", async () => {
    vi.mocked(desktopSettings.get).mockResolvedValue(settings({ problem: "/opt/RepoMemo/repomemo-desktop.json: server: A server on the internet must use https://." }));
    vi.mocked(desktopServer.current).mockResolvedValue("https://saved.example.com");
    render(<DesktopApp />);
    expect(await screen.findByText("This app's configuration cannot be used.")).toBeTruthy();
    expect(screen.getByRole("alert").textContent).toContain("repomemo-desktop.json: server: A server on the internet must use https://.");
    expect(screen.queryByLabelText("Server address")).toBeNull();
    expect(desktopServer.current).not.toHaveBeenCalled();
    expect(pointAtServer).not.toHaveBeenCalled();
  });
});

describe("a configuration file that can", () => {
  it("names the app: the page's title follows", async () => {
    vi.mocked(desktopSettings.get).mockResolvedValue(settings({ name: "Acme Memo" }));
    render(<DesktopApp />);
    await screen.findByLabelText("Server address");
    expect(document.title).toBe("Acme Memo");
  });

  it("offers a server it does not lock: the field is filled in and can be changed", async () => {
    vi.mocked(desktopSettings.get).mockResolvedValue(settings({ suggestedServer: "https://memo.example.com" }));
    render(<DesktopApp />);
    const field = (await screen.findByLabelText("Server address")) as HTMLInputElement;
    expect(field.value).toBe("https://memo.example.com");
    expect(field.disabled).toBe(false);
  });

  it("opens on the server the shell chose, which is the file's when it is locked", async () => {
    vi.mocked(desktopSettings.get).mockResolvedValue(settings({ source: "/opt/RepoMemo/repomemo-desktop.json" }));
    vi.mocked(desktopServer.current).mockResolvedValue("https://memo.example.com");
    render(<DesktopApp />);
    expect(await screen.findByText("the web client")).toBeTruthy();
    expect(pointAtServer).toHaveBeenCalledWith("https://memo.example.com");
  });

  it("shows the help link on the connect screen and in the account area", async () => {
    vi.mocked(desktopSettings.get).mockResolvedValue(settings({ supportUrl: "https://help.example.com/memo" }));
    const first = render(<DesktopApp />);
    const link = await screen.findByRole("link", { name: "Ask here" });
    expect(link.getAttribute("href")).toBe("https://help.example.com/memo");
    first.unmount();
    render(<DesktopServerNote signedIn />);
    expect((await screen.findByRole("link", { name: "Help" })).getAttribute("href")).toBe("https://help.example.com/memo");
  });

  it("goes on as before when the shell cannot say", async () => {
    vi.mocked(desktopSettings.get).mockRejectedValue("no such command");
    render(<DesktopApp />);
    expect(await screen.findByLabelText("Server address")).toBeTruthy();
  });
});
