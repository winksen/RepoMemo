#!/usr/bin/env node
// Checks the workspace templates (roadmap F26a, V0.2.188). It reads files only; nothing is started or built.
//
//   node scripts/check-templates.mjs [--base <git ref>] [file...]      (npm run templates:check)
//
//   1. Every template file of crates/templates/definitions/ (and the example, _example.json) is valid: a known key
//      in every place, a value out of a closed list, a number inside its bounds, a text inside its length. The lists
//      and the bounds are read from the Rust source of crates/templates, so the crate stays the one place they are
//      written; a file that does not pass here is refused by the server too, and the message names the file, the
//      line and the column.
//   2. A file is named after its template's id, and an id is used once.
//   3. A template whose file changed since the base (the last commit, or --base) has a higher meta.version, and a
//      version is never raised for a file that did not change.
//   4. No template id is written in the code: nothing but the definitions, the tests and the documents may name one.
//
// The server's own validator (repomemo-templates) stays the final word; this is the same check made before a
// release, on a machine with only Node, and with a position for every problem.

import { execFileSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const definitions = join(root, "crates", "templates", "definitions");
const rel = (path) => relative(root, path).replace(/\\/g, "/");
const read = (path) => readFileSync(path, "utf8").replace(/\r\n/g, "\n");

// The lists and bounds, from the Rust source ------------------------------------------------------------------
const source = (file) => read(join(root, "crates", "templates", "src", file));
function words(file, name) {
  const match = new RegExp(`pub const ${name}: \\[&str; \\d+\\] = \\[([^\\]]*)\\]`).exec(source(file));
  if (!match) throw new Error(`crates/templates/src/${file} has no list ${name}`);
  return [...match[1].matchAll(/"([^"]*)"/g)].map((entry) => entry[1]);
}
function number(file, name) {
  const match = new RegExp(`pub const ${name}: [a-z0-9]+ = ([^;]+);`).exec(source(file));
  if (!match) throw new Error(`crates/templates/src/${file} has no number ${name}`);
  const expression = match[1].replace(/_/g, "");
  if (!/^[0-9.*\s]+$/.test(expression)) throw new Error(`crates/templates/src/${file}: ${name} is not a plain number`);
  return Function(`return (${expression})`)();
}
const text = (file, name) => new RegExp(`pub const ${name}: &str = "([^"]*)";`).exec(source(file))?.[1];

const limits = Object.fromEntries(
  ["FILE_BYTES", "NESTING", "ID_MIN", "ID_MAX", "VERSION_MAX", "NAME_MAX", "SUMMARY_MAX", "STARTER_FOLDERS_MAX", "FOLDER_NAME_MAX", "LABEL_MAX"].map((name) => [name, number("limits.rs", name)]),
);
const FORMAT = number("lib.rs", "FORMAT");
const GENERAL_ID = text("general.rs", "GENERAL_ID");
const lists = {
  icons: words("meta.rs", "WORKSPACE_ICONS"),
  sections: words("sections.rs", "WORKSPACE_SECTIONS"),
  adminOnly: words("sections.rs", "ADMIN_ONLY_SECTIONS"),
  artifactTypes: words("labels.rs", "ARTIFACT_TYPES"),
  pulseCards: words("overview.rs", "PULSE_CARDS"),
  headlineCards: words("overview.rs", "HEADLINE_CARDS"),
  views: words("evidence.rs", "EVIDENCE_VIEWS"),
  fileTypes: words("evidence.rs", "FILE_TYPES"),
  capabilities: words("assistant.rs", "CAPABILITIES"),
  detectors: words("health.rs", "HEALTH_DETECTORS"),
  reviewSources: words("review.rs", "REVIEW_SOURCES"),
  relations: words("map.rs", "MAP_RELATIONS"),
};
const bounds = {
  cardsMax: number("overview.rs", "CARDS_MAX"),
  suggestionsMax: number("assistant.rs", "SUGGESTIONS_MAX"),
  suggestionMax: number("assistant.rs", "SUGGESTION_MAX"),
  phrasesMax: number("assistant.rs", "PHRASES_MAX"),
  phraseMax: number("assistant.rs", "PHRASE_MAX"),
  focusMax: number("assistant.rs", "FOCUS_MAX"),
  chipsMax: number("retrieval.rs", "TYPE_CHIPS_MAX"),
  factorMin: number("ranking.rs", "FACTOR_MIN"),
  factorMax: number("ranking.rs", "FACTOR_MAX"),
  boostsMax: number("ranking.rs", "BOOSTS_MAX"),
  patternMax: number("ranking.rs", "PATTERN_MAX"),
  patternSegmentsMax: number("ranking.rs", "PATTERN_SEGMENTS_MAX"),
  halfLifeMin: number("ranking.rs", "HALF_LIFE_MIN"),
  halfLifeMax: number("ranking.rs", "HALF_LIFE_MAX"),
  recencyMax: number("ranking.rs", "RECENCY_MAX"),
  staleMin: number("health.rs", "STALE_DAYS_MIN"),
  staleMax: number("health.rs", "STALE_DAYS_MAX"),
  repositoryPatternsMax: number("repositories.rs", "REPOSITORY_PATTERNS_MAX"),
};

// A JSON reader that keeps the line and column of every key and value -----------------------------------------
class Syntax extends Error {
  constructor(message, line, column) {
    super(message);
    this.line = line;
    this.column = column;
  }
}

function parseJson(input) {
  let at = 0;
  let line = 1;
  let column = 1;
  let deepest = 0;
  let depth = 0;
  const fail = (message) => {
    throw new Syntax(message, line, column);
  };
  const peek = () => input[at];
  const next = () => {
    const character = input[at++];
    if (character === "\n") {
      line += 1;
      column = 1;
    } else column += 1;
    return character;
  };
  const skip = () => {
    while (at < input.length && " \t\n\r".includes(peek())) next();
  };
  const here = () => ({ line, column });
  function string() {
    const start = here();
    next();
    let value = "";
    for (;;) {
      if (at >= input.length) fail("a string is not closed");
      const character = next();
      if (character === '"') return { kind: "string", value, ...start };
      if (character === "\n") fail("a string runs over the end of the line");
      if (character === "\\") {
        const escape = next();
        if (escape === "u") {
          const code = input.slice(at, at + 4);
          if (!/^[0-9a-fA-F]{4}$/.test(code)) fail("a \\u escape needs four hexadecimal digits");
          for (let i = 0; i < 4; i += 1) next();
          value += String.fromCharCode(parseInt(code, 16));
        } else if ('"\\/'.includes(escape)) value += escape;
        else if (escape in { b: 1, f: 1, n: 1, r: 1, t: 1 }) value += { b: "\b", f: "\f", n: "\n", r: "\r", t: "\t" }[escape];
        else fail(`\\${escape} is not an escape`);
      } else value += character;
    }
  }
  function value() {
    skip();
    const start = here();
    const character = peek();
    if (character === "{") {
      next();
      depth += 1;
      deepest = Math.max(deepest, depth);
      const entries = new Map();
      const duplicates = [];
      skip();
      if (peek() === "}") next();
      else {
        for (;;) {
          skip();
          if (peek() !== '"') fail("a key is a string in double quotes");
          const key = string();
          skip();
          if (peek() !== ":") fail("a colon is expected after the key");
          next();
          const node = value();
          if (entries.has(key.value)) duplicates.push(key);
          else entries.set(key.value, { key, node });
          skip();
          if (peek() === ",") {
            next();
            continue;
          }
          if (peek() === "}") {
            next();
            break;
          }
          fail("a comma or a closing } is expected");
        }
      }
      depth -= 1;
      return { kind: "object", entries, duplicates, ...start };
    }
    if (character === "[") {
      next();
      depth += 1;
      deepest = Math.max(deepest, depth);
      const items = [];
      skip();
      if (peek() === "]") next();
      else {
        for (;;) {
          items.push(value());
          skip();
          if (peek() === ",") {
            next();
            continue;
          }
          if (peek() === "]") {
            next();
            break;
          }
          fail("a comma or a closing ] is expected");
        }
      }
      depth -= 1;
      return { kind: "array", items, ...start };
    }
    if (character === '"') return string();
    const literal = /^(true|false|null|-?\d+(\.\d+)?([eE][+-]?\d+)?)/.exec(input.slice(at));
    if (!literal) fail(at >= input.length ? "the file ends too soon" : `unexpected ${JSON.stringify(character)}`);
    for (let i = 0; i < literal[0].length; i += 1) next();
    const raw = literal[0];
    if (raw === "true" || raw === "false") return { kind: "boolean", value: raw === "true", ...start };
    if (raw === "null") return { kind: "null", value: null, ...start };
    return { kind: "number", value: Number(raw), integer: !/[.eE]/.test(raw), ...start };
  }
  const node = value();
  skip();
  if (at < input.length) fail("there is more after the end of the template");
  return { node, deepest };
}

// Reading a template: every problem is kept with its place ------------------------------------------------------
function check(file, input) {
  const problems = [];
  const add = (node, path, message) => problems.push({ file, line: node?.line ?? 1, column: node?.column ?? 1, path, message });
  let parsed;
  try {
    if (Buffer.byteLength(input) > limits.FILE_BYTES) {
      add(null, "", `the file is ${Buffer.byteLength(input)} bytes; a template is at most ${limits.FILE_BYTES}`);
    }
    parsed = parseJson(input);
  } catch (error) {
    if (!(error instanceof Syntax)) throw error;
    problems.push({ file, line: error.line, column: error.column, path: "", message: error.message });
    return { problems, template: null };
  }
  if (parsed.deepest > limits.NESTING) add(parsed.node, "", `the file is nested ${parsed.deepest} levels deep; a template is at most ${limits.NESTING}`);

  const describe = (node) => (node.kind === "object" ? "an object" : node.kind === "array" ? "a list" : node.kind === "null" ? "null" : `a ${node.kind}`);

  /** An object whose keys are all in `keys`; returns a getter for the keys present. */
  function shape(node, path, keys) {
    if (node.kind !== "object") {
      add(node, path, `this is ${describe(node)}; an object is expected`);
      return () => undefined;
    }
    for (const key of node.duplicates) add(key, `${path}.${key.value}`, `the key ${key.value} is written twice`);
    for (const [name, { key }] of node.entries) {
      if (!keys.includes(name)) add(key, path ? `${path}.${name}` : name, `unknown key \`${name}\`; the keys here are ${keys.map((entry) => `\`${entry}\``).join(", ")}`);
    }
    return (name) => node.entries.get(name)?.node;
  }
  function required(get, node, path, name) {
    const found = get(name);
    if (!found) add(node, `${path}.${name}`.replace(/^\./, ""), `\`${name}\` is missing`);
    return found;
  }
  /** A text: not empty, no longer than `max`, nothing around it, no control character. */
  function words_(node, path, max, noun = "it") {
    if (node.kind !== "string") return add(node, path, `this is ${describe(node)}; a text is expected`), null;
    const value = node.value;
    const length = [...value].length;
    const problem = !value.trim()
      ? `${noun} is empty`
      : length > max
        ? `${noun} is ${length} characters; the most is ${max}`
        : /\p{Cc}/u.test(value)
          ? `${noun} has a control character`
          : value.trim() !== value
            ? `${noun} starts or ends with a space`
            : null;
    if (problem) add(node, path, problem);
    return problem ? null : value;
  }
  function choice(node, path, vocabulary, noun) {
    if (node.kind !== "string") return add(node, path, `this is ${describe(node)}; a text is expected`), null;
    if (!vocabulary.includes(node.value)) return add(node, path, `\`${node.value}\` is not ${noun}`), null;
    return node.value;
  }
  function array(node, path) {
    if (node.kind !== "array") return add(node, path, `this is ${describe(node)}; a list is expected`), [];
    return node.items;
  }
  /** A list of words of a closed list, each once. */
  function members(node, path, vocabulary, noun, { max, min = 0, emptyMessage } = {}) {
    const items = array(node, path);
    if (items.length < min) add(node, path, emptyMessage ?? `list at least ${min}`);
    if (max !== undefined && items.length > max) add(node, path, `there are ${items.length}; at most ${max}`);
    const seen = new Set();
    const named = [];
    items.forEach((item, index) => {
      const value = choice(item, `${path}[${index}]`, vocabulary, noun);
      if (value === null) return;
      if (seen.has(value)) add(item, `${path}[${index}]`, `\`${value}\` is listed twice`);
      seen.add(value);
      named.push(value);
    });
    return named;
  }
  function factor(node, path) {
    if (node.kind !== "number") return add(node, path, `this is ${describe(node)}; a number is expected`), null;
    if (node.value < bounds.factorMin || node.value > bounds.factorMax) {
      add(node, path, `the factor is ${node.value}; it is from ${bounds.factorMin} to ${bounds.factorMax}`);
      return null;
    }
    return node.value;
  }
  function whole(node, path, min, max, noun) {
    if (node.kind !== "number" || !node.integer) return add(node, path, `${noun} is a whole number from ${min} to ${max}`), null;
    if (node.value < min || node.value > max) return add(node, path, `${noun} is ${node.value}; it is from ${min} to ${max}`), null;
    return node.value;
  }
  /** The folder pattern of the ranking and repositories blocks: names, `*` and `**`, never a regular expression. */
  function pattern(node, path) {
    if (node.kind !== "string") return add(node, path, `this is ${describe(node)}; a text is expected`), null;
    const value = node.value;
    const length = [...value].length;
    const segments = value.split("/");
    let problem = null;
    if (!value || length > bounds.patternMax) problem = `the pattern is ${length} characters; it is 1 to ${bounds.patternMax}`;
    else if (segments.length > bounds.patternSegmentsMax) problem = `the pattern has ${segments.length} folders; the most is ${bounds.patternSegmentsMax}`;
    else {
      for (const segment of segments) {
        if (!segment) problem = "the pattern has an empty folder name (a leading, trailing or doubled /)";
        else if (segment === ".." || segment === ".") problem = "the pattern cannot name . or ..";
        else if (segment !== "**" && segment.includes("**")) problem = "** stands for whole folders: write it between slashes";
        else {
          const bad = [...segment].find((character) => !(/[\p{L}\p{N}]/u.test(character) || "*-_. @+,=~#%".includes(character)));
          if (bad !== undefined) problem = `\`${bad}\` is not allowed in a pattern; only names, * and **`;
        }
        if (problem) break;
      }
    }
    if (problem) add(node, path, problem);
    return problem ? null : value;
  }
  function patterns(node, path) {
    const items = array(node, path);
    if (items.length > bounds.repositoryPatternsMax) add(node, path, `there are ${items.length} patterns; a template offers at most ${bounds.repositoryPatternsMax}`);
    const seen = new Set();
    items.forEach((item, index) => {
      const value = pattern(item, `${path}[${index}]`);
      if (value !== null && seen.has(value)) add(item, `${path}[${index}]`, `\`${value}\` is listed twice`);
      if (value !== null) seen.add(value);
    });
  }

  // The blocks ----------------------------------------------------------------------------------------------------
  const top = shape(parsed.node, "", ["format", "meta", "starter_folders", "sections", "labels", "overview", "evidence", "assistant", "retrieval", "ranking", "health", "review", "map", "repositories"]);
  const template = { id: null, version: null };

  const format = required(top, parsed.node, "", "format");
  if (format && !(format.kind === "number" && format.value === FORMAT)) {
    add(format, "format", format.kind === "number" ? `the file is written for format ${format.value}, which is ${format.value > FORMAT ? "newer" : "older"} than the format ${FORMAT} this server reads` : `the format is the number ${FORMAT}`);
  }

  const meta = required(top, parsed.node, "", "meta");
  if (meta) {
    const get = shape(meta, "meta", ["id", "version", "name", "summary", "icon"]);
    const id = required(get, meta, "meta", "id");
    if (id) {
      if (id.kind !== "string") add(id, "meta.id", "the id is a text");
      else {
        template.id = id.value;
        const length = [...id.value].length;
        if (length < limits.ID_MIN || length > limits.ID_MAX) add(id, "meta.id", `the id is ${length} characters; it is ${limits.ID_MIN} to ${limits.ID_MAX}`);
        else if (!/^[a-z0-9-]+$/.test(id.value)) add(id, "meta.id", "the id uses only a-z, 0-9 and -");
        else if (id.value.startsWith("-") || id.value.endsWith("-") || id.value.includes("--")) add(id, "meta.id", "the id does not start or end with -, and has no --");
      }
    }
    const version = required(get, meta, "meta", "version");
    if (version) template.version = whole(version, "meta.version", 1, limits.VERSION_MAX, "the version");
    const name = required(get, meta, "meta", "name");
    if (name) words_(name, "meta.name", limits.NAME_MAX);
    const summary = required(get, meta, "meta", "summary");
    if (summary) words_(summary, "meta.summary", limits.SUMMARY_MAX);
    const icon = required(get, meta, "meta", "icon");
    if (icon) choice(icon, "meta.icon", lists.icons, "a workspace icon");
  }

  const folders = top("starter_folders");
  if (folders) {
    let total = 0;
    const names = (items, path) => {
      const seen = new Set();
      items.forEach((item, index) => {
        const value = (() => {
          if (item.kind !== "string") return add(item, `${path}[${index}]`, `this is ${describe(item)}; a text is expected`), null;
          const length = [...item.value].length;
          if (!item.value.trim()) return add(item, `${path}[${index}]`, "the name is empty"), null;
          if (length > limits.FOLDER_NAME_MAX) return add(item, `${path}[${index}]`, `the name is ${length} characters; the most is ${limits.FOLDER_NAME_MAX}`), null;
          if (item.value.trim() !== item.value) return add(item, `${path}[${index}]`, "the name starts or ends with a space"), null;
          if (/[\p{Cc}/\\]/u.test(item.value)) return add(item, `${path}[${index}]`, "the name has a slash or a control character"), null;
          return item.value;
        })();
        if (value !== null && seen.has(value.toLowerCase())) add(item, `${path}[${index}]`, `another folder beside this one is already named ${value}`);
        if (value !== null) seen.add(value.toLowerCase());
      });
    };
    const entries = array(folders, "starter_folders");
    total += entries.length;
    const topNames = [];
    entries.forEach((folder, index) => {
      const path = `starter_folders[${index}]`;
      const get = shape(folder, path, ["name", "children"]);
      const name = required(get, folder, path, "name");
      if (name) topNames.push(name);
      const children = get("children");
      if (children) {
        const items = array(children, `${path}.children`);
        total += items.length;
        names(items, `${path}.children`);
      }
    });
    // The names of the top level are checked as a list of texts like the children.
    names(topNames, "starter_folders[].name");
    if (total > limits.STARTER_FOLDERS_MAX) add(folders, "starter_folders", `there are ${total} folders; a template creates at most ${limits.STARTER_FOLDERS_MAX}`);
  }

  const sections = top("sections");
  if (sections) {
    const get = shape(sections, "sections", ["order", "more", "landing"]);
    const named = new Set();
    for (const list of ["order", "more"]) {
      const node = get(list);
      if (!node) continue;
      array(node, `sections.${list}`).forEach((item, index) => {
        const value = choice(item, `sections.${list}[${index}]`, lists.sections, "a workspace section");
        if (value === null) return;
        if (named.has(value)) add(item, `sections.${list}[${index}]`, `\`${value}\` is already named in \`order\` or \`more\``);
        named.add(value);
      });
    }
    const landing = get("landing");
    if (landing) {
      const value = choice(landing, "sections.landing", lists.sections, "a workspace section");
      if (value !== null && lists.adminOnly.includes(value)) add(landing, "sections.landing", `\`${value}\` is only for administrators, so a member could not open it`);
    }
  }

  const labels = top("labels");
  if (labels) {
    const get = shape(labels, "labels", ["sections", "artifact_types"]);
    for (const [list, vocabulary, noun] of [["sections", lists.sections, "a known section"], ["artifact_types", lists.artifactTypes, "a known artifact type"]]) {
      const node = get(list);
      if (!node) continue;
      if (node.kind !== "object") {
        add(node, `labels.${list}`, `this is ${describe(node)}; an object is expected`);
        continue;
      }
      for (const [id, { key, node: label }] of node.entries) {
        if (!vocabulary.includes(id)) add(key, `labels.${list}.${id}`, `\`${id}\` is not ${noun}`);
        else words_(label, `labels.${list}.${id}`, limits.LABEL_MAX, "the name");
      }
    }
  }

  const overview = top("overview");
  if (overview) {
    const get = shape(overview, "overview", ["cards", "headline"]);
    const cards = get("cards");
    if (cards) members(cards, "overview.cards", lists.pulseCards, "a pulse card", { max: bounds.cardsMax, min: 1, emptyMessage: "list at least one card, or leave the block out" });
    const headline = get("headline");
    if (headline) {
      if (headline.kind !== "string" || !lists.headlineCards.includes(headline.value)) {
        add(headline, "overview.headline", `${headline.kind === "string" ? `\`${headline.value}\`` : describe(headline)} cannot be a headline; it is one of ${lists.headlineCards.join(", ")}`);
      }
    }
    if (!cards) add(overview, "overview.cards", "list at least one card, or leave the block out");
  }

  const evidence = top("evidence");
  if (evidence) {
    const get = shape(evidence, "evidence", ["default_view", "types_first"]);
    if (get("default_view")) choice(get("default_view"), "evidence.default_view", lists.views, "a view; use grid or list");
    if (get("types_first")) members(get("types_first"), "evidence.types_first", lists.fileTypes, "a file type of the ledger");
  }

  const assistant = top("assistant");
  if (assistant) {
    const get = shape(assistant, "assistant", ["order", "suggestions", "phrases", "focus"]);
    if (get("order")) members(get("order"), "assistant.order", lists.capabilities, "something the assistant can do");
    if (get("suggestions")) {
      const items = array(get("suggestions"), "assistant.suggestions");
      if (items.length > bounds.suggestionsMax) add(get("suggestions"), "assistant.suggestions", `there are ${items.length} suggestions; a template gives at most ${bounds.suggestionsMax}`);
      const asked = new Set();
      items.forEach((item, index) => {
        const value = words_(item, `assistant.suggestions[${index}]`, bounds.suggestionMax, "the suggestion");
        if (value !== null && asked.has(value.toLowerCase())) add(item, `assistant.suggestions[${index}]`, "the suggestion is given twice");
        if (value !== null) asked.add(value.toLowerCase());
      });
    }
    if (get("phrases")) {
      const items = array(get("phrases"), "assistant.phrases");
      if (items.length > bounds.phrasesMax) add(get("phrases"), "assistant.phrases", `there are ${items.length} phrases; a template gives at most ${bounds.phrasesMax}`);
      const said = new Set();
      items.forEach((item, index) => {
        const path = `assistant.phrases[${index}]`;
        const rule = shape(item, path, ["phrase", "capability"]);
        const phrase = required(rule, item, path, "phrase");
        if (phrase) {
          const value = words_(phrase, `${path}.phrase`, bounds.phraseMax, "the phrase");
          if (value !== null && [...value].some((character) => !(/[\p{L}\p{N}]/u.test(character) || " -'".includes(character)))) {
            add(phrase, `${path}.phrase`, "a phrase is words: letters, digits, spaces, hyphens and apostrophes");
          } else if (value !== null && said.has(value.toLowerCase())) add(phrase, `${path}.phrase`, "the phrase is given twice");
          if (value !== null) said.add(value.toLowerCase());
        }
        const capability = required(rule, item, path, "capability");
        if (capability) choice(capability, `${path}.capability`, lists.capabilities, "something the assistant can do");
      });
    }
    if (get("focus")) words_(get("focus"), "assistant.focus", bounds.focusMax, "the focus");
  }

  const retrieval = top("retrieval");
  if (retrieval) {
    const get = shape(retrieval, "retrieval", ["type_chips"]);
    if (get("type_chips")) members(get("type_chips"), "retrieval.type_chips", lists.artifactTypes, "a kind of evidence", { max: bounds.chipsMax });
  }

  const ranking = top("ranking");
  if (ranking) {
    const get = shape(ranking, "ranking", ["verified", "boosts", "recency"]);
    if (get("verified")) factor(get("verified"), "ranking.verified");
    const recency = get("recency");
    if (recency) {
      const inner = shape(recency, "ranking.recency", ["half_life_days", "max_factor"]);
      const days = required(inner, recency, "ranking.recency", "half_life_days");
      if (days) whole(days, "ranking.recency.half_life_days", bounds.halfLifeMin, bounds.halfLifeMax, "the half-life in days");
      const most = required(inner, recency, "ranking.recency", "max_factor");
      if (most && (most.kind !== "number" || most.value <= 1 || most.value > bounds.recencyMax)) {
        add(most, "ranking.recency.max_factor", `the factor is ${most.kind === "number" ? most.value : describe(most)}; it is above 1 and at most ${bounds.recencyMax}`);
      }
    }
    const boosts = get("boosts");
    if (boosts) {
      const items = array(boosts, "ranking.boosts");
      if (items.length > bounds.boostsMax) add(boosts, "ranking.boosts", `there are ${items.length} boosts; a template lists at most ${bounds.boostsMax}`);
      items.forEach((item, index) => {
        const path = `ranking.boosts[${index}]`;
        const inner = shape(item, path, ["artifact_type", "document_kind", "path", "factor"]);
        if (item.kind === "object" && ["artifact_type", "document_kind", "path"].filter((name) => inner(name)).length !== 1) {
          add(item, path, "a boost names exactly one of artifact_type, document_kind and path");
        }
        if (inner("artifact_type")) choice(inner("artifact_type"), `${path}.artifact_type`, lists.artifactTypes, "a kind of evidence");
        if (inner("document_kind")) choice(inner("document_kind"), `${path}.document_kind`, lists.fileTypes, "a kind of document");
        if (inner("path")) pattern(inner("path"), `${path}.path`);
        const value = required(inner, item, path, "factor");
        if (value) factor(value, `${path}.factor`);
      });
    }
  }

  const health = top("health");
  if (health) {
    const get = shape(health, "health", ["detectors", "stale_after_days"]);
    const detectors = get("detectors") ? members(get("detectors"), "health.detectors", lists.detectors, "a health check", { min: 1, emptyMessage: "list at least one check, or leave the block out" }) : [];
    if (!get("detectors")) add(health, "health.detectors", "list at least one check, or leave the block out");
    const days = get("stale_after_days");
    const stale = detectors.includes("stale_evidence");
    if (stale && !days) add(health, "health.stale_after_days", "stale_evidence needs the number of days after which evidence is stale");
    else if (!stale && days) add(days, "health.stale_after_days", "the days are for the stale_evidence check, which is not listed");
    else if (stale && days) whole(days, "health.stale_after_days", bounds.staleMin, bounds.staleMax, "the days");
  }

  const review = top("review");
  if (review) {
    const get = shape(review, "review", ["order"]);
    if (!get("order")) add(review, "review.order", "list at least one kind of item, or leave the block out");
    else members(get("order"), "review.order", lists.reviewSources, "something the review queue lists", { min: 1, emptyMessage: "list at least one kind of item, or leave the block out" });
  }

  const map = top("map");
  if (map) {
    const get = shape(map, "map", ["coverage_first", "relations_first"]);
    if (get("coverage_first")) members(get("coverage_first"), "map.coverage_first", lists.fileTypes, "a file type");
    if (get("relations_first")) members(get("relations_first"), "map.relations_first", lists.relations, "a kind of link");
  }

  const repositories = top("repositories");
  if (repositories) {
    const get = shape(repositories, "repositories", ["include", "exclude"]);
    if (get("include")) patterns(get("include"), "repositories.include");
    if (get("exclude")) patterns(get("exclude"), "repositories.exclude");
  }

  return { problems, template, tree: parsed.node };
}

// The same template, ignoring the version, for comparing with the base ------------------------------------------
function withoutVersion(input) {
  try {
    const value = JSON.parse(input);
    if (value?.meta) delete value.meta.version;
    return JSON.stringify(value);
  } catch {
    return null;
  }
}

// What to check ------------------------------------------------------------------------------------------------
const args = process.argv.slice(2);
let base = "HEAD";
const named = [];
for (let i = 0; i < args.length; i += 1) {
  if (args[i] === "--base") base = args[(i += 1)];
  else named.push(args[i]);
}
const files = named.length
  ? named.map((file) => resolve(process.cwd(), file))
  : readdirSync(definitions).filter((file) => file.endsWith(".json")).sort().map((file) => join(definitions, file));
const problems = [];
const ids = new Map();
const registered = [];
const notes = [];

function git(...parts) {
  try {
    return execFileSync("git", parts, { cwd: root, encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }).replace(/\r\n/g, "\n");
  } catch {
    return null;
  }
}
const baseKnown = Boolean(base) && git("rev-parse", "--verify", "--quiet", `${base}^{commit}`) !== null;
if (!baseKnown) notes.push(`no base to compare versions with (${base || "none"}): the versions were not compared`);

for (const file of files) {
  const input = read(file);
  const { problems: found, template } = check(rel(file), input);
  problems.push(...found);
  const name = basename(file);
  const isExample = name.startsWith("_");
  if (template?.id) {
    if (!isExample) {
      registered.push(template.id);
      if (name !== `${template.id}.json`) problems.push({ file: rel(file), line: 1, column: 1, path: "meta.id", message: `the file is named ${name}; a template's file is named after its id: ${template.id}.json` });
      if (template.id === GENERAL_ID) problems.push({ file: rel(file), line: 1, column: 1, path: "meta.id", message: `${GENERAL_ID} is defined in code and cannot be replaced by a file` });
      if (ids.has(template.id)) problems.push({ file: rel(file), line: 1, column: 1, path: "meta.id", message: `the id ${template.id} is already used by ${ids.get(template.id)}` });
      ids.set(template.id, rel(file));
    }
  }
  if (!isExample && baseKnown && template?.version) {
    const before = git("show", `${base}:${rel(file)}`);
    if (before !== null) {
      let beforeVersion;
      try {
        beforeVersion = JSON.parse(before)?.meta?.version;
      } catch {
        beforeVersion = undefined;
      }
      const changed = withoutVersion(before) !== withoutVersion(input);
      const place = { file: rel(file), line: 1, column: 1, path: "meta.version" };
      if (Number.isInteger(beforeVersion)) {
        if (template.version < beforeVersion) problems.push({ ...place, message: `the version went down, from ${beforeVersion} to ${template.version}` });
        else if (changed && template.version === beforeVersion) problems.push({ ...place, message: `the file changed since ${base} but its version is still ${beforeVersion}; raise it, so a workspace that chose the template can tell` });
        else if (!changed && template.version !== beforeVersion) problems.push({ ...place, message: `the version went from ${beforeVersion} to ${template.version} but nothing else in the file changed` });
      }
    }
  }
}

// No template id in the code ------------------------------------------------------------------------------------
if (!named.length) {
  const codeRoots = [join(root, "crates"), join(root, "apps")];
  const skipDirectories = new Set(["node_modules", "target", "dist", ".git", "workspace-data", "generated", "definitions", "test", "tests", "gen"]);
  function* code(directory) {
    for (const entry of readdirSync(directory)) {
      const path = join(directory, entry);
      if (statSync(path).isDirectory()) {
        if (!skipDirectories.has(entry)) yield* code(path);
      } else if (/\.(rs|ts|tsx)$/.test(entry) && !/(^tests\.rs$|\.test\.(ts|tsx)$)/.test(entry)) yield path;
    }
  }
  const quoted = new RegExp(`["'\`](${registered.map((id) => id.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("|")})["'\`]`);
  if (registered.length) {
    for (const directory of codeRoots) {
      for (const path of code(directory)) {
        const lines = read(path).split("\n");
        // A Rust file ends with its tests; those may name a template.
        const stop = path.endsWith(".rs") ? lines.findIndex((line) => line.trim() === "#[cfg(test)]") : -1;
        lines.slice(0, stop === -1 ? lines.length : stop).forEach((line, index) => {
          const match = quoted.exec(line);
          if (match && !line.trim().startsWith("//")) {
            problems.push({ file: rel(path), line: index + 1, column: match.index + 1, path: "", message: `the template id ${match[1]} is written in code; ask the template for what it arranges instead (ids belong in the definitions, the tests and the documents)` });
          }
        });
      }
    }
  }
}

for (const note of notes) console.log(`note: ${note}`);
if (problems.length) {
  for (const problem of problems) console.error(`${problem.file}:${problem.line}:${problem.column}  ${problem.path ? `${problem.path}  ` : ""}${problem.message}`);
  console.error(`\n${problems.length} problem${problems.length === 1 ? "" : "s"} in the templates.`);
  process.exit(1);
}
console.log(`Templates: ${files.length} file${files.length === 1 ? "" : "s"} valid${named.length ? "" : `, ${registered.length} registered (${registered.join(", ")}), no template id in code`}.`);
