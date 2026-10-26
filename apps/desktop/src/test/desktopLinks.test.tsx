import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const links = vi.hoisted(() => ({ onPath: null as null | ((path: string) => void), onRefused: null as null | ((reason: string) => void), stop: vi.fn() }));
vi.mock("../lib/desktop", () => ({
  isDesktopRuntime: true,
  installExternalLinkHandler: () => () => undefined,
  followPageTitle: () => () => undefined,
  saveThroughShell: vi.fn(),
  shellNotifier: {},
  desktopSettings: { get: vi.fn(async () => ({ name: null, supportUrl: null, suggestedServer: null, source: null, problem: null })) },
  desktopServer: { current: vi.fn(async () => "https://memo.example.com"), locked: vi.fn(), prepare: vi.fn(), remember: vi.fn(), forget: vi.fn() },
  appLinks: {
    takePending: vi.fn(),
    listen: vi.fn(async (onPath: (path: string) => void, onRefused: (reason: string) => void) => { links.onPath = onPath; links.onRefused = onRefused; return links.stop; }),
  },
}));
vi.mock("../lib/desktopConnection", () => ({ pointAtServer: vi.fn(async () => undefined) }));
vi.mock("../web/router", () => ({ navigate: vi.fn() }));
vi.mock("../components/ui/toast", () => ({ showToast: vi.fn(), Toast: () => null }));
vi.mock("../SharedWebApp", () => ({ SharedWebApp: () => <p>the web client</p> }));

import { DesktopApp } from "../components/DesktopApp";
import { showToast } from "../components/ui/toast";
import { appLinks } from "../lib/desktop";
import { navigate } from "../web/router";

beforeEach(() => {
  links.onPath = null;
  links.onRefused = null;
  vi.mocked(appLinks.takePending).mockResolvedValue(null);
});
afterEach(() => { cleanup(); vi.clearAllMocks(); });

describe("a repomemo:// link reaching the app (V0.4.54)", () => {
  it("shows the page of the link the app was started with, once the client is up", async () => {
    vi.mocked(appLinks.takePending).mockResolvedValue("/workspaces/w1/evidence");
    render(<DesktopApp />);
    await screen.findByText("the web client");
    await waitFor(() => expect(navigate).toHaveBeenCalledWith("/workspaces/w1/evidence"));
  });

  it("shows the page of a link opened while it runs", async () => {
    render(<DesktopApp />);
    await screen.findByText("the web client");
    await waitFor(() => expect(links.onPath).not.toBeNull());
    links.onPath!("/notifications");
    expect(navigate).toHaveBeenCalledWith("/notifications");
  });

  it("says why when the shell turned a link away, and shows nothing", async () => {
    render(<DesktopApp />);
    await screen.findByText("the web client");
    await waitFor(() => expect(links.onRefused).not.toBeNull());
    links.onRefused!("That link is not for a page of RepoMemo.");
    expect(showToast).toHaveBeenCalledWith("error", "That link is not for a page of RepoMemo.");
    expect(navigate).not.toHaveBeenCalled();
  });

  it("stops listening when the window goes away", async () => {
    const view = render(<DesktopApp />);
    await screen.findByText("the web client");
    await waitFor(() => expect(links.onPath).not.toBeNull());
    view.unmount();
    await waitFor(() => expect(links.stop).toHaveBeenCalled());
  });
});
