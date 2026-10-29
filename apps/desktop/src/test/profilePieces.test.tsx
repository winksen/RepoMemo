import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({
  createPersonalToken: vi.fn(),
  listPersonalTokens: vi.fn(),
  listSharedWorkspaces: vi.fn().mockResolvedValue([]),
  limitPersonalTokenWorkspaces: vi.fn(),
  revokePersonalToken: vi.fn(),
}));
vi.mock("../components/ui/toast", () => ({ showToast: vi.fn() }));

import { createPersonalToken, listPersonalTokens } from "../lib/sharedApi";
import { ApiTokensPanel } from "../components/ApiTokensPanel";

beforeEach(() => { vi.clearAllMocks(); });
afterEach(() => { cleanup(); });

describe("API tokens on the shared pieces", () => {
  it("shows the loading state, then the empty state with the token form's labelled fields", async () => {
    vi.mocked(listPersonalTokens).mockResolvedValue([]);
    render(<ApiTokensPanel accessToken="t" isAdministrator={false} />);
    expect(screen.getByRole("status").textContent).toBe("Loading API tokens…");
    await screen.findByText("No tokens yet");
    const name = screen.getByLabelText("Name") as HTMLInputElement;
    expect(name.required).toBe(true);
    expect(name.closest(".rm-field")).not.toBeNull();
    expect(screen.getByLabelText("Access").closest(".rm-field")).not.toBeNull();
    expect(screen.getByLabelText("Expires after").textContent).toContain("90 days");
  });

  it("marks the create button busy while the token is created and keeps it disabled without a name", async () => {
    vi.mocked(listPersonalTokens).mockResolvedValue([]);
    vi.mocked(createPersonalToken).mockReturnValue(new Promise(() => undefined));
    render(<ApiTokensPanel accessToken="t" isAdministrator={false} />);
    await screen.findByText("No tokens yet");
    const create = screen.getByRole("button", { name: /Create token/ }) as HTMLButtonElement;
    expect(create.disabled).toBe(true);
    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "CI" } });
    fireEvent.click(screen.getByRole("button", { name: /Create token/ }));
    expect(screen.getByRole("button", { name: /Create token/ }).getAttribute("aria-busy")).toBe("true");
  });
});
