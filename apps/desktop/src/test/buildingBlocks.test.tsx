import { act, cleanup, fireEvent, render, renderHook, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

vi.mock("../components/ui/toast", () => ({ Toast: () => null, showToast: vi.fn() }));

import { Badge } from "../components/ui/badge";
import { Button } from "../components/ui/button";
import { Checkbox } from "../components/ui/checkbox";
import { Combobox } from "../components/ui/combobox";
import { ConfirmProvider, useConfirm } from "../components/ui/confirm-dialog";
import { CopyButton } from "../components/ui/copy-button";
import { Dropdown } from "../components/ui/dropdown";
import { EmptyState } from "../components/ui/empty-state";
import { Field } from "../components/ui/field";
import { Input } from "../components/ui/input";
import { LoadingState } from "../components/ui/loading-state";
import { SearchField } from "../components/ui/search-field";
import { SegmentedControl } from "../components/ui/segmented-control";
import { Switch } from "../components/ui/switch";
import { showToast } from "../components/ui/toast";
import { useFormState } from "../lib/useFormState";

// Radix measures and scrolls; jsdom has neither.
beforeAll(() => {
  Object.assign(window.HTMLElement.prototype, {
    hasPointerCapture: () => false,
    releasePointerCapture: () => undefined,
    scrollIntoView: () => undefined,
    setPointerCapture: () => undefined,
  });
  window.ResizeObserver = window.ResizeObserver ?? class { disconnect() { /* none */ } observe() { /* none */ } unobserve() { /* none */ } };
});

afterEach(() => { cleanup(); vi.clearAllMocks(); });

describe("Field", () => {
  it("ties the label, the hint and the message to the control", () => {
    render(<Field hint="Letters only" invalid label="Name" message="Enter a name." required><Input /></Field>);
    const control = screen.getByLabelText("Name");
    expect(control.getAttribute("aria-invalid")).toBe("true");
    expect((control as HTMLInputElement).required).toBe(true);
    const described = control.getAttribute("aria-describedby") ?? "";
    expect(described.split(" ")).toHaveLength(2);
    expect(screen.getByText("Letters only").id).toBe(described.split(" ")[0]);
    expect(screen.getByText("Enter a name.").id).toBe(described.split(" ")[1]);
  });

  it("shows no message while the field is valid", () => {
    render(<Field label="Name" message="Enter a name."><Input /></Field>);
    expect(screen.queryByText("Enter a name.")).toBeNull();
    expect(screen.getByLabelText("Name").getAttribute("aria-invalid")).toBeNull();
  });
});

describe("Checkbox and Switch", () => {
  it("toggle from their label and report the new state", () => {
    const onCheck = vi.fn();
    const onSwitch = vi.fn();
    render(<><Checkbox checked={false} label="Remember me" onCheckedChange={onCheck} /><Switch checked={false} label="Email me" onCheckedChange={onSwitch} /></>);
    fireEvent.click(screen.getByText("Remember me"));
    expect(onCheck).toHaveBeenCalledWith(true);
    fireEvent.click(screen.getByText("Email me"));
    expect(onSwitch).toHaveBeenCalledWith(true);
    expect(screen.getByRole("switch").getAttribute("aria-checked")).toBe("false");
    expect(screen.getByRole("checkbox").getAttribute("aria-checked")).toBe("false");
  });

  it("do nothing while disabled", () => {
    const onSwitch = vi.fn();
    render(<Switch aria-label="Notifications" checked disabled onCheckedChange={onSwitch} />);
    fireEvent.click(screen.getByRole("switch", { name: "Notifications" }));
    expect(onSwitch).not.toHaveBeenCalled();
  });
});

describe("Dropdown", () => {
  it("opens its list in a portal and chooses with the keyboard", async () => {
    const onValueChange = vi.fn();
    const { container } = render(<Dropdown aria-label="Level" onValueChange={onValueChange} options={[{ label: "Info", value: "info" }, { label: "Warn", value: "warn" }]} value="info" />);
    const trigger = screen.getByRole("combobox", { name: "Level" });
    trigger.focus();
    fireEvent.keyDown(trigger, { key: "ArrowDown" });
    const option = await screen.findByRole("option", { name: "Warn" });
    expect(container.contains(option)).toBe(false);
    fireEvent.keyDown(option, { key: "Enter" });
    await waitFor(() => expect(onValueChange).toHaveBeenCalledWith("warn"));
  });
});

describe("ConfirmDialog and useConfirm", () => {
  function Ask({ onAnswer }: { onAnswer: (answer: boolean) => void }) {
    const confirm = useConfirm();
    return <Button onClick={() => void confirm({ action: "Delete", danger: true, title: "Delete it?" }).then(onAnswer)}>Ask</Button>;
  }

  it("resolves true on the decisive action and false on Cancel", async () => {
    const onAnswer = vi.fn();
    render(<ConfirmProvider><Ask onAnswer={onAnswer} /></ConfirmProvider>);
    fireEvent.click(screen.getByRole("button", { name: "Ask" }));
    expect(await screen.findByText("Delete it?")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Delete" }).className).toContain("rm-button-danger");
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));
    await waitFor(() => expect(onAnswer).toHaveBeenLastCalledWith(true));
    fireEvent.click(screen.getByRole("button", { name: "Ask" }));
    fireEvent.click(await screen.findByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(onAnswer).toHaveBeenLastCalledWith(false));
  });

  it("is not available outside its provider", () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    expect(() => renderHook(() => useConfirm())).toThrow();
  });
});

