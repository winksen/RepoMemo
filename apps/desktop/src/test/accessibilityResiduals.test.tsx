import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { Button } from "../components/ui/button";
import { Dialog, DialogCancel } from "../components/ui/dialog";
import { useToolbarKeys } from "../lib/useToolbarKeys";

afterEach(() => cleanup());

function Form({ withField = true }: { withField?: boolean }) {
  return <Dialog footer={<><DialogCancel onClick={() => undefined} /><Button type="submit" variant="main">Save</Button></>} onClose={() => undefined} onSubmit={() => undefined} open title="Rename">
    {withField ? <input aria-label="Name" /> : <p>Nothing to type here.</p>}
  </Dialog>;
}

describe("a skip link in dialogs", () => {
  it("starts a dialog that has a body, and jumps to its buttons without closing it", () => {
    render(<Form />);
    expect(document.activeElement).toBe(screen.getByLabelText("Name"));
    const skip = screen.getByRole("link", { name: "Skip to the buttons" });
    // It comes before the body, so going back from the first field reaches it.
    expect(skip.compareDocumentPosition(screen.getByLabelText("Name")) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    fireEvent.click(skip);
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.getByRole("dialog")).toBeTruthy();
  });

  it("is not what a dialog without a field focuses, and a question has none", () => {
    const { rerender } = render(<Form withField={false} />);
    expect(document.activeElement).not.toBe(screen.getByRole("link", { name: "Skip to the buttons" }));
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Cancel" }));
    rerender(<Dialog footer={<DialogCancel onClick={() => undefined} />} onClose={() => undefined} open title="Delete it?" />);
    expect(screen.queryByRole("link", { name: "Skip to the buttons" })).toBeNull();
  });

  it("points at the footer of its own dialog", () => {
    render(<Form />);
    const target = screen.getByRole("link", { name: "Skip to the buttons" }).getAttribute("href")?.slice(1);
    expect(target).toBeTruthy();
    expect(document.getElementById(target ?? "")?.className).toBe("rm-dialog-footer");
  });
});

function Tools({ disabled = false, orientation = "horizontal" as "horizontal" | "vertical" }) {
  const toolbar = useToolbarKeys<HTMLDivElement>(orientation);
  return <div aria-label="Formatting" aria-orientation={orientation} onFocus={toolbar.onFocus} onKeyDown={toolbar.onKeyDown} ref={toolbar.ref} role="toolbar">
    <button type="button">Bold</button>
    <button disabled={disabled} type="button">Italic</button>
    <button type="button">Link</button>
  </div>;
}

const stops = () => screen.getAllByRole("button").map((button) => button.tabIndex);

describe("a toolbar", () => {
  it("is one stop in the tab order, on the first button until another has focus", () => {
    render(<Tools />);
    expect(stops()).toEqual([0, -1, -1]);
  });

  it("moves between its buttons with the arrow keys, Home and End, and wraps", () => {
    render(<Tools />);
    const [bold, italic, link] = screen.getAllByRole("button");
    bold.focus();
    fireEvent.keyDown(bold, { key: "ArrowRight" });
    expect(document.activeElement).toBe(italic);
    expect(stops()).toEqual([-1, 0, -1]);
    fireEvent.keyDown(italic, { key: "End" });
    expect(document.activeElement).toBe(link);
    fireEvent.keyDown(link, { key: "ArrowRight" });
    expect(document.activeElement).toBe(bold);
    fireEvent.keyDown(bold, { key: "ArrowLeft" });
    expect(document.activeElement).toBe(link);
    fireEvent.keyDown(link, { key: "Home" });
    expect(document.activeElement).toBe(bold);
    expect(stops()).toEqual([0, -1, -1]);
  });

  it("leaves other keys and keys with a modifier alone, and skips a disabled button", () => {
    render(<Tools disabled />);
    const [bold, , link] = screen.getAllByRole("button");
    bold.focus();
    fireEvent.keyDown(bold, { key: "ArrowDown" });
    fireEvent.keyDown(bold, { key: "ArrowRight", ctrlKey: true });
    fireEvent.keyDown(bold, { key: "a" });
    expect(document.activeElement).toBe(bold);
    fireEvent.keyDown(bold, { key: "ArrowRight" });
    expect(document.activeElement).toBe(link);
  });

  it("follows the focus when it is given to a button some other way, and a vertical one uses up and down", () => {
    const { unmount } = render(<Tools />);
    screen.getByRole("button", { name: "Link" }).focus();
    expect(stops()).toEqual([-1, -1, 0]);
    unmount();
    render(<Tools orientation="vertical" />);
    const [bold, italic] = screen.getAllByRole("button");
    bold.focus();
    fireEvent.keyDown(bold, { key: "ArrowRight" });
    expect(document.activeElement).toBe(bold);
    fireEvent.keyDown(bold, { key: "ArrowDown" });
    expect(document.activeElement).toBe(italic);
  });
});
