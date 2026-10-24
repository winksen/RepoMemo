#!/usr/bin/env node
// Accessibility checks for the web client (roadmap X1). They read files only:
// nothing is started or built.
//
//   node scripts/check-accessibility.mjs     (npm run a11y:check)
//
// 1. Contrast: the colour tokens that win the cascade (the last light and dark
//    definitions in src/styles) are paired the way the stylesheets use them,
//    and text must reach 4.5:1 (WCAG 1.4.3), the focus ring 3:1 (1.4.11).
// 2. Source rules, read from the syntax tree of every component of the web
//    client: images have `alt`; controls have an accessible name (a label, an
//    `aria-label`, text, or an icon with a label); a click handler on an
//    element that is not a control comes with a role and the keyboard;
//    `aria-label` only on elements that expose it; table headers say what they
//    head (`scope`); no positive `tabIndex`.
//
// A deliberate exception is written next to the element:
//   {/* a11y-allow: <rule> <why> */}  or  // a11y-allow: <rule> <why>
// on the line before it. The rules are `name`, `click`, `aria-label`,
// `tabindex`, `scope` and `alt`.

import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const src = join(root, "apps", "desktop", "src");
const problems = [];

// ---------------------------------------------------------------- contrast

function parseColor(value) {
  const text = value.trim().toLowerCase();
  let match = /^#([0-9a-f]{3,8})$/.exec(text);
  if (match) {
    let hex = match[1];
    if (hex.length === 3 || hex.length === 4) hex = [...hex].map((digit) => digit + digit).join("");
    const channel = (index) => parseInt(hex.slice(index * 2, index * 2 + 2), 16);
    return { r: channel(0), g: channel(1), b: channel(2), a: hex.length === 8 ? channel(3) / 255 : 1 };
  }
  match = /^hsla?\(\s*([\d.]+)(?:deg)?[\s,]+([\d.]+)%[\s,]+([\d.]+)%(?:\s*[,/]\s*([\d.]+)(%?))?\s*\)$/.exec(text);
  if (match) {
    const h = Number(match[1]) % 360;
    const s = Number(match[2]) / 100;
    const l = Number(match[3]) / 100;
    const k = (n) => (n + h / 30) % 12;
    const a = s * Math.min(l, 1 - l);
    const f = (n) => l - a * Math.max(-1, Math.min(k(n) - 3, Math.min(9 - k(n), 1)));
    const alpha = match[4] === undefined ? 1 : match[5] ? Number(match[4]) / 100 : Number(match[4]);
    return { r: Math.round(f(0) * 255), g: Math.round(f(8) * 255), b: Math.round(f(4) * 255), a: alpha };
  }
  match = /^rgba?\(\s*([\d.]+)[\s,]+([\d.]+)[\s,]+([\d.]+)(?:\s*[,/]\s*([\d.]+)(%?))?\s*\)$/.exec(text);
  if (match) {
    const alpha = match[4] === undefined ? 1 : match[5] ? Number(match[4]) / 100 : Number(match[4]);
    return { r: Number(match[1]), g: Number(match[2]), b: Number(match[3]), a: alpha };
  }
  return null;
}

const over = (top, below) => ({
  r: top.r * top.a + below.r * (1 - top.a),
  g: top.g * top.a + below.g * (1 - top.a),
  b: top.b * top.a + below.b * (1 - top.a),
  a: 1,
});

function luminance({ r, g, b }) {
  const linear = (channel) => {
    const value = channel / 255;
    return value <= 0.03928 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b);
}

function contrast(foreground, background) {
  const a = luminance(foreground);
  const b = luminance(background);
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}

/** The tokens that win: `:root` blocks in file order, then the dark blocks over them. */
function themeTokens() {
  const stylesDir = join(src, "styles");
  const files = readdirSync(stylesDir).filter((name) => /^\d\d-.*\.css$/.test(name)).sort();
  const light = {};
  const dark = {};
  for (const file of files) {
    const css = readFileSync(join(stylesDir, file), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
    for (const block of css.matchAll(/(?:^|[\s}])(:root(?:\[data-theme="dark"\])?)\s*\{([^}]*)\}/g)) {
      const target = block[1] === ":root" ? light : dark;
      for (const declaration of block[2].matchAll(/(--[a-z0-9-]+)\s*:\s*([^;]+);/g)) target[declaration[1]] = declaration[2].trim();
    }
  }
  return { light, dark: { ...light, ...dark } };
}

function resolveToken(tokens, name, seen = new Set()) {
  let value = tokens[name];
  if (value === undefined || seen.has(name)) return null;
  seen.add(name);
  const reference = /^var\((--[a-z0-9-]+)\)$/.exec(value);
  if (reference) return resolveToken(tokens, reference[1], seen);
  return parseColor(value);
}

const TEXT = ["--color-text", "--color-text-muted", "--color-text-subtle"];
const SURFACES = ["--color-bg", "--color-canvas", "--color-surface", "--color-surface-2", "--color-surface-3", "--color-surface-hover", "--color-code-surface"];
const STATUS = [
  ["--color-danger", "--color-danger-soft"],
  ["--color-success", "--color-success-soft"],
  ["--color-warning", "--color-warning-soft"],
  ["--color-info", "--color-info-soft"],
  ["--color-accent-text", "--color-accent-soft"],
];

