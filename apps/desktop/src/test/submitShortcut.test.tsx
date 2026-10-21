import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Textarea } from "../components/ui/textarea";
import { isSubmitShortcut, submitHint, submitShortcutLabel } from "../lib/submitShortcut";

afterEach(() => { cleanup(); vi.unstubAllGlobals(); });

describe("the send shortcut", () => {
  it("is Ctrl+Enter, and Cmd+Enter on macOS", () => {
    vi.stubGlobal("navigator", { platform: "Win32", userAgent: "" });
    expect(isSubmitShortcut({ ctrlKey: true, key: "Enter", metaKey: false })).toBe(true);
    expect(isSubmitShortcut({ ctrlKey: false, key: "Enter", metaKey: true })).toBe(false);
    expect(submitShortcutLabel()).toBe("Ctrl+Enter");
    vi.stubGlobal("navigator", { platform: "MacIntel", userAgent: "" });
    expect(isSubmitShortcut({ ctrlKey: false, key: "Enter", metaKey: true })).toBe(true);
    expect(submitHint("saves")).toBe("Cmd+Enter saves.");
  });

  it("ignores a plain Enter and a key pressed during composition", () => {
    vi.stubGlobal("navigator", { platform: "Win32", userAgent: "" });
    expect(isSubmitShortcut({ ctrlKey: false, key: "Enter", metaKey: false })).toBe(false);
    expect(isSubmitShortcut({ ctrlKey: true, isComposing: true, key: "Enter", metaKey: false })).toBe(false);
  });

  it("submits the form of a textarea, but a plain Enter only adds a line", () => {
    vi.stubGlobal("navigator", { platform: "Win32", userAgent: "" });
    const onSubmit = vi.fn((event: { preventDefault: () => void }) => event.preventDefault());
    render(<form onSubmit={onSubmit}><Textarea aria-label="Message" defaultValue="hello" /><button type="submit">Send</button></form>);
    fireEvent.keyDown(screen.getByLabelText("Message"), { key: "Enter" });
    expect(onSubmit).not.toHaveBeenCalled();
    fireEvent.keyDown(screen.getByLabelText("Message"), { ctrlKey: true, key: "Enter" });
    expect(onSubmit).toHaveBeenCalledTimes(1);
  });

  it("leaves the key to a field that handled it itself", () => {
    vi.stubGlobal("navigator", { platform: "Win32", userAgent: "" });
    const onSubmit = vi.fn((event: { preventDefault: () => void }) => event.preventDefault());
    render(<form onSubmit={onSubmit}><Textarea aria-label="Message" onKeyDown={(event) => event.preventDefault()} /></form>);
    fireEvent.keyDown(screen.getByLabelText("Message"), { ctrlKey: true, key: "Enter" });
    expect(onSubmit).not.toHaveBeenCalled();
  });
});
