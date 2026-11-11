import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { clearAllDrafts, discardDraft, loadDraft, saveDraft } from "../lib/drafts";

const ada = { server: "https://one.example", userId: "ada" };

describe("drafts", () => {
  beforeEach(() => window.sessionStorage.clear());
  afterEach(() => window.sessionStorage.clear());

  it("keeps an interrupted edit with the revision it was based on", () => {
    saveDraft(ada, "memory-card", "c1", { title: "T", bodyMarkdown: "B" }, 3);
    const draft = loadDraft<{ title: string; bodyMarkdown: string }>(ada, "memory-card", "c1");
    expect(draft?.value).toEqual({ title: "T", bodyMarkdown: "B" });
    expect(draft?.base).toBe(3);
    expect(Number.isNaN(Date.parse(draft!.savedAt))).toBe(false);
  });

  it("is not visible to another person or another server in the same tab", () => {
    saveDraft(ada, "note", "w1", { title: "T", content: "C" });
    expect(loadDraft({ ...ada, userId: "grace" }, "note", "w1")).toBeNull();
    expect(loadDraft({ ...ada, server: "https://two.example" }, "note", "w1")).toBeNull();
    expect(loadDraft(ada, "note", "w2")).toBeNull();
  });

  it("goes when it is discarded or when the session ends", () => {
    saveDraft(ada, "note", "w1", { title: "T", content: "C" });
    saveDraft(ada, "memory-card", "c1", { title: "T" });
    discardDraft(ada, "note", "w1");
    expect(loadDraft(ada, "note", "w1")).toBeNull();
    clearAllDrafts();
    expect(loadDraft(ada, "memory-card", "c1")).toBeNull();
  });

  it("ignores a stored value that is not a draft", () => {
    window.sessionStorage.setItem("repomemo.draft.https%3A%2F%2Fone.example|ada|note|w1", "not json");
    expect(loadDraft(ada, "note", "w1")).toBeNull();
  });
});
