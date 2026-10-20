import { QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({
  downloadSystemAuditExport: vi.fn(),
  getSystemUsage: vi.fn(),
  listSystemAuditEvents: vi.fn(),
}));
vi.mock("../components/ui/toast", () => ({ Toast: () => null, showToast: vi.fn() }));

import { listSystemAuditEvents } from "../lib/sharedApi";
import { queryClient } from "../lib/queryClient";
import { AuditSection } from "../web/system/Audit";

beforeEach(() => { queryClient.clear(); });
afterEach(() => { cleanup(); vi.clearAllMocks(); });

function renderAudit() {
  return render(<QueryClientProvider client={queryClient}><AuditSection accessToken="t" canExport /></QueryClientProvider>);
}

describe("the System toolbars on the shared pieces", () => {
  it("labels the export format and the day range as fields, the days as date inputs bound to each other", async () => {
    vi.mocked(listSystemAuditEvents).mockResolvedValue([]);
    renderAudit();
    expect(screen.getByLabelText("Format").closest(".rm-field")).not.toBeNull();
    const from = screen.getByLabelText("From") as HTMLInputElement;
    const to = screen.getByLabelText("To") as HTMLInputElement;
    expect(from.type).toBe("date");
    expect(to.type).toBe("date");
    fireEvent.change(from, { target: { value: "2026-10-01" } });
    expect((screen.getByLabelText("To") as HTMLInputElement).min).toBe("2026-10-01");
    fireEvent.change(screen.getByLabelText("To"), { target: { value: "2026-10-15" } });
    expect((screen.getByLabelText("From") as HTMLInputElement).max).toBe("2026-10-15");
  });

  it("shows the loading state, then the shared empty state when nothing is recorded", async () => {
    vi.mocked(listSystemAuditEvents).mockResolvedValue([]);
    renderAudit();
    expect(screen.getByRole("status").textContent).toBe("Loading the audit trail…");
    expect((await screen.findByText("Nothing recorded yet")).closest(".rm-empty-state")).not.toBeNull();
  });
});
