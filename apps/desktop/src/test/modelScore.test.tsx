import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { ModelPicker, modelLabel, toolChoiceNote } from "../components/ModelPicker";
import type { OfferedModel } from "../types";

afterEach(cleanup);

const measured: OfferedModel = { id: "m1", name: "Qwen 32B", locality: "local", agent: true, default: true, evaluation: { tool_accuracy: 0.87, citation_precision: 0.95, cases_run: 47, run_on: "2026-11-30", passed: true } };
const failing: OfferedModel = { id: "m2", name: "Tiny 3B", locality: "local", agent: true, default: false, evaluation: { tool_accuracy: 0.52, citation_precision: 0.7, cases_run: 47, run_on: "2026-11-30", passed: false } };
const unmeasured: OfferedModel = { id: "m3", name: "Cloud Pro", locality: "cloud", agent: true, default: false, evaluation: null };

describe("a model's tool-choice score (V0.9.144)", () => {
  it("shows the score and the date of a model with a report", () => {
    expect(toolChoiceNote(measured.evaluation)).toBe("Tool choice 87% · 2026-11-30");
    expect(modelLabel(measured)).toBe("Qwen 32B · Local · Agent · Tool choice 87% · 2026-11-30");
  });

  it("says so when a report did not pass, instead of hiding it", () => {
    expect(toolChoiceNote(failing.evaluation)).toBe("Tool choice 52% · 2026-11-30 · did not pass");
  });

  it("says Not measured for a model without a report, whether the field is empty or missing", () => {
    expect(toolChoiceNote(unmeasured.evaluation)).toBe("Not measured");
    expect(modelLabel({ ...unmeasured, evaluation: undefined })).toBe("Cloud Pro · Cloud · Agent · Not measured");
  });

  it("reaches the chat's picker", () => {
    render(<ModelPicker models={[measured, failing, unmeasured]} onChange={() => undefined} selected={null} />);
    const trigger = screen.getByRole("combobox", { name: "Model for this chat" });
    expect(trigger.textContent).toContain("Tool choice 87% · 2026-11-30");
  });
});
