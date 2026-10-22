# Built-in templates

Every `*.json` file in this folder whose name does not start with `_` is a built-in workspace template. `build.rs` registers it when the crate is built, so adding a template means adding a file here and nothing else.

- The file is named after the template's id: `operations.json` holds the template with `"id": "operations"`.
- `general` is not here. It is defined in code (`general()`), so no file can replace or remove it.
- A file that does not validate, or that repeats an id, is skipped and reported; a test fails while one exists.
- The format and its limits are in [mindmap/technical/workspace-templates.md](../../../mindmap/technical/workspace-templates.md).
