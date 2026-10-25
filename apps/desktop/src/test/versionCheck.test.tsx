import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { VersionMismatch } from "../components/VersionMismatch";
import { checkServerVersion, versionAdvice } from "../lib/versionCheck";

describe("whether the app and the server fit", () => {
  it("accepts the same minor release whatever the patch", () => {
    expect(checkServerVersion("0.4.0", "0.4.21")).toEqual({ ok: true });
    expect(checkServerVersion("0.4.36", "0.4.21")).toEqual({ ok: true });
  });

  it("calls a server on an earlier minor old, and one on a later minor new", () => {
    expect(checkServerVersion("0.3.27", "0.4.21")).toMatchObject({ ok: false, reason: "older" });
    expect(checkServerVersion("0.5.0", "0.4.21")).toMatchObject({ ok: false, reason: "newer" });
    expect(checkServerVersion("1.0.0", "0.4.21")).toMatchObject({ ok: false, reason: "newer" });
    expect(checkServerVersion("0.9.9", "1.0.0")).toMatchObject({ ok: false, reason: "older" });
  });

  it("takes a server that does not say its version as old", () => {
    for (const missing of [undefined, null, "", "not a version"]) {
      expect(checkServerVersion(missing, "0.4.21")).toMatchObject({ ok: false, reason: "older", serverVersion: null });
    }
  });

  it("does not block when the app does not know its own version", () => {
    expect(checkServerVersion("0.3.0", "0.0.0")).toMatchObject({ ok: false });
    expect(checkServerVersion("0.3.0", "unknown")).toEqual({ ok: true });
  });

  it("gives advice for the browser and for the desktop", () => {
    const old = checkServerVersion("0.3.27", "0.4.21");
    if (old.ok) throw new Error("expected a mismatch");
    expect(versionAdvice(old, false).body).toContain("update it to 0.4.x");
    expect(versionAdvice(old, true).body).toContain("install the app that matches the server");
    const newer = checkServerVersion("0.5.0", "0.4.21");
    if (newer.ok) throw new Error("expected a mismatch");
    expect(versionAdvice(newer, false).title).toBe("This app is too old for this server");
    expect(versionAdvice(newer, true).body).toContain("Install the desktop app");
  });

  it("shows both versions on the page instead of a broken screen", () => {
    const verdict = checkServerVersion("0.3.27", "0.4.21");
    if (verdict.ok) throw new Error("expected a mismatch");
    render(<VersionMismatch verdict={verdict} desktop={false} />);
    expect(screen.getByRole("heading", { name: "This server is too old for this app" })).toBeTruthy();
    expect(screen.getByText("0.4.21")).toBeTruthy();
    expect(screen.getByText("0.3.27")).toBeTruthy();
  });
});
