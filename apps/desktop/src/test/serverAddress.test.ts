import { describe, expect, it } from "vitest";
import { normalizeServerAddress } from "../lib/serverAddress";

const origin = (input: string) => {
  const result = normalizeServerAddress(input);
  if (!result.ok) throw new Error(result.message);
  return result.origin;
};
const refused = (input: string) => {
  const result = normalizeServerAddress(input);
  return result.ok ? null : result.message;
};

// The same cases as the unit tests of `normalize_server_url` in src-tauri/src/connection.rs.
describe("server addresses", () => {
  it("get https when none is typed, and lose the slash, the default port and capitals", () => {
    expect(origin("repomemo.example.com")).toBe("https://repomemo.example.com");
    expect(origin("  https://repomemo.example.com/ ")).toBe("https://repomemo.example.com");
    expect(origin("https://Repomemo.Example.com:8443")).toBe("https://repomemo.example.com:8443");
    expect(origin("https://repomemo.example.com:443")).toBe("https://repomemo.example.com");
  });

  it("may use plain http only on the own network", () => {
    for (const local of ["http://localhost:3020", "http://127.0.0.1:3020", "http://192.168.1.20", "http://10.0.0.5:3020", "http://172.20.1.1", "http://repomemo", "http://nas.local", "http://[::1]:3020"]) {
      expect(normalizeServerAddress(local).ok, local).toBe(true);
    }
    for (const outside of ["http://repomemo.example.com", "http://8.8.8.8", "http://203.0.113.9:3020", "http://172.32.0.1"]) {
      expect(refused(outside), outside).toContain("https");
    }
  });

  it("refuse what is not the address of a server", () => {
    for (const bad of ["", "   ", "ftp://example.com", "file:///etc/passwd", "javascript:alert(1)", "https://user:secret@example.com", "https://example.com/app", "https://example.com/?a=1", "https://example.com/#x", "https://"]) {
      expect(refused(bad), JSON.stringify(bad)).not.toBeNull();
    }
  });
});
