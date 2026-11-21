import type { CSSProperties } from "react";
import type { Workspace } from "../types";

/** The colours of one theme: the accent, its hover, the soft background, the text that reads on the page, and the text on the accent. */
export interface ColorTones { accent: string; hover: string; soft: string; text: string; on: string }

/**
 * The colours a workspace can take as its main colour, which shows on its tabs and highlights. Each has two variants,
 * so that it reads well on both themes: a deeper one for the light theme (white on it at 6.5:1 or better) and a
 * lighter one for the dark theme (white on it too, at 4.8:1 or better, and clear of the dark surface at 3:1). White
 * icons and labels sit on the colour in both themes; the `text` tone is for coloured text on the page. The default
 * (no colour) is the app's own indigo.
 */
export const WORKSPACE_COLORS: { key: string; label: string; light: ColorTones; dark: ColorTones }[] = [
  { key: "blue", label: "Blue", light: { accent: "#0c5bb6", hover: "#094890", soft: "#f0f7ff", text: "#094890", on: "#ffffff" }, dark: { accent: "#076ee4", hover: "#0664d0", soft: "#14273d", text: "#76aeef", on: "#ffffff" } },
  { key: "teal", label: "Teal", light: { accent: "#0c6a60", hover: "#08453f", soft: "#f0fffd", text: "#08453f", on: "#ffffff" }, dark: { accent: "#0a8074", hover: "#086d63", soft: "#143d39", text: "#27ddcb", on: "#ffffff" } },
  { key: "green", label: "Green", light: { accent: "#126836", hover: "#0c4524", soft: "#f0fff6", text: "#0c4524", on: "#ffffff" }, dark: { accent: "#128241", hover: "#0f7038", soft: "#143d25", text: "#3ad47a", on: "#ffffff" } },
  { key: "orange", label: "Orange", light: { accent: "#9b4308", hover: "#743206", soft: "#fff6f0", text: "#743206", on: "#ffffff" }, dark: { accent: "#be4e04", hover: "#aa4603", soft: "#3d2514", text: "#ed8e50", on: "#ffffff" } },
  { key: "red", label: "Red", light: { accent: "#b61624", hover: "#91121d", soft: "#fff0f1", text: "#91121d", on: "#ffffff" }, dark: { accent: "#e11425", hover: "#ce1222", soft: "#3d1418", text: "#ea858d", on: "#ffffff" } },
  { key: "pink", label: "Pink", light: { accent: "#af1863", hover: "#8b134f", soft: "#fff0f7", text: "#8b134f", on: "#ffffff" }, dark: { accent: "#da1678", hover: "#c8146e", soft: "#3d1429", text: "#e882b5", on: "#ffffff" } },
  { key: "purple", label: "Purple", light: { accent: "#7f26d9", hover: "#6b20b6", soft: "#f7f0ff", text: "#6b20b6", on: "#ffffff" }, dark: { accent: "#9647e6", hover: "#8c35e3", soft: "#29143d", text: "#d4b8ef", on: "#ffffff" } },
  { key: "slate", label: "Slate", light: { accent: "#535f6e", hover: "#424b57", soft: "#f6f7f9", text: "#424b57", on: "#ffffff" }, dark: { accent: "#5e718d", hover: "#566881", soft: "#24282e", text: "#a5b0c0", on: "#ffffff" } },
  { key: "cyan", label: "Cyan", light: { accent: "#07667e", hover: "#054757", soft: "#f0fcff", text: "#054757", on: "#ffffff" }, dark: { accent: "#037996", hover: "#036882", soft: "#14353d", text: "#2bc3e9", on: "#ffffff" } },
  { key: "amber", label: "Amber", light: { accent: "#865203", hover: "#5e3a02", soft: "#fff9f0", text: "#5e3a02", on: "#ffffff" }, dark: { accent: "#a36200", hover: "#8f5600", soft: "#3d2d14", text: "#eea22f", on: "#ffffff" } },
  { key: "lime", label: "Lime", light: { accent: "#406812", hover: "#2b450c", soft: "#f8fff0", text: "#2b450c", on: "#ffffff" }, dark: { accent: "#4b7e11", hover: "#406c0f", soft: "#2a3d14", text: "#8ad336", on: "#ffffff" } },
];

/** The key of the colour a workspace uses, or `null` for the app's default when none was chosen or the key is unknown. */
export function workspaceColorKey(workspace: Pick<Workspace, "settings">): string | null {
  const chosen = workspace.settings?.color;
  return typeof chosen === "string" && WORKSPACE_COLORS.some((entry) => entry.key === chosen) ? chosen : null;
}

/**
 * The same colour as React props, for an element a component draws itself (a chip in a list): spread `data-workspace-color`
 * and `style` on it. `null`, or a key not in the palette, gives the app's own accent.
 */
export function workspaceColorProps(key: string | null): { "data-workspace-color"?: string; style?: CSSProperties } {
  const color = key ? WORKSPACE_COLORS.find((entry) => entry.key === key) : undefined;
  if (!color) return {};
  const style: Record<string, string> = {};
  for (const token of ["accent", "hover", "soft", "text", "on"] as const) {
    style[`--ws-light-${token}`] = color.light[token];
    style[`--ws-dark-${token}`] = color.dark[token];
  }
  return { "data-workspace-color": color.key, style: style as CSSProperties };
}

const TOKENS: (keyof ColorTones)[] = ["accent", "hover", "soft", "text", "on"];

/**
 * Gives `element` (and what is inside it) the colour: both variants are set as custom properties and the stylesheet
 * picks the one of the current theme (`[data-workspace-color]` in `01-tokens-and-shared-web.css`), so changing the theme needs no work here.
 * `null` takes the colour away. Returns nothing; call it again, or with `null`, to change it.
 */
export function applyWorkspaceColor(element: HTMLElement | null, key: string | null) {
  if (!element) return;
  const color = key ? WORKSPACE_COLORS.find((entry) => entry.key === key) : undefined;
  if (!color) {
    element.removeAttribute("data-workspace-color");
    for (const token of TOKENS) { element.style.removeProperty(`--ws-light-${token}`); element.style.removeProperty(`--ws-dark-${token}`); }
    return;
  }
  element.setAttribute("data-workspace-color", color.key);
  for (const token of TOKENS) {
    element.style.setProperty(`--ws-light-${token}`, color.light[token]);
    element.style.setProperty(`--ws-dark-${token}`, color.dark[token]);
  }
}
