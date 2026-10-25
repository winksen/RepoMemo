import { afterEach, describe, expect, it, vi } from "vitest";
import { applyRuntimeConfig, loadRuntimeConfig, parseRuntimeConfig } from "../lib/runtimeConfig";
import * as sharedApi from "../lib/sharedApi";

describe("the configuration a host gives the web app", () => {
  afterEach(() => sharedApi.configureWebApi("http://127.0.0.1:3020"));

  it("keeps a valid address, drops its slash and keeps a path prefix", () => {
    expect(parseRuntimeConfig({ apiUrl: "https://api.example.com/" }).apiUrl).toBe("https://api.example.com");
    expect(parseRuntimeConfig({ apiUrl: " http://localhost:3020 " }).apiUrl).toBe("http://localhost:3020");
    expect(parseRuntimeConfig({ apiUrl: "https://example.com/memo/api/" }).apiUrl).toBe("https://example.com/memo/api");
  });

  it("reads an empty address as the page's own host", () => {
    expect(parseRuntimeConfig({ apiUrl: "" }).apiUrl).toBe("");
  });

  it("ignores an address that is not a plain http(s) one", () => {
    for (const apiUrl of ["javascript:alert(1)", "ftp://example.com", "//example.com", "https://user:pw@example.com", "https://example.com/?x=1", "https://example.com/#a", "not an address", 3]) {
      expect(parseRuntimeConfig({ apiUrl }).apiUrl, String(apiUrl)).toBeUndefined();
    }
  });

  it("keeps a one-line product name of reasonable length", () => {
    expect(parseRuntimeConfig({ productName: "  Acme   Memo " }).productName).toBe("Acme Memo");
    expect(parseRuntimeConfig({ productName: "" }).productName).toBeUndefined();
    expect(parseRuntimeConfig({ productName: "x".repeat(61) }).productName).toBeUndefined();
    expect(parseRuntimeConfig({ productName: "bell\u0007" }).productName).toBeUndefined();
  });

  it("takes nothing from what is not an object", () => {
    for (const value of [null, "text", 4, [1], undefined]) expect(parseRuntimeConfig(value)).toEqual({});
  });

  it("loads /config.json and treats a missing, failing or non-JSON answer as no configuration", async () => {
    const answering = (response: Response) => vi.fn(async () => response) as unknown as typeof fetch;
    expect(await loadRuntimeConfig(answering(new Response(JSON.stringify({ apiUrl: "https://a.example.com" }))))).toEqual({ apiUrl: "https://a.example.com" });
    expect(await loadRuntimeConfig(answering(new Response("nope", { status: 404 })))).toEqual({});
    expect(await loadRuntimeConfig(answering(new Response("<!doctype html><html></html>")))).toEqual({});
    const failing = vi.fn(async () => {
      throw new TypeError("network");
    }) as unknown as typeof fetch;
    expect(await loadRuntimeConfig(failing)).toEqual({});
  });

  it("points the client at the configured server and names the page", () => {
    const page = document.implementation.createHTMLDocument("RepoMemo");
    applyRuntimeConfig({ apiUrl: "https://api.example.com", productName: "Acme Memo" }, page);
    expect(sharedApi.sharedApiUrl).toBe("https://api.example.com");
    expect(page.title).toBe("Acme Memo");
    applyRuntimeConfig({}, page);
    expect(sharedApi.sharedApiUrl).toBe("https://api.example.com");
  });

  it("serves one page on the host itself when the address is empty", () => {
    applyRuntimeConfig({ apiUrl: "" }, document.implementation.createHTMLDocument("x"));
    expect(sharedApi.sharedApiUrl).toBe("");
  });
});
