import { QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen } from "@testing-library/react";
// The browser tsconfig intentionally omits Node types; these imports run in Vitest only.
// @ts-expect-error Node type declarations are outside the browser bundle.
import { readFileSync } from "node:fs";
// @ts-expect-error Node type declarations are outside the browser bundle.
import { resolve } from "node:path";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({
  getSharedMyWork: vi.fn(),
  getSharedOnboarding: vi.fn(),
  getSharedReviewAssignment: vi.fn(),
  listSharedWorkspaceMembers: vi.fn(),
  setSharedOnboardingDismissed: vi.fn(),
  setSharedReviewAssignment: vi.fn(),
  completeSharedReview: vi.fn(),
}));
vi.mock("../components/ui/toast", () => ({ Toast: () => null, showToast: vi.fn() }));
vi.mock("../lib/liveEvents", () => ({ useLiveRefresh: vi.fn() }));

import { getSharedMyWork, getSharedOnboarding, getSharedReviewAssignment, listSharedWorkspaceMembers } from "../lib/sharedApi";
import { queryClient } from "../lib/queryClient";
import { formatDateValue, formatDateTimeValue, formatNumber, setLocale } from "../lib/locale";
import { messageKeys, setMessages, t } from "../lib/messages";
import { MyWorkPanel } from "../components/MyWorkPanel";
import { OnboardingChecklist } from "../components/OnboardingChecklist";
import { ReviewOwnershipPanel } from "../components/ReviewOwnershipPanel";

afterEach(() => { cleanup(); setLocale(null); setMessages(null); });

describe("the one formatter for dates and numbers", () => {
  it("follows the locale it is given, so the same day reads as each person writes it", () => {
    setLocale("en-US");
    expect(formatDateValue("2026-12-01T12:00:00Z", { dateStyle: "medium", timeZone: "UTC" })).toBe("Dec 1, 2026");
    expect(formatNumber(1234567.5)).toBe("1,234,567.5");
    setLocale("de-DE");
    expect(formatDateValue("2026-12-01T12:00:00Z", { dateStyle: "medium", timeZone: "UTC" })).toBe("01.12.2026");
    expect(formatNumber(1234567.5)).toBe("1.234.567,5");
  });

  it("gives back a value that is not a date as it was, and puts the time after the date", () => {
    expect(formatDateValue("not a date")).toBe("not a date");
    expect(formatDateTimeValue("2026-12-01T14:05:00Z").length).toBeGreaterThan(formatDateValue("2026-12-01T14:05:00Z").length);
  });

  it("is the only place the core journeys format a date: none of their files calls toLocale directly", () => {
    const files = [
      "components/OnboardingChecklist.tsx", "components/MyWorkPanel.tsx", "components/ReviewOwnershipPanel.tsx", "components/MemoryApprovalPanel.tsx",
      "components/MemoryHistoryPanel.tsx", "components/MemoryChangesPanel.tsx", "components/WorkspaceTrashSettings.tsx", "components/OrganizationTemplatesPanel.tsx",
      "components/SearchFeedbackPanel.tsx", "web/format.ts",
    ];
    for (const file of files) expect(readFileSync(resolve("src", file), "utf8"), file).not.toMatch(/toLocale/);
  });
});

describe("the message catalogue", () => {
  it("fills in values, falls back to English for a key a language leaves out, and can be reset", () => {
    expect(t("onboarding.progress", { done: 2, total: 4 })).toBe("2 of 4 done");
    setMessages({ "onboarding.progress": "{done} von {total} erledigt" });
    expect(t("onboarding.progress", { done: 2, total: 4 })).toBe("2 von 4 erledigt");
    expect(t("onboarding.hide")).toBe("Hide for now");
    setMessages(null);
    expect(t("onboarding.progress", { done: 1, total: 3 })).toBe("1 of 3 done");
  });

  it("leaves a value it was not given visible rather than blank", () => {
    expect(t("onboarding.title")).toBe("Getting started with {template}");
  });
});

describe("a test language proves the core journeys read their words from the catalogue", () => {
  beforeEach(() => { vi.clearAllMocks(); queryClient.clear(); });

  const marked = () => {
    const english = Object.fromEntries(messageKeys.map((key) => [key, `«${t(key)}»`]));
    setMessages(english);
  };

  it("first value", async () => {
    vi.mocked(getSharedOnboarding).mockResolvedValue({ steps: [{ id: "add_evidence", title: "Add evidence", detail: "Upload a file.", done: true }], done_count: 1, total_count: 2, dismissed: false, template_name: "General" });
    marked();
    render(<OnboardingChecklist accessToken="t" workspaceId="w1" />);
    expect(await screen.findByText("«Getting started with General»")).toBeTruthy();
    expect(screen.getByText("«1 of 2 done»")).toBeTruthy();
    expect(screen.getByText("«Done»")).toBeTruthy();
    expect(screen.getByRole("button", { name: "«Hide for now»" })).toBeTruthy();
  });

  it("My work", async () => {
    vi.mocked(getSharedMyWork).mockResolvedValue({ tasks: [], reviews: [], mentions: [], overdue_task_count: 0, total_count: 0 });
    marked();
    render(<QueryClientProvider client={queryClient}><MyWorkPanel accessToken="t" userId="u1" /></QueryClientProvider>);
    expect(await screen.findByText(/«Nothing is waiting for you/)).toBeTruthy();
    expect(screen.getByText("«My work»")).toBeTruthy();
  });

  it("review", async () => {
    vi.mocked(getSharedReviewAssignment).mockResolvedValue({ kind: "memory_card", item_id: "c1", reviewer: null, reviewer_departed: false, reviewer_label: null, next_review_at: null, last_reviewed_at: null, overdue: false });
    vi.mocked(listSharedWorkspaceMembers).mockResolvedValue([]);
    marked();
    render(<ReviewOwnershipPanel accessToken="t" canWrite={false} itemId="c1" kind="memory_card" workspaceId="w1" />);
    expect(await screen.findByText("«No review date»")).toBeTruthy();
    expect(screen.getByText(/«Nobody is accountable for reviewing this yet\.»/)).toBeTruthy();
  });
});
