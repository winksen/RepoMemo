import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({
  getSystemMail: vi.fn(),
  saveSystemMail: vi.fn(),
  sendSystemTestMail: vi.fn(),
}));
vi.mock("../components/ui/toast", () => ({ Toast: () => null, showToast: vi.fn() }));

import { getSystemMail } from "../lib/sharedApi";
import { MailSettingsPanel } from "../components/MailSettingsPanel";

const overview = {
  settings: { enabled: false, configured: true, host: "smtp.example.com", port: 587, security: "starttls", username: "mailer", password_set: true, from_address: "repomemo@example.com", from_name: "RepoMemo" },
  queued_count: 0,
  sent_last_day: 0,
  failed_last_day: 0,
  entries: [],
};

beforeEach(() => { vi.mocked(getSystemMail).mockResolvedValue(overview as never); });
afterEach(() => { cleanup(); vi.clearAllMocks(); });

describe("mail settings on the shared pieces", () => {
  it("shows the loading state, then labelled fields, a switch for sending and a checkbox for removing the password", async () => {
    render(<MailSettingsPanel accessToken="t" canChange />);
    expect(screen.getByRole("status").textContent).toBe("Loading mail settings…");
    const host = await screen.findByLabelText("Mail server") as HTMLInputElement;
    expect(host.value).toBe("smtp.example.com");
    for (const label of ["Port", "Connection", "User name", "Password", "Sender address", "Sender name"]) {
      expect(screen.getByLabelText(label).closest(".rm-field")).not.toBeNull();
    }
    const sending = screen.getByRole("switch", { name: "Send mail" });
    expect(sending.getAttribute("aria-checked")).toBe("false");
    fireEvent.click(sending);
    expect(screen.getByRole("switch", { name: "Send mail" }).getAttribute("aria-checked")).toBe("true");
    const remove = screen.getByRole("checkbox", { name: "Remove the saved password" });
    fireEvent.click(remove);
    expect((screen.getByLabelText("Password") as HTMLInputElement).disabled).toBe(true);
  });

  it("is read only without the right to change: disabled fields, no save button and an empty state for the outbox", async () => {
    render(<MailSettingsPanel accessToken="t" canChange={false} />);
    expect(((await screen.findByLabelText("Mail server")) as HTMLInputElement).disabled).toBe(true);
    expect(screen.queryByRole("button", { name: "Save mail settings" })).toBeNull();
    expect(screen.queryByRole("checkbox", { name: "Remove the saved password" })).toBeNull();
    expect(screen.getByText("No messages yet").closest(".rm-empty-state")).not.toBeNull();
  });
});
