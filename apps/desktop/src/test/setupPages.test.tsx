import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({
  attachEnvironment: vi.fn(),
  createEnvironment: vi.fn(),
  createSetupAdmin: vi.fn(),
  getSetupChecks: vi.fn(),
  listEnvironments: vi.fn(),
  verifySetupCode: vi.fn(),
  SharedApiError: class SharedApiError extends Error { status = 0; },
}));
vi.mock("../components/ui/toast", () => ({ showToast: vi.fn() }));
vi.mock("../components/SharedLayout", () => ({ ThemeToggle: () => null }));

import { createSetupAdmin, getSetupChecks, listEnvironments, verifySetupCode } from "../lib/sharedApi";
import { PlatformSetup } from "../components/PlatformSetup";
import { AdminStep } from "../web/setup/AdminStep";
import { ChecksStep } from "../web/setup/ChecksStep";
import { SettingsStep } from "../web/setup/SettingsStep";
import { WelcomeStep } from "../web/setup/WelcomeStep";
import type { SystemSetting } from "../types";

beforeEach(() => { vi.clearAllMocks(); });
afterEach(() => { cleanup(); });

describe("the setup wizard on the shared pieces", () => {
  it("asks for the setup code in a labelled, required field and enables Continue once there is one", async () => {
    vi.mocked(verifySetupCode).mockResolvedValue(undefined as never);
    const onVerified = vi.fn();
    render(<WelcomeStep code="" onCode={vi.fn()} onVerified={onVerified} />);
    const field = screen.getByLabelText("Setup code") as HTMLInputElement;
    expect(field.required).toBe(true);
    expect(field.closest(".rm-field")).not.toBeNull();
    expect((screen.getByRole("button", { name: "Continue" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("labels the four fields of the administrator step and marks the button busy while it saves", async () => {
    vi.mocked(createSetupAdmin).mockReturnValue(new Promise(() => undefined));
    render(<AdminStep code="ABCD" onBack={vi.fn()} onCodeRejected={vi.fn()} onCreated={vi.fn()} />);
    for (const label of ["Name", "Email", "Password", "Confirm password"]) {
      expect((screen.getByLabelText(label) as HTMLInputElement).required).toBe(true);
    }
    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "Ada" } });
    fireEvent.change(screen.getByLabelText("Email"), { target: { value: "ada@example.com" } });
    fireEvent.change(screen.getByLabelText("Password"), { target: { value: "correct horse battery" } });
    fireEvent.change(screen.getByLabelText("Confirm password"), { target: { value: "correct horse battery" } });
    fireEvent.click(screen.getByRole("button", { name: "Create administrator" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Create administrator" }).getAttribute("aria-busy")).toBe("true"));
  });

  it("shows the shared loading state while the server is checked", () => {
    vi.mocked(getSetupChecks).mockReturnValue(new Promise(() => undefined));
    render(<ChecksStep accessToken="t" onNext={vi.fn()} onWarnings={vi.fn()} />);
    expect(screen.getByRole("status").textContent).toBe("Checking the server…");
  });

  it("shows a boolean setting as a switch and a missing list as the loading state", () => {
    const setting = { key: "registration", label: "Open registration", description: "Anyone can register.", kind: "boolean", value: false, default_value: false, group: "Access", options: [] } as unknown as SystemSetting;
    const { rerender } = render(<SettingsStep accessToken="t" description="Access" keys={["registration"]} onBack={vi.fn()} onNext={vi.fn()} onSaved={vi.fn()} settings={null} title="Access" />);
    expect(screen.getByRole("status").textContent).toBe("Loading settings…");
    rerender(<SettingsStep accessToken="t" description="Access" keys={["registration"]} onBack={vi.fn()} onNext={vi.fn()} onSaved={vi.fn()} settings={[setting]} title="Access" />);
    const toggle = screen.getByRole("switch");
    expect(toggle.getAttribute("aria-checked")).toBe("false");
    fireEvent.click(toggle);
    expect(screen.getByRole("switch").getAttribute("aria-checked")).toBe("true");
    expect(screen.getByRole("button", { name: /Save and continue/ })).toBeTruthy();
  });
});

describe("the environment menu on the shared pieces", () => {
  it("unlocks with the code from a labelled field, then offers a labelled name field and an empty state", async () => {
    vi.mocked(listEnvironments).mockResolvedValue([]);
    render(<PlatformSetup folder="environments" onAttached={vi.fn()} pinned={null} />);
    const code = screen.getByLabelText("Code") as HTMLInputElement;
    expect(code.required).toBe(true);
    fireEvent.change(code, { target: { value: "ABCD-EFGH-JKLM" } });
    fireEvent.click(screen.getByRole("button", { name: "Unlock" }));
    await screen.findByText("No environment yet");
    expect(listEnvironments).toHaveBeenCalledWith("ABCD-EFGH-JKLM");
    const name = screen.getByLabelText("Name") as HTMLInputElement;
    expect(name.closest(".rm-field")).not.toBeNull();
    expect(screen.getByText(/It is created as/)).toBeTruthy();
  });
});
