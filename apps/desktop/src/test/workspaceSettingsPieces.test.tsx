import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({
  getSharedAiPolicy: vi.fn(),
  saveSharedAiPolicy: vi.fn(),
  saveSharedWorkspaceAiProvider: vi.fn(),
  testSharedWorkspaceAiProvider: vi.fn(),
}));
vi.mock("../components/ui/toast", () => ({ Toast: () => null, showToast: vi.fn() }));

import { getSharedAiPolicy, saveSharedWorkspaceAiProvider } from "../lib/sharedApi";
import { AiPolicySettings } from "../components/AiPolicySettings";
import { AiProviderForm } from "../components/AiProviderForm";

beforeEach(() => { vi.clearAllMocks(); });
afterEach(() => { cleanup(); });

function renderProvider() {
  return render(<AiProviderForm accessToken="t" onSaved={vi.fn()} providers={[]} purpose="text" workspaceId="w1" />);
}

describe("the AI provider form on the shared pieces", () => {
  it("labels its fields, marking the name and model as required", () => {
    renderProvider();
    expect((screen.getByLabelText("Provider name") as HTMLInputElement).required).toBe(true);
    expect((screen.getByLabelText("Chat model") as HTMLInputElement).required).toBe(true);
    expect((screen.getByLabelText("Base URL") as HTMLInputElement).closest(".rm-field")).not.toBeNull();
    expect(screen.getByLabelText("Provider").closest(".rm-field")).not.toBeNull();
    expect(screen.queryByLabelText("API key")).toBeNull();
  });

  it("asks for the cloud acknowledgement with the shared checkbox and marks only the save button busy while saving", async () => {
    vi.mocked(saveSharedWorkspaceAiProvider).mockReturnValue(new Promise(() => undefined));
    renderProvider();
    fireEvent.click(screen.getByLabelText("Provider"));
    fireEvent.click(await screen.findByRole("option", { name: "OpenRouter (cloud)" }));
    expect(screen.getByLabelText("API key")).toBeTruthy();
    const acknowledgement = screen.getByRole("checkbox");
    expect(acknowledgement.getAttribute("aria-checked")).toBe("false");
    fireEvent.click(acknowledgement);
    expect(screen.getByRole("checkbox").getAttribute("aria-checked")).toBe("true");
    fireEvent.click(screen.getByRole("button", { name: "Save provider" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Save provider" }).getAttribute("aria-busy")).toBe("true"));
    expect(screen.getByRole("button", { name: "Test connection" }).getAttribute("aria-busy")).toBeNull();
  });
});

describe("the AI policy on the shared pieces", () => {
  it("ties the role dropdown to its label", async () => {
    vi.mocked(getSharedAiPolicy).mockResolvedValue({ min_role: "member", requests_per_hour: 0 } as never);
    render(<AiPolicySettings accessToken="t" workspaceId="w1" />);
    const trigger = await screen.findByLabelText("AI features available to");
    expect(trigger.closest(".rm-field")).not.toBeNull();
    await waitFor(() => expect((trigger as HTMLButtonElement).disabled).toBe(false));
  });
});
