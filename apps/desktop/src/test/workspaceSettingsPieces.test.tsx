import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({
  getSharedAiPolicy: vi.fn(),
  saveSharedAiPolicy: vi.fn(),
  saveSharedWorkspaceAiProvider: vi.fn(),
  testSharedWorkspaceAiProvider: vi.fn(),
}));
vi.mock("../components/ui/toast", () => ({ Toast: () => null, showToast: vi.fn() }));

import { getSharedAiPolicy } from "../lib/sharedApi";
import { AiPolicySettings } from "../components/AiPolicySettings";

beforeEach(() => { vi.clearAllMocks(); });
afterEach(() => { cleanup(); });

describe("the AI policy on the shared pieces", () => {
  it("ties the role dropdown to its label", async () => {
    vi.mocked(getSharedAiPolicy).mockResolvedValue({ min_role: "member", requests_per_hour: 0 } as never);
    render(<AiPolicySettings accessToken="t" workspaceId="w1" />);
    const trigger = await screen.findByLabelText("AI features available to");
    expect(trigger.closest(".rm-field")).not.toBeNull();
    await waitFor(() => expect((trigger as HTMLButtonElement).disabled).toBe(false));
  });
});
