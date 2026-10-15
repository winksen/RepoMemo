import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({ listSharedMentionSuggestions: vi.fn() }));

import { listSharedMentionSuggestions } from "../lib/sharedApi";
import { MentionTextarea, mentionAt } from "../components/MentionTextarea";

const ada = { id: "u1", display_name: "Ada Lovelace", email: "ada@example.com" };
const grace = { id: "u2", display_name: "Grace Hopper", email: "grace@example.com" };

function Field({ initial = "" }: { initial?: string }) {
  const [value, setValue] = useState(initial);
  return <MentionTextarea accessToken="t" aria-label="Comment" onValueChange={setValue} value={value} workspaceId="w1" />;
}

function type(field: HTMLElement, text: string) {
  const element = field as HTMLTextAreaElement;
  fireEvent.change(element, { target: { value: text, selectionStart: text.length, selectionEnd: text.length } });
}

describe("mentionAt", () => {
  it("finds the @word the caret ends", () => {
    expect(mentionAt("hello @ad", 9)).toEqual({ start: 6, query: "ad" });
    expect(mentionAt("@", 1)).toEqual({ start: 0, query: "" });
    expect(mentionAt("line one\n@gr", 12)).toEqual({ start: 9, query: "gr" });
  });

  it("ignores an @ inside a word, an address that is finished and plain text", () => {
    expect(mentionAt("mail me at ada@ex", 17)).toBeNull();
    expect(mentionAt("@ada@example.com ", 17)).toBeNull();
    expect(mentionAt("no mention", 10)).toBeNull();
  });
});

describe("MentionTextarea", () => {
  beforeEach(() => {
    vi.mocked(listSharedMentionSuggestions).mockReset();
    vi.mocked(listSharedMentionSuggestions).mockResolvedValue([ada, grace]);
  });
  afterEach(() => cleanup());

  it("suggests members while an @ is typed and writes the chosen address", async () => {
    render(<Field />);
    const field = screen.getByRole("combobox", { name: "Comment" }) as HTMLTextAreaElement;
    type(field, "Thanks @a");
    await screen.findByRole("option", { name: /Ada Lovelace/ });
    expect(listSharedMentionSuggestions).toHaveBeenCalledWith("t", "w1", "a");
    fireEvent.keyDown(field, { key: "ArrowDown" });
    fireEvent.keyDown(field, { key: "Enter" });
    expect(field.value).toBe("Thanks @grace@example.com ");
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  it("takes the first suggestion with Enter and closes with Escape", async () => {
    render(<Field />);
    const field = screen.getByRole("combobox", { name: "Comment" }) as HTMLTextAreaElement;
    type(field, "@");
    await screen.findByRole("listbox");
    fireEvent.keyDown(field, { key: "Escape" });
    expect(screen.queryByRole("listbox")).toBeNull();
    type(field, "@ad");
    await screen.findByRole("option", { name: /Ada Lovelace/ });
    fireEvent.keyDown(field, { key: "Enter" });
    expect(field.value).toBe("@ada@example.com ");
  });

  it("asks for nothing when no @ is being typed", async () => {
    render(<Field />);
    const field = screen.getByRole("combobox", { name: "Comment" });
    type(field, "just a comment");
    await new Promise((resolve) => setTimeout(resolve, 250));
    expect(listSharedMentionSuggestions).not.toHaveBeenCalled();
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  it("leaves the field alone when the server suggests nobody", async () => {
    vi.mocked(listSharedMentionSuggestions).mockResolvedValue([]);
    render(<Field />);
    const field = screen.getByRole("combobox", { name: "Comment" }) as HTMLTextAreaElement;
    type(field, "@zz");
    await waitFor(() => expect(listSharedMentionSuggestions).toHaveBeenCalled());
    expect(screen.queryByRole("listbox")).toBeNull();
    fireEvent.keyDown(field, { key: "Enter" });
    expect(field.value).toBe("@zz");
  });
});
