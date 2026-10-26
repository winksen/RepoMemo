import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// The shell's calls, in memory.
const shell = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => shell);
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({ setTitle: vi.fn(async () => undefined) }) }));

import { saveThroughShell } from "../lib/desktop";
import { saveBlob, setFileSaver } from "../lib/saveBlob";

beforeEach(() => {
  shell.invoke.mockReset();
  URL.createObjectURL = vi.fn(() => "blob:test");
  URL.revokeObjectURL = vi.fn();
});
afterEach(() => { setFileSaver(null); vi.useRealTimers(); });

describe("saving a file in a browser", () => {
  it("clicks an anchor that downloads the blob under its name, then lets the address go", async () => {
    vi.useFakeTimers();
    const clicked: Array<{ href: string; download: string }> = [];
    const click = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(function (this: HTMLAnchorElement) { clicked.push({ href: this.href, download: this.download }); });
    const saved = await saveBlob(new Blob(["x"]), "memory.md");
    expect(saved).toBe(true);
    expect(clicked).toEqual([{ href: "blob:test", download: "memory.md" }]);
    expect(document.querySelector("a[download]")).toBeNull();
    vi.advanceTimersByTime(10_000);
    expect(URL.revokeObjectURL).toHaveBeenCalledWith("blob:test");
    click.mockRestore();
  });
});

describe("saving a file in the app", () => {
  it("sends the bytes, not JSON, with the name in a header, and says whether the person saved it", async () => {
    shell.invoke.mockResolvedValue("C:/Users/ada/Documents/archive.zip");
    const saved = await saveThroughShell(new Blob([new Uint8Array([1, 2, 3, 250])]), "workspace archive é.zip");
    expect(saved).toBe(true);
    const [command, body, options] = shell.invoke.mock.calls[0];
    expect(command).toBe("save_file");
    expect(body).toBeInstanceOf(Uint8Array);
    expect([...(body as Uint8Array)]).toEqual([1, 2, 3, 250]);
    expect(options).toEqual({ headers: { "x-file-name": "workspace%20archive%20%C3%A9.zip" } });
  });

  it("says no when the person cancels the dialog", async () => {
    shell.invoke.mockResolvedValue(null);
    expect(await saveThroughShell(new Blob(["x"]), "a.md")).toBe(false);
  });

  it("is what every download uses once it is installed, and a browser's again when it is removed", async () => {
    const saver = vi.fn(async () => false);
    setFileSaver(saver);
    expect(await saveBlob(new Blob(["x"]), "audit.csv")).toBe(false);
    expect(saver).toHaveBeenCalledWith(expect.any(Blob), "audit.csv");
    expect(URL.createObjectURL).not.toHaveBeenCalled();
    setFileSaver(null);
    const click = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => undefined);
    expect(await saveBlob(new Blob(["x"]), "audit.csv")).toBe(true);
    click.mockRestore();
  });

  it("lets the error of a refused save reach the caller, which toasts it", async () => {
    shell.invoke.mockRejectedValue("The file could not be saved to D:/x.zip: access is denied");
    await expect(saveThroughShell(new Blob(["x"]), "x.zip")).rejects.toMatch(/access is denied/);
  });
});
