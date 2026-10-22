# Built-in templates

Every `*.json` file in this folder whose name does not start with `_` is a built-in workspace template. `build.rs` registers it when the crate is built, so adding a template means adding a file here and nothing else.

- The file is named after the template's id: `operations.json` holds the template with `"id": "operations"`.
- `general` is not here. It is defined in code (`general()`), so no file can replace or remove it.
- A file that does not validate, or that repeats an id, is skipped and reported; a test fails while one exists.
- `_example.json` is the file to copy when writing a template. It uses every block and is not registered, because its name starts with `_`; a test keeps it valid. The guide is [docs/TEMPLATES.md](../../../docs/TEMPLATES.md), and `npm run templates:check` is the checker (it reads the lists and limits from this crate's source).
- The format and its limits are in [mindmap/technical/workspace-templates.md](../../../mindmap/technical/workspace-templates.md).
