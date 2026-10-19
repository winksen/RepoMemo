import { act, cleanup, render, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../components/ui/toast", () => ({ showToast: vi.fn() }));

import { showToast } from "../components/ui/toast";
import { sendToAssistant } from "../lib/assistantBridge";
import { useAssistantCommands } from "../lib/useAssistantCommands";
import { useErrorToast } from "../lib/useErrorToast";
import { useObjectUrl } from "../lib/useObjectUrl";

beforeEach(() => { vi.mocked(showToast).mockClear(); });
afterEach(cleanup);

describe("useObjectUrl", () => {
  it("makes an address for a file, releases it when the file goes and when the page is left", () => {
    const create = vi.fn().mockReturnValueOnce("blob:one").mockReturnValueOnce("blob:two");
    const revoke = vi.fn();
    Object.assign(URL, { createObjectURL: create, revokeObjectURL: revoke });
    const first = new Blob(["a"]);
    const { result, rerender, unmount } = renderHook(({ file }) => useObjectUrl(file), { initialProps: { file: first as Blob | null } });
    expect(result.current).toBe("blob:one");
    rerender({ file: null });
    expect(result.current).toBeNull();
    expect(revoke).toHaveBeenCalledWith("blob:one");
    rerender({ file: new Blob(["b"]) });
    expect(result.current).toBe("blob:two");
    unmount();
    expect(revoke).toHaveBeenCalledWith("blob:two");
  });
});

describe("useErrorToast", () => {
  it("shows the message of an error once, or the fallback when it has none, and nothing without an error", () => {
    const { rerender } = renderHook(({ error }) => useErrorToast(error, "Could not load."), { initialProps: { error: null as unknown } });
    expect(showToast).not.toHaveBeenCalled();
    const failure = new Error("The server said no.");
    rerender({ error: failure });
    rerender({ error: failure });
    expect(showToast).toHaveBeenCalledTimes(1);
    expect(showToast).toHaveBeenCalledWith("error", "The server said no.");
    rerender({ error: new Error("") });
    expect(showToast).toHaveBeenLastCalledWith("error", "Could not load.");
  });
});

describe("useAssistantCommands", () => {
  function Probe({ handle, ready }: { handle: (command: { capability: string | null; message: string }) => void; ready: boolean }) {
    useAssistantCommands({ handle: handle as never, ready });
    return null;
  }

  it("holds a command until the assistant is ready, then hands it over once", async () => {
    const handle = vi.fn();
    const { rerender } = render(<Probe handle={handle} ready={false} />);
    act(() => sendToAssistant({ capability: "workspace_overview", message: "" }));
    expect(handle).not.toHaveBeenCalled();
    rerender(<Probe handle={handle} ready />);
    await waitFor(() => expect(handle).toHaveBeenCalledTimes(1));
    expect(handle).toHaveBeenCalledWith({ capability: "workspace_overview", message: "" });
    rerender(<Probe handle={handle} ready={false} />);
    rerender(<Probe handle={handle} ready />);
    expect(handle).toHaveBeenCalledTimes(1);
  });

  it("hands over a command that was sent before the assistant mounted", async () => {
    const handle = vi.fn();
    sendToAssistant({ capability: null, message: "what changed?" });
    render(<Probe handle={handle} ready />);
    await waitFor(() => expect(handle).toHaveBeenCalledWith({ capability: null, message: "what changed?" }));
  });
});
