import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// The window of the app, in memory.
const window_ = vi.hoisted(() => ({ setTitle: vi.fn(async (_title: string) => undefined) }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => window_ }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { followPageTitle } from "../lib/desktop";

beforeEach(() => {
  document.title = "RepoMemo";
  window_.setTitle.mockClear();
});
afterEach(() => { document.title = "RepoMemo"; });

/** A change of the page's title reaches the observer in a later task. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

describe("the window's title follows the page's", () => {
  it("is the page's from the start", () => {
    document.title = "Platform · Evidence · RepoMemo";
    const stop = followPageTitle();
    expect(window_.setTitle).toHaveBeenCalledWith("Platform · Evidence · RepoMemo");
    stop();
  });

  it("changes when the client names another page, so the title bar names the workspace", async () => {
    const stop = followPageTitle();
    window_.setTitle.mockClear();
    document.title = "Billing · Decisions · RepoMemo";
    await settle();
    expect(window_.setTitle).toHaveBeenLastCalledWith("Billing · Decisions · RepoMemo");
    stop();
  });

  it("stops when it is told to, and survives a window that refuses", async () => {
    window_.setTitle.mockRejectedValueOnce(new Error("not allowed"));
    const stop = followPageTitle();
    await settle();
    stop();
    window_.setTitle.mockClear();
    document.title = "Elsewhere";
    await settle();
    expect(window_.setTitle).not.toHaveBeenCalled();
  });
});
