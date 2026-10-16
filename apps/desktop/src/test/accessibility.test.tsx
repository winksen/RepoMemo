import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SharedLayout } from "../components/SharedLayout";
import { SheetsPreview } from "../components/DocumentViewer";
import { ToastViewport, showToast } from "../components/ui/toast";
import { RouteAnnouncer } from "../web/RouteAnnouncer";
import { pageKey, pageTitle } from "../web/pageTitle";
import { parseRoute } from "../web/router";
import type { SharedSession } from "../types";

afterEach(() => { cleanup(); vi.useRealTimers(); });

const session = { user: { id: "u1", email: "a@example.com", display_name: "Ada" }, authentication: "jwt", memberships: [] } as unknown as SharedSession;

describe("the page frame", () => {
  it("has a skip link first, a header, a labelled rail and one main landmark that can take focus", () => {
    render(<SharedLayout apiAvailable onNavigate={() => undefined} session={session} sidebar={<p>rail</p>} signOut={() => undefined}><h1>Page</h1></SharedLayout>);
    const skip = screen.getByRole("link", { name: "Skip to the page" });
    expect(document.body.querySelector("a")).toBe(skip);
    expect(screen.getAllByRole("main")).toHaveLength(1);
    expect(screen.getByRole("banner")).toBeTruthy();
    expect(screen.getByRole("complementary", { name: "Workspaces and organizations" })).toBeTruthy();
    // The header is not inside the main landmark.
    expect(screen.getByRole("main").contains(screen.getByRole("banner"))).toBe(false);
    fireEvent.click(skip);
    expect(document.activeElement).toBe(screen.getByRole("main"));
  });
});

describe("page titles", () => {
  it("name the page, with the workspace and the section where there is one", () => {
    expect(pageTitle(parseRoute("/dashboard"))).toBe("Dashboard");
    expect(pageTitle(parseRoute("/workspaces/w1/people"), { workspace: "Atlas" })).toBe("Atlas · People & Activity");
    expect(pageTitle(parseRoute("/workspaces/w1/retrieval"), { workspace: "Atlas" })).toBe("Atlas · Retrieval");
    expect(pageTitle(parseRoute("/system/audit"))).toBe("System · Audit trail");
    expect(pageTitle(parseRoute("/nowhere"))).toBe("Page not found");
  });

  it("change the page key for another page but not for another section of the same workspace", () => {
    expect(pageKey(parseRoute("/workspaces/w1/evidence"))).toBe(pageKey(parseRoute("/workspaces/w1/memory")));
    expect(pageKey(parseRoute("/workspaces/w1/evidence"))).not.toBe(pageKey(parseRoute("/workspaces/w2/evidence")));
    expect(pageKey(parseRoute("/workspaces/w1/artifacts/a1"))).not.toBe(pageKey(parseRoute("/workspaces/w1/evidence")));
    expect(pageKey(parseRoute("/system/logs"))).toBe(pageKey(parseRoute("/system/jobs")));
  });
});

describe("RouteAnnouncer", () => {
  beforeEach(() => { document.title = "RepoMemo"; });

  function Page({ keyName, title }: { keyName: string; title: string }) {
    return <><RouteAnnouncer pageKey={keyName} title={title} /><main id="main-content" tabIndex={-1}>{title}</main><button type="button">tab</button></>;
  }

  it("sets the tab title and announces it, and leaves focus alone on the first page", () => {
    render(<Page keyName="dashboard" title="Dashboard" />);
    expect(document.title).toBe("Dashboard · RepoMemo");
    expect(screen.getByRole("status").textContent).toBe("Dashboard");
    expect(document.activeElement).toBe(document.body);
  });

  it("moves focus to the page after a move to another page, but not after a change of section", () => {
    const { rerender } = render(<Page keyName="workspace:w1" title="Atlas · Evidence" />);
    screen.getByRole("button", { name: "tab" }).focus();
    rerender(<Page keyName="workspace:w1" title="Atlas · Memory" />);
    expect(screen.getByRole("status").textContent).toBe("Atlas · Memory");
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "tab" }));
    rerender(<Page keyName="dashboard" title="Dashboard" />);
    expect(document.activeElement).toBe(screen.getByRole("main"));
  });
});

describe("toasts", () => {
  it("stay while a control inside has keyboard focus, and Escape dismisses one", () => {
    vi.useFakeTimers();
    render(<ToastViewport />);
    act(() => showToast("error", "It failed."));
    const close = screen.getByRole("button", { name: "Dismiss notification" });
    act(() => close.focus());
    fireEvent.focus(close);
    act(() => { vi.advanceTimersByTime(30_000); });
    expect(screen.getByText("It failed.")).toBeTruthy();
    fireEvent.keyDown(close, { key: "Escape" });
    expect(screen.queryByText("It failed.")).toBeNull();
  });
});

describe("workbook sheets", () => {
  const sheets = ["Q1", "Q2", "Q3"].map((name) => ({ name, rows: [["a"]], total_rows: 1, total_columns: 1 }));
  it("are tabs: one in the tab order, arrow keys move between them", () => {
    render(<SheetsPreview sheets={sheets} totalSheets={3} />);
    const tabs = screen.getAllByRole("tab");
    expect(tabs.map((tab) => tab.getAttribute("tabindex"))).toEqual(["0", "-1", "-1"]);
    fireEvent.keyDown(tabs[0], { key: "ArrowRight" });
    expect(screen.getAllByRole("tab").map((tab) => tab.getAttribute("aria-selected"))).toEqual(["false", "true", "false"]);
    expect(document.activeElement).toBe(screen.getAllByRole("tab")[1]);
    fireEvent.keyDown(screen.getAllByRole("tab")[1], { key: "End" });
    expect(document.activeElement).toBe(screen.getAllByRole("tab")[2]);
    fireEvent.keyDown(screen.getAllByRole("tab")[2], { key: "ArrowRight" });
    expect(document.activeElement).toBe(screen.getAllByRole("tab")[0]);
    expect(screen.getByRole("tabpanel").getAttribute("aria-labelledby")).toBe(screen.getAllByRole("tab")[0].id);
  });
});

