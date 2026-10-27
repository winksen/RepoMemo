import { vi } from "vitest";

// jsdom has no layout observers or scrolling. Components still call these
// browser APIs while tests assert their visible content and behavior.
class TestResizeObserver implements ResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
}

vi.stubGlobal("ResizeObserver", TestResizeObserver);
Element.prototype.scrollIntoView = vi.fn();
