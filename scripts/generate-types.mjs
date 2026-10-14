#!/usr/bin/env node
// Generates the web client's types for the API's data transfer objects from
// the Rust source (`crates/domain/src/lib.rs`), so the two cannot drift apart
// silently (roadmap C7).
//
//   node scripts/generate-types.mjs           writes apps/desktop/src/generated/domain.ts
//   node scripts/generate-types.mjs --check   fails when that file is out of date (CI)
//
// The domain crate keeps to a small, regular subset of Rust: public structs
// with public fields, unit and struct-variant enums, `Option`, `Vec`, maps,
// tuples and `serde_json::Value`. Anything else stops the generator with the
// name of the type, instead of guessing.

import { readFileSync, writeFileSync, mkdirSync, existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const source = join(root, "crates/domain/src/lib.rs");
const target = join(root, "apps/desktop/src/generated/domain.ts");

const NUMBERS = new Set(["u8", "u16", "u32", "u64", "usize", "i8", "i16", "i32", "i64", "isize", "f32", "f64"]);

function snake(name) {
  return name.replace(/([a-z0-9])([A-Z])/g, "$1_$2").toLowerCase();
}

/** Splits `A, B<C, D>, (E, F)` at top-level commas. */
function splitTop(text) {
  const parts = [];
  let depth = 0;
  let current = "";
  for (const character of text) {
    if ("<([".includes(character)) depth += 1;
    if (">)]".includes(character)) depth -= 1;
    if (character === "," && depth === 0) {
      parts.push(current.trim());
      current = "";
    } else {
      current += character;
    }
  }
  if (current.trim()) parts.push(current.trim());
  return parts;
}

function tsType(rust, context) {
  const type = rust.trim().replace(/^&'?\w*\s*/, "");
  if (type === "String" || type === "str") return "string";
  if (type === "bool") return "boolean";
  if (NUMBERS.has(type)) return "number";
  if (type === "Value" || type === "serde_json::Value") return "unknown";
  let match = /^Option<(.+)>$/s.exec(type);
  if (match) return `${tsType(match[1], context)} | null`;
  match = /^(?:Vec|BTreeSet|HashSet)<(.+)>$/s.exec(type);
  if (match) {
    const inner = tsType(match[1], context);
    return inner.includes(" ") ? `Array<${inner}>` : `${inner}[]`;
  }
  match = /^(?:BTreeMap|HashMap)<\s*String\s*,(.+)>$/s.exec(type);
  if (match) return `Record<string, ${tsType(match[1], context)}>`;
  match = /^\((.+)\)$/s.exec(type);
  if (match) return `[${splitTop(match[1]).map((part) => tsType(part, context)).join(", ")}]`;
  if (/^[A-Z][A-Za-z0-9]*$/.test(type)) return type;
  throw new Error(`${context}: the type \`${type}\` is not one the generator knows`);
}

const lines = readFileSync(source, "utf8").replace(/\r\n/g, "\n").split("\n");
const items = [];
let docs = [];
let attributes = [];
for (let index = 0; index < lines.length; index += 1) {
  const line = lines[index];
  if (line.startsWith("///")) { docs.push(line.slice(3).trim()); continue; }
  if (line.startsWith("#[")) { attributes.push(line.trim()); continue; }
  const header = /^pub (struct|enum) ([A-Za-z0-9]+)\s*\{$/.exec(line);
  if (header) {
    const body = [];
    let end = index + 1;
    for (; end < lines.length && lines[end] !== "}"; end += 1) body.push(lines[end]);
    items.push({ kind: header[1], name: header[2], docs, attributes, body });
    index = end;
  }
  if (!line.startsWith("#[") && !line.startsWith("///")) { docs = []; attributes = []; }
}

/** Fields of a struct body (or of a struct variant): `name: Type,` with optional doc comments and attributes. */
function fieldsOf(body, context) {
  const fields = [];
  let doc = [];
  let skipped = false;
  const text = [];
  for (const raw of body) {
    const line = raw.trim();
    if (line.startsWith("///")) { doc.push(line.slice(3).trim()); continue; }
    if (line.startsWith("//")) continue;
    if (line.startsWith("#[")) {
      if (/skip_serializing\b/.test(line) && !/skip_serializing_if/.test(line)) skipped = true;
      continue;
    }
    text.push(line);
    const field = /^(?:pub\s+)?(?:r#)?([a-z_][a-z0-9_]*)\s*:\s*(.+?),?$/.exec(line);
    if (field && !skipped) fields.push({ name: field[1], type: tsType(field[2], `${context}.${field[1]}`), doc });
    doc = [];
    skipped = false;
  }
  return fields;
}

function comment(doc, indent = "") {
  if (!doc.length) return "";
  if (doc.length === 1) return `${indent}/** ${doc[0]} */\n`;
  return `${indent}/**\n${doc.map((line) => `${indent} * ${line}`.trimEnd()).join("\n")}\n${indent} */\n`;
}

function render(item) {
  const context = item.name;
  const serde = item.attributes.filter((attribute) => attribute.startsWith("#[serde")).join(" ");
  if (item.kind === "struct") {
    const fields = fieldsOf(item.body, context);
    const body = fields.map((field) => `${comment(field.doc, "  ")}  ${field.name}: ${field.type};`).join("\n");
    return `${comment(item.docs)}export interface ${item.name} {\n${body}\n}\n`;
  }
  const rename = /rename_all = "snake_case"/.test(serde);
  const tag = /tag = "([a-z_]+)"/.exec(serde)?.[1];
  const variants = [];
  let doc = [];
  for (let index = 0; index < item.body.length; index += 1) {
    const line = item.body[index].trim();
    if (line.startsWith("///")) { doc.push(line.slice(3).trim()); continue; }
    if (line.startsWith("//") || line.startsWith("#[") || !line) continue;
    const unit = /^([A-Za-z0-9]+),?$/.exec(line);
    if (unit) { variants.push({ name: unit[1], doc, fields: null }); doc = []; continue; }
    const inline = /^([A-Za-z0-9]+)\s*\{(.*)\},?$/.exec(line);
    if (inline) {
      const fields = splitTop(inline[2]).map((part) => {
        const [name, ...type] = part.split(":");
        return { name: name.trim(), type: tsType(type.join(":"), `${context}.${inline[1]}.${name.trim()}`), doc: [] };
      });
      variants.push({ name: inline[1], doc, fields });
      doc = [];
      continue;
    }
    const structVariant = /^([A-Za-z0-9]+)\s*\{$/.exec(line);
    if (structVariant) {
      const inner = [];
      let end = index + 1;
      for (; end < item.body.length && item.body[end].trim() !== "}," && item.body[end].trim() !== "}"; end += 1) inner.push(item.body[end]);
      variants.push({ name: structVariant[1], doc, fields: fieldsOf(inner, `${context}.${structVariant[1]}`) });
      doc = [];
      index = end;
      continue;
    }
    throw new Error(`${context}: cannot read the variant \`${line}\``);
  }
  const wire = (name) => (rename ? snake(name) : name);
  if (variants.every((variant) => variant.fields === null)) {
    const union = variants.map((variant) => `  | "${wire(variant.name)}"`).join("\n");
    return `${comment(item.docs)}export type ${item.name} =\n${union};\n`;
  }
  if (!tag) throw new Error(`${context}: an enum with data needs a serde tag`);
  const union = variants
    .map((variant) => {
      const fields = (variant.fields ?? []).map((field) => `${field.name}: ${field.type}`);
      return `${comment(variant.doc, "  ")}  | { ${[`${tag}: "${wire(variant.name)}"`, ...fields].join("; ")} }`;
    })
    .join("\n");
  return `${comment(item.docs)}export type ${item.name} =\n${union};\n`;
}

const output = [
  "// GENERATED by scripts/generate-types.mjs from crates/domain/src/lib.rs.",
  "// Do not edit: change the Rust type and run `npm run types:generate`.",
  "",
  ...items.map(render),
].join("\n");

if (process.argv.includes("--check")) {
  const current = existsSync(target) ? readFileSync(target, "utf8").replace(/\r\n/g, "\n") : "";
  if (current !== output) {
    console.error("apps/desktop/src/generated/domain.ts is out of date. Run `npm run types:generate`.");
    process.exit(1);
  }
  console.log(`Generated types are current (${items.length} types).`);
} else {
  mkdirSync(dirname(target), { recursive: true });
  writeFileSync(target, output);
  console.log(`Wrote ${items.length} types to apps/desktop/src/generated/domain.ts`);
}
