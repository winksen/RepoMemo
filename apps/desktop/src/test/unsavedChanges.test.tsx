import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { ConfirmProvider } from "../components/ui/confirm-dialog";
import { useUnsavedChanges } from "../lib/unsavedChanges";
import { currentLocation, navigate } from "../web/router";

afterEach(() => { cleanup(); window.history.replaceState({}, "", "/"); });

function Form({ dirty, keepsOn }: { dirty: boolean; keepsOn?: (to: string) => boolean }) {
  useUnsavedChanges(dirty, { keepsOn, subject: "your edits" });
  return <p>form</p>;
}

describe("unsaved changes", () => {
  it("moves without confirmation when nothing is unsaved", async () => {
    render(<ConfirmProvider><Form dirty={false} /></ConfirmProvider>);
    navigate("/dashboard");
    await waitFor(() => expect(currentLocation()).toBe("/dashboard"));
  });

  it("asks before leaving and stays when the person keeps editing", async () => {
    render(<ConfirmProvider><Form dirty /></ConfirmProvider>);
    navigate("/dashboard");
    expect(currentLocation()).toBe("/");
    fireEvent.click(await screen.findByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByText("Discard unsaved changes?")).toBeNull());
    expect(currentLocation()).toBe("/");
  });

  it("leaves once the person agrees to discard", async () => {
    render(<ConfirmProvider><Form dirty /></ConfirmProvider>);
    navigate("/dashboard");
    fireEvent.click(await screen.findByRole("button", { name: "Discard changes" }));
    await waitFor(() => expect(currentLocation()).toBe("/dashboard"));
  });

  it("does not ask for a redirect or where the draft is kept", () => {
    render(<ConfirmProvider><Form dirty keepsOn={(to) => to.startsWith("/workspaces/w1")} /></ConfirmProvider>);
    navigate("/login", true);
    expect(currentLocation()).toBe("/login");
    window.history.replaceState({}, "", "/workspaces/w1/tasks");
    navigate("/workspaces/w1/memory");
    return waitFor(() => expect(currentLocation()).toBe("/workspaces/w1/memory"));
  });

  it("warns before the tab is closed only while something is unsaved", () => {
    const { rerender } = render(<ConfirmProvider><Form dirty /></ConfirmProvider>);
    const dirtyEvent = new Event("beforeunload", { cancelable: true });
    window.dispatchEvent(dirtyEvent);
    expect(dirtyEvent.defaultPrevented).toBe(true);
    rerender(<ConfirmProvider><Form dirty={false} /></ConfirmProvider>);
    const cleanEvent = new Event("beforeunload", { cancelable: true });
    window.dispatchEvent(cleanEvent);
    expect(cleanEvent.defaultPrevented).toBe(false);
  });
});
