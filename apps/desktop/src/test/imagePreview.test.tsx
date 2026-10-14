import { render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/sharedApi", () => ({ downloadSharedArtifactFile: vi.fn() }));
import { downloadSharedArtifactFile } from "../lib/sharedApi";
import { ImagePreview } from "../components/ImagePreview";

beforeEach(() => {
  URL.createObjectURL = vi.fn(() => "blob:http://localhost/preview");
  URL.revokeObjectURL = vi.fn();
});

describe("ImagePreview", () => {
  it("offers a full-size tab only for raster images", async () => {
    vi.mocked(downloadSharedArtifactFile).mockResolvedValue(new Blob(["x"], { type: "image/png" }));
    const { container } = render(<ImagePreview accessToken="t" artifactId="a1" title="Diagram" />);
    await waitFor(() => expect(container.querySelector("img")).not.toBeNull());
    const link = container.querySelector("a");
    expect(link?.getAttribute("target")).toBe("_blank");
    expect(link?.getAttribute("rel")).toContain("noopener");
  });

  it("never links an SVG or an unknown type to a tab of its own", async () => {
    for (const type of ["image/svg+xml", "text/html", ""]) {
      vi.mocked(downloadSharedArtifactFile).mockResolvedValue(new Blob(["<svg onload=alert(1)/>"], { type }));
      const { container, unmount } = render(<ImagePreview accessToken="t" artifactId="a1" title="Diagram" />);
      await waitFor(() => expect(container.querySelector("img")).not.toBeNull());
      expect(container.querySelector("a"), type).toBeNull();
      unmount();
    }
  });
});
