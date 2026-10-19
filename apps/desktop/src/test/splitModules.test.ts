import { describe, expect, it } from "vitest";

/** The longest a file of a split folder may be (K3 of the frontend polish plan). */
const MAX_LINES = 300;

/** Folders a large component was split into: the source of each of their files, by path. */
const SPLIT_FOLDERS: Record<string, Record<string, string>> = {
  "web/system": import.meta.glob("../web/system/*.{ts,tsx}", { query: "?raw", import: "default", eager: true }) as Record<string, string>,
  "web/repositories": import.meta.glob("../web/repositories/*.{ts,tsx}", { query: "?raw", import: "default", eager: true }) as Record<string, string>,
  "web/setup": import.meta.glob("../web/setup/*.{ts,tsx}", { query: "?raw", import: "default", eager: true }) as Record<string, string>,
};

describe("split modules", () => {
  for (const [folder, files] of Object.entries(SPLIT_FOLDERS)) {
    it(`keeps every file of src/${folder} to ${MAX_LINES} lines or fewer`, () => {
      const entries = Object.entries(files);
      expect(entries.length).toBeGreaterThan(1);
      for (const [path, source] of entries) {
        expect(source.split("\n").length, path).toBeLessThanOrEqual(MAX_LINES);
      }
    });
  }
});
