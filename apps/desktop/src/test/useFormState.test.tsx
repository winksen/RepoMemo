import { act, renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { useFormState } from "../lib/useFormState";

describe("useFormState", () => {
  it("starts clean, becomes dirty when a field changes and clean again when it is typed back", () => {
    const { result } = renderHook(() => useFormState({ name: "Ada", admin: false }));
    expect(result.current.values).toEqual({ name: "Ada", admin: false });
    expect(result.current.isDirty).toBe(false);
    act(() => result.current.set("name", "Grace"));
    expect(result.current.values.name).toBe("Grace");
    expect(result.current.isDirty).toBe(true);
    act(() => result.current.set("name", "Ada"));
    expect(result.current.isDirty).toBe(false);
  });

  it("changes several fields at once and goes back to the initial values", () => {
    const { result } = renderHook(() => useFormState({ name: "Ada", admin: false }));
    act(() => result.current.patch({ name: "Grace", admin: true }));
    expect(result.current.values).toEqual({ name: "Grace", admin: true });
    expect(result.current.isDirty).toBe(true);
    act(() => result.current.reset());
    expect(result.current.values).toEqual({ name: "Ada", admin: false });
    expect(result.current.isDirty).toBe(false);
  });

  it("starts over from new initial values, as when the saved record is loaded or updated", () => {
    const { result, rerender } = renderHook(({ name }) => useFormState({ name }), { initialProps: { name: "Ada" } });
    act(() => result.current.set("name", "Typed"));
    rerender({ name: "Loaded" });
    expect(result.current.values.name).toBe("Loaded");
    expect(result.current.initial.name).toBe("Loaded");
    expect(result.current.isDirty).toBe(false);
  });

  it("keeps what was typed when the initial values are the same on a later render", () => {
    const { result, rerender } = renderHook(({ name }) => useFormState({ name }), { initialProps: { name: "Ada" } });
    act(() => result.current.set("name", "Typed"));
    rerender({ name: "Ada" });
    expect(result.current.values.name).toBe("Typed");
    expect(result.current.isDirty).toBe(true);
  });
});
