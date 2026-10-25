import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/desktop", () => ({
  isDesktopRuntime: true,
  installExternalLinkHandler: () => () => undefined,
  desktopServer: { current: vi.fn(), prepare: vi.fn(), remember: vi.fn(), forget: vi.fn() },
}));
vi.mock("../lib/sharedApi", () => ({ configureDesktopApi: vi.fn(), getSharedHealth: vi.fn(), sharedApiUrl: "https://repomemo.example.com" }));
vi.mock("@tauri-apps/plugin-http", () => ({ fetch: vi.fn() }));
vi.mock("../components/ui/toast", () => ({ showToast: vi.fn() }));
vi.mock("../components/SharedLayout", () => ({ ThemeToggle: () => null }));
vi.mock("../SharedWebApp", () => ({ SharedWebApp: () => <p>the web client</p> }));

import { DesktopApp } from "../components/DesktopApp";
import { desktopServer } from "../lib/desktop";
import { configureDesktopApi, getSharedHealth } from "../lib/sharedApi";
import { showToast } from "../components/ui/toast";
import { CLIENT_VERSION } from "../lib/versionCheck";

const healthy = { service: "repomemo-server", status: "ok", authentication: "jwt", version: CLIENT_VERSION };

function start() {
  render(<DesktopApp />);
}

beforeEach(() => {
  window.localStorage.clear();
  vi.mocked(desktopServer.current).mockResolvedValue(null);
  vi.mocked(desktopServer.prepare).mockImplementation(async (address) => `https://${address.replace(/^https?:\/\//, "")}`);
  vi.mocked(desktopServer.remember).mockResolvedValue(undefined);
  vi.mocked(getSharedHealth).mockResolvedValue(healthy);
});
afterEach(() => { cleanup(); vi.clearAllMocks(); });

const typeAddress = (value: string) => fireEvent.change(screen.getByLabelText("Server address"), { target: { value } });
const connect = () => fireEvent.click(screen.getByRole("button", { name: "Connect" }));

describe("the desktop app", () => {
  it("shows the web client on the saved server, pointed at it first", async () => {
    vi.mocked(desktopServer.current).mockResolvedValue("https://repomemo.example.com");
    start();
    expect(await screen.findByText("the web client")).toBeTruthy();
    expect(configureDesktopApi).toHaveBeenCalledWith("https://repomemo.example.com", expect.anything());
    expect(screen.queryByLabelText("Server address")).toBeNull();
  });

  it("asks for an address when there is none, and keeps the server once it answers like RepoMemo", async () => {
    start();
    await screen.findByLabelText("Server address");
    typeAddress("repomemo.example.com");
    connect();
    expect(await screen.findByText("the web client")).toBeTruthy();
    expect(desktopServer.prepare).toHaveBeenCalledWith("repomemo.example.com");
    expect(configureDesktopApi).toHaveBeenCalledWith("https://repomemo.example.com", expect.anything());
    expect(desktopServer.remember).toHaveBeenCalledTimes(1);
  });

  it("refuses a bad address on the page, without asking the shell or the network", async () => {
    start();
    await screen.findByLabelText("Server address");
    typeAddress("http://repomemo.example.com");
    connect();
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", expect.stringContaining("https://")));
    expect(desktopServer.prepare).not.toHaveBeenCalled();
    expect(getSharedHealth).not.toHaveBeenCalled();
  });

  it("does not keep a server that does not answer, or that is not RepoMemo", async () => {
    start();
    await screen.findByLabelText("Server address");
    vi.mocked(getSharedHealth).mockRejectedValueOnce(new Error("connection refused"));
    typeAddress("repomemo.example.com");
    connect();
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", expect.stringContaining("Nothing answered at https://repomemo.example.com")));

    vi.mocked(getSharedHealth).mockResolvedValueOnce({ hello: "world" } as never);
    connect();
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", expect.stringContaining("does not look like a RepoMemo server")));
    expect(desktopServer.remember).not.toHaveBeenCalled();
    expect(screen.queryByText("the web client")).toBeNull();
  });

  it("does not keep a server of another release line and says which is which", async () => {
    start();
    await screen.findByLabelText("Server address");
    vi.mocked(getSharedHealth).mockResolvedValueOnce({ ...healthy, version: "0.3.27" });
    typeAddress("repomemo.example.com");
    connect();
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", expect.stringContaining(`This app is version ${CLIENT_VERSION} and the server is version 0.3.27`)));
    expect(desktopServer.remember).not.toHaveBeenCalled();
  });

  it("shows the shell's own reason when it refuses an address", async () => {
    vi.mocked(desktopServer.prepare).mockRejectedValueOnce("That address cannot be used from here.");
    start();
    await screen.findByLabelText("Server address");
    typeAddress("repomemo.example.com");
    connect();
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", "That address cannot be used from here."));
  });

  it("has no local workspace: a window that chose one before opens on the server address screen and forgets the choice", async () => {
    window.localStorage.setItem("repomemo.desktop.mode", "local");
    window.localStorage.setItem("repomemo.desktop.handoff", JSON.stringify({ path: "C:\\exports\\notes.tar.gz", file_name: "notes.tar.gz", workspace_name: "Notes" }));
    start();
    expect(await screen.findByLabelText("Server address")).toBeTruthy();
    expect(screen.queryByRole("button", { name: /local workspace/i })).toBeNull();
    expect(screen.queryByRole("note")).toBeNull();
    expect(window.localStorage.getItem("repomemo.desktop.mode")).toBeNull();
    expect(window.localStorage.getItem("repomemo.desktop.handoff")).toBeNull();
  });
});
