import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { RepositoryFields } from "../web/repositories/LinkForm";
import { EMPTY_FORM, STATUS_LABEL, STATUS_TONE, type RepoForm } from "../web/repositories/shared";

afterEach(() => { cleanup(); });

function renderFields(form: RepoForm, props: Partial<Parameters<typeof RepositoryFields>[0]> = {}) {
  const onChange = vi.fn();
  render(<RepositoryFields form={form} onChange={onChange} showLink {...props} />);
  return onChange;
}

describe("the repository form on the shared fields", () => {
  it("labels the link, name, branch and patterns, with the explanation as each field's hint", () => {
    renderFields({ ...EMPTY_FORM, link: "C:\code\app" });
    for (const label of ["Repository link", "Name", "Branch", "Include only", "Exclude"]) {
      expect(screen.getByLabelText(label).closest(".rm-field")).not.toBeNull();
    }
    expect(screen.getByLabelText("Include only").getAttribute("aria-describedby")).toBeTruthy();
    expect(screen.getByText("One pattern per line. Leave empty to index everything.").className).toContain("rm-field-hint");
    expect(screen.queryByLabelText("Access token")).toBeNull();
  });

  it("asks for an access token and its account name side by side for an https link", () => {
    const onChange = renderFields({ ...EMPTY_FORM, link: "https://github.com/acme/app.git" });
    const token = screen.getByLabelText("Access token") as HTMLInputElement;
    expect(token.type).toBe("password");
    expect(token.closest(".rm-field-row")).toBe((screen.getByLabelText("Token account name") as HTMLInputElement).closest(".rm-field-row"));
    fireEvent.change(token, { target: { value: "secret" } });
    expect(onChange).toHaveBeenCalledWith(expect.objectContaining({ token: "secret" }));
  });

  it("asks for a deploy key and host key for an ssh link", () => {
    renderFields({ ...EMPTY_FORM, link: "git@github.com:acme/app.git" });
    expect(screen.getByLabelText("Deploy key")).toBeTruthy();
    expect(screen.getByLabelText("Host key")).toBeTruthy();
    expect(screen.queryByLabelText("Access token")).toBeNull();
  });

  it("gives every sync status a badge tone", () => {
    expect(Object.keys(STATUS_TONE).sort()).toEqual(Object.keys(STATUS_LABEL).sort());
    expect(STATUS_TONE.ready).toBe("success");
    expect(STATUS_TONE.error).toBe("danger");
  });
});
