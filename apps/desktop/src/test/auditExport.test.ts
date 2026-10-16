import { afterEach, describe, expect, it, vi } from "vitest";
import { downloadSystemAuditExport, SharedApiError } from "../lib/sharedApi";

afterEach(() => vi.restoreAllMocks());

describe("downloadSystemAuditExport", () => {
  it("asks for the format and the days, and returns the file under the server's name with its count", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("id,time\r\n", {
      status: 200,
      headers: {
        "Content-Type": "text/csv; charset=utf-8",
        "Content-Disposition": "attachment; filename=\"repomemo-audit-2026-10-01-2026-10-15.csv\"",
        "X-Total-Count": "42",
      },
    }));
    const result = await downloadSystemAuditExport("token", { format: "csv", from: "2026-10-01", to: "2026-10-15" });
    const [url, init] = fetchMock.mock.calls[0];
    const query = new URL(String(url), "http://localhost").searchParams;
    expect(String(url)).toContain("/v1/system/audit/export?");
    expect([query.get("format"), query.get("from"), query.get("to")]).toEqual(["csv", "2026-10-01", "2026-10-15"]);
    expect(new Headers(init?.headers).get("Authorization")).toBe("Bearer token");
    expect(result.filename).toBe("repomemo-audit-2026-10-01-2026-10-15.csv");
    expect(result.count).toBe(42);
    expect(await result.blob.text()).toBe("id,time\r\n");
  });

  it("leaves the days out when they are empty and names the file itself when the server did not", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("", { status: 200 }));
    const result = await downloadSystemAuditExport("token", { format: "jsonl", from: "", to: "" });
    const query = new URL(String(fetchMock.mock.calls[0][0]), "http://localhost").searchParams;
    expect([...query.keys()]).toEqual(["format"]);
    expect(result.filename).toBe("repomemo-audit.jsonl");
    expect(result.count).toBe(0);
  });

  it("raises the server's reason when the export is refused", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response(JSON.stringify({ error: { message: "\"from\" must be earlier than \"to\"." } }), {
      status: 400,
      headers: { "Content-Type": "application/json" },
    }));
    const failure = await downloadSystemAuditExport("token", { format: "jsonl", from: "2999-01-01" }).catch((error: unknown) => error);
    expect(failure).toBeInstanceOf(SharedApiError);
    expect((failure as SharedApiError).message).toContain("must be earlier");
    expect((failure as SharedApiError).status).toBe(400);
  });
});
