import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { Time } from "../components/ui/time";
import { formatBytes, formatDate, formatDuration, formatRecentTime, formatRelativeTime, formatShare, formatTime } from "../web/format";

afterEach(cleanup);

describe("formatting helpers", () => {
  it("shows a size in the largest unit that keeps it short", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(48 * 1024)).toBe("48 KB");
    expect(formatBytes(1.5 * 1024 * 1024)).toBe("1.5 MB");
    expect(formatBytes(3 * 1024 ** 3)).toBe("3.0 GB");
    expect(formatBytes(2 * 1024 ** 4)).toBe("2.0 TB");
  });

  it("shows how long a server has been up", () => {
    expect(formatDuration(12 * 60)).toBe("12 min");
    expect(formatDuration(5 * 3600 + 10 * 60)).toBe("5 h 10 min");
    expect(formatDuration(3 * 86400 + 4 * 3600)).toBe("3 d 4 h");
  });

  it("rounds a share and keeps a tiny one visible", () => {
    expect(formatShare(0.426)).toBe("43%");
    expect(formatShare(0.004)).toBe("<1%");
    expect(formatShare(0)).toBe("0%");
  });

  it("gives the fallback for an empty date and the raw text for one that is not a date", () => {
    expect(formatDate(null)).toBe("—");
    expect(formatDate("", "never")).toBe("never");
    expect(formatTime(undefined, "not yet")).toBe("not yet");
    expect(formatDate("soon")).toBe("soon");
    expect(formatTime("soon")).toBe("soon");
  });

  it("shows a date, and a date with its time", () => {
    const value = "2026-03-03T14:05:00Z";
    expect(formatDate(value)).toBe(new Date(value).toLocaleDateString(undefined, { dateStyle: "medium" }));
    expect(formatTime(value)).toBe(new Date(value).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" }));
  });

  it("counts how long ago something happened", () => {
    const now = Date.parse("2026-10-18T12:00:00Z");
    expect(formatRelativeTime("2026-10-18T11:59:50Z", now)).toBe("just now");
    expect(formatRelativeTime("2026-10-18T11:55:00Z", now)).toBe("5m ago");
    expect(formatRelativeTime("2026-10-18T09:00:00Z", now)).toBe("3h ago");
    expect(formatRelativeTime("2026-10-16T12:00:00Z", now)).toBe("2d ago");
    expect(formatRelativeTime("2026-10-01T12:00:00Z", now)).toBe(new Date("2026-10-01T12:00:00Z").toLocaleDateString(undefined, { day: "numeric", month: "short" }));
    expect(formatRelativeTime("not a date", now)).toBe("not a date");
  });

  it("shows the time of day for today and the day for an earlier date", () => {
    expect(formatRecentTime("not a date")).toBe("");
    expect(formatRecentTime("2020-01-02T10:00:00Z")).toBe(new Date("2020-01-02T10:00:00Z").toLocaleDateString(undefined, { day: "numeric", month: "short" }));
    const today = new Date();
    expect(formatRecentTime(today.toISOString())).toBe(today.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" }));
  });
});

describe("Time", () => {
  it("renders a time element with the exact moment and the relative text", () => {
    const value = new Date(Date.now() - 3 * 3600 * 1000).toISOString();
    render(<Time value={value} />);
    const element = screen.getByText("3h ago");
    expect(element.tagName).toBe("TIME");
    expect(element.getAttribute("datetime")).toBe(value);
  });
});