describe("LoadingState, EmptyState and Badge", () => {
  it("say what is loading as a status line", () => {
    render(<LoadingState label="Loading jobs" />);
    expect(screen.getByRole("status").textContent).toBe("Loading jobs…");
  });

  it("show a title, a sentence and one action", () => {
    render(<EmptyState action={<Button>Create one</Button>} description="Nothing here yet." title="No jobs" />);
    expect(screen.getByText("No jobs")).toBeTruthy();
    expect(screen.getByText("Nothing here yet.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Create one" })).toBeTruthy();
  });

  it("marks a tone with data and leaves neutral plain", () => {
    render(<><Badge>Plain</Badge><Badge tone="warning">Review</Badge></>);
    expect(screen.getByText("Plain").getAttribute("data-tone")).toBeNull();
    expect(screen.getByText("Review").getAttribute("data-tone")).toBe("warning");
  });
});

describe("SegmentedControl", () => {
  const options = [{ label: "Grid", value: "grid" }, { label: "List", value: "list" }];

  it("chooses another option and never turns the chosen one off", () => {
    const onValueChange = vi.fn();
    render(<SegmentedControl aria-label="View" onValueChange={onValueChange} options={options} value="grid" />);
    fireEvent.click(screen.getByRole("radio", { name: "List" }));
    expect(onValueChange).toHaveBeenCalledWith("list");
    onValueChange.mockClear();
    fireEvent.click(screen.getByRole("radio", { name: "Grid" }));
    expect(onValueChange).not.toHaveBeenCalled();
  });
});

describe("Combobox", () => {
  const options = [
    { label: "Runbook", value: "1", description: "docs/runbook.md" },
    { label: "Design notes", value: "2", description: "docs/design.md" },
    { label: "Changelog", value: "3" },
  ];

  function Picker({ onValueChange }: { onValueChange: (value: string) => void }) {
    const [value, setValue] = useState("");
    return <Combobox aria-label="File" onValueChange={(next) => { setValue(next); onValueChange(next); }} options={options} value={value} />;
  }

  it("filters by what is typed, in the label or the description", async () => {
    render(<Picker onValueChange={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "File" }));
    const search = await screen.findByRole("combobox", { name: "Search: File" });
    expect(screen.getAllByRole("option")).toHaveLength(3);
    fireEvent.change(search, { target: { value: "design" } });
    expect(screen.getAllByRole("option")).toHaveLength(1);
    expect(screen.getByRole("option").textContent).toContain("Design notes");
    fireEvent.change(search, { target: { value: "zzz" } });
    expect(screen.queryAllByRole("option")).toHaveLength(0);
    expect(screen.getByText("Nothing matches")).toBeTruthy();
  });

  it("chooses the active match with Enter", async () => {
    const onValueChange = vi.fn();
    render(<Picker onValueChange={onValueChange} />);
    fireEvent.click(screen.getByRole("button", { name: "File" }));
    const search = await screen.findByRole("combobox", { name: "Search: File" });
    fireEvent.change(search, { target: { value: "change" } });
    fireEvent.keyDown(search, { key: "Enter" });
    expect(onValueChange).toHaveBeenCalledWith("3");
  });
});

describe("SearchField", () => {
  function Filter({ initial = "abc" }: { initial?: string }) {
    const [value, setValue] = useState(initial);
    return <SearchField aria-label="Filter" onValueChange={setValue} value={value} />;
  }

  it("clears with Escape and with its clear button", () => {
    render(<Filter />);
    const field = screen.getByLabelText("Filter") as HTMLInputElement;
    fireEvent.keyDown(field, { key: "Escape" });
    expect(field.value).toBe("");
    expect(screen.queryByRole("button", { name: "Clear search" })).toBeNull();
    fireEvent.change(field, { target: { value: "xyz" } });
    fireEvent.click(screen.getByRole("button", { name: "Clear search" }));
    expect(field.value).toBe("");
    expect(document.activeElement).toBe(field);
  });

  it("lets Escape through when there is nothing to clear", () => {
    const onKeyDown = vi.fn();
    render(<div onKeyDown={onKeyDown}><SearchField aria-label="Filter" onValueChange={() => undefined} value="" /></div>);
    fireEvent.keyDown(screen.getByLabelText("Filter"), { key: "Escape" });
    expect(onKeyDown).toHaveBeenCalled();
  });
});

describe("CopyButton", () => {
  it("copies, says Copied for two seconds and then goes back", async () => {
    vi.useFakeTimers();
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });
    render(<CopyButton label="Copy hash" text="Copy" value="abc123" />);
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: /Copy/ })); });
    expect(writeText).toHaveBeenCalledWith("abc123");
    expect(screen.getByRole("button", { name: /Copied/ })).toBeTruthy();
    await act(async () => { vi.advanceTimersByTime(2100); });
    expect(screen.getByRole("button", { name: /Copy$/ })).toBeTruthy();
    vi.useRealTimers();
  });

  it("warns by toast and runs onUnavailable when the clipboard refuses", async () => {
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: vi.fn().mockRejectedValue(new Error("denied")) } });
    const onUnavailable = vi.fn();
    render(<CopyButton label="Copy hash" onUnavailable={onUnavailable} value="abc123" />);
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Copy hash" })); });
    expect(onUnavailable).toHaveBeenCalledTimes(1);
    expect(showToast).toHaveBeenCalledWith("warning", expect.stringContaining("Copy is not available"));
  });
});

describe("useFormState", () => {
  it("follows the saved record: a new initial value starts the form over", () => {
    const { result, rerender } = renderHook(({ name }) => useFormState({ name }), { initialProps: { name: "Ada" } });
    act(() => result.current.set("name", "Grace"));
    expect(result.current.isDirty).toBe(true);
    rerender({ name: "Grace" });
    expect(result.current.isDirty).toBe(false);
    expect(result.current.values.name).toBe("Grace");
  });
});
