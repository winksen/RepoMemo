import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Button } from "../components/ui/button";
import { Dialog, DialogCancel } from "../components/ui/dialog";

afterEach(() => { cleanup(); });

function Rename({ onClose = () => undefined, onSubmit = () => undefined }: { onClose?: () => void; onSubmit?: () => void }) {
  return <Dialog footer={<><DialogCancel onClick={onClose} /><Button type="submit" variant="main">Rename</Button></>} onClose={onClose} onSubmit={onSubmit} open title="Rename">
    <input aria-label="Name" />
  </Dialog>;
}

describe("dialog behaviour", () => {
  it("focuses its first field when it opens", () => {
    render(<Rename />);
    expect(document.activeElement).toBe(screen.getByLabelText("Name"));
  });

  it("submits on Enter in a field and closes on Escape", () => {
    const onSubmit = vi.fn();
    const onClose = vi.fn();
    render(<Rename onClose={onClose} onSubmit={onSubmit} />);
    fireEvent.submit(screen.getByLabelText("Name"));
    expect(onSubmit).toHaveBeenCalledTimes(1);
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(onClose).toHaveBeenCalled();
  });

  it("puts Cancel before the decisive action", () => {
    render(<Rename />);
    const labels = screen.getAllByRole("button").map((button) => button.textContent);
    expect(labels.indexOf("Cancel")).toBeLessThan(labels.indexOf("Rename"));
  });

  it("is small for a question, medium for a form and large when asked", () => {
    const footer = <DialogCancel onClick={() => undefined} />;
    const { rerender } = render(<Dialog footer={footer} onClose={() => undefined} open title="Question" />);
    expect(screen.getByRole("dialog").className).toContain("rm-dialog-sm");
    rerender(<Dialog footer={footer} onClose={() => undefined} open title="Form"><input aria-label="Field" /></Dialog>);
    expect(screen.getByRole("dialog").className).toContain("rm-dialog-md");
    rerender(<Dialog footer={footer} onClose={() => undefined} open size="lg" title="Long"><p>Text</p></Dialog>);
    expect(screen.getByRole("dialog").className).toContain("rm-dialog-lg");
  });
});
