import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import { FeedbackNotice } from "../components/ui/feedback-notice";
import { AppSkeleton, PageSkeleton, SkeletonCards, SkeletonList } from "../components/ui/skeleton";

afterEach(cleanup);

it("says once what is loading and hides the placeholder blocks from assistive technology", () => {
  render(<SkeletonList label="Loading jobs" rows={3} />);
  const status = screen.getByRole("status");
  expect(status.textContent).toBe("Loading jobs…");
  expect(status.getAttribute("aria-busy")).toBe("true");
  expect(status.querySelectorAll(".rm-skeleton-row")).toHaveLength(3);
  expect(status.querySelectorAll(".rm-skeleton-row[aria-hidden='true']")).toHaveLength(3);
});

it("draws a page, cards and the whole frame as placeholders", () => {
  render(<><PageSkeleton label="Opening the workspace" /><SkeletonCards count={2} label="Loading figures" /></>);
  expect(screen.getAllByRole("status").map((node) => node.textContent)).toEqual(["Opening the workspace…", "Loading figures…"]);
  cleanup();
  render(<AppSkeleton label="Restoring your shared session" />);
  expect(document.querySelector(".shared-home-rail")).toBeTruthy();
  expect(screen.getByRole("status").textContent).toContain("Restoring your shared session…");
});

it("shows a loading notice as placeholder rows and keeps the other kinds as they were", () => {
  render(<FeedbackNotice kind="loading" rows={2} title="Loading evidence" />);
  expect(screen.getByRole("status").textContent).toBe("Loading evidence…");
  expect(document.querySelectorAll(".rm-skeleton-row")).toHaveLength(2);
  cleanup();
  render(<FeedbackNotice detail="Try later" kind="error" title="Could not load" />);
  expect(screen.getByRole("alert").textContent).toContain("Could not load");
});
