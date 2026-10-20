import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { SettingControl, settingProblem } from "../web/system/SettingControl";
import type { SystemSetting } from "../types";

afterEach(() => { cleanup(); });

const base = { key: "k", group: "g", label: "Retention", description: "How long.", min: 1, max: 365, unit: "days", options: [], overridden: false, updated_at: null, updated_by: null };
const integer = { ...base, kind: "integer", value: 30, default_value: 30 } as SystemSetting;
const flag = { ...base, kind: "boolean", value: false, default_value: false } as SystemSetting;
const choice = { ...base, kind: "choice", value: "info", default_value: "info", options: [{ value: "info", label: "Info" }, { value: "debug", label: "Debug" }] } as SystemSetting;

describe("the control of a setting row", () => {
  it("finds a whole number outside its range, and nothing wrong with other kinds", () => {
    expect(settingProblem(integer, 30)).toBeNull();
    expect(settingProblem(integer, 0)).toBe("Between 1 and 365 days.");
    expect(settingProblem(integer, 366)).toBe("Between 1 and 365 days.");
    expect(settingProblem(integer, 2.5)).toBe("Between 1 and 365 days.");
    expect(settingProblem(flag, true)).toBeNull();
    expect(settingProblem(choice, "debug")).toBeNull();
  });

  it("marks a number outside its range invalid with a message tied to the field and the row's description", () => {
    render(<SettingControl describedBy="row-description" id="row" onChange={vi.fn()} setting={integer} value={400} />);
    const field = screen.getByRole("spinbutton") as HTMLInputElement;
    expect(field.getAttribute("aria-invalid")).toBe("true");
    expect(field.getAttribute("aria-describedby")).toBe("row-description row-message");
    expect(screen.getByText("Between 1 and 365 days.").id).toBe("row-message");
  });

  it("reports a typed number, and falls back to the minimum when the field is emptied", () => {
    const onChange = vi.fn();
    render(<SettingControl id="row" onChange={onChange} setting={integer} value={30} />);
    fireEvent.change(screen.getByRole("spinbutton"), { target: { value: "45" } });
    expect(onChange).toHaveBeenLastCalledWith(45);
    fireEvent.change(screen.getByRole("spinbutton"), { target: { value: "" } });
    expect(onChange).toHaveBeenLastCalledWith(1);
    expect(screen.getByRole("spinbutton").getAttribute("aria-invalid")).toBeNull();
  });

  it("shows a boolean as a switch labelled with its state and a choice as a dropdown", () => {
    const onChange = vi.fn();
    const { rerender } = render(<SettingControl id="flag" onChange={onChange} setting={flag} value={false} />);
    const toggle = screen.getByRole("switch");
    expect(screen.getByText("Off")).toBeTruthy();
    fireEvent.click(toggle);
    expect(onChange).toHaveBeenCalledWith(true);
    rerender(<SettingControl disabled id="pick" onChange={onChange} setting={choice} value="info" />);
    expect((screen.getByRole("combobox", { name: "Retention" }) as HTMLButtonElement).disabled).toBe(true);
  });
});