function checkContrast() {
  const themes = themeTokens();
  let pairs = 0;
  for (const [theme, tokens] of Object.entries(themes)) {
    const color = (name) => resolveToken(tokens, name);
    const need = (foreground, background, minimum, what) => {
      const front = color(foreground);
      const back = color(background);
      if (!front || !back) {
        problems.push(`contrast (${theme}): ${foreground} or ${background} is not a colour this check can read; keep tokens as hex, hsl or rgb`);
        return;
      }
      pairs += 1;
      const ratio = contrast(over(front, over(back, { r: 255, g: 255, b: 255, a: 1 })), over(back, { r: 255, g: 255, b: 255, a: 1 }));
      if (ratio < minimum) problems.push(`contrast (${theme}): ${what} ${foreground} on ${background} is ${ratio.toFixed(2)}:1, below ${minimum}:1`);
    };
    for (const text of TEXT) for (const surface of SURFACES) need(text, surface, 4.5, "text");
    for (const [strong, soft] of STATUS) {
      for (const surface of ["--color-surface", "--color-bg", "--color-surface-2"]) need(strong, surface, 4.5, "text");
      need(strong, soft, 4.5, "text");
    }
    need("--color-accent-text", "--color-surface", 4.5, "link text");
    need("--color-on-accent", "--color-accent", 4.5, "button text");
    need("--color-on-accent", "--color-accent-hover", 4.5, "button text");
    for (const surface of ["--color-bg", "--color-surface", "--color-surface-2"]) need("--color-focus", surface, 3, "focus ring");
  }
  return pairs;
}

// ----------------------------------------------------------------- source

function* sources(directory) {
  for (const entry of readdirSync(directory)) {
    const path = join(directory, entry);
    if (statSync(path).isDirectory()) {
      if (["test", "generated"].includes(entry)) continue;
      yield* sources(path);
    } else if (entry.endsWith(".tsx")) yield path;
  }
}

const FIELDS = new Set(["input", "select", "textarea", "Input", "Textarea"]);
const BUTTONS = new Set(["button", "Button"]);
const ARIA_LABEL_OK = new Set(["button", "a", "input", "select", "textarea", "nav", "main", "aside", "header", "footer", "section", "form", "article", "table", "ul", "ol", "dl", "svg", "img", "dialog", "fieldset", "figure", "iframe", "details", "summary", "audio", "video", "canvas", "meter", "progress", "output", "search", "menu", "h1", "h2", "h3", "h4", "h5", "h6", "li", "td", "th", "tr", "tbody", "thead"]);
const SEMANTIC_HOSTS = new Set(["div", "span", "p", "i", "b", "em", "strong", "small", "code", "pre"]);
const NON_CONTROLS = new Set(["li", "tr", "td", "section", "article", "svg", "g", "circle", "rect", "path"]);

function tagName(node) {
  return node.tagName.getText();
}

function attribute(opening, name) {
  return opening.attributes.properties.find((property) => ts.isJsxAttribute(property) && property.name.getText() === name);
}

function hasSpread(opening) {
  return opening.attributes.properties.some((property) => ts.isJsxSpreadAttribute(property));
}

function stringValue(attr) {
  if (!attr?.initializer) return "";
  if (ts.isStringLiteral(attr.initializer)) return attr.initializer.text;
  if (ts.isJsxExpression(attr.initializer) && attr.initializer.expression && ts.isStringLiteralLike(attr.initializer.expression)) return attr.initializer.expression.text;
  return null; // an expression: unknown
}

/** Whether the children give the element a name a person could read. */
function hasTextContent(element) {
  if (!ts.isJsxElement(element)) return false;
  for (const child of element.children) {
    if (ts.isJsxText(child) && child.text.trim()) return true;
    if (ts.isJsxExpression(child) && child.expression) {
      const expression = child.expression;
      // {" "} is not a name; anything else computed may be.
      if (ts.isStringLiteralLike(expression) && !expression.text.trim()) continue;
      return true;
    }
    if (ts.isJsxElement(child)) {
      const opening = child.openingElement;
      if (attribute(opening, "aria-hidden")) { /* decorative: not a name */ } else if (hasTextContent(child) || attribute(opening, "aria-label") || attribute(opening, "alt")) return true;
    }
    if (ts.isJsxSelfClosingElement(child)) {
      if (attribute(child, "aria-label") || attribute(child, "alt")) return true;
      // A component that is not an icon (a name starting with a capital that is not a known icon alias) may render text.
      const name = tagName(child);
      if (!ICON_LIKE.test(name) && /^[A-Z]/.test(name) && !attribute(child, "aria-hidden")) return true;
    }
  }
  return false;
}

