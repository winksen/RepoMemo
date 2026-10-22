import { QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, renderHook, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({
  getSharedWorkspaceTemplate: vi.fn(),
  listSharedTemplates: vi.fn(),
  saveSharedWorkspaceTemplate: vi.fn(),
}));

import { TemplatePicker } from "../components/TemplatePicker";
import { queryClient, serverKeys, workspaceKeys } from "../lib/queryClient";
import { getSharedWorkspaceTemplate, listSharedTemplates } from "../lib/sharedApi";
import type { TemplateSummary, WorkspaceTemplateView } from "../types";
import { useTemplate, useTemplateCatalogue } from "../web/workspace/data";

const general: TemplateSummary = { id: "general", version: 1, name: "General", summary: "Every section in its usual order.", icon: "stack" };
const operations: TemplateSummary = { id: "operations", version: 2, name: "Operations and incidents", summary: "Runbooks and incidents.", icon: "heartbeat" };
const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;

beforeEach(() => { vi.clearAllMocks(); queryClient.clear(); });
afterEach(cleanup);

describe("TemplatePicker", () => {
  it("shows nothing while the server offers only General, so a plain server looks as it did", () => {
    const { container } = render(<TemplatePicker name="t" onChange={vi.fn()} templates={[general]} value="general" />);
    expect(container.childElementCount).toBe(0);
    const empty = render(<TemplatePicker name="t" onChange={vi.fn()} templates={[]} value="general" />);
    expect(empty.container.childElementCount).toBe(0);
  });

  it("offers one radio card for each template, with its name and what it is for, and General checked", () => {
    render(<TemplatePicker name="t" onChange={vi.fn()} templates={[general, operations]} value="general" />);
    const group = screen.getByRole("group", { name: "Template" });
    expect(group).toBeTruthy();
    expect(screen.getAllByRole("radio")).toHaveLength(2);
    expect((screen.getByRole("radio", { name: /General/ }) as HTMLInputElement).checked).toBe(true);
    expect((screen.getByRole("radio", { name: /Operations and incidents/ }) as HTMLInputElement).checked).toBe(false);
    expect(screen.getByText("Runbooks and incidents.")).toBeTruthy();
  });

  it("reports the template that was chosen, and cannot be changed while the form is busy", () => {
    const onChange = vi.fn();
    const { rerender } = render(<TemplatePicker name="t" onChange={onChange} templates={[general, operations]} value="general" />);
    fireEvent.click(screen.getByRole("radio", { name: /Operations and incidents/ }));
    expect(onChange).toHaveBeenCalledWith("operations");
    rerender(<TemplatePicker disabled name="t" onChange={onChange} templates={[general, operations]} value="operations" />);
    expect((screen.getByRole("radio", { name: /Operations and incidents/ }) as HTMLInputElement).checked).toBe(true);
    expect((screen.getByRole("radio", { name: /General/ }) as HTMLInputElement).disabled).toBe(true);
  });
});

describe("the template hooks", () => {
  it("load the catalogue once for everyone who asks, and a workspace's template under its own key", async () => {
    vi.mocked(listSharedTemplates).mockResolvedValue([general, operations]);
    const view: WorkspaceTemplateView = {
      template: general, source: "default", reason: null, chosen_id: null, overrides: null,
      sections: { primary: ["overview", "evidence"], more: [], landing: "overview" },
      labels: { sections: {}, artifact_types: {} },
      overview: { cards: ["index_coverage"] },
      evidence: { default_view: "grid", types_first: [] },
      assistant: { order: [], suggestions: [] },
      retrieval: { type_chips: [] },
      ranking: { verified: 1.1, boosts: [], recency: null }, health: { detectors: ["index_failed"] },
    };
    vi.mocked(getSharedWorkspaceTemplate).mockResolvedValue(view);

    const catalogue = renderHook(() => useTemplateCatalogue("token"), { wrapper });
    const again = renderHook(() => useTemplateCatalogue("token"), { wrapper });
    expect(catalogue.result.current[0]).toEqual([]);
    await waitFor(() => expect(catalogue.result.current[0]).toEqual([general, operations]));
    await waitFor(() => expect(again.result.current[0]).toEqual([general, operations]));
    expect(listSharedTemplates).toHaveBeenCalledTimes(1);
    expect(queryClient.getQueryData(serverKeys.templates())).toEqual([general, operations]);

    const workspace = renderHook(() => useTemplate("token", "w1"), { wrapper });
    expect(workspace.result.current[0]).toBeNull();
    await waitFor(() => expect(workspace.result.current[0]).toEqual(view));
    expect(getSharedWorkspaceTemplate).toHaveBeenCalledWith("token", "w1");
    expect(queryClient.getQueryData(workspaceKeys.template("w1"))).toEqual(view);
  });

  it("do not ask the server when the page has no use for the answer", async () => {
    renderHook(() => useTemplateCatalogue("token", false), { wrapper });
    renderHook(() => useTemplate("token", "w1", false), { wrapper });
    await Promise.resolve();
    expect(listSharedTemplates).not.toHaveBeenCalled();
    expect(getSharedWorkspaceTemplate).not.toHaveBeenCalled();
  });
});
