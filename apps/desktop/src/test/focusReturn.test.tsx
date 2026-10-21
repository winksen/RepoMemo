import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { useState } from "react";
import { Button } from "../components/ui/button";
import { Dialog, DialogCancel } from "../components/ui/dialog";
import { watchFocusLoss } from "../lib/focusReturn";

afterEach(() => { cleanup(); });

function Rows({ initial = ["a", "b", "c"] }: { initial?: string[] }) {
  const [rows, setRows] = useState(initial);
  return <section>
    <h2>Rows</h2>
    <ul>{rows.map((row) => <li key={row}><button onClick={() => setRows((current) => current.filter((entry) => entry !== row))} type="button">Delete {row}</button></li>)}</ul>
  </section>;
}

async function deleteRow(name: string) {
  const button = screen.getByRole("button", { name: `Delete ${name}` });
  button.focus();
  fireEvent.click(button);
  await Promise.resolve();
}

describe("focus after a deletion", () => {
  it("moves to the next item", async () => {
    const stop = watchFocusLoss();
    render(<Rows />);
    await deleteRow("b");
    await Promise.resolve();
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Delete c" }));
    stop();
  });

  it("moves to the previous item after the last one", async () => {
    const stop = watchFocusLoss();
    render(<Rows />);
    await deleteRow("c");
    await Promise.resolve();
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Delete b" }));
    stop();
  });

  it("moves to the heading of the list when the last item goes", async () => {
    const stop = watchFocusLoss();
    render(<Rows initial={["a"]} />);
    await deleteRow("a");
    await Promise.resolve();
    expect(document.activeElement).toBe(screen.getByRole("heading", { name: "Rows" }));
    stop();
  });
});

describe("focus after a dialog", () => {
  function Opener() {
    const [open, setOpen] = useState(false);
    return <>
      <Button onClick={() => setOpen(true)}>Open it</Button>
      <Dialog footer={<DialogCancel onClick={() => setOpen(false)} />} onClose={() => setOpen(false)} open={open} title="Question" />
    </>;
  }

  it("returns to the control that opened it", async () => {
    render(<Opener />);
    const opener = screen.getByRole("button", { name: "Open it" });
    opener.focus();
    fireEvent.click(opener);
    fireEvent.click(await screen.findByRole("button", { name: "Cancel" }));
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(document.activeElement).toBe(opener);
  });
});