// Icons are imported as aliases of Tabler icons (`IconX as X`) or used from small local sets.
const ICON_LIKE = /^(Icon[A-Z]\w*|Loader|Download|Upload|Trash|Plus|Minus|X|Check|Copy|Refresh\w*|Search|Bell|Close|Pencil|Edit\w*|Sun|Moon|Logout|Server|Shield|UserCircle|Send|Stop|Play|Pause|Eye\w*|Lock\w*|Key\w*|Link\w*|Folder\w*|File\w*|Chevron\w*|Arrow\w*|Dots\w*|More\w*|Settings?|Gear|Info\w*|Alert\w*|Warning|Trash\w*|Save|Undo|Redo|Bold|Italic|List\w*|Heading\w*|Quote|Code|Sparkle\w*|Wand|Bolt|Star|Pin|Bookmark|History|Clock|Calendar|Filter|Sort\w*|Grid|Layout\w*|Maximize|Minimize|Expand|Collapse|Panel\w*|Sidebar|Menu|Home|Book\w*)$/;

function checkFile(path) {
  const text = readFileSync(path, "utf8");
  const file = relative(root, path).replaceAll("\\", "/");
  const source = ts.createSourceFile(path, text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const lines = text.split("\n");

  function allowed(node, rule) {
    const line = source.getLineAndCharacterOfPosition(node.getStart()).line;
    for (let at = line; at >= Math.max(0, line - 2); at -= 1) {
      if (new RegExp(`a11y-allow:\\s*${rule}\\b`).test(lines[at] ?? "")) return true;
    }
    return false;
  }
  function report(node, rule, message) {
    if (allowed(node, rule)) return;
    const line = source.getLineAndCharacterOfPosition(node.getStart()).line + 1;
    problems.push(`${file}:${line}: ${rule}: ${message}`);
  }
  const insideLabel = (node) => {
    for (let parent = node.parent; parent; parent = parent.parent) {
      if (ts.isJsxElement(parent) && ["label", "Label", "Field"].includes(tagName(parent.openingElement))) return true;
    }
    return false;
  };

  function visit(node) {
    const opening = ts.isJsxElement(node) ? node.openingElement : ts.isJsxSelfClosingElement(node) ? node : null;
    if (opening) {
      const name = tagName(opening);
      const spread = hasSpread(opening);

      if (name === "img" && !attribute(opening, "alt") && !spread) report(opening, "alt", "<img> without alt text (use alt=\"\" for a decorative image)");

      if (name === "th" && !attribute(opening, "scope") && !spread) report(opening, "scope", "<th> without scope: say whether it heads a column or a row");

      const tabIndex = attribute(opening, "tabIndex");
      if (tabIndex) {
        const value = tabIndex.initializer && ts.isJsxExpression(tabIndex.initializer) ? tabIndex.initializer.expression?.getText() : null;
        if (value && Number(value) > 0) report(opening, "tabindex", `tabIndex={${value}} changes the natural order; use 0 or -1`);
      }

      const label = attribute(opening, "aria-label");
      if (label && !spread && /^[a-z]/.test(name) && !attribute(opening, "role") && !ARIA_LABEL_OK.has(name)) {
        report(opening, "aria-label", `aria-label on <${name}> is not read without a role; give it a role or put the text in the element`);
      }

      if (attribute(opening, "onClick") && (SEMANTIC_HOSTS.has(name) || NON_CONTROLS.has(name))) {
        const role = stringValue(attribute(opening, "role"));
        const keyboard = attribute(opening, "onKeyDown") || attribute(opening, "onKeyUp") || attribute(opening, "onKeyPress");
        const focusable = attribute(opening, "tabIndex");
        if (!(role && keyboard && focusable) && !attribute(opening, "aria-hidden")) {
          report(opening, "click", `<${name}> with onClick is not a control: use a <button>, or give it a role, tabIndex and a key handler`);
        }
      }

      const ownName = attribute(opening, "aria-label") || attribute(opening, "aria-labelledby") || attribute(opening, "title");
      if (BUTTONS.has(name) && !spread && !ownName) {
        const element = ts.isJsxElement(node) ? node : null;
        if (!element || !hasTextContent(element)) report(opening, "name", `<${name}> without text, aria-label or title has no accessible name`);
      }
      if (name === "a" && attribute(opening, "href") && !spread && !ownName) {
        const element = ts.isJsxElement(node) ? node : null;
        if (!element || !hasTextContent(element)) report(opening, "name", "<a> without text or aria-label has no accessible name");
      }
      if (FIELDS.has(name) && !spread) {
        const type = stringValue(attribute(opening, "type"));
        const named = ownName || attribute(opening, "id") || insideLabel(node);
        if (type !== "hidden" && !attribute(opening, "hidden") && !named) report(opening, "name", `<${name}> has no label: wrap it in a <label>, or add aria-label or an id for a <label htmlFor>`);
      }
    }
    ts.forEachChild(node, visit);
  }
  visit(source);
}

let checked = 0;
for (const path of sources(src)) {
  checked += 1;
  checkFile(path);
}
const pairs = checkContrast();

if (problems.length) {
  for (const problem of problems) console.error(`✗ ${problem}`);
  console.error(`\n${problems.length} accessibility problem${problems.length === 1 ? "" : "s"}.`);
  process.exit(1);
}
console.log(`Accessibility: ${checked} components read, ${pairs} colour pairs reach their contrast.`);
